//! zombie.iso.weather.WeatherPeriod: a weather pattern (a list of stages with a strength each, from an air front):
//! how one is made from the front and the season, how the running one moves through its stages and sets the
//! climate's overrides, and what a save stores of it.
//!
//! Not modelled: the Lua events OnWeatherPeriodStart/Stage/Stop/Complete (the game's own handlers only rebuild
//! the foraging icons), the climate packet a server sends when a period starts or ends, and OnInitModdedWeatherStage
//! (stage 10, made by mods).
use super::climate_color_info::ClimateColorInfo;
use super::climate_manager::{
    clamp, clamp01, clerp, AirFront, ClimateEnv, ClimateManager, FLOAT_AMBIENT,
    FLOAT_CLOUD_INTENSITY, FLOAT_DAYLIGHT_STRENGTH, FLOAT_DESATURATION, FLOAT_FOG_INTENSITY,
    FLOAT_GLOBAL_LIGHT_INTENSITY, FLOAT_NIGHT_STRENGTH, FLOAT_PRECIPITATION_INTENSITY,
    FLOAT_TEMPERATURE, FLOAT_WIND_ANGLE_INTENSITY, FLOAT_WIND_INTENSITY, COLOR_GLOBAL_LIGHT,
    COLOR_NEW_FOG, BOOL_IS_SNOW,
};
use super::climate_values::{ClimateValues, ValuesEnv};
use super::simplex_noise;
use crate::java::{math_max_f32, math_min_f32, JavaRandom, Random};
use crate::java_nio::{read_string_stream, ByteBuffer};
use crate::zombie::core::random::rand_abstract::RandAbstract;
use crate::zombie::erosion::season::erosion_season::ErosionSeason;
use crate::zombie::game_time::GameTime;

pub const STAGE_START: i32 = 0;
pub const STAGE_SHOWERS: i32 = 1;
pub const STAGE_HEAVY_PRECIP: i32 = 2;
pub const STAGE_STORM: i32 = 3;
pub const STAGE_CLEARING: i32 = 4;
pub const STAGE_MODERATE: i32 = 5;
pub const STAGE_DRIZZLE: i32 = 6;
pub const STAGE_BLIZZARD: i32 = 7;
pub const STAGE_TROPICAL_STORM: i32 = 8;
pub const STAGE_INTERMEZZO: i32 = 9;
pub const STAGE_MODDED: i32 = 10;
pub const STAGE_KATEBOB_STORM: i32 = 11;
pub const STAGE_MAX: i32 = 12;
/// zombie.iso.weather.WeatherPeriod.maxTemperatureInfluence
pub const MAX_TEMPERATURE_INFLUENCE: f32 = 7.0;

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

    /// zombie.iso.weather.WeatherPeriod.WeatherStage.startStage: the stage runs from `exit_time`.
    pub fn start_stage(&mut self, exit_time: f64) {
        self.stage_start = exit_time;
        self.stage_end = exit_time + self.stage_duration;
        self.has_started_cloud = false;
    }

    /// zombie.iso.weather.WeatherPeriod.WeatherStage.setStageStart: returns the end.
    pub fn set_stage_start(&mut self, world_age_hours: f64) -> f64 {
        self.stage_start = world_age_hours;
        self.stage_end = world_age_hours + self.stage_duration;
        self.stage_end
    }

    /// zombie.iso.weather.WeatherPeriod.WeatherStage.lerpEntryTo(StrLerpVal mid, StrLerpVal end): nothing once the
    /// pattern is linked (creationFinished).
    pub fn lerp_entry_to(&mut self, mid: StrLerpVal, end: StrLerpVal) {
        if !self.creation_finished {
            self.lerp_mid_val = Some(mid);
            self.lerp_end_val = Some(end);
        }
    }

    /// zombie.iso.weather.WeatherPeriod.WeatherStage.getPeriodLerpT: how far `hour` is through the stage, 0..1.
    fn get_period_lerp_t(&self, hour: f64) -> f32 {
        if hour < self.stage_start {
            0.0
        } else if hour > self.stage_end {
            1.0
        } else {
            ((hour - self.stage_start) / self.stage_duration) as f32
        }
    }

    /// zombie.iso.weather.WeatherPeriod.WeatherStage.updateT: the linear and parabolic progress (a Kate and Bob
    /// storm starts at its progress offset).
    fn update_t(&mut self, hour: f64, kate_bob_storm_progress: f32) {
        self.linear_t = self.get_period_lerp_t(hour);
        if self.stage_id == STAGE_KATEBOB_STORM {
            self.linear_t =
                kate_bob_storm_progress + (1.0 - kate_bob_storm_progress) * self.linear_t;
        }
        if self.linear_t < 0.5 {
            self.parabolic_t = self.linear_t * 2.0;
            self.is_cycle_first_half = true;
        } else {
            self.parabolic_t = 2.0 - self.linear_t * 2.0;
            self.is_cycle_first_half = false;
        }
    }

    /// zombie.iso.weather.WeatherPeriod.WeatherStage.getLerpValue: `next_target` is the next stage's target
    /// strength (0 when there is none).
    fn get_lerp_value(&self, lerp_val: Option<StrLerpVal>, next_target: f32) -> f32 {
        match lerp_val {
            Some(StrLerpVal::Entry) => self.entry_strength,
            Some(StrLerpVal::Target) => self.target_strength,
            Some(StrLerpVal::NextTarget) => next_target,
            _ => 0.0,
        }
    }

    /// zombie.iso.weather.WeatherPeriod.WeatherStage.getStageCurrentStrength: cosine interpolation of the
    /// strength through the stage, entry to end value, or by halves through the mid value.
    fn get_stage_current_strength(&self, next_target: f32) -> f32 {
        let m = self.get_lerp_value(self.lerp_mid_val, next_target);
        let e = self.get_lerp_value(self.lerp_end_val, next_target);
        if self.lerp_mid_val == Some(StrLerpVal::None) {
            clerp(self.linear_t, self.entry_strength, e)
        } else if self.is_cycle_first_half {
            clerp(self.parabolic_t, self.entry_strength, m)
        } else {
            clerp(self.parabolic_t, e, m)
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

/// Which colour object `WeatherPeriod.cloudColor` points at (the original shares the preset objects).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloudColorRef {
    Own,
    Reddish,
    Greenish,
    Blueish,
    Purplish,
    Tropical,
    Blizzard,
}

/// What the weather period reads of the climate manager and the game while it makes a pattern (the manager is
/// borrowed apart so the period, which the manager holds, can be a separate value; the forecaster's dummy periods
/// make patterns without touching the manager).
pub struct PeriodCtx<'a> {
    /// The manager's simplex offsets (live).
    pub offsets: [f64; 4],
    /// ClimateManager.season: the season object day infos are cloned from.
    pub season: &'a ErosionSeason,
    /// ClimateManager.getSeason(): the current day's season.
    pub current_season: &'a ErosionSeason,
    pub values_env: ValuesEnv,
    pub winter_is_coming: bool,
}

