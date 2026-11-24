pub use telemetry_schema::{MetricContract, MetricsDiscoveryResponse};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PreferenceDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<mongodb::bson::oid::ObjectId>,
    pub building_id: String,
    // a document first written by a cards save has no columns yet
    #[serde(default)]
    pub allowed_columns: Vec<String>,
    // absent until someone saves the building's cards: the client then shows its default
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cards: Option<Vec<String>>,
}
