pub use telemetry_schema::BoundDirection;

use crate::types::reading::Reading;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    NonEmptyString,
    Finite,
    NonNegativeInt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldSpec {
    pub name: &'static str,
    pub kind: FieldKind,
    pub required: bool,
}

/// The ends a metric's limit has. `Some(start)` means the end can be set; `start` is where a
/// new building begins, `None` when it has no starting value. `None` means it cannot be set.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LimitSpec {
    pub min: Option<Option<f64>>,
    pub max: Option<Option<f64>>,
}

impl LimitSpec {
    pub fn is_empty(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    /// Whether `end` (`"min"` or `"max"`) can be set on this metric.
    pub fn has(&self, end: &str) -> bool {
        match end {
            MIN => self.min.is_some(),
            MAX => self.max.is_some(),
            _ => false,
        }
    }
}

pub const MIN: &str = "min";
pub const MAX: &str = "max";

#[derive(Debug, Clone, PartialEq)]
pub struct MetricDescriptor {
    pub key: &'static str,
    pub label: &'static str,
    pub interface_name: &'static str,
    pub unit: Option<&'static str>,
    /// What `value` must be: `Finite`, or `NonNegativeInt` for a count.
    pub value_kind: FieldKind,
    pub limit: LimitSpec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionSpec {
    pub name: &'static str,
    pub label: &'static str,
    pub parameters: &'static [FieldSpec],
}

/// A metric telemetry accepts. Every reading has the same shape, so a plugin is mostly data:
/// its descriptor, and the commands its devices take.
pub trait SensorPlugin: Send + Sync {
    fn descriptor(&self) -> &MetricDescriptor;

    fn key(&self) -> &'static str {
        self.descriptor().key
    }

    fn validate(&self, payload: &Value) -> Result<Reading, Vec<String>> {
        validate_reading(self.descriptor(), payload)
    }

    fn actions(&self) -> &'static [ActionSpec] {
        &[]
    }
}

/// The names a reading may carry once telemetry has stamped the batch's building onto it.
pub const READING_FIELDS: &[&str] = &["buildingId", "roomId", "timestamp", "value"];