/// zombie.iso.weather.WeatherPeriod
#[derive(Clone, Debug, PartialEq)]
pub struct WeatherPeriod {
    pub front_cache: AirFront,
    pub start_time: f64,
    pub duration: f64,
    pub current_time: f64,
    pub weather_stage_index: i32,
    pub is_running: bool,
    pub total_progress: f32,
    pub stage_progress: f32,
    pub weather_noise: f32,
    pub temperature_influence: f32,
    pub current_strength: f32,
    pub rain_threshold: f32,
    pub wind_angle_dir_mod: f32,
    pub is_thunder_storm: bool,
    pub is_tropical_storm: bool,
    pub is_blizzard: bool,
    pub precipitation_final: f32,
    /// `cloudColor`: the period's own colour or one of the presets (see `cloud_color`).
    pub cloud_color_ref: CloudColorRef,
    pub own_cloud_color: ClimateColorInfo,
    pub cloud_color_reddish: ClimateColorInfo,
    pub cloud_color_greenish: ClimateColorInfo,
    pub cloud_color_blueish: ClimateColorInfo,
    pub cloud_color_purplish: ClimateColorInfo,
    pub cloud_color_tropical: ClimateColorInfo,
    pub cloud_color_blizzard: ClimateColorInfo,
    pub weather_stages: Vec<WeatherStage>,
    /// The stage pool (`Stack`: the last pushed is popped first).
    pub stages_pool: Vec<WeatherStage>,
    /// Index into `weather_stages`. After a load whose stage index is out of range the original keeps its old
    /// stage object; here the old index is kept.
    pub current_stage: Option<usize>,
    /// WeatherPeriod.kateBobStormProgress (a static, 0.45) and the storm's coordinates.
    pub kate_bob_storm_progress: f32,
    pub kate_bob_storm_x: i32,
    pub kate_bob_storm_y: i32,
    pub seeded_random: Random,
    pub climate_values: ClimateValues,
    pub is_dummy: bool,
    pub has_started_init: bool,
}

impl Default for WeatherPeriod {
    /// zombie.iso.weather.WeatherPeriod(ClimateManager, ThunderStorm) with a fresh climate values copy.
    fn default() -> Self {
        WeatherPeriod::new(ClimateValues::new([0.0; 4]))
    }
}

impl WeatherPeriod {
    /// zombie.iso.weather.WeatherPeriod(ClimateManager, ThunderStorm): 30 new stages in the pool, a `Random(1984)`
    /// and the manager's climate values copy (`getClimateValuesCopy()`).
    pub fn new(climate_values: ClimateValues) -> Self {
        WeatherPeriod {
            front_cache: AirFront::default(),
            start_time: 0.0,
            duration: 0.0,
            current_time: 0.0,
            weather_stage_index: 0,
            is_running: false,
            total_progress: 0.0,
            stage_progress: 0.0,
            weather_noise: 0.0,
            temperature_influence: 0.0,
            current_strength: 0.0,
            rain_threshold: 0.0,
            wind_angle_dir_mod: 1.0,
            is_thunder_storm: false,
            is_tropical_storm: false,
            is_blizzard: false,
            precipitation_final: 0.0,
            cloud_color_ref: CloudColorRef::Own,
            own_cloud_color: ClimateColorInfo::new(0.4, 0.2, 0.2, 0.4),
            cloud_color_reddish: ClimateColorInfo::new(0.66, 0.12, 0.12, 0.4),
            cloud_color_greenish: ClimateColorInfo::new(0.32, 0.48, 0.12, 0.4),
            cloud_color_blueish: ClimateColorInfo::new(0.16, 0.48, 0.48, 0.4),
            cloud_color_purplish: ClimateColorInfo::new(0.66, 0.12, 0.66, 0.4),
            cloud_color_tropical: ClimateColorInfo::new(0.4, 0.2, 0.2, 0.4),
            cloud_color_blizzard: ClimateColorInfo::new8(0.12, 0.13, 0.21, 0.5, 0.38, 0.4, 0.5, 0.8),
            weather_stages: Vec::new(),
            stages_pool: vec![WeatherStage::default(); 30],
            current_stage: None,
            kate_bob_storm_progress: 0.45,
            kate_bob_storm_x: 2000,
            kate_bob_storm_y: 2000,
            seeded_random: Random::new(1984),
            climate_values,
            is_dummy: false,
            has_started_init: false,
        }
    }

    /// zombie.iso.weather.WeatherPeriod.setDummy
    pub fn set_dummy(&mut self, b: bool) {
        self.is_dummy = b;
    }

