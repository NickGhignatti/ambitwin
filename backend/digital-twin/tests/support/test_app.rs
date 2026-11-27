use std::sync::Arc;

use axum::Router;
use uuid::Uuid;

use digital_twin::adapters::driven::outbound::OutboundConfig;
use digital_twin::adapters::driven::persistence::db::{self, MongoBuildings};
use digital_twin::adapters::driven::persistence::jobs::MongoUploadQueue;
use digital_twin::adapters::driven::persistence::placements::MongoPlacements;
use digital_twin::adapters::driving::worker;
use digital_twin::adapters::ratelimit::RateLimiter;
use digital_twin::build_router;
use digital_twin::domain::Building;
use digital_twin::service::buildings::Buildings;
use digital_twin::service::placements::Placements;
use digital_twin::service::ports::RegistrationEvents;
use digital_twin::service::provisioning::Provisioning;
use digital_twin::state::AppState;

struct InstantlyResolvingEvents {
    tx: tokio::sync::mpsc::UnboundedSender<String>,
}

#[async_trait::async_trait]
impl RegistrationEvents for InstantlyResolvingEvents {
    async fn publish_building_registration_request(
        &self,
        building: &Building,
    ) -> anyhow::Result<()> {
        let _ = self.tx.send(building.id.clone());
        Ok(())
    }
}

pub struct TestApp {
    pub router: Router,
    pub provisioning: Arc<Provisioning>,
}

/// A full app on its own throwaway database; registrations resolve at once, no broker.
/// `label` names the suite in the database name.
pub async fn build(label: &str) -> TestApp {
    let uri = test_mongo_uri().await;
    let buildings = db::connect(&uri, &fresh_database_name(label))
        .await
        .expect("connect to test MongoDB");

    let outbound = OutboundConfig {
        telemetry_url: "http://127.0.0.1:1".to_string(),
        dashboard_url: "http://127.0.0.1:1".to_string(),
        notification_url: "http://127.0.0.1:1".to_string(),
        sync_enabled: false,
        client: reqwest::Client::new(),
    };
    let placements = Arc::new(MongoPlacements::from_building_collection(&buildings));

    let store = Arc::new(MongoBuildings::new(buildings.clone()));
    let queue = Arc::new(
        MongoUploadQueue::from_building_collection(&buildings)
            .await
            .expect("create upload queue indexes"),
    );
    let downstream = Arc::new(outbound);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let events = Arc::new(InstantlyResolvingEvents { tx });

    let provisioning = Arc::new(Provisioning::new(
        store.clone(),
        queue,
        downstream.clone(),
        events,
    ));
    worker::spawn(provisioning.clone());
    let resolver = provisioning.clone();
    tokio::spawn(async move {
        while let Some(id) = rx.recv().await {
            let _ = resolver.resolve(&id, None).await;
        }
    });

    let router = build_router(AppState {
        buildings: Arc::new(Buildings::new(store.clone(), downstream)),
        placements: Arc::new(Placements::new(store, placements)),
        provisioning: provisioning.clone(),
        rate_limiter: RateLimiter::new(false),
    });

    TestApp {
        router,
        provisioning,
    }
}

/// The router alone, for tests that only speak HTTP.
pub async fn app() -> Router {
    build("api").await.router
}

/// A fresh, randomly named database on `MONGO_URI`, for tests that talk to one
/// adapter directly. `label` names the suite, so a leftover is easy to trace.
pub async fn test_database(label: &str) -> mongodb::Database {
    let uri = test_mongo_uri().await;
    mongodb::Client::with_uri_str(&uri)
        .await
        .expect("connect to test MongoDB")
        .database(&fresh_database_name(label))
}

fn fresh_database_name(label: &str) -> String {
    format!("digital_twin_{label}_{}", Uuid::new_v4().simple())
}

static SWEEP: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

/// `MONGO_URI`, once the databases earlier runs left behind are dropped.
async fn test_mongo_uri() -> String {
    let uri = std::env::var("MONGO_URI").expect(
        "MONGO_URI must be set — refusing to guess localhost and write into a dev database",
    );
    // every caller waits, so no test creates its database while the sweep still lists names
    SWEEP.get_or_init(|| sweep_stale_test_databases(&uri)).await;
    uri
}

/// Drops the test databases earlier runs left. A test cannot drop its own on the way
/// out (the `Router` outlives it), so each run cleans up after the last one.
async fn sweep_stale_test_databases(uri: &str) {
    let Ok(client) = mongodb::Client::with_uri_str(uri).await else {
        return;
    };
    let Ok(names) = client.list_database_names().await else {
        return;
    };
    for name in names.into_iter().filter(|name| is_test_database(name)) {
        let _ = client.database(&name).drop().await;
    }
}

fn is_test_database(name: &str) -> bool {
    let Some((_label, suffix)) = name
        .strip_prefix("digital_twin_")
        .and_then(|rest| rest.rsplit_once('_'))
    else {
        return false;
    };
    suffix.len() == 32 && suffix.chars().all(|c| c.is_ascii_hexdigit())
}