/// Checks one reading (its `type` already removed) and builds it. A reading is one metric and
/// one number, so any other key is refused rather than stored.
pub fn validate_reading(
    descriptor: &MetricDescriptor,
    payload: &Value,
) -> Result<Reading, Vec<String>> {
    let fields = [
        FieldSpec {
            name: "buildingId",
            kind: FieldKind::NonEmptyString,
            required: true,
        },
        FieldSpec {
            name: "roomId",
            kind: FieldKind::NonEmptyString,
            required: true,
        },
        FieldSpec {
            name: "timestamp",
            kind: FieldKind::Finite,
            required: true,
        },
        FieldSpec {
            name: "value",
            kind: descriptor.value_kind,
            required: true,
        },
    ];
    let mut errors = check_fields(&fields, payload);
    if let Some(object) = payload.as_object() {
        errors.extend(
            object
                .keys()
                .filter(|key| !READING_FIELDS.contains(&key.as_str()))
                .map(|key| format!("{key}: not a field of {}.", descriptor.key)),
        );
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(Reading {
        building_id: payload["buildingId"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        room_id: payload["roomId"].as_str().unwrap_or_default().to_owned(),
        metric: descriptor.key.to_owned(),
        ts_ms: payload["timestamp"].as_f64().unwrap_or_default() as i64,
        value: payload["value"].as_f64().unwrap_or_default(),
    })
}

pub fn check_fields(fields: &[FieldSpec], payload: &Value) -> Vec<String> {
    fields
        .iter()
        .filter_map(|spec| check_field(spec, payload.get(spec.name)))
        .collect()
}

fn check_field(spec: &FieldSpec, value: Option<&Value>) -> Option<String> {
    match value {
        None | Some(Value::Null) if !spec.required => None,
        Some(value) if accepts(spec.kind, value) => None,
        _ => Some(format!(
            "{}: must be {}.",
            spec.name,
            error_kind_to_string(spec.kind)
        )),
    }
}

fn accepts(kind: FieldKind, value: &Value) -> bool {
    match kind {
        FieldKind::NonEmptyString => value.as_str().is_some_and(|s| !s.is_empty()),
        FieldKind::Finite => value.as_f64().is_some(),
        FieldKind::NonNegativeInt => value.as_f64().is_some_and(|n| n >= 0.0 && n.fract() == 0.0),
    }
}

fn error_kind_to_string(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::NonEmptyString => "a non-empty string",
        FieldKind::Finite => "a finite number",
        FieldKind::NonNegativeInt => "a non-negative integer",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const AQI: &[FieldSpec] = &[FieldSpec {
        name: "aqi",
        kind: FieldKind::NonNegativeInt,
        required: true,
    }];

    const MESSAGE: &str = "aqi: must be a non-negative integer.";

    fn errors_for(value: Value) -> Vec<String> {
        check_fields(AQI, &json!({ "aqi": value }))
    }

    #[test]
    fn a_positive_integer_is_accepted() {
        assert!(errors_for(json!(31)).is_empty());
    }

    #[test]
    fn zero_is_accepted() {
        assert!(errors_for(json!(0)).is_empty());
    }

    #[test]
    fn an_integral_float_is_accepted() {
        assert!(errors_for(json!(31.0)).is_empty());
    }

    #[test]
    fn a_negative_number_is_rejected() {
        assert_eq!(errors_for(json!(-3)), vec![MESSAGE]);
    }

    #[test]
    fn a_fractional_number_is_rejected() {
        assert_eq!(errors_for(json!(3.5)), vec![MESSAGE]);
    }

    #[test]
    fn a_numeric_string_is_rejected() {
        assert_eq!(errors_for(json!("31")), vec![MESSAGE]);
    }

    #[test]
    fn a_null_value_is_rejected_when_required() {
        assert_eq!(errors_for(json!(null)), vec![MESSAGE]);
    }

    #[test]
    fn a_missing_value_is_rejected_when_required() {
        assert_eq!(check_fields(AQI, &json!({})), vec![MESSAGE]);
    }

    #[test]
    fn a_missing_value_is_accepted_when_optional() {
        const OPTIONAL_AQI: &[FieldSpec] = &[FieldSpec {
            name: "aqi",
            kind: FieldKind::NonNegativeInt,
            required: false,
        }];
        assert!(check_fields(OPTIONAL_AQI, &json!({})).is_empty());
    }

    const COUNT: MetricDescriptor = MetricDescriptor {
        key: "people_count",
        label: "People Count",
        interface_name: "IPeopleCount",
        unit: Some("people"),
        value_kind: FieldKind::NonNegativeInt,
        limit: LimitSpec {
            min: None,
            max: Some(None),
        },
    };

    fn count_reading(value: Value) -> Value {
        json!({ "buildingId": "b1", "roomId": "r1", "timestamp": 1_700_000_000_000i64, "value": value })
    }

    #[test]
    fn a_reading_becomes_its_metric_and_its_one_number() {
        let reading = validate_reading(&COUNT, &count_reading(json!(14))).unwrap();
        assert_eq!(reading.metric, "people_count");
        assert_eq!(reading.room_id, "r1");
        assert_eq!(reading.value, 14.0);
    }

    #[test]
    fn a_count_must_be_whole_and_a_second_measure_is_refused() {
        assert_eq!(
            validate_reading(&COUNT, &count_reading(json!(14.5))).unwrap_err(),
            vec!["value: must be a non-negative integer."]
        );
        let mut riding = count_reading(json!(14));
        riding["co2"] = json!(612.0);
        assert_eq!(
            validate_reading(&COUNT, &riding).unwrap_err(),
            vec!["co2: not a field of people_count."]
        );
    }

    #[test]
    fn a_limit_offers_only_the_ends_it_declares() {
        assert!(COUNT.limit.has("max"));
        assert!(!COUNT.limit.has("min"));
        assert!(!COUNT.limit.has("maxPeople"));
        assert!(LimitSpec::default().is_empty());
    }
}
