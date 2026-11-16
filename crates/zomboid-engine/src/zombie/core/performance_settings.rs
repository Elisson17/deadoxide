//! zombie.core.PerformanceSettings: the static settings with their initialisers (only the ones options.ini reads
//! and writes; the Lua setters that also touch IsoWater/IsoPuddles are not ported).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerformanceSettings {
    /// `lockFps = 60`, read through `getLockFPS()`.
    pub lock_fps: i32,
    /// `uncappedFps`, read through `instance.isFramerateUncapped()`.
    pub uncapped_fps: bool,
    pub water_quality: i32,
    pub puddles_quality: i32,
    /// `lightingFps = 15`.
    pub lighting_fps: i32,
    pub fog_quality: i32,
    /// `viewConeOpacity = 3`.
    pub view_cone_opacity: i32,
}

impl Default for PerformanceSettings {
    /// The static initialisers.
    fn default() -> Self {
        PerformanceSettings {
            lock_fps: 60,
            uncapped_fps: false,
            water_quality: 0,
            puddles_quality: 0,
            lighting_fps: 15,
            fog_quality: 0,
            view_cone_opacity: 3,
        }
    }
}
