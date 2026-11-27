use axum::http::StatusCode;
use cucumber::{then, when};
use serde_json::Value;

use crate::support::fixtures::building_with;
use crate::support::registration::wait_for_status;
use crate::support::world::TwinWorld;

async fn upload(world: &mut TwinWorld, domain: &str, room_width: i64) {
    let payload =
        building_with(serde_json::json!({ "width": room_width, "height": 10, "depth": 10 }));
    world.call("POST", "/register", domain, Some(payload)).await;
    world.building_id = world
        .body
        .get("buildingId")
        .and_then(Value::as_str)
        .map(str::to_owned);
}

#[when(expr = "I upload a valid building description to organization {string}")]
pub async fn upload_valid(world: &mut TwinWorld, domain: String) {
    upload(world, &domain, 10).await;
}

#[when(expr = "I upload a building description with an invalid room to organization {string}")]
async fn upload_invalid(world: &mut TwinWorld, domain: String) {
    upload(world, &domain, -1).await;
}

#[then("the upload is acknowledged with a tracking handle")]
pub async fn acknowledged(world: &mut TwinWorld) {
    assert_eq!(
        world.status,
        StatusCode::ACCEPTED,
        "expected the upload to be accepted for later provisioning, body was {}",
        world.body
    );
    assert!(
        world.building_id.is_some(),
        "no tracking handle in the acknowledgement: {}",
        world.body
    );
}

#[then("the upload is refused without a tracking handle")]
async fn refused(world: &mut TwinWorld) {
    assert_eq!(
        world.status,
        StatusCode::BAD_REQUEST,
        "expected the malformed description to be refused outright, body was {}",
        world.body
    );
    assert!(
        world.building_id.is_none(),
        "a malformed upload must never be acknowledged, got handle {:?}",
        world.building_id
    );
}

#[then(expr = "the tracking handle eventually reports {string}")]
pub async fn handle_reports(world: &mut TwinWorld, expected: String) {
    wait_for_status(&world.router, world.handle(), &expected).await;
}
