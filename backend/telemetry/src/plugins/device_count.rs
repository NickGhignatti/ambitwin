use crate::plugins::common::Metric;
use crate::types::plugin::{Aggregation, FieldKind, LimitSpec, MetricDescriptor};

// counts of devices a router hears; a phone count is no measure to alert on, so no limit

pub fn total_device_count() -> Metric {
    Metric {
        descriptor: MetricDescriptor {
            key: "total_device_count",
            label: "Total Device Count",
            interface_name: "ITotalDeviceCount",
            unit: Some("devices"),
            value_kind: FieldKind::NonNegativeInt,
            limit: LimitSpec {
                min: None,
                max: None,
            },
            aggregation: Aggregation::Sum,
        },
        actions: &[],
    }
}

pub fn ratio_device_count() -> Metric {
    Metric {
        descriptor: MetricDescriptor {
            key: "ratio_device_count",
            label: "Estimated People Count",
            interface_name: "IRatioDeviceCount",
            unit: Some("people"),
            value_kind: FieldKind::NonNegativeInt,
            limit: LimitSpec {
                min: None,
                max: None,
            },
            aggregation: Aggregation::Sum,
        },
        actions: &[],
    }
}

pub fn estimated_occupancy() -> Metric {
    Metric {
        descriptor: MetricDescriptor {
            key: "estimated_occupancy",
            label: "Estimated Occupancy",
            interface_name: "IEstimatedOccupancy",
            unit: Some("devices"),
            value_kind: FieldKind::NonNegativeInt,
            limit: LimitSpec {
                min: None,
                max: None,
            },
            aggregation: Aggregation::Sum,
        },
        actions: &[],
    }
}
