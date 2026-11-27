use async_trait::async_trait;
use futures::TryStreamExt;
use mongodb::{
    Client, Collection, IndexModel,
    bson::{Document, doc},
    options::{ClientOptions, IndexOptions},
};
use std::collections::HashMap;

use crate::domain::Building;
use crate::service::ports::BuildingStore;

pub struct MongoBuildings {
    col: Collection<Building>,
}

impl MongoBuildings {
    pub fn new(col: Collection<Building>) -> Self {
        Self { col }
    }
}

#[async_trait]
impl BuildingStore for MongoBuildings {
    async fn find_by_id(&self, id: &str) -> anyhow::Result<Option<Building>> {
        Ok(self.col.find_one(doc! { "id": { "$eq": id } }).await?)
    }

    async fn find_by_domain(&self, domain: &str) -> anyhow::Result<Vec<Building>> {
        let cursor = self.col.find(doc! { "domains": { "$eq": domain } }).await?;
        Ok(cursor.try_collect().await?)
    }

    async fn find_by_name(&self, name: &str) -> anyhow::Result<Vec<Building>> {
        let cursor = self.col.find(doc! { "name": { "$eq": name } }).await?;
        Ok(cursor.try_collect().await?)
    }

    async fn upsert(&self, building: &Building) -> anyhow::Result<()> {
        self.col
            .replace_one(doc! { "id": &building.id }, building)
            .upsert(true)
            .await?;
        Ok(())
    }

    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        self.col.delete_one(doc! { "id": id }).await?;
        Ok(())
    }

    async fn counts_by_domain(&self, domains: &[String]) -> anyhow::Result<HashMap<String, i64>> {
        if domains.is_empty() {
            return Ok(HashMap::new());
        }
        let pipeline = vec![
            doc! { "$unwind": "$domains" },
            doc! { "$match": { "domains": { "$in": domains } } },
            doc! { "$group": { "_id": "$domains", "count": { "$sum": 1 } } },
        ];
        let docs: Vec<Document> = self.col.aggregate(pipeline).await?.try_collect().await?;
        Ok(parse_domain_counts(&docs))
    }
}

pub async fn connect(uri: &str, db_name: &str) -> anyhow::Result<Collection<Building>> {
    let opts = ClientOptions::parse(uri).await?;
    let client = Client::with_options(opts)?;
    let col = client.database(db_name).collection::<Building>("buildings");

    let unique_id = IndexModel::builder()
        .keys(doc! { "id": 1 })
        .options(IndexOptions::builder().unique(true).build())
        .build();
    col.create_index(unique_id).await?;

    let domains_index = IndexModel::builder().keys(doc! { "domains": 1 }).build();
    col.create_index(domains_index).await?;

    Ok(col)
}

fn parse_domain_counts(docs: &[Document]) -> HashMap<String, i64> {
    let mut result = HashMap::new();
    for doc in docs {
        let Ok(domain) = doc.get_str("_id") else {
            continue;
        };
        let count = doc.get_i32("count").unwrap_or(0);
        result.insert(domain.to_string(), count as i64);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_domain_counts_reads_id_and_count_per_document() {
        let docs = vec![
            doc! { "_id": "eng", "count": 2 },
            doc! { "_id": "other", "count": 1 },
        ];
        let counts = parse_domain_counts(&docs);
        assert_eq!(counts.get("eng"), Some(&2));
        assert_eq!(counts.get("other"), Some(&1));
    }

    #[test]
    fn parse_domain_counts_defaults_missing_count_to_zero() {
        let docs = vec![doc! { "_id": "eng" }];
        assert_eq!(parse_domain_counts(&docs).get("eng"), Some(&0));
    }

    #[test]
    fn parse_domain_counts_skips_documents_without_an_id() {
        let docs = vec![doc! { "count": 5 }];
        assert!(parse_domain_counts(&docs).is_empty());
    }
}