    /// zombie.iso.weather.WeatherPeriod.getCloudColor: the colour `cloudColor` points at.
    pub fn cloud_color(&self) -> &ClimateColorInfo {
        match self.cloud_color_ref {
            CloudColorRef::Own => &self.own_cloud_color,
            CloudColorRef::Reddish => &self.cloud_color_reddish,
            CloudColorRef::Greenish => &self.cloud_color_greenish,
            CloudColorRef::Blueish => &self.cloud_color_blueish,
            CloudColorRef::Purplish => &self.cloud_color_purplish,
            CloudColorRef::Tropical => &self.cloud_color_tropical,
            CloudColorRef::Blizzard => &self.cloud_color_blizzard,
        }
    }

    /// The colour object `cloudColor` points at, for writing (a load writes into the preset it points at).
    pub fn cloud_color_mut(&mut self) -> &mut ClimateColorInfo {
        match self.cloud_color_ref {
            CloudColorRef::Own => &mut self.own_cloud_color,
            CloudColorRef::Reddish => &mut self.cloud_color_reddish,
            CloudColorRef::Greenish => &mut self.cloud_color_greenish,
            CloudColorRef::Blueish => &mut self.cloud_color_blueish,
            CloudColorRef::Purplish => &mut self.cloud_color_purplish,
            CloudColorRef::Tropical => &mut self.cloud_color_tropical,
            CloudColorRef::Blizzard => &mut self.cloud_color_blizzard,
        }
    }

    /// zombie.iso.weather.WeatherPeriod.getTotalStrength: the front's strength.
    pub fn get_total_strength(&self) -> f32 {
        self.front_cache.strength
    }

    /// zombie.iso.weather.WeatherPeriod.getWindAngleDegrees
    pub fn get_wind_angle_degrees(&self) -> f32 {
        self.front_cache.front_wind_angle_degrees
    }

    /// zombie.iso.weather.WeatherPeriod.getStageForWorldAge: the stage whose [start, end) holds the world age.
    pub fn get_stage_for_world_age(&self, world_age_hours: f64) -> Option<&WeatherStage> {
        self.weather_stages
            .iter()
            .find(|s| world_age_hours >= s.stage_start && world_age_hours < s.stage_end)
    }

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

    /// zombie.iso.weather.WeatherPeriod.init(front, hoursSinceStart, year, month, day): the period's climate values
    /// polled at that date, the random generator reseeded from the date and the manager's offsets, and, when
    /// `startInit` accepts the front (strength 0.1 or more, nothing running), a pattern made and the period
    /// started. `ctx` is what the manager gives; Lua event OnWeatherPeriodStart and the climate packet are not
    /// modelled.
    pub fn init(
        &mut self,
        front: &AirFront,
        hours_since_start: f64,
        year: i32,
        month: i32,
        day: i32,
        ctx: &PeriodCtx,
        gt: &GameTime,
        env: &ClimateEnv,
    ) {
        self.init_stage(front, hours_since_start, year, month, day, -1, -1.0, ctx, gt, env);
    }

    /// zombie.iso.weather.WeatherPeriod.init(..., doThisStageOnly, singleStageDuration)
    #[allow(clippy::too_many_arguments)]
    pub fn init_stage(
        &mut self,
        front: &AirFront,
        hours_since_start: f64,
        year: i32,
        month: i32,
        day: i32,
        do_this_stage_only: i32,
        single_stage_duration: f32,
        ctx: &PeriodCtx,
        gt: &GameTime,
        env: &ClimateEnv,
    ) {
        self.climate_values
            .poll_date_ymd(year, month, day, gt, &ctx.values_env, ctx.season);
        self.reseed(year, month, day, ctx);
        self.has_started_init = false;
        if self.start_init(front, hours_since_start, env) {
            if do_this_stage_only >= 0 && do_this_stage_only < STAGE_MAX {
                self.create_single_stage(do_this_stage_only, single_stage_duration);
            } else {
                self.create_weather_pattern(ctx, env);
            }
            self.end_init(env);
        }
    }

    /// zombie.iso.weather.WeatherPeriod.reseed: the generator's seed from the date and the manager's offsets A/B.
    fn reseed(&mut self, year: i32, month: i32, day: i32, ctx: &PeriodCtx) {
        let base_seed_a = ctx.offsets[0] as i32;
        let base_seed_b = ctx.offsets[1] as i32;
        let mut seed = i64::from((year - 1990).wrapping_mul(100_000));
        seed += i64::from(month.wrapping_mul(day).wrapping_mul(1234));
        seed += i64::from((year - 1990).wrapping_mul(month).wrapping_mul(10_000));
        seed += i64::from(base_seed_b.wrapping_sub(base_seed_a).wrapping_mul(day));
        self.seeded_random.set_seed(seed);
    }

    /// zombie.iso.weather.WeatherPeriod.RandNext(float, float): `max` is also taken for `min` when min > max
    /// (the original's `min = max; max = max;`), and a draw is still made.
    fn rand_next_f(&mut self, mut min: f32, max: f32) -> f32 {
        if min == max {
            return min;
        }
        if min > max {
            min = max;
        }
        min + self.seeded_random.next_float() * (max - min)
    }

    /// zombie.iso.weather.WeatherPeriod.RandNext(int, int)
    fn rand_next_i(&mut self, mut min: i32, max: i32) -> i32 {
        if min == max {
            return min;
        }
        if min > max {
            min = max;
        }
        min + self.seeded_random.next_int_bound(max - min)
    }

    /// zombie.iso.weather.WeatherPeriod.startInit: nothing running, a front of strength 0.1 or more (not on a
    /// client): the front copied, the wind direction modifier set.
    fn start_init(&mut self, front: &AirFront, hours_since_start: f64, env: &ClimateEnv) -> bool {
        if !self.is_running && !env.client && !(front.strength < 0.1) {
            self.start_time = hours_since_start;
            self.front_cache.copy_from(front);
            let angle = self.front_cache.front_wind_angle_degrees;
            self.wind_angle_dir_mod = if angle >= 90.0 && angle < 270.0 { 1.0 } else { -1.0 };
            self.has_started_init = true;
            true
        } else {
            false
        }
    }

