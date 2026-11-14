//! zombie.iso.weather.WeatherPeriod: a running weather pattern (its stages, air front and cloud colour), as far as a
//! save stores it.
use super::climate_color_info::ClimateColorInfo;
use super::climate_manager::AirFront;
use crate::java_nio::{read_string_stream, ByteBuffer};

/// zombie.iso.weather.WeatherPeriod.StrLerpVal
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrLerpVal {
    Entry,
    Target,
    NextTarget,
    None,
}

impl StrLerpVal {
    /// zombie.iso.weather.WeatherPeriod.StrLerpVal.getValue
    pub fn value(self) -> i32 {
        match self {
            StrLerpVal::Entry => 1,
            StrLerpVal::Target => 2,
            StrLerpVal::NextTarget => 3,
            StrLerpVal::None => 0,
        }
    }

    /// `Enum.name()`
    pub fn name(self) -> &'static str {
        match self {
            StrLerpVal::Entry => "Entry",
            StrLerpVal::Target => "Target",
            StrLerpVal::NextTarget => "NextTarget",
            StrLerpVal::None => "None",
        }
    }

    /// zombie.iso.weather.WeatherPeriod.StrLerpVal.fromValue: an unknown id is `None`, and the original logs it.
    pub fn from_value(id: i32, log: &mut Vec<String>) -> Self {
        match id {
            1 => StrLerpVal::Entry,
            2 => StrLerpVal::Target,
            3 => StrLerpVal::NextTarget,
            0 => StrLerpVal::None,
            _ => {
                log.push(format!("StrLerpVal, trying to get from invalid id: {id}"));
                StrLerpVal::None
            }
        }
    }
}

/// zombie.iso.weather.WeatherPeriod.WeatherStage: the saved fields and the links between stages (indices into the
/// period's `weather_stages` instead of object references).
#[derive(Clone, Debug, PartialEq)]
pub struct WeatherStage {
    pub previous_stage: Option<usize>,
    pub next_stage: Option<usize>,
    pub stage_start: f64,
    pub stage_end: f64,
    pub stage_duration: f64,
    pub stage_id: i32,
    pub entry_strength: f32,
    pub exit_strength: f32,
    pub target_strength: f32,
    /// null in a stage that was never reset or loaded.
    pub lerp_mid_val: Option<StrLerpVal>,
    pub lerp_end_val: Option<StrLerpVal>,
    pub has_started_cloud: bool,
    pub fog_strength: f32,
    pub linear_t: f32,
    pub parabolic_t: f32,
    pub is_cycle_first_half: bool,
    pub creation_finished: bool,
    pub mod_id: Option<String>,
}

impl Default for WeatherStage {
    /// zombie.iso.weather.WeatherPeriod.WeatherStage()
    fn default() -> Self {
        WeatherStage {
            previous_stage: None,
            next_stage: None,
            stage_start: 0.0,
            stage_end: 0.0,
            stage_duration: 0.0,
            stage_id: 0,
            entry_strength: 0.0,
            exit_strength: 0.0,
            target_strength: 0.0,
            lerp_mid_val: None,
            lerp_end_val: None,
            has_started_cloud: false,
            fog_strength: 0.0,
            linear_t: 0.0,
            parabolic_t: 0.0,
            is_cycle_first_half: true,
            creation_finished: false,
            mod_id: None,
        }
    }
}

impl WeatherStage {
    /// zombie.iso.weather.WeatherPeriod.WeatherStage.reset (target strength, times and stage id stay).
    pub fn reset(&mut self) {
        self.previous_stage = None;
        self.next_stage = None;
        self.is_cycle_first_half = true;
        self.has_started_cloud = false;
        self.lerp_mid_val = Some(StrLerpVal::None);
        self.lerp_end_val = Some(StrLerpVal::None);
        self.entry_strength = 0.0;
        self.exit_strength = 0.0;
        self.mod_id = None;
        self.creation_finished = false;
        self.fog_strength = 0.0;
    }

    /// zombie.iso.weather.WeatherPeriod.WeatherStage.setStageDuration: at least one hour.
    pub fn set_stage_duration(&mut self, time: f64) {
        self.stage_duration = time;
        if self.stage_duration < 1.0 {
            self.stage_duration = 1.0;
        }
    }

    /// zombie.iso.weather.WeatherPeriod.WeatherStage.load(DataInputStream, int): the start (the end follows from
    /// the duration set before), the three strengths, the two lerp kinds, the cloud flag, an optional mod id (kept
    /// when absent) and the fog strength.
    pub fn load(
        &mut self,
        input: &mut ByteBuffer,
        _world_version: i32,
        log: &mut Vec<String>,
    ) -> Result<(), String> {
        self.stage_start = input.get_double()?;
        self.stage_end = self.stage_start + self.stage_duration;
        self.entry_strength = input.get_float()?;
        self.exit_strength = input.get_float()?;
        self.target_strength = input.get_float()?;
        self.lerp_mid_val = Some(StrLerpVal::from_value(input.get_int()?, log));
        self.lerp_end_val = Some(StrLerpVal::from_value(input.get_int()?, log));
        self.has_started_cloud = input.get()? != 0;
        if input.get()? == 1 {
            self.mod_id = Some(read_string_stream(input)?);
        }
        self.fog_strength = input.get_float()?;
        Ok(())
    }
}

