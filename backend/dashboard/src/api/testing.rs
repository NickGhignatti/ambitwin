//! Shared handler-test setup: callers with one membership, and a digital-twin that places
//! every building in `eng`.

use claims_schema::{ClaimsPayload, Membership};
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::infra::claims::GatewayClaims;
use crate::models::PreferenceDocument;
use crate::state::AppState;

/// Staff in `eng`: may read and edit every building the test twin answers for.
pub fn claims() -> GatewayClaims {
    claims_in("eng", "business_staff")
}

pub fn claims_in(domain: &str, role: &str) -> GatewayClaims {
    GatewayClaims {
        payload: ClaimsPayload {
            sub: Some("u1".to_string()),
            memberships: vec![Membership {
                domain: domain.to_string(),
                role: Some(role.to_string()),
                external_id: None,
            }],
            ..ClaimsPayload::default()
        },
        raw: "caller-claims".to_string(),
    }
}

/// A digital-twin that places every building in `eng`, or answers `500` when `is_down`.
async fn twin(is_down: bool) -> MockServer {
    let server = MockServer::start().await;
    let response = if is_down {
        ResponseTemplate::new(500)
    } else {
        ResponseTemplate::new(200).set_body_json(["eng"])
    };
    Mock::given(method("GET"))
        .and(path_regex("^/domain/"))
        .respond_with(response)
        .mount(&server)
        .await;
    server
}

// the twin is returned so it outlives the test: a dropped MockServer goes back to a shared pool
pub async fn make_state() -> (AppState, MockServer) {
    state_with(false).await
}

pub async fn state_with(is_twin_down: bool) -> (AppState, MockServer) {
    let twin = twin(is_twin_down).await;
    let options = mongodb::options::ClientOptions::parse("mongodb://localhost:27017")
        .await
        .unwrap();
    let collection = mongodb::Client::with_options(options)
        .unwrap()
        .database("_unit_test")
        .collection::<PreferenceDocument>("_prefs");
    (AppState::new(collection, twin.uri()), twin)
}
