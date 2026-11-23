pub mod co2;
pub mod common;
pub mod device_count;
pub mod humidity;
pub mod indoor_aqi;
pub mod people_count;
pub mod temperature;

use crate::types::device::DeviceKind;
use crate::types::plugin::SensorPlugin;

/// Every plugin the service registers; `main.rs` builds the registry from this list.
pub fn all() -> Vec<Box<dyn SensorPlugin>> {
    vec![
        Box::new(temperature::metric()),
        Box::new(people_count::metric()),
        Box::new(co2::metric()),
        Box::new(indoor_aqi::metric()),
        Box::new(humidity::metric()),
        Box::new(device_count::total_device_count()),
        Box::new(device_count::ratio_device_count()),
        Box::new(device_count::estimated_occupancy()),
    ]
}

/// What a sensor is registered as: the physical device, which may report several metrics.
pub fn devices() -> Vec<DeviceKind> {
    vec![
        DeviceKind {
            key: "router",
            label: "Router",
            metrics: &[
                "total_device_count",
                "ratio_device_count",
                "estimated_occupancy",
            ],
        },
        DeviceKind {
            key: "temperature",
            label: "Thermostat",
            metrics: &["temperature"],
        },
        DeviceKind {
            key: "air_quality",
            label: "Air quality station",
            metrics: &["co2", "indoor_aqi", "humidity"],
        },
        DeviceKind {
            key: "people_count",
            label: "People counter",
            metrics: &["people_count"],
        },
    ]
}

#[cfg(test)]
mod tests {
    use crate::kernel::devices::DeviceCatalog;
    use crate::kernel::registry::PluginRegistry;
    use crate::types::plugin::LimitSpec;
    use telemetry_schema::ALERTABLE_METRICS;

    #[test]
    fn every_metric_comes_from_exactly_one_device() {
        let registry = PluginRegistry::new(super::all()).unwrap();
        DeviceCatalog::new(super::devices(), &registry).unwrap();
    }

    #[test]
    fn every_plugin_with_a_limit_is_alertable_and_nothing_else_is() {
        let mut limited: Vec<&str> = super::all()
            .iter()
            .filter(|plugin| !plugin.descriptor().limit.is_empty())
            .map(|plugin| plugin.key())
            .collect();
        limited.sort_unstable();
        let mut alertable = ALERTABLE_METRICS.to_vec();
        alertable.sort_unstable();
        assert_eq!(limited, alertable);
    }

    // where a new building starts; people_count's max comes from each room's capacity
    #[test]
    fn each_metric_declares_its_ends_and_their_starts() {
        let limit = |min, max| LimitSpec { min, max };
        let declared: Vec<(&str, LimitSpec)> = super::all()
            .iter()
            .map(|plugin| (plugin.key(), plugin.descriptor().limit))
            .collect();
        assert_eq!(
            declared,
            vec![
                ("temperature", limit(Some(Some(18.0)), Some(Some(27.0)))),
                ("people_count", limit(None, Some(None))),
                ("co2", limit(None, Some(Some(1000.0)))),
                ("indoor_aqi", limit(None, Some(Some(75.0)))),
                ("humidity", limit(Some(Some(30.0)), Some(Some(60.0)))),
                ("total_device_count", LimitSpec::default()),
                ("ratio_device_count", LimitSpec::default()),
                ("estimated_occupancy", LimitSpec::default()),
            ]
        );
    }
}