    /// zombie.iso.weather.WeatherPeriod.endInit: the period runs, when it has stages.
    fn end_init(&mut self, env: &ClimateEnv) -> bool {
        if self.has_started_init && !self.is_running && !env.client && !self.weather_stages.is_empty() {
            self.current_strength = 0.0;
            self.total_progress = 0.0;
            self.stage_progress = 0.0;
            self.is_running = true;
            self.has_started_init = false;
            true
        } else {
            self.has_started_init = false;
            false
        }
    }

    /// zombie.iso.weather.WeatherPeriod.stopWeatherPeriod: stages cleared, not running. Returns whether the climate
    /// manager's overrides must be reset too (`resetClimateManagerOverrides`, not for a dummy period): the
    /// caller does it.
    pub fn stop_weather_period(&mut self) -> bool {
        self.clear_current_weather_stages();
        self.current_stage = None;
        self.is_running = false;
        self.total_progress = 0.0;
        self.stage_progress = 0.0;
        !self.is_dummy
    }

    /// zombie.iso.weather.WeatherPeriod.createSingleStage
    fn create_single_stage(&mut self, stage: i32, duration: f32) {
        if stage == STAGE_TROPICAL_STORM {
            self.cloud_color_ref = CloudColorRef::Tropical;
        } else if stage == STAGE_BLIZZARD {
            self.cloud_color_ref = CloudColorRef::Blizzard;
        }
        self.clear_current_weather_stages();
        self.create_and_add_stage(STAGE_START, 1.0);
        self.create_and_add_stage(stage, f64::from(duration));
        self.create_and_add_stage(STAGE_CLEARING, 1.0);
        self.link_weather_stages();
        self.duration = 0.0;
        for s in &self.weather_stages {
            self.duration += s.stage_duration;
        }
        self.weather_stage_index = 0;
        self.weather_stages[0].start_stage(self.start_time);
        self.current_stage = Some(0);
    }

    /// zombie.iso.weather.WeatherPeriod.createAndAddStage: only while making a pattern.
    fn create_and_add_stage(&mut self, typeid: i32, duration: f64) {
        if !self.is_running && self.has_started_init {
            let ws = self.create_stage(typeid, duration);
            self.weather_stages.push(ws);
        }
    }

