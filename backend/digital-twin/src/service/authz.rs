use cedar_policy::{
    Authorizer, Context, Decision, Entities, Entity, EntityId, EntityTypeName, EntityUid,
    PolicySet, Request, RestrictedExpression, Schema,
};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::LazyLock;

use crate::domain::identity::GatewayClaims;
use crate::domain::{Building, DomainError};

static ROLE_WEIGHTS: LazyLock<HashMap<String, i64>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../../libs/auth-contracts/roles.json"))
        .expect("roles.json parses")
});

static SCHEMA: LazyLock<Schema> = LazyLock::new(|| {
    let (schema, _warnings) =
        Schema::from_cedarschema_str(include_str!("../../../libs/auth-policy/schema.cedarschema"))
            .expect("cedar schema parses");
    schema
});

static POLICIES: LazyLock<PolicySet> = LazyLock::new(|| {
    PolicySet::from_str(include_str!("../../../libs/auth-policy/policy.cedar"))
        .expect("cedar policy parses")
});

fn euid(type_name: &str, id: &str) -> EntityUid {
    EntityUid::from_type_name_and_id(
        EntityTypeName::from_str(type_name).expect("static entity type name is valid"),
        EntityId::new(id),
    )
}

fn set_expr(values: HashSet<String>) -> RestrictedExpression {
    RestrictedExpression::new_set(values.into_iter().map(RestrictedExpression::new_string))
}

/// Returns a new [`Entity`] representing an account with the given [`EntityUid`] and [`GatewayClaims`].
fn account_entity(uid: EntityUid, claims: &GatewayClaims) -> Entity {
    let mut standard_customer = HashSet::new();
    let mut business_staff = HashSet::new();
    let mut business_admin = HashSet::new();
    let mut admin = HashSet::new();
    let mut max_weight: i64 = 0;

    let staff_weight = *ROLE_WEIGHTS.get("business_staff").unwrap_or(&i64::MAX);
    let business_admin_weight = *ROLE_WEIGHTS.get("business_admin").unwrap_or(&i64::MAX);
    let admin_weight = *ROLE_WEIGHTS.get("admin").unwrap_or(&i64::MAX);

    for m in &claims.payload.memberships {
        let Some(role) = &m.role else { continue };
        let Some(&weight) = ROLE_WEIGHTS.get(role) else {
            continue;
        };
        if weight > max_weight {
            max_weight = weight;
        }
        standard_customer.insert(m.domain.clone());
        if weight >= staff_weight {
            business_staff.insert(m.domain.clone());
        }
        if weight >= business_admin_weight {
            business_admin.insert(m.domain.clone());
        }
        if weight >= admin_weight {
            admin.insert(m.domain.clone());
        }
    }

    let mut attrs = HashMap::new();
    attrs.insert(
        "domainsAsStandardCustomer".to_string(),
        set_expr(standard_customer),
    );
    attrs.insert(
        "domainsAsBusinessStaff".to_string(),
        set_expr(business_staff),
    );
    attrs.insert(
        "domainsAsBusinessAdmin".to_string(),
        set_expr(business_admin),
    );
    attrs.insert("domainsAsAdmin".to_string(), set_expr(admin));
    attrs.insert(
        "maxRoleWeight".to_string(),
        RestrictedExpression::new_long(max_weight),
    );

    Entity::new(uid, attrs, HashSet::new()).expect("account entity attrs match schema")
}

/// Returns a new [`Entity`] representing a resource with the given [`EntityUid`] and domain.
fn resource_entity(uid: EntityUid, domain: &str) -> Entity {
    let mut attrs = HashMap::new();
    attrs.insert(
        "domain".to_string(),
        RestrictedExpression::new_string(domain.to_string()),
    );
    Entity::new(uid, attrs, HashSet::new()).expect("resource entity attrs match schema")
}

fn authorize(
    claims: &GatewayClaims,
    action: &str,
    domain: &str,
    required_weight: Option<i64>,
) -> bool {
    let principal_uid = euid("Account", "caller");
    let resource_uid = euid("Resource", domain);
    let principal = account_entity(principal_uid.clone(), claims);
    let resource = resource_entity(resource_uid.clone(), domain);

    let context = match required_weight {
        Some(w) => Context::from_pairs([(
            "requiredWeight".to_string(),
            RestrictedExpression::new_long(w),
        )]),
        None => Ok(Context::empty()),
    };
    let context = match context {
        Ok(c) => c,
        Err(e) => {
            log::error!("cedar context build failed: {e}");
            return false;
        }
    };

    let entities = match Entities::from_entities([principal, resource], Some(&SCHEMA)) {
        Ok(e) => e,
        Err(e) => {
            log::error!("cedar entities build failed: {e}");
            return false;
        }
    };
    let request = match Request::new(
        principal_uid,
        euid("Action", action),
        resource_uid,
        context,
        Some(&SCHEMA),
    ) {
        Ok(r) => r,
        Err(e) => {
            log::error!("cedar request build failed: {e}");
            return false;
        }
    };

    let response = Authorizer::new().is_authorized(&request, &POLICIES, &entities);
    response.decision() == Decision::Allow
}

/// Internal callers that look up any building's owners: notification routes an alert to every one.
const OWNER_RESOLVERS: &[&str] = &["system:notification-service"];

pub fn resolves_any_owner(claims: &GatewayClaims) -> bool {
    claims
        .payload
        .sub
        .as_deref()
        .is_some_and(|sub| OWNER_RESOLVERS.contains(&sub))
}

