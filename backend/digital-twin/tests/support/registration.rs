use std::time::{Duration, Instant};

use axum::Router;
use axum::http::StatusCode;
use serde_json::Value;

use crate::support::fixtures::{editor_token, mock_building, token};
use crate::support::http_client::send;

const READY_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Polls `/building/{handle}/status` until it reports `expected`, panicking after
/// `READY_TIMEOUT`. Uploads are provisioned in the background, so a test cannot
/// read the building straight after posting it.
pub async fn wait_for_status(router: &Router, handle: &str, expected: &str) {
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        let res = send(
            router.clone(),
            "GET",
            &format!("/building/{handle}/status"),
            Some(&token()),
            None,
        )
        .await;
        if res.body["status"] == expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "upload {handle} still reported {} after {READY_TIMEOUT:?}, expected {expected:?}",
            res.body["status"]
        );
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

pub async fn register(router: &Router, payload: Value) -> String {
    let res = send(
        router.clone(),
        "POST",
        "/register",
        Some(&editor_token()),
        Some(payload),
    )
    .await;
    assert_eq!(res.status, StatusCode::ACCEPTED, "body was {}", res.body);
    let handle = res.body["buildingId"]
        .as_str()
        .expect("a tracking handle")
        .to_string();
    wait_for_status(router, &handle, "ready").await;
    handle
}

pub async fn register_building(router: Router) -> String {
    register(&router, mock_building()).await
}
