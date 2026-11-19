use crate::types::plugin::{BoundDirection, MAX, MIN};
use serde_json::{Map, Value};

/// A stored limit: `{"min"?: n, "max"?: n}`.
pub type Bounds = Map<String, Value>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Breach {
    pub direction: BoundDirection,
    pub value: f64,
    pub threshold: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoomTemperatureLimit {
    pub room_id: String,
    pub max_temperature: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemperatureLimits {
    pub building_id: String,
    pub max_temperature: Option<f64>,
    pub rooms: Vec<RoomTemperatureLimit>,
}

/// The limit a room is held to: each end the room sets wins over the building's same end.
pub fn resolve(room: Option<&Bounds>, building: Option<&Bounds>) -> Option<Bounds> {
    match (room, building) {
        (None, None) => None,
        (room, building) => {
            let mut merged = building.cloned().unwrap_or_default();
            merged.extend(room.cloned().unwrap_or_default());
            Some(merged)
        }
    }
}

/// The end `value` crosses, if any; above `max` is checked before below `min`.
pub fn breach(bounds: &Bounds, value: f64) -> Option<Breach> {
    let end = |key: &str| bounds.get(key).and_then(Value::as_f64);
    let crossed = |direction, threshold| Breach {
        direction,
        value,
        threshold,
    };
    match (end(MAX), end(MIN)) {
        (Some(max), _) if value > max => Some(crossed(BoundDirection::Above, max)),
        (_, Some(min)) if value < min => Some(crossed(BoundDirection::Below, min)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bounds(value: Value) -> Bounds {
        value.as_object().unwrap().clone()
    }

    // building 16 to 28, room sets only max 26: the room is held to 16 to 26
    #[test]
    fn a_room_end_wins_over_the_building_end_one_end_at_a_time() {
        let room = bounds(json!({ "max": 26.0 }));
        let building = bounds(json!({ "min": 16.0, "max": 28.0 }));
        let resolved = resolve(Some(&room), Some(&building)).unwrap();
        assert_eq!(resolved, bounds(json!({ "min": 16.0, "max": 26.0 })));
    }

    #[test]
    fn no_limit_anywhere_means_none() {
        assert!(resolve(None, None).is_none());
        assert_eq!(
            resolve(None, Some(&bounds(json!({ "max": 28.0 })))),
            Some(bounds(json!({ "max": 28.0 })))
        );
    }

    #[test]
    fn above_max_breaches_high_and_below_min_breaches_low() {
        let limit = bounds(json!({ "min": 18.0, "max": 25.0 }));
        let high = breach(&limit, 26.0).unwrap();
        assert_eq!(
            (high.direction, high.threshold),
            (BoundDirection::Above, 25.0)
        );
        let low = breach(&limit, 17.0).unwrap();
        assert_eq!(
            (low.direction, low.threshold),
            (BoundDirection::Below, 18.0)
        );
    }

    #[test]
    fn a_value_on_an_end_or_a_non_numeric_end_does_not_breach() {
        let limit = bounds(json!({ "min": 18.0, "max": 25.0 }));
        assert!(breach(&limit, 25.0).is_none());
        assert!(breach(&limit, 18.0).is_none());
        assert!(breach(&bounds(json!({ "max": "25" })), 26.0).is_none());
    }

    #[test]
    fn crossing_both_ends_reports_the_max_once() {
        let limit = bounds(json!({ "min": 30.0, "max": 10.0 }));
        assert_eq!(breach(&limit, 20.0).unwrap().threshold, 10.0);
    }
}
