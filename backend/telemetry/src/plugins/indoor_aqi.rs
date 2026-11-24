use crate::plugins::common::Metric;
use crate::types::plugin::{Aggregation, FieldKind, LimitSpec, MetricDescriptor};

/// Indoor air quality index, worked out by the air station from what it measures.
pub fn metric() -> Metric {
    Metric {
        descriptor: MetricDescriptor {
            key: "indoor_aqi",
            label: "Indoor AQI",
            interface_name: "IIndoorAqi",
            unit: None,
            value_kind: FieldKind::Finite,
            limit: LimitSpec {
                min: None,
                max: Some(Some(75.0)),
            },
            aggregation: Aggregation::Average,
        },
        actions: &[],
    }
}
