use crate::plugins::common::Metric;
use crate::types::plugin::{
    ActionSpec, Aggregation, FieldKind, FieldSpec, LimitSpec, MetricDescriptor,
};

static ACTIONS: &[ActionSpec] = &[
    ActionSpec {
        name: "setTarget",
        label: "Set target temperature",
        parameters: &[FieldSpec {
            name: "target",
            kind: FieldKind::Finite,
            required: true,
        }],
    },
    ActionSpec {
        name: "increase",
        label: "Increase temperature",
        parameters: &[FieldSpec {
            name: "step",
            kind: FieldKind::Finite,
            required: false,
        }],
    },
    ActionSpec {
        name: "decrease",
        label: "Decrease temperature",
        parameters: &[FieldSpec {
            name: "step",
            kind: FieldKind::Finite,
            required: false,
        }],
    },
];

pub fn metric() -> Metric {
    Metric {
        descriptor: MetricDescriptor {
            key: "temperature",
            label: "Temperature",
            interface_name: "ITemperature",
            unit: Some("C"),
            value_kind: FieldKind::Finite,
            limit: LimitSpec {
                min: Some(Some(18.0)),
                max: Some(Some(27.0)),
            },
            aggregation: Aggregation::Average,
        },
        actions: ACTIONS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temperature_declares_an_absolute_and_two_relative_actions() {
        let names: Vec<&str> = ACTIONS.iter().map(|spec| spec.name).collect();
        assert_eq!(names, vec!["setTarget", "increase", "decrease"]);
    }

    #[test]
    fn set_target_requires_its_target_but_a_step_is_optional() {
        assert!(ACTIONS[0].parameters[0].required);
        assert!(!ACTIONS[1].parameters[0].required);
    }
}
