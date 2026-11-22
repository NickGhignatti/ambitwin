use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const RAW_CHANNEL: &str = "telemetry:raw";
pub const FILTERED_CHANNEL_PREFIX: &str = "telemetry:filtered:";
pub const FILTERED_CHANNEL_PATTERN: &str = "telemetry:filtered:*";

pub fn filtered_channel(building_id: &str) -> String {
    format!("{FILTERED_CHANNEL_PREFIX}{building_id}")
}

pub fn building_of_filtered_channel(channel: &str) -> &str {
    channel
        .strip_prefix(FILTERED_CHANNEL_PREFIX)
        .unwrap_or(channel)
}

/// One metric, one number. A device measuring several metrics sends one reading for each.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TelemetryReading {
    #[serde(rename = "type")]
    pub metric: String,
    pub room_id: String,
    #[serde(rename = "timestamp")]
    pub ts_ms: i64,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryEnvelope {
    pub building_id: String,
    #[serde(rename = "ingestedAt")]
    pub ingested_at_ms: i64,
    pub readings: Vec<Value>,
}

impl TelemetryEnvelope {
    pub fn channel(&self) -> String {
        filtered_channel(&self.building_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn reading() -> TelemetryReading {
        TelemetryReading {
            metric: "temperature".to_string(),
            room_id: "r1".to_string(),
            ts_ms: 1_700_000_000_000,
            value: 21.5,
        }
    }

    #[test]
    fn a_reading_carries_the_names_the_browser_reads() {
        assert_eq!(
            serde_json::to_value(reading()).unwrap(),
            json!({
                "type": "temperature",
                "roomId": "r1",
                "timestamp": 1_700_000_000_000i64,
                "value": 21.5,
            }),
        );
    }

    #[test]
    fn an_integer_value_is_read_as_a_number() {
        let raw = json!({
            "type": "people_count",
            "roomId": "r1",
            "timestamp": 1_700_000_000_000i64,
            "value": 20,
        });
        let parsed: TelemetryReading = serde_json::from_value(raw).unwrap();
        assert_eq!(parsed.value, 20.0);
    }

    #[test]
    fn the_envelope_is_a_building_a_timestamp_and_its_tick_of_readings() {
        let envelope = TelemetryEnvelope {
            building_id: "b1".to_string(),
            ingested_at_ms: 1_700_000_000_500,
            readings: vec![serde_json::to_value(reading()).unwrap()],
        };

        let encoded = serde_json::to_value(&envelope).unwrap();
        assert_eq!(encoded["buildingId"], "b1");
        assert_eq!(encoded["ingestedAt"], 1_700_000_000_500i64);
        assert_eq!(encoded["readings"].as_array().unwrap().len(), 1);
        assert_eq!(
            serde_json::from_value::<TelemetryEnvelope>(encoded).unwrap(),
            envelope,
        );
    }

    #[test]
    fn an_envelope_without_its_readings_or_building_is_not_an_envelope() {
        assert!(
            serde_json::from_value::<TelemetryEnvelope>(
                json!({ "buildingId": "b1", "ingestedAt": 1 })
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<TelemetryEnvelope>(json!({ "ingestedAt": 1, "readings": [] }))
                .is_err()
        );
    }

    #[test]
    fn a_building_round_trips_through_its_own_channel_name() {
        for building_id in ["b1", "site:b1"] {
            let channel = filtered_channel(building_id);
            assert!(channel.starts_with(FILTERED_CHANNEL_PREFIX));
            assert_eq!(building_of_filtered_channel(&channel), building_id);
        }
    }

    #[test]
    fn a_channel_without_the_prefix_is_its_own_building_id() {
        assert_eq!(
            building_of_filtered_channel("notifications"),
            "notifications"
        );
    }
}
