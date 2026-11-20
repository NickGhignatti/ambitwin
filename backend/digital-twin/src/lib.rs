use axum::routing::{get, post};
use axum::{Json, Router};

pub mod adapters;
pub mod domain;
pub mod service;
pub mod state;

use state::AppState;
use telemetry_schema::{LimitContract, MetricContract, ServiceMetricsContract};

/// The dashboard's column catalog. Built from `telemetry_schema`, the same types
/// telemetry's `/contracts` serves and the dashboard parses - a hand-rolled `json!`
/// here is how `key`/`metricKey` once drifted and emptied the catalog at runtime.
async fn contracts() -> Json<ServiceMetricsContract> {
    let room_column = |kind: &str, label: &str, interface: &str, unit: &str| MetricContract {
        kind: kind.to_owned(),
        label: label.to_owned(),
        interface: interface.to_owned(),
        unit: Some(unit.to_owned()),
        actions: vec![],
        limit: LimitContract::default(),
        source: None,
    };
    Json(ServiceMetricsContract {
        service: "digital-twin".to_owned(),
        metrics: vec![
            room_column("room_name", "Room Name", "IRoomName", "string"),
            room_column(
                "room_max_occupancy",
                "Room Max Occupancy",
                "IRoomMaxOccupancy",
                "people",
            ),
        ],
    })
}

fn public_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(adapters::metrics::health))
        .route("/health/", get(adapters::metrics::health))
        .route("/metrics", get(adapters::metrics::metrics_handler))
        .route("/metrics/", get(adapters::metrics::metrics_handler))
        .route("/contracts", get(contracts))
}

fn protected_routes() -> Router<AppState> {
    use adapters::driving::http_api::controllers::*;

    Router::new()
        .route("/register", post(add_building))
        .route(
            "/building/{id}",
            get(get_building_by_id).patch(update_building),
        )
        .route(
            "/building/{id}/placements",
            get(get_placements).put(save_placements),
        )
        .route("/building/{id}/status", get(get_upload_status))
        .route("/building/{id}/sync", post(resync_building))
        .route("/buildings/counts", post(get_building_counts))
        .route("/buildings/{domain}", get(get_building_by_domain))
        .route("/domain/{building}", get(get_domains_by_building))
}

pub fn build_router(state: AppState) -> Router {
    let rate_limiter = state.rate_limiter.clone();
    public_routes()
        .merge(protected_routes())
        .layer(axum::middleware::from_fn_with_state(
            rate_limiter,
            adapters::ratelimit::rate_limit,
        ))
        .layer(axum::middleware::from_fn(adapters::metrics::track_metrics))
        .with_state(state)
}

#[cfg(test)]
mod contracts_tests {
    use super::contracts;

    /// The dashboard parses this with `telemetry_schema::MetricsDiscoveryResponse`, so the
    /// wire names here are the shared contract's, not this service's own spelling.
    #[tokio::test]
    async fn the_catalog_is_served_under_the_services_real_name() {
        let body = serde_json::to_value(contracts().await.0).unwrap();
        assert_eq!(body["service"], "digital-twin");
    }

    #[tokio::test]
    async fn every_column_is_a_snake_case_id_with_no_limit() {
        let body = serde_json::to_value(contracts().await.0).unwrap();
        let metrics = body["metrics"].as_array().unwrap();

        let kinds: Vec<&str> = metrics
            .iter()
            .map(|m| m["kind"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, vec!["room_name", "room_max_occupancy"]);
        assert!(metrics.iter().all(|m| m["limit"] == serde_json::json!({})));
    }

    #[tokio::test]
    async fn the_catalog_round_trips_through_the_shared_contract() {
        let encoded = serde_json::to_value(contracts().await.0).unwrap();
        let decoded: telemetry_schema::MetricsDiscoveryResponse =
            serde_json::from_value(encoded).expect("dashboard's parser accepts this body");
        assert!(matches!(
            decoded,
            telemetry_schema::MetricsDiscoveryResponse::ServiceContract(_)
        ));
    }
}
