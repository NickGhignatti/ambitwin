use futures::TryStreamExt;
use mongodb::{
    Client, Collection, IndexModel,
    bson::doc,
    options::{ClientOptions, IndexOptions},
};

use crate::models::PreferenceDocument;

pub async fn connect(uri: &str) -> anyhow::Result<Collection<PreferenceDocument>> {
    let opts = ClientOptions::parse(uri).await?;
    let client = Client::with_options(opts)?;
    let col = client
        .database("ambitwin")
        .collection::<PreferenceDocument>("preferences");

    // Unique index: enforces one document per building at the database level.
    let index = IndexModel::builder()
        .keys(doc! { "building_id": 1 })
        .options(IndexOptions::builder().unique(true).build())
        .build();
    col.create_index(index).await?;

    Ok(col)
}

/// Fetches every preference document. Called once at startup to seed in-memory DashMap.
pub async fn load_all(
    col: &Collection<PreferenceDocument>,
) -> anyhow::Result<Vec<PreferenceDocument>> {
    let cursor = col.find(doc! {}).await?;
    Ok(cursor.try_collect().await?)
}

/// Upserts a single building's columns, leaving its cards as they are.
pub async fn upsert_preference(
    col: &Collection<PreferenceDocument>,
    building_id: &str,
    allowed_columns: &[String],
) -> anyhow::Result<()> {
    set_field(
        col,
        building_id,
        doc! { "allowed_columns": allowed_columns },
    )
    .await
}

/// Upserts a single building's cards, leaving its columns as they are.
pub async fn upsert_cards(
    col: &Collection<PreferenceDocument>,
    building_id: &str,
    cards: &[String],
) -> anyhow::Result<()> {
    set_field(col, building_id, doc! { "cards": cards }).await
}

// `$set`, not a replace: columns and cards are saved apart, and a replace would drop the other
async fn set_field(
    col: &Collection<PreferenceDocument>,
    building_id: &str,
    field: mongodb::bson::Document,
) -> anyhow::Result<()> {
    col.update_one(doc! { "building_id": building_id }, doc! { "$set": field })
        .upsert(true)
        .await?;
    Ok(())
}
