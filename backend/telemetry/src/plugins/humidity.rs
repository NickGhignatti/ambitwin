use crate::plugins::common::Metric;
use crate::types::plugin::{Aggregation, FieldKind, LimitSpec, MetricDescriptor};

/// Relative humidity: below 30 % the air is dry, above 60 % mould can grow.
pub fn metric() -> Metric {
    Metric {
        descriptor: MetricDescriptor {
            key: "humidity",
            label: "Humidity",
            interface_name: "IHumidity",
            unit: Some("%"),
            value_kind: FieldKind::Finite,
            limit: LimitSpec {
                min: Some(Some(30.0)),
                max: Some(Some(60.0)),
            },
            aggregation: Aggregation::Average,
        },
        actions: &[],
    }
}
