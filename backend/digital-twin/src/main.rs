use env_logger::Env;
use log::info;
use std::env;
use std::net::SocketAddr;
use tokio::net::TcpListener;

use std::sync::Arc;

use digital_twin::adapters::driven::kafka_producer::KafkaEventProducer;
use digital_twin::adapters::driven::persistence::db::{self, MongoBuildings};
use digital_twin::adapters::driven::persistence::jobs::MongoUploadQueue;
use digital_twin::adapters::driven::persistence::placements::MongoPlacements;
use digital_twin::adapters::driven::redis_devices::RedisDevicesFeed;
use digital_twin::adapters::driving::{kafka_consumer, wifi_consumer, worker};
use digital_twin::adapters::{
    driven::outbound::{OutboundConfig, client},
    ratelimit::RateLimiter,
};
use digital_twin::build_router;
use digital_twin::service::buildings::Buildings;
use digital_twin::service::placements::Placements;
use digital_twin::service::provisioning::Provisioning;
use digital_twin::service::tracking::Tracking;
use digital_twin::state::AppState;

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let mongo_uri = env_or("MONGO_URI", "mongodb://localhost:27017");
    let buildings = db::connect(&mongo_uri, "ambitwin")
        .await
        .expect("Failed to connect to MongoDB");

    let outbound = OutboundConfig {
        telemetry_url: env_or("TELEMETRY_URL", "http://localhost:3000"),
        dashboard_url: env_or("DASHBOARD_URL", "http://localhost:3001"),
        notification_url: env_or("NOTIFICATION_URL", "http://localhost:3004"),
        sync_enabled: true,
        client: client(),
    };

    let kafka_brokers = env_or("KAFKA_BROKERS", "localhost:9092");
    let events = Arc::new(
        KafkaEventProducer::new(&kafka_brokers)
            .await
            .expect("Failed to configure Kafka producer"),
    );

    let placements = Arc::new(MongoPlacements::from_building_collection(&buildings));

    let store = Arc::new(MongoBuildings::new(buildings.clone()));
    let queue = Arc::new(
        MongoUploadQueue::from_building_collection(&buildings)
            .await
            .expect("create upload queue indexes"),
    );
    let downstream = Arc::new(outbound);

    let provisioning = Arc::new(Provisioning::new(
        store.clone(),
        queue,
        downstream.clone(),
        events.clone(),
    ));
    worker::spawn(provisioning.clone());
    kafka_consumer::spawn(&kafka_brokers, provisioning.clone());
    let redis_url = env_or("REDIS_URL", "redis://localhost:6379");
    let feed = Arc::new(RedisDevicesFeed::new(&redis_url).expect("REDIS_URL is a Redis URL"));
    wifi_consumer::spawn(
        &kafka_brokers,
        Arc::new(Tracking::new(
            store.clone(),
            placements.clone(),
            feed,
            events,
        )),
    );

    let state = AppState {
        buildings: Arc::new(Buildings::new(store.clone(), downstream)),
        placements: Arc::new(Placements::new(store, placements)),
        provisioning,
        rate_limiter: RateLimiter::new(true),
    };

    let app = build_router(state);
    let port = env_or("PORT", "3000");
    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("port is free");
    info!("Twin Service started on port {port}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .expect("server runs");
}

fn env_or(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_owned())
}
