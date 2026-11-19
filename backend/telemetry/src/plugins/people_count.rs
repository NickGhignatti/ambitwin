use crate::plugins::common::Metric;
use crate::types::plugin::{FieldKind, LimitSpec, MetricDescriptor};

/// People in a room. The max has no start: each room's comes from its capacity.
pub fn metric() -> Metric {
    Metric {
        descriptor: MetricDescriptor {
            key: "people_count",
            label: "People Count",
            interface_name: "IPeopleCount",
            unit: Some("people"),
            value_kind: FieldKind::NonNegativeInt,
            limit: LimitSpec {
                min: None,
                max: Some(None),
            },
        },
        actions: &[],
    }
}
