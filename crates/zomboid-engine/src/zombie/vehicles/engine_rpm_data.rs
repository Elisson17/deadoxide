//! zombie.vehicles.EngineRPMData: the RPM an engine gear changes at and drops to after the change.

/// zombie.vehicles.EngineRPMData
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EngineRPMData {
    pub gear_change: f32,
    pub after_gear_change: f32,
}

impl EngineRPMData {
    /// zombie.vehicles.EngineRPMData.reset
    pub fn reset(&mut self) {
        self.gear_change = 0.0;
        self.after_gear_change = 0.0;
    }
}