/// zombie.iso.weather.WeatherPeriod: the saved state.
#[derive(Clone, Debug, PartialEq)]
pub struct WeatherPeriod {
    pub front_cache: AirFront,
    pub duration: f64,
    pub weather_stage_index: i32,
    pub is_running: bool,
    pub current_strength: f32,
    pub rain_threshold: f32,
    pub wind_angle_dir_mod: f32,
    pub is_thunder_storm: bool,
    pub is_tropical_storm: bool,
    pub is_blizzard: bool,
    /// The colour object the period uses. The original may point it at one of its preset colours (tropical,
    /// blizzard...), which a load then overwrites as well; the presets are not ported.
    pub cloud_color: ClimateColorInfo,
    pub weather_stages: Vec<WeatherStage>,
    /// The stage pool (`Stack`: the last pushed is popped first).
    pub stages_pool: Vec<WeatherStage>,
    /// Index into `weather_stages`. After a load whose stage index is out of range the original keeps its old
    /// stage object; here the old index is kept.
    pub current_stage: Option<usize>,
}

impl Default for WeatherPeriod {
    /// zombie.iso.weather.WeatherPeriod(ClimateManager, ThunderStorm): 30 new stages in the pool.
    fn default() -> Self {
        WeatherPeriod {
            front_cache: AirFront::default(),
            duration: 0.0,
            weather_stage_index: 0,
            is_running: false,
            current_strength: 0.0,
            rain_threshold: 0.0,
            wind_angle_dir_mod: 1.0,
            is_thunder_storm: false,
            is_tropical_storm: false,
            is_blizzard: false,
            cloud_color: ClimateColorInfo::new(0.4, 0.2, 0.2, 0.4),
            weather_stages: Vec::new(),
            stages_pool: vec![WeatherStage::default(); 30],
            current_stage: None,
        }
    }
}

impl WeatherPeriod {
    /// zombie.iso.weather.WeatherPeriod.clearCurrentWeatherStages: every stage reset and back into the pool.
    pub fn clear_current_weather_stages(&mut self) {
        for mut ws in std::mem::take(&mut self.weather_stages) {
            ws.reset();
            self.stages_pool.push(ws);
        }
    }

    /// zombie.iso.weather.WeatherPeriod.linkWeatherStages
    pub fn link_weather_stages(&mut self) {
        let n = self.weather_stages.len();
        for (i, ws) in self.weather_stages.iter_mut().enumerate() {
            ws.previous_stage = i.checked_sub(1);
            ws.next_stage = (i + 1 < n).then_some(i + 1);
            ws.creation_finished = true;
        }
    }

    /// zombie.iso.weather.WeatherPeriod.load(DataInputStream, int): nothing unless the first byte is 1; then the
    /// running flag and, for a running period, its stage index, strengths, storm kinds, air front, the stages (each
    /// from the pool), the cloud colour, and the summed duration. A stage index outside the stages stops the period.
    /// The original's debug prints are not modelled.
    pub fn load(
        &mut self,
        input: &mut ByteBuffer,
        world_version: i32,
        log: &mut Vec<String>,
    ) -> Result<(), String> {
        if input.get()? != 1 {
            return Ok(());
        }
        self.is_running = input.get()? != 0;
        if !self.is_running {
            return Ok(());
        }
        self.weather_stage_index = input.get_int()?;
        self.current_strength = input.get_float()?;
        self.rain_threshold = input.get_float()?;
        self.is_thunder_storm = input.get()? != 0;
        self.is_tropical_storm = input.get()? != 0;
        self.is_blizzard = input.get()? != 0;
        self.front_cache.load(input)?;
        let angle = self.front_cache.front_wind_angle_degrees;
        self.wind_angle_dir_mod = if (90.0..270.0).contains(&angle) {
            1.0
        } else {
            -1.0
        };
        self.clear_current_weather_stages();
        let stages_count = input.get_int()?;
        for _ in 0..stages_count {
            let id = input.get_int()?;
            let duration = input.get_double()?;
            let mut ws = self.stages_pool.pop().unwrap_or_default();
            ws.stage_id = id;
            ws.set_stage_duration(duration);
            ws.load(input, world_version, log)?;
            self.weather_stages.push(ws);
        }
        self.cloud_color.load(input, world_version)?;
        self.link_weather_stages();
        self.duration = 0.0;
        for ws in &self.weather_stages {
            self.duration += ws.stage_duration;
        }
        if self.weather_stage_index >= 0
            && (self.weather_stage_index as usize) < self.weather_stages.len()
        {
            self.current_stage = Some(self.weather_stage_index as usize);
        } else {
            self.is_running = false;
        }
        Ok(())
    }
}
