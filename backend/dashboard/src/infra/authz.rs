use axum::http::StatusCode;
use cedar_policy::{
    Authorizer, Context, Decision, Entities, Entity, EntityId, EntityTypeName, EntityUid,
    PolicySet, Request, RestrictedExpression, Schema,
};
use claims_schema::ClaimsPayload;

use crate::infra::claims::GatewayClaims;
use crate::infra::twin::TwinDirectory;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::LazyLock;

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

/// `Ok` when the caller may see the building's view: Cedar `Read` on one of its domains.
/// `403` when not, `503` when digital-twin cannot say. Never allows on doubt.
pub async fn require_read(
    twin: &TwinDirectory,
    claims: &GatewayClaims,
    building_id: &str,
) -> Result<(), StatusCode> {
    require(twin, claims, building_id, can_read).await
}

/// `Ok` when the caller may change the building's view: Cedar `Edit` on one of its domains.
/// `403` when not, `503` when digital-twin cannot say. Never allows on doubt.
pub async fn require_edit(
    twin: &TwinDirectory,
    claims: &GatewayClaims,
    building_id: &str,
) -> Result<(), StatusCode> {
    require(twin, claims, building_id, can_edit).await
}

async fn require(
    twin: &TwinDirectory,
    claims: &GatewayClaims,
    building_id: &str,
    is_allowed: fn(&ClaimsPayload, &[String]) -> bool,
) -> Result<(), StatusCode> {
    let domains = twin
        .domains_of(building_id, &claims.raw)
        .await
        .map_err(|error| {
            log::error!("cannot read the domains of {building_id}: {error}");
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    if is_allowed(&claims.payload, &domains) {
        Ok(())
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

/// Whether the caller may see a building's view: Cedar `Read` on any of its domains.
pub fn can_read(claims: &ClaimsPayload, building_domains: &[String]) -> bool {
    building_domains
        .iter()
        .any(|domain| authorize(claims, "Read", domain))
}

/// Whether the caller may change a building's view: Cedar `Edit` on any of its domains.
/// An unknown building has no domains, so nobody may edit it.
pub fn can_edit(claims: &ClaimsPayload, building_domains: &[String]) -> bool {
    building_domains
        .iter()
        .any(|domain| authorize(claims, "Edit", domain))
}

fn authorize(claims: &ClaimsPayload, action: &str, domain: &str) -> bool {
    let principal_uid = euid("Account", "caller");
    let resource_uid = euid("Resource", domain);
    let entities = Entities::from_entities(
        [
            account_entity(principal_uid.clone(), claims),
            resource_entity(resource_uid.clone(), domain),
        ],
        Some(&SCHEMA),
    );
    let request = Request::new(
        principal_uid,
        euid("Action", action),
        resource_uid,
        Context::empty(),
        Some(&SCHEMA),
    );
    match (entities, request) {
        (Ok(entities), Ok(request)) => {
            Authorizer::new()
                .is_authorized(&request, &POLICIES, &entities)
                .decision()
                == Decision::Allow
        }
        (Err(error), _) => deny(&error),
        (_, Err(error)) => deny(&error),
    }
}

fn deny(error: &dyn std::fmt::Display) -> bool {
    log::error!("cedar request build failed: {error}");
    false
}

fn euid(type_name: &str, id: &str) -> EntityUid {
    EntityUid::from_type_name_and_id(
        EntityTypeName::from_str(type_name).expect("static entity type name is valid"),
        EntityId::new(id),
    )
}

fn set_expr(values: HashSet<String>) -> RestrictedExpression {
    RestrictedExpression::new_set(values.into_iter().map(RestrictedExpression::new_string))
}

// the schema requires every attribute, so all four role sets are built even though Edit reads one
fn account_entity(uid: EntityUid, claims: &ClaimsPayload) -> Entity {
    let weight_of = |role: &str| *ROLE_WEIGHTS.get(role).unwrap_or(&i64::MAX);
    let tiers = [
        ("domainsAsStandardCustomer", 0),
        ("domainsAsBusinessStaff", weight_of("business_staff")),
        ("domainsAsBusinessAdmin", weight_of("business_admin")),
        ("domainsAsAdmin", weight_of("admin")),
    ];
    let mut domains_by_tier: Vec<HashSet<String>> = vec![HashSet::new(); tiers.len()];
    let mut max_weight: i64 = 0;

    for membership in &claims.memberships {
        let Some(weight) = membership
            .role
            .as_deref()
            .and_then(|role| ROLE_WEIGHTS.get(role))
        else {
            continue;
        };
        max_weight = max_weight.max(*weight);
        for (tier, (_, threshold)) in tiers.iter().enumerate() {
            if weight >= threshold {
                domains_by_tier[tier].insert(membership.domain.clone());
            }
        }
    }

    let mut attributes: HashMap<String, RestrictedExpression> = tiers
        .iter()
        .zip(domains_by_tier)
        .map(|((name, _), domains)| (name.to_string(), set_expr(domains)))
        .collect();
    attributes.insert(
        "maxRoleWeight".to_string(),
        RestrictedExpression::new_long(max_weight),
    );
    Entity::new(uid, attributes, HashSet::new()).expect("account entity attrs match schema")
}

fn resource_entity(uid: EntityUid, domain: &str) -> Entity {
    let attributes = HashMap::from([(
        "domain".to_string(),
        RestrictedExpression::new_string(domain.to_string()),
    )]);
    Entity::new(uid, attributes, HashSet::new()).expect("resource entity attrs match schema")
}

#[cfg(test)]
mod tests {
    use super::{can_edit, can_read};
    use claims_schema::{ClaimsPayload, Membership};
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Case {
        name: String,
        memberships: Vec<CaseMembership>,
        action: String,
        domain: String,
        context: Option<serde_json::Value>,
        expected: String,
    }

    #[derive(Deserialize)]
    struct CaseMembership {
        domain: String,
        role: String,
    }

    fn claims_with(memberships: &[(&str, &str)]) -> ClaimsPayload {
        ClaimsPayload {
            sub: Some("caller".to_string()),
            memberships: memberships
                .iter()
                .map(|(domain, role)| Membership {
                    domain: domain.to_string(),
                    role: Some(role.to_string()),
                    external_id: None,
                })
                .collect(),
            ..ClaimsPayload::default()
        }
    }

    // the bundle is shared but this glue is dashboard's own copy: the golden cases keep it in step
    #[test]
    fn every_golden_read_and_edit_case_decides_as_in_every_other_service() {
        #[derive(Deserialize)]
        struct Fixture {
            cases: Vec<Case>,
        }
        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../../libs/auth-policy/fixtures/conformance.json"
        ))
        .expect("fixture parses");

        let cases: Vec<&Case> = fixture
            .cases
            .iter()
            .filter(|case| ["Read", "Edit"].contains(&case.action.as_str()))
            .filter(|case| case.context.is_none())
            .collect();
        assert!(cases.iter().any(|case| case.action == "Read"));
        assert!(cases.iter().any(|case| case.action == "Edit"));
        for case in cases {
            let decide = if case.action == "Read" {
                can_read
            } else {
                can_edit
            };
            let memberships: Vec<(&str, &str)> = case
                .memberships
                .iter()
                .map(|membership| (membership.domain.as_str(), membership.role.as_str()))
                .collect();
            let allowed = decide(
                &claims_with(&memberships),
                std::slice::from_ref(&case.domain),
            );
            assert_eq!(
                if allowed { "allow" } else { "deny" },
                case.expected,
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn staff_of_any_one_of_the_buildings_domains_may_edit() {
        let staff = claims_with(&[("eng", "business_staff")]);
        assert!(can_edit(&staff, &["sales".to_string(), "eng".to_string()]));
    }

    #[test]
    fn nobody_may_edit_a_building_with_no_domains() {
        assert!(!can_edit(&claims_with(&[("eng", "business_admin")]), &[]));
    }
}
