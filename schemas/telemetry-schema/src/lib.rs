pub mod alerts;
pub mod telemetry;

pub use alerts::{ALERTABLE_METRICS, ALERTS_DLQ_TOPIC, ALERTS_TOPIC, AlertEvent, BoundDirection};
pub use telemetry::{
    FILTERED_CHANNEL_PATTERN, FILTERED_CHANNEL_PREFIX, RAW_CHANNEL, TelemetryEnvelope,
    TelemetryReading, building_of_filtered_channel, filtered_channel,
};

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionParameterContract {
    pub name: String,
    pub r#type: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionContract {
    pub name: String,
    pub label: String,
    pub parameters: Vec<ActionParameterContract>,
}

/// The ends a metric's limit has. A present end can be set; its number is where a new building
/// starts, `null` when it has no starting value (a room's people limit comes from its capacity).
/// An absent end cannot be set, so a metric with neither takes no limit at all.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LimitContract {
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub min: Option<Option<f64>>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub max: Option<Option<f64>>,
}

// serde reads an explicit null as absent; a present null must stay "settable, no start"
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Option<f64>>, D::Error> {
    Option::<f64>::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricContract {
    pub kind: String,
    pub label: String,
    pub interface: String,
    pub unit: Option<String>,
    #[serde(default)]
    pub actions: Vec<ActionContract>,
    #[serde(default)]
    pub limit: LimitContract,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceMetricsContract {
    pub service: String,
    pub metrics: Vec<MetricContract>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum MetricsDiscoveryResponse {
    ServiceContract(ServiceMetricsContract),
    Metrics(Vec<MetricContract>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn metric() -> MetricContract {
        MetricContract {
            kind: "temperature".to_owned(),
            label: "Temperature".to_owned(),
            interface: "ITemperature".to_owned(),
            unit: Some("C".to_owned()),
            actions: vec![],
            limit: LimitContract {
                min: Some(Some(18.0)),
                max: Some(Some(27.0)),
            },
            source: None,
        }
    }

    #[test]
    fn a_metric_serialises_with_the_single_word_names_the_frontend_reads() {
        let body = serde_json::to_value(metric()).unwrap();
        assert_eq!(body["kind"], "temperature");
        assert_eq!(body["interface"], "ITemperature");
        assert_eq!(body["limit"], json!({ "min": 18.0, "max": 27.0 }));
    }

    #[test]
    fn an_absent_source_is_omitted_but_empty_actions_stay_an_array() {
        let body = serde_json::to_value(metric()).unwrap();
        assert!(body.get("source").is_none());
        assert_eq!(body["actions"], json!([]));
    }

    // a present null means "this end can be set, with no starting value"; absent means it cannot
    #[test]
    fn a_limit_tells_a_settable_end_without_a_start_from_a_missing_end() {
        let limit: LimitContract = serde_json::from_value(json!({ "max": null })).unwrap();
        assert_eq!(limit.max, Some(None));
        assert_eq!(limit.min, None);
        assert_eq!(
            serde_json::to_value(&limit).unwrap(),
            json!({ "max": null })
        );
    }

    #[test]
    fn a_catalog_without_actions_or_limit_still_parses() {
        let raw = json!({
            "kind": "occupancy", "label": "Occupancy",
            "interface": "IOccupancy", "unit": null
        });
        let decoded: MetricContract = serde_json::from_value(raw).unwrap();
        assert!(decoded.actions.is_empty());
        assert_eq!(decoded.limit, LimitContract::default());
    }

    #[test]
    fn a_service_contract_round_trips() {
        let contract = ServiceMetricsContract {
            service: "telemetry".to_owned(),
            metrics: vec![metric()],
        };
        let encoded = serde_json::to_string(&contract).unwrap();
        let decoded: ServiceMetricsContract = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, contract);
    }

    #[test]
    fn a_service_shaped_body_decodes_as_the_service_variant() {
        let raw = json!({ "service": "telemetry", "metrics": [] });
        let decoded: MetricsDiscoveryResponse = serde_json::from_value(raw).unwrap();
        assert!(matches!(
            decoded,
            MetricsDiscoveryResponse::ServiceContract(_)
        ));
    }

    #[test]
    fn a_bare_array_decodes_as_the_metrics_variant() {
        let raw = json!([]);
        let decoded: MetricsDiscoveryResponse = serde_json::from_value(raw).unwrap();
        assert!(matches!(decoded, MetricsDiscoveryResponse::Metrics(_)));
    }

    #[test]
    fn actions_carry_their_parameters() {
        let mut with_action = metric();
        with_action.actions = vec![ActionContract {
            name: "setTarget".to_owned(),
            label: "Set target temperature".to_owned(),
            parameters: vec![ActionParameterContract {
                name: "target".to_owned(),
                r#type: "Finite".to_owned(),
                required: true,
            }],
        }];
        let body = serde_json::to_value(&with_action).unwrap();
        assert_eq!(body["actions"][0]["name"], "setTarget");
        assert_eq!(body["actions"][0]["parameters"][0]["type"], "Finite");
    }
}
