use crate::types::plugin::{ActionSpec, MetricDescriptor, SensorPlugin};

/// A metric that needs nothing beyond its data: every reading has the same shape, so checking
/// one is the same for all of them.
pub struct Metric {
    pub descriptor: MetricDescriptor,
    pub actions: &'static [ActionSpec],
}

impl SensorPlugin for Metric {
    fn descriptor(&self) -> &MetricDescriptor {
        &self.descriptor
    }

    fn actions(&self) -> &'static [ActionSpec] {
        self.actions
    }
}
