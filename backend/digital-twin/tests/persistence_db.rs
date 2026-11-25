use mongodb::options::ClientOptions;
use uuid::Uuid;

use digital_twin::adapters::driven::persistence::db::MongoBuildings;
use digital_twin::domain::{Building, Coordinates, Dimensions, Room};
use digital_twin::service::ports::BuildingStore;

fn dummy_room(id: &str) -> Room {
    Room {
        id: id.to_string(),
        name: id.to_string(),
        capacity: 10.0,
        position: Coordinates {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        dimensions: Dimensions {
            width: 1.0,
            height: 1.0,
            depth: 1.0,
        },
        color: None,
    }
}

fn dummy_building(domains: Vec<&str>) -> Building {
    Building {
        id: Uuid::new_v4().to_string(),
        name: "Test Building".to_string(),
        rooms: vec![dummy_room("r1")],
        domains: domains.into_iter().map(str::to_string).collect(),
    }
}

async fn test_store() -> MongoBuildings {
    let uri =
        std::env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
    let opts = ClientOptions::parse(&uri).await.unwrap();
    let client = mongodb::Client::with_options(opts).unwrap();
    MongoBuildings::new(
        client
            .database("digital_twin_test")
            .collection::<Building>(&format!("buildings_{}", Uuid::new_v4())),
    )
}

#[tokio::test]
async fn upsert_and_find_by_id_round_trips() {
    let store = test_store().await;
    let building = dummy_building(vec!["eng"]);
    store.upsert(&building).await.unwrap();

    let found = store.find_by_id(&building.id).await.unwrap();
    assert_eq!(found.unwrap().id, building.id);
}

#[tokio::test]
async fn find_by_id_returns_none_for_unknown_id() {
    let store = test_store().await;
    assert!(store.find_by_id("missing").await.unwrap().is_none());
}

#[tokio::test]
async fn find_by_domain_returns_only_matching_buildings() {
    let store = test_store().await;
    store.upsert(&dummy_building(vec!["eng"])).await.unwrap();
    store.upsert(&dummy_building(vec!["other"])).await.unwrap();

    let found = store.find_by_domain("eng").await.unwrap();
    assert_eq!(found.len(), 1);
}

#[tokio::test]
async fn upsert_replaces_an_existing_building() {
    let store = test_store().await;
    let mut building = dummy_building(vec!["eng"]);
    store.upsert(&building).await.unwrap();

    building.name = "Renamed".to_string();
    store.upsert(&building).await.unwrap();

    let found = store.find_by_id(&building.id).await.unwrap().unwrap();
    assert_eq!(found.name, "Renamed");
}

#[tokio::test]
async fn counts_by_domain_only_counts_requested_domains() {
    let store = test_store().await;
    store.upsert(&dummy_building(vec!["eng"])).await.unwrap();
    store.upsert(&dummy_building(vec!["eng"])).await.unwrap();
    store.upsert(&dummy_building(vec!["other"])).await.unwrap();

    let counts = store
        .counts_by_domain(&["eng".to_string(), "unknown".to_string()])
        .await
        .unwrap();
    assert_eq!(counts.get("eng"), Some(&2));
    assert_eq!(counts.get("other"), None);
    assert_eq!(counts.get("unknown"), None);
}

#[tokio::test]
async fn counts_by_domain_returns_empty_map_for_empty_request() {
    let store = test_store().await;
    let counts = store.counts_by_domain(&[]).await.unwrap();
    assert!(counts.is_empty());
}
