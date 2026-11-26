use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::json;
use std::time::Duration;

use crate::domain::{Building, normalize_room_name};
use crate::service::ports::DownstreamSync;

const TIMEOUT: Duration = Duration::from_secs(5);

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .expect("an HTTP client with a timeout is constructible")
}

#[derive(Clone)]
pub struct OutboundConfig {
    pub telemetry_url: String,
    pub dashboard_url: String,
    pub notification_url: String,
    pub sync_enabled: bool,
    pub client: reqwest::Client,
}

#[async_trait]
impl DownstreamSync for OutboundConfig {
    async fn clone_thresholds(&self, building: &Building, claims: &str) -> anyhow::Result<()> {
        if !self.sync_enabled {
            return Ok(());
        }
        let payload = json!({
            "name": building.name,
            "rooms": building.rooms.iter().map(|r| json!({
                "id": r.id,
                "name": normalize_room_name(Some(&r.name), &r.id),
            })).collect::<Vec<_>>(),
        });
        let url = format!(
            "{}/thresholds/buildings/{}",
            self.telemetry_url,
            urlencoding::encode(&building.id)
        );
        let response = self
            .client
            .put(&url)
            .headers(auth_headers(claims))
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let details = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to sync sensor threshold clone: {status} {details}");
        }
        Ok(())
    }

    async fn init_preferences(&self, building_id: &str, claims: &str) {
        if !self.sync_enabled {
            return;
        }
        let url = format!(
            "{}/preferences/init/{}",
            self.dashboard_url,
            urlencoding::encode(building_id)
        );
        // a refused init (403) is a response, not a transport error: without this it passes silently
        if let Err(err) = self
            .client
            .post(&url)
            .headers(auth_headers(claims))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
        {
            log::error!("failed to init building preferences: {err}");
        }
    }

    async fn notify_provisioning_failed(&self, building_id: &str, error: &str) {
        if !self.sync_enabled {
            return;
        }
        let url = format!("{}/trigger", self.notification_url);
        let payload = json!({
            "buildingName": building_id,
            "message": format!("Provisioning failed for building {building_id}: {error}"),
            "type": "danger",
        });
        if let Err(err) = self
            .client
            .post(&url)
            .headers(auth_headers(&system_claims_header()))
            .json(&payload)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
        {
            log::error!(
                "[notification] failed to notify provisioning failure for {building_id}: {err}"
            );
        }
    }
}

fn auth_headers(claims: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("content-type", "application/json".parse().unwrap());
    if let Ok(value) = claims.parse() {
        headers.insert("x-gateway-claims", value);
    }
    headers
}

// the failure alert is sent by the worker, with no end user to forward claims from
fn system_claims_header() -> String {
    STANDARD.encode(
        r#"{"sub":"system:digital-twin","accountName":"system:digital-twin","memberships":[]}"#,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::fakes::building;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn config(server: &MockServer) -> OutboundConfig {
        OutboundConfig {
            telemetry_url: server.uri(),
            dashboard_url: server.uri(),
            notification_url: server.uri(),
            sync_enabled: true,
            client: client(),
        }
    }

    #[tokio::test]
    async fn a_stalled_downstream_times_out_instead_of_hanging() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/thresholds/buildings/b1"))
            .respond_with(ResponseTemplate::new(200).set_delay(TIMEOUT * 3))
            .mount(&server)
            .await;

        let cfg = config(&server).await;
        let started = std::time::Instant::now();
        let result = cfg.clone_thresholds(&building("b1"), "tok-123").await;

        assert!(result.is_err());
        assert!(started.elapsed() < TIMEOUT * 2);
    }

    #[tokio::test]
    async fn forwards_the_callers_claims_header_verbatim() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/thresholds/buildings/b1"))
            .and(header("x-gateway-claims", "tok-123"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let cfg = config(&server).await;
        cfg.clone_thresholds(&building("b1"), "tok-123")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn skipped_entirely_when_sync_is_disabled() {
        let cfg = OutboundConfig {
            telemetry_url: "http://127.0.0.1:1".to_string(),
            dashboard_url: "http://127.0.0.1:1".to_string(),
            notification_url: "http://127.0.0.1:1".to_string(),
            sync_enabled: false,
            client: reqwest::Client::new(),
        };
        cfg.clone_thresholds(&building("b1"), "tok-123")
            .await
            .unwrap();
        cfg.init_preferences("b1", "tok-123").await;
        cfg.notify_provisioning_failed("b1", "boom").await;
    }

    #[tokio::test]
    async fn init_preferences_forwards_the_callers_claims_header() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/preferences/init/b1"))
            .and(header("x-gateway-claims", "tok-123"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let cfg = config(&server).await;
        cfg.init_preferences("b1", "tok-123").await;
    }

    #[tokio::test]
    async fn notify_provisioning_failed_never_fails_the_caller_on_transport_error() {
        let cfg = OutboundConfig {
            telemetry_url: "http://127.0.0.1:1".to_string(),
            dashboard_url: "http://127.0.0.1:1".to_string(),
            notification_url: "http://127.0.0.1:1".to_string(),
            sync_enabled: true,
            client: reqwest::Client::new(),
        };
        cfg.notify_provisioning_failed("b1", "boom").await;
    }

    #[tokio::test]
    async fn notify_provisioning_failed_posts_to_the_trigger_endpoint_as_a_system_caller() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/trigger"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let cfg = config(&server).await;
        cfg.notify_provisioning_failed("b1", "telemetry said no")
            .await;
    }
}
