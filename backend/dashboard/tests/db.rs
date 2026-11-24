//! MongoDB adapter, against a real MongoDB.
//!
//! Picked up by `scripts/rust-integration-tests.sh`, which runs `tests/*.rs`
//! and nothing else — the same runner every other Rust service uses. These moved
//! here from `src/infra/db.rs`, where they sat behind `#[ignore]` and ran nowhere:
//! CI started no Mongo for this service, nothing in the repo passes `--ignored`,
//! and the runner could not see in-module tests. All three had to be fixed for
//! the adapter to be covered at all.
//!
//! MONGO_URI selects the server (defaults to localhost:27017); CI sets it.

use dashboard::infra::db::{load_all, upsert_cards, upsert_preference};
use dashboard::models::PreferenceDocument;
use mongodb::{Client, Collection, options::ClientOptions};

/// A collection in a dedicated test database, named per test so cases that run
/// concurrently in one binary cannot drop each other's documents.
async fn test_col(name: &str) -> Collection<PreferenceDocument> {
    let uri =
        std::env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
    let opts = ClientOptions::parse(&uri).await.unwrap();
    let client = Client::with_options(opts).unwrap();
    let col = client
        .database("ambitwin_test")
        .collection::<PreferenceDocument>(name);
    col.drop().await.unwrap();
    col
}

#[tokio::test]
async fn upsert_then_load_returns_the_document() {
    let col = test_col("upsert_and_load").await;

    upsert_preference(&col, "building-1", &["room_name".to_string()])
        .await
        .unwrap();

    let all = load_all(&col).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].building_id, "building-1");
    assert_eq!(all[0].allowed_columns, vec!["room_name"]);
}

/// Upsert, not insert: the unique index on building_id means a second write for
/// the same building must replace the first rather than fail or duplicate it.
#[tokio::test]
async fn upsert_replaces_the_existing_document_for_a_building() {
    let col = test_col("upsert_replaces").await;

    upsert_preference(&col, "building-1", &["room_name".to_string()])
        .await
        .unwrap();
    upsert_preference(&col, "building-1", &["co2".to_string(), "temp".to_string()])
        .await
        .unwrap();

    let all = load_all(&col).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].allowed_columns, vec!["co2", "temp"]);
}

#[tokio::test]
async fn load_all_on_an_empty_collection_is_empty_not_an_error() {
    let col = test_col("load_empty").await;

    let all = load_all(&col).await.unwrap();

    assert!(all.is_empty());
}

/// Two buildings must not bleed into each other: load_all backs the in-memory
/// preference map the tunnel consults per tick, so a cross-building leak here
/// would forward one building's telemetry onto another's channel.
#[tokio::test]
async fn load_all_returns_every_building_separately() {
    let col = test_col("load_many").await;

    upsert_preference(&col, "building-a", &["temp".to_string()])
        .await
        .unwrap();
    upsert_preference(&col, "building-b", &["co2".to_string()])
        .await
        .unwrap();

    let mut all = load_all(&col).await.unwrap();
    all.sort_by(|a, b| a.building_id.cmp(&b.building_id));

    assert_eq!(all.len(), 2);
    assert_eq!(all[0].building_id, "building-a");
    assert_eq!(all[0].allowed_columns, vec!["temp"]);
    assert_eq!(all[1].building_id, "building-b");
    assert_eq!(all[1].allowed_columns, vec!["co2"]);
}

/// Columns and cards share one document but are saved apart, so neither save may drop the other.
#[tokio::test]
async fn saving_columns_keeps_the_cards_and_saving_cards_keeps_the_columns() {
    let col = test_col("columns_and_cards").await;

    upsert_cards(&col, "building-1", &["alerts".to_string()])
        .await
        .unwrap();
    upsert_preference(&col, "building-1", &["room_name".to_string()])
        .await
        .unwrap();
    upsert_cards(
        &col,
        "building-1",
        &["rooms".to_string(), "co2".to_string()],
    )
    .await
    .unwrap();

    let all = load_all(&col).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].allowed_columns, vec!["room_name"]);
    assert_eq!(
        all[0].cards,
        Some(vec!["rooms".to_string(), "co2".to_string()])
    );
}

/// A building whose cards were saved before any column still loads, with no columns.
#[tokio::test]
async fn a_document_with_only_cards_loads_with_no_columns() {
    let col = test_col("only_cards").await;

    upsert_cards(&col, "building-1", &["rooms".to_string()])
        .await
        .unwrap();

    let all = load_all(&col).await.unwrap();
    assert!(all[0].allowed_columns.is_empty());
    assert_eq!(all[0].cards, Some(vec!["rooms".to_string()]));
}