    /// zombie.iso.weather.WeatherPeriod.createWeatherPattern: the cloud colour and the chances of storms by season
    /// (and, in winter, of a blizzard by the day's mean temperature); then stages of rain and drizzle by front
    /// type and strength until the duration is reached (a warm-to-cold front first with a tropical storm,
    /// blizzard or storm when chosen), shuffled with the period's generator, with clearing and intermezzo
    /// stages between some, start and end stages, every duration scaled by the rain-time multiplier (the
    /// season's and the sandbox Rain option's), linked and timed from the start. All draws are from the period's
    /// own `Random`, in the original's order.
    fn create_weather_pattern(&mut self, ctx: &PeriodCtx, env: &ClimateEnv) {
        self.clear_current_weather_stages();
        let season = ctx.current_season;
        let mean_temperature = self.climate_values.get_day_mean_temperature();
        let mut storm_chance = 0.0f32;
        let mut tropical_chance = 0.0f32;
        let mut blizzard_chance = 0.0f32;
        let mut rain_time_multiplier = 1.0f32;
        let mut rnd = self.rand_next_f(0.0, 100.0);
        let mut season_id = season.get_season();
        let winter_is_coming = ctx.winter_is_coming;
        if winter_is_coming {
            season_id = 5;
        }
        match season_id {
            1 => {
                self.cloud_color_ref = if rnd < 75.0 {
                    CloudColorRef::Greenish
                } else {
                    CloudColorRef::Blueish
                };
                storm_chance = 75.0;
                tropical_chance = 10.0;
                blizzard_chance = 0.0;
                rain_time_multiplier = 1.25;
            }
            2 => {
                self.cloud_color_ref = if rnd < 25.0 {
                    CloudColorRef::Greenish
                } else {
                    CloudColorRef::Reddish
                };
                storm_chance = 60.0;
                tropical_chance = 55.0;
                blizzard_chance = 0.0;
            }
            3 => {
                self.cloud_color_ref = CloudColorRef::Reddish;
                storm_chance = 75.0;
                tropical_chance = 80.0;
                blizzard_chance = 0.0;
                rain_time_multiplier = 1.15;
            }
            4 => {
                self.cloud_color_ref = if rnd < 50.0 {
                    CloudColorRef::Reddish
                } else if rnd < 75.0 {
                    CloudColorRef::Purplish
                } else {
                    CloudColorRef::Blueish
                };
                storm_chance = 100.0;
                tropical_chance = 25.0;
                blizzard_chance = 0.0;
                rain_time_multiplier = 1.35;
            }
            5 => {
                self.cloud_color_ref = if rnd < 45.0 {
                    CloudColorRef::Purplish
                } else {
                    CloudColorRef::Blueish
                };
                storm_chance = 10.0;
                tropical_chance = 0.0;
                if mean_temperature < 5.5 {
                    blizzard_chance = clamp(0.0, 85.0, (5.5 - mean_temperature) * 3.0);
                    blizzard_chance += 25.0;
                    if mean_temperature < 2.5 {
                        blizzard_chance += 55.0;
                    } else if mean_temperature < 0.0 {
                        blizzard_chance += 75.0;
                    }
                    if blizzard_chance > 95.0 {
                        blizzard_chance = 95.0;
                    }
                } else {
                    blizzard_chance = 0.0;
                }
                if winter_is_coming {
                    blizzard_chance = if self.front_cache.strength > 0.75 { 100.0 } else { 75.0 };
                    if self.front_cache.strength > 0.5 {
                        rain_time_multiplier = 1.45;
                    }
                }
            }
            _ => {}
        }
        rain_time_multiplier *= ClimateManager::get_rain_time_multiplier_mod(ctx.values_env.rain_modifier);
        let strength = self.front_cache.strength;
        let mut stages: Vec<WeatherStage> = Vec::new();
        if self.front_cache.type_ == 1 {
            let mut is_blizzard = false;
            let mut is_tropical = false;
            let mut is_storm = false;
            if strength > 0.75 {
                if tropical_chance > 0.0 && self.rand_next_f(0.0, 100.0) < tropical_chance {
                    is_tropical = true;
                } else if blizzard_chance > 0.0 && self.rand_next_f(0.0, 100.0) < blizzard_chance {
                    is_blizzard = true;
                }
            }
            if !is_blizzard
                && !is_tropical
                && strength > 0.5
                && storm_chance > 0.0
                && self.rand_next_f(0.0, 100.0) < storm_chance
            {
                is_storm = true;
            }
            let max_duration = self.rand_next_f(24.0, 48.0) * strength;
            let mut dur = 0.0f32;
            if is_tropical {
                let d = 8.0 + self.rand_next_f(0.0, 16.0 * strength);
                stages.push(self.create_stage(STAGE_TROPICAL_STORM, f64::from(d)));
                self.cloud_color_ref = CloudColorRef::Tropical;
                if self.rand_next_f(0.0, 100.0) < 60.0 * strength {
                    let d = 5.0 + self.rand_next_f(0.0, 5.0 * strength);
                    stages.push(self.create_stage(STAGE_STORM, f64::from(d)));
                }
                if self.rand_next_f(0.0, 100.0) < 30.0 * strength {
                    let d = 5.0 + self.rand_next_f(0.0, 5.0 * strength);
                    stages.push(self.create_stage(STAGE_STORM, f64::from(d)));
                }
            } else if is_blizzard {
                let d = 24.0 + self.rand_next_f(0.0, 24.0 * strength);
                stages.push(self.create_stage(STAGE_BLIZZARD, f64::from(d)));
                self.cloud_color_ref = CloudColorRef::Blizzard;
            } else if is_storm {
                let d = 5.0 + self.rand_next_f(0.0, 5.0 * strength);
                stages.push(self.create_stage(STAGE_STORM, f64::from(d)));
                if self.rand_next_f(0.0, 100.0) < 70.0 * strength {
                    let d = 4.0 + self.rand_next_f(0.0, 4.0 * strength);
                    stages.push(self.create_stage(STAGE_STORM, f64::from(d)));
                }
                if self.rand_next_f(0.0, 100.0) < 50.0 * strength {
                    let d = 4.0 + self.rand_next_f(0.0, 4.0 * strength);
                    stages.push(self.create_stage(STAGE_STORM, f64::from(d)));
                }
                if self.rand_next_f(0.0, 100.0) < 25.0 * strength {
                    let d = 4.0 + self.rand_next_f(0.0, 3.0 * strength);
                    stages.push(self.create_stage(STAGE_STORM, f64::from(d)));
                }
                if self.rand_next_f(0.0, 100.0) < 12.5 * strength {
                    let d = 4.0 + self.rand_next_f(0.0, 2.0 * strength);
                    stages.push(self.create_stage(STAGE_STORM, f64::from(d)));
                }
            }
            for s in &stages {
                dur = (f64::from(dur) + s.stage_duration) as f32;
            }
            while dur < max_duration {
                let ws = match self.rand_next_i(0, 10) {
                    0 => {
                        let d = 1.0 + self.rand_next_f(0.0, 3.0 * strength);
                        self.create_stage(STAGE_MODERATE, f64::from(d))
                    }
                    1..=3 => {
                        let d = 2.0 + self.rand_next_f(0.0, 4.0 * strength);
                        self.create_stage(STAGE_SHOWERS, f64::from(d))
                    }
                    _ => {
                        let d = 2.0 + self.rand_next_f(0.0, 4.0 * strength);
                        self.create_stage(STAGE_HEAVY_PRECIP, f64::from(d))
                    }
                };
                dur = (f64::from(dur) + ws.stage_duration) as f32;
                stages.push(ws);
            }
        } else {
            if self.cloud_color_ref == CloudColorRef::Reddish {
                rnd = self.rand_next_f(0.0, 100.0);
                self.cloud_color_ref = if rnd < 50.0 {
                    CloudColorRef::Blueish
                } else {
                    CloudColorRef::Purplish
                };
            }
            let max_duration = self.rand_next_f(12.0, 24.0) * strength;
            let mut dur = 0.0f32;
            while dur < max_duration {
                let ws = match self.rand_next_i(0, 10) {
                    0 => {
                        let d = 2.0 + self.rand_next_f(0.0, 3.0 * strength);
                        self.create_stage(STAGE_SHOWERS, f64::from(d))
                    }
                    1..=4 => {
                        let d = 2.0 + self.rand_next_f(0.0, 3.0 * strength);
                        self.create_stage(STAGE_DRIZZLE, f64::from(d))
                    }
                    _ => {
                        let d = 2.0 + self.rand_next_f(0.0, 3.0 * strength);
                        self.create_stage(STAGE_MODERATE, f64::from(d))
                    }
                };
                dur = (f64::from(dur) + ws.stage_duration) as f32;
                stages.push(ws);
            }
        }
        // Collections.shuffle(stages, seededRandom): for (i = size; i > 1; i--) swap(i - 1, nextInt(i)).
        let mut i = stages.len();
        while i > 1 {
            let j = self.seeded_random.next_int_bound(i as i32) as usize;
            stages.swap(i - 1, j);
            i -= 1;
        }
        let mut chance = self.rand_next_f(30.0, 60.0);
        let d = 1.0 + self.rand_next_f(0.0, 2.0 * strength);
        let first = self.create_stage(STAGE_START, f64::from(d));
        self.weather_stages.push(first);
        let n = stages.len();
        for (i, st) in stages.into_iter().enumerate() {
            self.weather_stages.push(st);
            if i < n - 1 && self.rand_next_f(0.0, 100.0) < chance {
                let d = 1.0 + self.rand_next_f(0.0, 2.0 * strength);
                let clearing = self.create_stage(STAGE_CLEARING, f64::from(d));
                self.weather_stages.push(clearing);
                let d = 1.0 + self.rand_next_f(0.0, 3.0 * strength);
                let intermezzo = self.create_stage(STAGE_INTERMEZZO, f64::from(d));
                self.weather_stages.push(intermezzo);
                chance = self.rand_next_f(30.0, 60.0);
            }
        }
        let last_is_intermezzo = self
            .weather_stages
            .last()
            .is_some_and(|s| s.stage_id == STAGE_INTERMEZZO);
        if !last_is_intermezzo {
            let d = 2.0 + self.rand_next_f(0.0, 3.0 * strength);
            let clearing = self.create_stage(STAGE_CLEARING, f64::from(d));
            self.weather_stages.push(clearing);
        }
        for s in self.weather_stages.iter_mut() {
            s.stage_duration *= f64::from(rain_time_multiplier);
        }
        self.link_weather_stages();
        self.duration = 0.0;
        for s in &self.weather_stages {
            self.duration += s.stage_duration;
        }
        let mut time_world_age_hours = self.start_time;
        for s in self.weather_stages.iter_mut() {
            time_world_age_hours = s.set_stage_start(time_world_age_hours);
        }
        let _ = env;
        self.weather_stage_index = 0;
        self.weather_stages[0].start_stage(self.start_time);
        self.current_stage = Some(0);
    }

