use std::time::Duration;

use async_trait::async_trait;
use mongodb::Collection;
use mongodb::bson::{DateTime, Document, doc};
use mongodb::options::ReturnDocument;
use serde::{Deserialize, Serialize};

use crate::domain::{AcceptedUpload, Building, UploadStatus};
use crate::service::ports::UploadQueue;
use mongodb::IndexModel;
use mongodb::options::IndexOptions;

// The browser learns an upload finished by polling its status for up to 30s, so a finished
// record must outlive that wait before the TTL index removes it.
const FINISHED_RETENTION: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingUpload {
    id: String,
    building: Building,
    claims: String,
    status: UploadStatus,
    leased_until: Option<DateTime>,
    error: Option<String>,
    accepted_at: DateTime,
    finished_at: Option<DateTime>,
}

impl PendingUpload {
    fn accepted(upload: &AcceptedUpload) -> Self {
        Self {
            id: upload.id.clone(),
            building: upload.building.clone(),
            claims: upload.claims.clone(),
            status: UploadStatus::Pending,
            leased_until: None,
            error: None,
            accepted_at: DateTime::now(),
            finished_at: None,
        }
    }
}

impl From<PendingUpload> for AcceptedUpload {
    fn from(job: PendingUpload) -> Self {
        Self {
            id: job.id,
            building: job.building,
            claims: job.claims,
        }
    }
}

pub struct MongoUploadQueue {
    col: Collection<PendingUpload>,
}

impl MongoUploadQueue {
    /// Opens `pending_uploads` beside `buildings` and ensures the indexes its lookups need.
    pub async fn from_building_collection(
        buildings: &Collection<Building>,
    ) -> anyhow::Result<Self> {
        let col = buildings
            .client()
            .database(&buildings.namespace().db)
            .collection("pending_uploads");
        // Status polls and claims run constantly: without these, each one reads every queued upload,
        // and every upload carries its whole building.
        col.create_index(
            IndexModel::builder()
                .keys(doc! { "id": 1 })
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;
        col.create_index(
            IndexModel::builder()
                .keys(doc! { "status": 1, "leased_until": 1 })
                .build(),
        )
        .await?;
        col.create_index(
            IndexModel::builder()
                .keys(doc! { "finished_at": 1 })
                .options(
                    IndexOptions::builder()
                        .expire_after(FINISHED_RETENTION)
                        .build(),
                )
                .build(),
        )
        .await?;
        // A TTL index never removes a record without its field, so uploads finished before
        // expiry existed would otherwise stay forever.
        col.update_many(
            doc! { "status": { "$in": ["ready", "failed"] }, "finished_at": null },
            doc! { "$set": { "finished_at": DateTime::now() } },
        )
        .await?;
        Ok(Self { col })
    }

    async fn resolve(&self, id: &str, set: Document) -> anyhow::Result<Option<Duration>> {
        Ok(self
            .col
            .find_one_and_update(
                doc! { "id": { "$eq": id }, "status": "pending" },
                doc! { "$set": set },
            )
            .return_document(ReturnDocument::Before)
            .await?
            .map(|job| {
                let elapsed_ms =
                    DateTime::now().timestamp_millis() - job.accepted_at.timestamp_millis();
                Duration::from_millis(elapsed_ms.max(0) as u64)
            }))
    }
}

#[async_trait]
impl UploadQueue for MongoUploadQueue {
    async fn enqueue(&self, upload: &AcceptedUpload) -> anyhow::Result<()> {
        self.col.insert_one(PendingUpload::accepted(upload)).await?;
        Ok(())
    }

    async fn claim(&self, lease: Duration) -> anyhow::Result<Option<AcceptedUpload>> {
        let now = DateTime::now();
        let expires_at = DateTime::from_millis(now.timestamp_millis() + lease.as_millis() as i64);
        let claimable = doc! {
            "status": "pending",
            "$or": [{ "leased_until": null }, { "leased_until": { "$lte": now } }],
        };
        let lease = doc! { "$set": { "leased_until": expires_at } };

        Ok(self
            .col
            .find_one_and_update(claimable, lease)
            .return_document(ReturnDocument::After)
            .await?
            .map(AcceptedUpload::from))
    }

    async fn mark_ready(&self, id: &str) -> anyhow::Result<Option<Duration>> {
        self.resolve(
            id,
            doc! { "status": "ready", "leased_until": null, "finished_at": DateTime::now() },
        )
        .await
    }

    async fn mark_failed(&self, id: &str, error: &str) -> anyhow::Result<Option<Duration>> {
        self.resolve(
            id,
            doc! { "status": "failed", "leased_until": null, "error": error, "finished_at": DateTime::now() },
        )
        .await
    }

    async fn status(&self, id: &str) -> anyhow::Result<Option<UploadStatus>> {
        Ok(self
            .col
            .find_one(doc! { "id": { "$eq": id } })
            .await?
            .map(|job| job.status))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::fakes::building;

    #[test]
    fn a_freshly_accepted_upload_starts_pending_with_no_lease() {
        let upload = AcceptedUpload {
            id: "u1".to_string(),
            building: building("b1"),
            claims: "tok".to_string(),
        };

        let job = PendingUpload::accepted(&upload);

        assert_eq!(job.id, "u1");
        assert_eq!(job.status, UploadStatus::Pending);
        assert_eq!(job.leased_until, None);
        assert_eq!(job.error, None);
    }

    #[test]
    fn converting_a_job_back_to_an_accepted_upload_drops_queue_only_fields() {
        let upload = AcceptedUpload {
            id: "u1".to_string(),
            building: building("b1"),
            claims: "tok".to_string(),
        };
        let job = PendingUpload::accepted(&upload);

        let round_tripped = AcceptedUpload::from(job);

        assert_eq!(round_tripped.id, "u1");
        assert_eq!(round_tripped.claims, "tok");
        assert_eq!(round_tripped.building.id, "b1");
    }
}
