use serde::{Deserialize, Serialize};

pub const ALERTS_TOPIC: &str = "alerts";
pub const ALERTS_DLQ_TOPIC: &str = "alerts.dlq";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoundDirection {
    #[serde(rename = "high")]
    Above,
    #[serde(rename = "low")]
    Below,
}

impl BoundDirection {
    pub fn wire_name(self) -> &'static str {
        match self {
            BoundDirection::Above => "high",
            BoundDirection::Below => "low",
        }
    }
}

/// A reading outside its limit. `value` is the reading, `threshold` the end it crossed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertEvent {
    pub building_id: String,
    pub room_id: String,
    /// Display text from twin's registration; the ids stay the keys, since names can repeat.
    pub building_name: String,
    pub room_name: String,
    #[serde(rename = "type")]
    pub metric: String,
    pub value: f64,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub direction: BoundDirection,
    pub threshold: f64,
    #[serde(rename = "timestamp")]
    pub ts_ms: i64,
}

/// The metrics notification delivers; telemetry pins it to exactly the plugins that declare a limit.
pub const ALERTABLE_METRICS: &[&str] = &[
    "temperature",
    "people_count",
    "co2",
    "indoor_aqi",
    "humidity",
];

impl AlertEvent {
    pub fn partition_key(&self) -> String {
        format!("{}:{}", self.building_id, self.room_id)
    }

    pub fn is_alertable(&self) -> bool {
        ALERTABLE_METRICS.contains(&self.metric.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn breach() -> AlertEvent {
        AlertEvent {
            building_id: "b1".to_string(),
            room_id: "r1".to_string(),
            building_name: "HQ".to_string(),
            room_name: "Lab 1".to_string(),
            metric: "temperature".to_string(),
            value: 40.0,
            label: "Temperature".to_string(),
            unit: Some("°C".to_string()),
            direction: BoundDirection::Above,
            threshold: 25.0,
            ts_ms: 1_700_000_000_000,
        }
    }

    #[test]
    fn the_wire_shape_carries_the_metric_and_its_value() {
        assert_eq!(
            serde_json::to_value(breach()).unwrap(),
            json!({
                "buildingId": "b1",
                "roomId": "r1",
                "buildingName": "HQ",
                "roomName": "Lab 1",
                "type": "temperature",
                "value": 40.0,
                "label": "Temperature",
                "unit": "°C",
                "direction": "high",
                "threshold": 25.0,
                "timestamp": 1_700_000_000_000i64,
            }),
        );
    }

    #[test]
    fn a_metric_without_a_unit_omits_it() {
        let aqi = AlertEvent {
            metric: "indoor_aqi".to_string(),
            unit: None,
            ..breach()
        };
        assert!(serde_json::to_value(aqi).unwrap().get("unit").is_none());
    }

    #[test]
    fn a_below_limit_breach_is_low_on_the_wire() {
        let mut alert = breach();
        alert.direction = BoundDirection::Below;
        assert_eq!(serde_json::to_value(alert).unwrap()["direction"], "low");
    }

    #[test]
    fn what_the_producer_writes_is_what_the_consumer_reads() {
        let encoded = serde_json::to_string(&breach()).unwrap();
        assert_eq!(
            serde_json::from_str::<AlertEvent>(&encoded).unwrap(),
            breach()
        );
    }

    #[test]
    fn alerts_are_keyed_so_one_room_keeps_its_order_in_a_partition() {
        assert_eq!(breach().partition_key(), "b1:r1");
    }

    #[test]
    fn only_a_metric_notification_delivers_is_alertable() {
        assert!(breach().is_alertable());
        let device_count = AlertEvent {
            metric: "total_device_count".to_string(),
            ..breach()
        };
        assert!(!device_count.is_alertable());
    }
}