    /// zombie.iso.weather.WeatherPeriod.createStage: a stage from the pool (a new one when it is empty) with its
    /// id, duration (at least 1 hour), target strength and lerp kinds by type; storms get a fog strength with a
    /// one-in-three chance, blizzards and tropical storms always. A field a type does not set (the start stage's
    /// target strength) keeps what the pooled stage had.
    fn create_stage(&mut self, type_id: i32, duration: f64) -> WeatherStage {
        let mut stage = self.stages_pool.pop().unwrap_or_default();
        let strength = self.front_cache.strength;
        stage.stage_id = type_id;
        stage.mod_id = None;
        stage.set_stage_duration(duration);
        use StrLerpVal::{NextTarget, Target};
        let none = StrLerpVal::None;
        match type_id {
            STAGE_START => stage.lerp_entry_to(none, NextTarget),
            STAGE_SHOWERS => {
                stage.target_strength = strength * 0.5;
                stage.lerp_entry_to(Target, NextTarget);
            }
            STAGE_HEAVY_PRECIP => {
                stage.target_strength = strength;
                stage.lerp_entry_to(Target, Target);
            }
            STAGE_STORM | STAGE_KATEBOB_STORM => {
                stage.target_strength = strength;
                stage.lerp_entry_to(Target, Target);
                if self.rand_next_i(0, 100) < 33 {
                    stage.fog_strength = 0.1 + self.rand_next_f(0.0, 0.4);
                }
            }
            STAGE_CLEARING => {
                stage.target_strength = strength * 0.25;
                stage.lerp_entry_to(Target, none);
            }
            STAGE_MODERATE => {
                stage.target_strength = strength * 0.5;
                stage.lerp_entry_to(Target, NextTarget);
            }
            STAGE_DRIZZLE => {
                stage.target_strength = strength * 0.25;
                stage.lerp_entry_to(Target, NextTarget);
            }
            STAGE_BLIZZARD => {
                stage.target_strength = 1.0;
                stage.lerp_entry_to(Target, Target);
                stage.fog_strength = 0.55 + self.rand_next_f(0.0, 0.2);
            }
            STAGE_TROPICAL_STORM => {
                stage.target_strength = 1.0;
                stage.lerp_entry_to(Target, Target);
                stage.fog_strength = 0.6 + self.rand_next_f(0.0, 0.4);
            }
            STAGE_INTERMEZZO => {
                stage.target_strength = 0.0;
                stage.lerp_entry_to(Target, NextTarget);
            }
            _ => {}
        }
        stage
    }

