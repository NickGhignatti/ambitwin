use crate::plugins::common::Metric;
use crate::types::plugin::{FieldKind, LimitSpec, MetricDescriptor};

/// Carbon dioxide in the air of a room.
pub fn metric() -> Metric {
    Metric {
        descriptor: MetricDescriptor {
            key: "co2",
            label: "CO2",
            interface_name: "ICo2",
            unit: Some("ppm"),
            value_kind: FieldKind::Finite,
            limit: LimitSpec {
                min: None,
                max: Some(Some(1000.0)),
            },
        },
        actions: &[],
    }
}