/// Internal callers that read any building's geometry; the edge never mints a `system:` subject.
const GEOMETRY_READERS: &[&str] = &["system:ap-simulator"];

pub fn reads_any_geometry(claims: &GatewayClaims) -> bool {
    claims
        .payload
        .sub
        .as_deref()
        .is_some_and(|sub| GEOMETRY_READERS.contains(&sub))
}

pub fn is_member_of(claims: &GatewayClaims, domain: &str) -> bool {
    authorize(claims, "Read", domain, None)
}

/// Forbidden unless the caller is a member of one of the building's domains, or reads all geometry.
pub fn require_read(claims: &GatewayClaims, building: &Building) -> Result<(), DomainError> {
    let allowed = reads_any_geometry(claims)
        || building
            .domains
            .iter()
            .any(|domain| is_member_of(claims, domain));
    allowed.then_some(()).ok_or_else(|| {
        DomainError::Forbidden("Requires membership in one of this building's domains".to_string())
    })
}

/// Forbidden unless the caller has an editing role in one of the building's domains.
pub fn require_edit(claims: &GatewayClaims, building: &Building) -> Result<(), DomainError> {
    let allowed = building
        .domains
        .iter()
        .any(|domain| authorize(claims, "Edit", domain, None));
    allowed.then_some(()).ok_or_else(|| {
        DomainError::Forbidden(
            "Requires an editing role in one of this building's domains".to_string(),
        )
    })
}

/// A new building may only be filed under domains the caller edits, all of them.
pub fn can_edit_every(claims: &GatewayClaims, domains: &[String]) -> bool {
    !domains.is_empty()
        && domains
            .iter()
            .all(|domain| authorize(claims, "Edit", domain, None))
}

pub fn filter_readable_domains_from_memberships(
    requested: &[String],
    claims: &GatewayClaims,
) -> Vec<String> {
    requested
        .iter()
        .filter(|domain| authorize(claims, "Read", domain, None))
        .cloned()
        .collect()
}

pub fn authorize_any(
    claims: &GatewayClaims,
    action: &str,
    domain: &str,
    required_weight: Option<i64>,
) -> bool {
    authorize(claims, action, domain, required_weight)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::identity::{ClaimsPayload, Membership};

    fn claims_with(memberships: Vec<(&str, &str)>) -> GatewayClaims {
        GatewayClaims {
            payload: ClaimsPayload {
                sub: Some("u1".to_string()),
                memberships: memberships
                    .into_iter()
                    .map(|(domain, role)| Membership {
                        domain: domain.to_string(),
                        role: Some(role.to_string()),
                        external_id: None,
                    })
                    .collect(),
                ..ClaimsPayload::default()
            },
            raw: "raw".to_string(),
        }
    }

    #[test]
    fn is_member_of_allows_any_role_in_the_domain() {
        let claims = claims_with(vec![("eng", "standard_customer")]);
        assert!(is_member_of(&claims, "eng"));
        assert!(!is_member_of(&claims, "other"));
    }

    fn owned_by(domains: &[&str]) -> Building {
        Building {
            id: "b1".to_string(),
            name: "b1".to_string(),
            rooms: vec![],
            domains: domains.iter().map(|domain| domain.to_string()).collect(),
        }
    }

    #[test]
    fn require_edit_needs_business_staff_or_higher_in_any_of_the_buildings_domains() {
        let building = owned_by(&["other", "eng"]);
        let staff = claims_with(vec![("eng", "business_staff")]);
        assert!(require_edit(&staff, &building).is_ok());

        let customer = claims_with(vec![("eng", "standard_customer")]);
        assert!(matches!(
            require_edit(&customer, &building),
            Err(DomainError::Forbidden(_))
        ));
    }

    #[test]
    fn require_read_needs_one_shared_domain_or_the_simulator() {
        let building = owned_by(&["eng", "ops"]);
        let member = claims_with(vec![("ops", "standard_customer")]);
        assert!(require_read(&member, &building).is_ok());

        let outsider = claims_with(vec![("other", "business_admin")]);
        assert!(require_read(&outsider, &building).is_err());

        let platform_admin = claims_with(vec![("other", "admin")]);
        assert!(
            require_read(&platform_admin, &building).is_err(),
            "a platform admin is not a member of every domain"
        );

        let mut simulator = claims_with(vec![]);
        simulator.payload.sub = Some("system:ap-simulator".to_string());
        assert!(require_read(&simulator, &building).is_ok());
    }

    #[test]
    fn can_edit_every_refuses_one_foreign_domain_and_an_empty_list() {
        let staff = claims_with(vec![
            ("eng", "business_staff"),
            ("ops", "standard_customer"),
        ]);
        assert!(can_edit_every(&staff, &["eng".to_string()]));
        assert!(!can_edit_every(
            &staff,
            &["eng".to_string(), "ops".to_string()]
        ));
        assert!(!can_edit_every(&staff, &[]));
    }

    #[test]
    fn scope_to_memberships_drops_domains_the_caller_cannot_read() {
        let claims = claims_with(vec![("eng", "standard_customer")]);
        let scoped = filter_readable_domains_from_memberships(
            &["eng".to_string(), "other".to_string()],
            &claims,
        );
        assert_eq!(scoped, vec!["eng".to_string()]);
    }

    #[test]
    fn unrecognized_role_is_ignored_not_treated_as_a_wildcard_grant() {
        let claims = claims_with(vec![("eng", "some_future_role")]);
        assert!(!is_member_of(&claims, "eng"));
    }
}