    /// zombie.iso.weather.WeatherPeriod.update: a running period advances to the next stage when the current
    /// one's end has passed (the stage entered from the current strength; the last one ends the period and
    /// resets the manager's overrides), then, for the current time, the weather noise (simplex noise of the world
    /// age, pulled toward the front's strength), the stage's progress and strength, the stage's overrides
    /// (`updateCurrentStage`) and the precipitation, clouds, light, desaturation, temperature, ambient and snow
    /// overrides on the climate manager from the strength.
    pub fn update<R: JavaRandom>(
        &mut self,
        hours_since_start: f64,
        cm: &mut ClimateManager,
        gt: &GameTime,
        env: &ClimateEnv,
        rand: &mut RandAbstract<R>,
    ) {
        if env.client || self.is_dummy {
            return;
        }
        if self.is_running
            && self.current_stage.is_some()
            && self.weather_stage_index >= 0
            && !self.weather_stages.is_empty()
        {
            let cs = self.current_stage.unwrap_or(0);
            if self.current_time > self.weather_stages[cs].stage_end {
                self.weather_stage_index += 1;
                if self.weather_stage_index as usize >= self.weather_stages.len() {
                    self.is_running = false;
                    self.current_stage = None;
                    cm.reset_overrides();
                    return;
                }
                if let Some(c) = self.current_stage {
                    self.weather_stages[c].exit_strength = self.current_strength;
                }
                let idx = self.weather_stage_index as usize;
                self.current_stage = Some(idx);
                self.weather_stages[idx].entry_strength = self.current_strength;
                self.weather_stages[idx].start_stage(hours_since_start);
            }
            let cs = self.current_stage.unwrap_or(0);
            self.current_time = hours_since_start;
            let fs = self.front_cache.strength;
            self.weather_noise = 0.3 * fs
                + simplex_noise::noise(hours_since_start, 24000.0) as f32 * (1.0 - 0.3 * fs);
            self.weather_noise = (self.weather_noise + 1.0) * 0.5;
            let kb = self.kate_bob_storm_progress;
            self.weather_stages[cs].update_t(self.current_time, kb);
            self.stage_progress = self.weather_stages[cs].linear_t;
            self.total_progress = (self.current_time - self.weather_stages[0].stage_start) as f32
                / self.duration as f32;
            self.total_progress = clamp01(self.total_progress);
            let next_target = match self.weather_stages[cs].next_stage {
                Some(n) => self.weather_stages[n].target_strength,
                None => 0.0,
            };
            self.current_strength = self.weather_stages[cs].get_stage_current_strength(next_target);
            self.update_current_stage(cm, gt, env, rand);
            let temperature_mod = clamp(-1.0, 1.0, self.current_strength * 2.0) * 7.0;
            let temp_internal = cm.climate_floats[FLOAT_TEMPERATURE].internal_value;
            if self.front_cache.type_ == 1 {
                self.temperature_influence = temp_internal - temperature_mod;
            } else {
                self.temperature_influence = temp_internal + temperature_mod;
            }
            if self.is_running {
                if self.weather_noise > self.rain_threshold {
                    self.precipitation_final =
                        (self.weather_noise - self.rain_threshold) / (1.0 - self.rain_threshold);
                    self.precipitation_final *= self.current_strength;
                } else {
                    self.precipitation_final = 0.0;
                }
                let night = cm.climate_floats[FLOAT_NIGHT_STRENGTH].internal_value;
                let current_darkness = self.precipitation_final;
                let str_night_fix = current_darkness * (1.0 - night);
                let mut val_cloud = 0.5f32;
                val_cloud += 0.5 * (1.0 - night);
                val_cloud = math_max_f32(val_cloud, cm.climate_floats[FLOAT_CLOUD_INTENSITY].internal_value);
                let mut base_amb_mod = 0.55f32;
                let stage_id = self.weather_stages[cs].stage_id;
                let parabolic_t = self.weather_stages[cs].parabolic_t;
                if env.fog_quality != 2 && stage_id == STAGE_TROPICAL_STORM {
                    base_amb_mod += 0.35 * parabolic_t;
                }
                let mut ambience = 1.0 - base_amb_mod * current_darkness;
                ambience = math_min_f32(ambience, 1.0 - night);
                if env.fog_quality != 2 && stage_id == STAGE_BLIZZARD {
                    let cloudmod = 1.0 - 0.75 * parabolic_t;
                    val_cloud *= cloudmod;
                }
                let cs_ = self.current_strength;
                cm.climate_floats[FLOAT_CLOUD_INTENSITY].set_override(val_cloud, cs_);
                cm.climate_floats[FLOAT_PRECIPITATION_INTENSITY]
                    .set_override(self.precipitation_final, 1.0);
                let cc = *self.cloud_color();
                cm.climate_colors[COLOR_GLOBAL_LIGHT].set_override(&cc, str_night_fix);
                cm.climate_floats[FLOAT_GLOBAL_LIGHT_INTENSITY].set_override(0.4, str_night_fix);
                cm.climate_floats[FLOAT_DESATURATION].set_override(0.3, cs_);
                cm.climate_floats[FLOAT_TEMPERATURE].set_override(self.temperature_influence, cs_);
                cm.climate_floats[FLOAT_AMBIENT].set_override(ambience, current_darkness);
                cm.climate_floats[FLOAT_DAYLIGHT_STRENGTH].set_override(ambience, current_darkness);
                let below_zero_winter = cm.get_temperature() < 0.0 && cm.get_season().is_season(5);
                if !below_zero_winter && !cm.winter_is_coming {
                    cm.climate_booleans[BOOL_IS_SNOW].set_enable_override(false);
                } else {
                    cm.climate_booleans[BOOL_IS_SNOW].set_override(true);
                }
            }
        } else if self.is_running {
            cm.reset_overrides();
            self.is_running = false;
        }
    }

