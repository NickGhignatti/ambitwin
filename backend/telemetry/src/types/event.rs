use crate::types::reading::Reading;

#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryEvent {
    pub metric: String,
    pub building_id: String,
    pub room_id: String,
    pub ts_ms: i64,
    pub value: f64,
    pub ingested_at_ms: i64,
}

impl TelemetryEvent {
    pub fn from_reading(reading: &Reading, ingested_at_ms: i64) -> Self {
        Self {
            metric: reading.metric.clone(),
            building_id: reading.building_id.clone(),
            room_id: reading.room_id.clone(),
            ts_ms: reading.ts_ms,
            value: reading.value,
            ingested_at_ms,
        }
    }
}

pub use telemetry_schema::AlertEvent as AlertPayload;