    /// zombie.iso.weather.WeatherPeriod.updateCurrentStage: per stage kind the overrides it puts on fog, wind and
    /// wind angle (a start stage's rain threshold; heavy precipitation, blizzard and storms push the noise up
    /// at their start and end); a storm stage starts 1 or 2 thunder clouds once (the first following a player
    /// when the front is strong); blizzards and storms colour the fog.
    fn update_current_stage<R: JavaRandom>(
        &mut self,
        cm: &mut ClimateManager,
        gt: &GameTime,
        env: &ClimateEnv,
        rand: &mut RandAbstract<R>,
    ) {
        if self.is_dummy {
            return;
        }
        let cs = self.current_stage.unwrap_or(0);
        self.is_blizzard = false;
        self.is_thunder_storm = false;
        self.is_tropical_storm = false;
        let (stage_id, linear_t, parabolic_t, stage_start, stage_end, stage_duration, fog_strength) = {
            let s = &self.weather_stages[cs];
            (s.stage_id, s.linear_t, s.parabolic_t, s.stage_start, s.stage_end, s.stage_duration, s.fog_strength)
        };
        let fs = self.front_cache.strength;
        // The push-up of the weather noise at the start and end of a stage (heavy precipitation, blizzard, storm).
        let pushup = |current_time: f64| -> f32 {
            let mut p = fs * 0.5;
            if linear_t < 0.1 {
                p = clerp(((current_time - stage_start) / (stage_duration * 0.1)) as f32, 0.0, fs * 0.5);
            } else if linear_t > 0.9 {
                p = clerp(
                    1.0 - ((stage_end - current_time) / (stage_duration * 0.1)) as f32,
                    fs * 0.5,
                    0.0,
                );
            }
            p
        };
        match stage_id {
            STAGE_START => {
                self.rain_threshold = 0.35 - fs * 0.2;
                cm.climate_floats[FLOAT_FOG_INTENSITY].set_override(0.0, linear_t);
            }
            STAGE_SHOWERS => {
                cm.climate_floats[FLOAT_FOG_INTENSITY].set_override(0.0, 1.0);
                let lerpmod = clamp01(parabolic_t * 3.0);
                cm.climate_floats[FLOAT_WIND_INTENSITY].set_override(0.1 * self.weather_noise, lerpmod);
                cm.climate_floats[FLOAT_WIND_ANGLE_INTENSITY].set_override(0.0, lerpmod);
            }
            STAGE_HEAVY_PRECIP => {
                let p = pushup(self.current_time);
                self.weather_noise = p + self.weather_noise * (1.0 - p);
                cm.climate_floats[FLOAT_FOG_INTENSITY].set_override(0.0, 1.0);
                let lerpmod = clamp01(parabolic_t * 3.0);
                cm.climate_floats[FLOAT_WIND_INTENSITY].set_override(0.5 * self.weather_noise, lerpmod);
                cm.climate_floats[FLOAT_WIND_ANGLE_INTENSITY]
                    .set_override(0.7 * self.weather_noise * self.wind_angle_dir_mod, lerpmod);
            }
            STAGE_CLEARING => {
                cm.climate_floats[FLOAT_FOG_INTENSITY].set_override(0.0, 1.0 - linear_t);
            }
            STAGE_MODERATE | STAGE_DRIZZLE => {
                cm.climate_floats[FLOAT_FOG_INTENSITY].set_override(0.0, 1.0);
            }
            STAGE_BLIZZARD => {
                self.is_blizzard = true;
                let p = pushup(self.current_time);
                self.weather_noise = p + self.weather_noise * (1.0 - p);
                let lerpmod = clamp01(parabolic_t * 3.0);
                cm.climate_floats[FLOAT_WIND_INTENSITY]
                    .set_override(0.75 + 0.25 * self.weather_noise, lerpmod);
                cm.climate_floats[FLOAT_WIND_ANGLE_INTENSITY]
                    .set_override(0.7 * self.weather_noise * self.wind_angle_dir_mod, lerpmod);
                if env.fog_quality != 2 {
                    if fog_strength > 0.0 {
                        cm.climate_floats[FLOAT_FOG_INTENSITY].set_override(fog_strength, lerpmod);
                    } else {
                        cm.climate_floats[FLOAT_FOG_INTENSITY].set_override(1.0, lerpmod);
                    }
                }
            }
            STAGE_TROPICAL_STORM | STAGE_STORM | STAGE_KATEBOB_STORM => {
                if stage_id == STAGE_TROPICAL_STORM {
                    self.is_tropical_storm = true;
                }
                self.is_thunder_storm = !self.is_tropical_storm;
                if !self.weather_stages[cs].has_started_cloud {
                    let angl = self.front_cache.front_wind_angle_degrees;
                    let mut strength = self.front_cache.strength;
                    let mut radius = 8000.0 * strength;
                    let event_freq = strength;
                    let mut thunder_ratio = 0.6 * strength;
                    let duration = stage_duration;
                    let mut target_random_player = f64::from(strength) > 0.7;
                    let mut clouds = rand.next_range(1, 3);
                    if stage_id == STAGE_TROPICAL_STORM {
                        clouds = 1;
                        radius = 15000.0;
                        thunder_ratio = 0.8;
                        target_random_player = true;
                        strength = 1.0;
                    }
                    for _ in 0..clouds {
                        let offset = if stage_id == STAGE_KATEBOB_STORM {
                            self.kate_bob_storm_progress
                        } else {
                            0.0
                        };
                        let cloud = cm.thunder_storm.start_thunder_cloud(
                            strength,
                            angl,
                            radius,
                            event_freq,
                            thunder_ratio,
                            duration,
                            target_random_player,
                            offset,
                            gt,
                            env,
                            rand,
                        );
                        if stage_id == STAGE_KATEBOB_STORM && target_random_player {
                            if let Some(ci) = cloud {
                                let diag = cm.thunder_storm.get_map_diagonal();
                                cm.thunder_storm.clouds[ci].set_center(
                                    self.kate_bob_storm_x,
                                    self.kate_bob_storm_y,
                                    angl,
                                    diag,
                                    rand,
                                );
                            }
                        }
                        target_random_player = false;
                    }
                    self.weather_stages[cs].has_started_cloud = true;
                }
                let p = pushup(self.current_time);
                self.weather_noise = p + self.weather_noise * (1.0 - p);
                let lerpmod = clamp01(parabolic_t * 3.0);
                if stage_id == STAGE_TROPICAL_STORM {
                    cm.climate_floats[FLOAT_WIND_INTENSITY]
                        .set_override(0.4 + 0.6 * self.weather_noise, lerpmod);
                } else {
                    cm.climate_floats[FLOAT_WIND_INTENSITY]
                        .set_override(0.2 + 0.5 * self.weather_noise, lerpmod);
                }
                cm.climate_floats[FLOAT_WIND_ANGLE_INTENSITY]
                    .set_override(0.7 * self.weather_noise * self.wind_angle_dir_mod, lerpmod);
                if env.fog_quality != 2 {
                    if fog_strength > 0.0 {
                        cm.climate_floats[FLOAT_FOG_INTENSITY].set_override(fog_strength, lerpmod);
                        let tint = if stage_id == STAGE_TROPICAL_STORM {
                            cm.fog_tint_tropical
                        } else {
                            cm.fog_tint_storm
                        };
                        cm.climate_colors[COLOR_NEW_FOG].set_override(&tint, lerpmod);
                    } else {
                        cm.climate_floats[FLOAT_FOG_INTENSITY].set_override(0.0, 1.0);
                    }
                }
            }
            STAGE_INTERMEZZO => {
                cm.climate_floats[FLOAT_FOG_INTENSITY].set_override(0.0, 1.0);
            }
            STAGE_MODDED => {}
            _ => {
                cm.reset_overrides();
                self.is_running = false;
            }
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
        self.cloud_color_mut().load(input, world_version)?;
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
