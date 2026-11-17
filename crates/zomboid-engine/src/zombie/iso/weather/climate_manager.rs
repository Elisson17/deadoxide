//! zombie.iso.weather.ClimateManager: the world's climate (temperature, wind, fog, clouds, light colour, snow), its
//! weather periods, thunder and forecasts, and the part of `map_t.bin` that stores it after the game time.
//!
//! Ownership: the original's singletons and globals the climate reads (GameTime, SandboxOptions, GameServer/
//! GameClient flags, PerformanceSettings, Rand) are passed in (`GameTime`, `ClimateEnv`, a `RandAbstract`);
//! ErosionMain's season object, which the original shares by reference, is held here (`season`) and the host
//! applies ErosionMain's daily `setDay` to it. Network packets, IsoPuddles/IsoWeatherFX/SkyBox/IsoWater updates,
//! snow sprites, world flares and the Lua events with no climate effect are not modelled (each noted where it
//! happens).
use super::climate_color_info::ClimateColorInfo;
use super::climate_forecaster::ClimateForecaster;
use super::climate_moon::ClimateMoon;
use super::climate_values::{ClimateValues, ValuesEnv};
use super::simplex_noise;
use super::thunder_storm::ThunderStorm;
use super::weather_period::{PeriodCtx, WeatherPeriod};
use crate::java::{math_max_f32, math_min_f32, JavaRandom};
use crate::java_calendar::GregorianCalendar;
use crate::java_fdlibm;
use crate::java_nio::ByteBuffer;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTable;
use crate::zombie::core::random::rand_abstract::RandAbstract;
use crate::zombie::erosion::season::erosion_season::ErosionSeason;
use crate::zombie::game_time::GameTime;
use crate::zombie::iso::map_meta::MetaSide;

pub const FRONT_COLD: i32 = -1;
pub const FRONT_STATIONARY: i32 = 0;
pub const FRONT_WARM: i32 = 1;
pub const FLOAT_DESATURATION: usize = 0;
pub const FLOAT_GLOBAL_LIGHT_INTENSITY: usize = 1;
pub const FLOAT_NIGHT_STRENGTH: usize = 2;
pub const FLOAT_PRECIPITATION_INTENSITY: usize = 3;
pub const FLOAT_TEMPERATURE: usize = 4;
pub const FLOAT_FOG_INTENSITY: usize = 5;
pub const FLOAT_WIND_INTENSITY: usize = 6;
pub const FLOAT_WIND_ANGLE_INTENSITY: usize = 7;
pub const FLOAT_CLOUD_INTENSITY: usize = 8;
pub const FLOAT_AMBIENT: usize = 9;
pub const FLOAT_VIEW_DISTANCE: usize = 10;
pub const FLOAT_DAYLIGHT_STRENGTH: usize = 11;
pub const FLOAT_HUMIDITY: usize = 12;
pub const FLOAT_MAX: usize = 13;
pub const COLOR_GLOBAL_LIGHT: usize = 0;
pub const COLOR_NEW_FOG: usize = 1;
pub const COLOR_MAX: usize = 2;
pub const BOOL_IS_SNOW: usize = 0;
pub const BOOL_MAX: usize = 1;

/// What the climate reads from the rest of the game, which the original takes from globals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClimateEnv {
    /// GameServer.server
    pub server: bool,
    /// GameClient.client
    pub client: bool,
    /// Core.debug
    pub debug: bool,
    /// SandboxOptions.getRainModifier (the Rain option, 1..=5)
    pub rain_modifier: i32,
    /// SandboxOptions.climateCycle
    pub climate_cycle: i32,
    /// SandboxOptions.fogCycle
    pub fog_cycle: i32,
    /// SandboxOptions.dayNightCycle (GameTime/ErosionSeason.isEndlessDay = 2, isEndlessNight = 3)
    pub day_night_cycle: i32,
    /// PerformanceSettings.fogQuality
    pub fog_quality: i32,
    /// `SceneShaderStore.weatherShader != null && Core.getInstance().getOffscreenBuffer() != null`
    pub weather_shader: bool,
    /// GameServer.Players.isEmpty(): a thunder cloud meant to follow a player is dropped.
    pub no_players: bool,
    /// Core.getOptionDisableLightningDuringStorms
    pub disable_lightning_during_storms: bool,
}

/// zombie.iso.weather.ClimateManager.AirFront
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirFront {
    pub days: f32,
    pub max_noise: f32,
    pub total_noise: f32,
    pub type_: i32,
    pub strength: f32,
    pub tmp_noise_abs: f32,
    pub noise_cache: [f32; 2],
    pub noise_cache_value: f32,
    pub front_wind_angle_degrees: f32,
}

impl Default for AirFront {
    /// zombie.iso.weather.ClimateManager.AirFront(): `reset()`, so the noise cache is -1.
    fn default() -> Self {
        AirFront {
            days: 0.0,
            max_noise: 0.0,
            total_noise: 0.0,
            type_: 0,
            strength: 0.0,
            tmp_noise_abs: 0.0,
            noise_cache: [-1.0; 2],
            noise_cache_value: 0.0,
            front_wind_angle_degrees: 0.0,
        }
    }
}

impl AirFront {
    /// zombie.iso.weather.ClimateManager.AirFront.setFrontType: reset, then the type.
    pub fn set_front_type(&mut self, type_: i32) {
        self.reset();
        self.type_ = type_;
    }

    /// zombie.iso.weather.ClimateManager.AirFront.setFrontWind
    pub fn set_front_wind(&mut self, windangledegrees: f32) {
        self.front_wind_angle_degrees = windangledegrees;
    }

    /// zombie.iso.weather.ClimateManager.AirFront.setStrength
    pub fn set_strength(&mut self, str_: f32) {
        self.strength = str_;
    }

    /// zombie.iso.weather.ClimateManager.AirFront.reset (tmpNoiseAbs and noiseCacheValue stay).
    pub fn reset(&mut self) {
        self.days = 0.0;
        self.max_noise = 0.0;
        self.total_noise = 0.0;
        self.type_ = 0;
        self.strength = 0.0;
        self.front_wind_angle_degrees = 0.0;
        self.noise_cache = [-1.0; 2];
    }

    /// zombie.iso.weather.ClimateManager.AirFront.addDaySample: a day of air-mass noise; a sample on the front's
    /// side strengthens it (three quarters the largest of the last two days, a quarter the largest ever), one on
    /// the other side zeroes the strength.
    pub fn add_day_sample(&mut self, noiseval: f32) {
        self.days += 1.0;
        if (self.type_ != 1 || noiseval > 0.0) && (self.type_ != -1 || noiseval < 0.0) {
            self.tmp_noise_abs = noiseval.abs();
            if self.tmp_noise_abs > self.max_noise {
                self.max_noise = self.tmp_noise_abs;
            }
            self.total_noise += self.tmp_noise_abs;
            self.noise_cache_value = 0.0;
            let len = self.noise_cache.len();
            for i in (0..len).rev() {
                if self.noise_cache[i] > self.noise_cache_value {
                    self.noise_cache_value = self.noise_cache[i];
                }
                if i < len - 1 {
                    self.noise_cache[i + 1] = self.noise_cache[i];
                }
            }
            self.noise_cache[0] = self.tmp_noise_abs;
            if self.tmp_noise_abs > self.noise_cache_value {
                self.noise_cache_value = self.tmp_noise_abs;
            }
            self.strength = self.noise_cache_value * 0.75 + self.max_noise * 0.25;
        } else {
            self.strength = 0.0;
        }
    }

    /// zombie.iso.weather.ClimateManager.AirFront.copyFrom (not the noise cache).
    pub fn copy_from(&mut self, other: &AirFront) {
        self.days = other.days;
        self.max_noise = other.max_noise;
        self.total_noise = other.total_noise;
        self.type_ = other.type_;
        self.strength = other.strength;
        self.front_wind_angle_degrees = other.front_wind_angle_degrees;
    }

    /// zombie.iso.weather.ClimateManager.AirFront.load(DataInputStream): days, noise, type, strength and wind angle,
    /// then the saved noise cache: values past the two it holds are read and dropped, missing ones become -1.
    pub fn load(&mut self, input: &mut ByteBuffer) -> Result<(), String> {
        self.days = input.get_float()?;
        self.max_noise = input.get_float()?;
        self.total_noise = input.get_float()?;
        self.type_ = input.get_int()?;
        self.strength = input.get_float()?;
        self.front_wind_angle_degrees = input.get_float()?;
        let len = input.get_int()?;
        let cache_len = self.noise_cache.len() as i32;
        let max = if len > cache_len { len } else { cache_len };
        for i in 0..max {
            if i < len {
                let val = input.get_float()?;
                if i < cache_len {
                    self.noise_cache[i as usize] = val;
                }
            } else if i < cache_len {
                self.noise_cache[i as usize] = -1.0;
            }
        }
        Ok(())
    }
}

/// zombie.iso.weather.ClimateManager.ClimateFloat: a climate value as the simulation computes it (internal), with
/// the weather's override, a mod's value and an admin's value on top (final).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClimateFloat {
    pub internal_value: f32,
    pub final_value: f32,
    pub is_override: bool,
    pub override_: f32,
    pub is_override_value: bool,
    pub override_internal: f32,
    pub interpolate: f32,
    pub is_modded: bool,
    pub modded_value: f32,
    pub mod_interpolate: f32,
    pub is_admin_override: bool,
    pub admin_value: f32,
    pub min: f32,
    pub max: f32,
    pub id: i32,
    pub name: &'static str,
}

impl Default for ClimateFloat {
    /// zombie.iso.weather.ClimateManager.ClimateFloat(): max 1, the rest 0.
    fn default() -> Self {
        ClimateFloat {
            internal_value: 0.0,
            final_value: 0.0,
            is_override: false,
            override_: 0.0,
            is_override_value: false,
            override_internal: 0.0,
            interpolate: 0.0,
            is_modded: false,
            modded_value: 0.0,
            mod_interpolate: 0.0,
            is_admin_override: false,
            admin_value: 0.0,
            min: 0.0,
            max: 1.0,
            id: 0,
            name: "",
        }
    }
}

impl ClimateFloat {
    /// zombie.iso.weather.ClimateManager.ClimateFloat.init
    fn init(&mut self, id: i32, name: &'static str) {
        self.id = id;
        self.name = name;
    }

    /// zombie.iso.weather.ClimateManager.ClimateFloat.getFinalValue
    pub fn get_final_value(&self) -> f32 {
        self.final_value
    }

    /// zombie.iso.weather.ClimateManager.ClimateFloat.getInternalValue
    pub fn get_internal_value(&self) -> f32 {
        self.internal_value
    }

    /// zombie.iso.weather.ClimateManager.ClimateFloat.setOverride
    pub fn set_override(&mut self, targ: f32, inter: f32) {
        self.override_ = targ;
        self.interpolate = inter;
        self.is_override = true;
    }

    /// zombie.iso.weather.ClimateManager.ClimateFloat.setOverrideValue
    pub fn set_override_value(&mut self, override_value: bool) {
        self.is_override_value = override_value;
        self.is_override = override_value;
    }

    /// zombie.iso.weather.ClimateManager.ClimateFloat.setEnableOverride
    pub fn set_enable_override(&mut self, b: bool) {
        self.is_override = b;
    }

    /// zombie.iso.weather.ClimateManager.ClimateFloat.setEnableAdmin
    pub fn set_enable_admin(&mut self, b: bool) {
        self.is_admin_override = b;
    }

    /// zombie.iso.weather.ClimateManager.ClimateFloat.setEnableModded
    pub fn set_enable_modded(&mut self, b: bool) {
        self.is_modded = b;
    }

    /// zombie.iso.weather.ClimateManager.ClimateFloat.calculate: the admin value (not on a client), else the
    /// internal value moved toward a mod's value, then toward the override (from `overrideInternal` when the
    /// override replaces the value).
    fn calculate(&mut self, client: bool) {
        if self.is_admin_override && !client {
            self.final_value = self.admin_value;
        } else {
            if self.is_modded && self.mod_interpolate > 0.0 {
                self.internal_value =
                    lerp(self.mod_interpolate, self.internal_value, self.modded_value);
            }
            if !self.is_override || self.interpolate <= 0.0 || self.interpolate.is_nan() {
                self.final_value = self.internal_value;
            } else if self.is_override_value {
                self.final_value = lerp(self.interpolate, self.override_internal, self.override_);
            } else {
                self.final_value = lerp(self.interpolate, self.internal_value, self.override_);
            }
        }
    }

    /// zombie.iso.weather.ClimateManager.ClimateFloat.loadAdmin
    pub fn load_admin(
        &mut self,
        input: &mut ByteBuffer,
        _world_version: i32,
    ) -> Result<(), String> {
        self.is_admin_override = input.get()? != 0;
        self.admin_value = input.get_float()?;
        Ok(())
    }
}

/// zombie.iso.weather.ClimateManager.ClimateColor
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClimateColor {
    pub internal_value: ClimateColorInfo,
    pub final_value: ClimateColorInfo,
    pub is_override: bool,
    pub override_: ClimateColorInfo,
    pub interpolate: f32,
    pub is_modded: bool,
    pub modded_value: ClimateColorInfo,
    pub mod_interpolate: f32,
    pub is_admin_override: bool,
    pub admin_value: ClimateColorInfo,
    pub id: i32,
    pub name: &'static str,
}

impl Default for ClimateColor {
    /// zombie.iso.weather.ClimateManager.ClimateColor(): new colour infos.
    fn default() -> Self {
        ClimateColor {
            internal_value: ClimateColorInfo::default(),
            final_value: ClimateColorInfo::default(),
            is_override: false,
            override_: ClimateColorInfo::default(),
            interpolate: 0.0,
            is_modded: false,
            modded_value: ClimateColorInfo::default(),
            mod_interpolate: 0.0,
            is_admin_override: false,
            admin_value: ClimateColorInfo::default(),
            id: 0,
            name: "",
        }
    }
}

impl ClimateColor {
    /// zombie.iso.weather.ClimateManager.ClimateColor.init
    fn init(&mut self, id: i32, name: &'static str) {
        self.id = id;
        self.name = name;
    }

    /// zombie.iso.weather.ClimateManager.ClimateColor.setOverride(ClimateColorInfo, float)
    pub fn set_override(&mut self, targ: &ClimateColorInfo, inter: f32) {
        self.override_ = *targ;
        self.interpolate = inter;
        self.is_override = true;
    }

    /// zombie.iso.weather.ClimateManager.ClimateColor.setEnableOverride
    pub fn set_enable_override(&mut self, b: bool) {
        self.is_override = b;
    }

    /// zombie.iso.weather.ClimateManager.ClimateColor.setEnableAdmin
    pub fn set_enable_admin(&mut self, b: bool) {
        self.is_admin_override = b;
    }

    /// zombie.iso.weather.ClimateManager.ClimateColor.setEnableModded
    pub fn set_enable_modded(&mut self, b: bool) {
        self.is_modded = b;
    }

    /// zombie.iso.weather.ClimateManager.ClimateColor.calculate
    fn calculate(&mut self, client: bool) {
        if self.is_admin_override && !client {
            self.final_value = self.admin_value;
        } else {
            if self.is_modded && self.mod_interpolate > 0.0 {
                self.internal_value = self
                    .internal_value
                    .interp(&self.modded_value, self.mod_interpolate);
            }
            if self.is_override && self.interpolate > 0.0 {
                self.final_value = self
                    .internal_value
                    .interp(&self.override_, self.interpolate);
            } else {
                self.final_value = self.internal_value;
            }
        }
    }

    /// zombie.iso.weather.ClimateManager.ClimateColor.loadAdmin
    pub fn load_admin(&mut self, input: &mut ByteBuffer, world_version: i32) -> Result<(), String> {
        self.is_admin_override = input.get()? != 0;
        self.admin_value.load(input, world_version)
    }
}

/// zombie.iso.weather.ClimateManager.ClimateBool
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClimateBool {
    pub internal_value: bool,
    pub final_value: bool,
    pub is_override: bool,
    pub override_: bool,
    pub is_modded: bool,
    pub modded_value: bool,
    pub is_admin_override: bool,
    pub admin_value: bool,
    pub id: i32,
    pub name: &'static str,
}

impl ClimateBool {
    /// zombie.iso.weather.ClimateManager.ClimateBool.init
    fn init(&mut self, id: i32, name: &'static str) {
        self.id = id;
        self.name = name;
    }

    /// zombie.iso.weather.ClimateManager.ClimateBool.setOverride
    pub fn set_override(&mut self, b: bool) {
        self.is_override = true;
        self.override_ = b;
    }

    /// zombie.iso.weather.ClimateManager.ClimateBool.setEnableOverride
    pub fn set_enable_override(&mut self, b: bool) {
        self.is_override = b;
    }

    /// zombie.iso.weather.ClimateManager.ClimateBool.setEnableAdmin
    pub fn set_enable_admin(&mut self, b: bool) {
        self.is_admin_override = b;
    }

    /// zombie.iso.weather.ClimateManager.ClimateBool.setEnableModded
    pub fn set_enable_modded(&mut self, b: bool) {
        self.is_modded = b;
    }

    /// zombie.iso.weather.ClimateManager.ClimateBool.calculate
    fn calculate(&mut self, client: bool) {
        if self.is_admin_override && !client {
            self.final_value = self.admin_value;
        } else if self.is_modded {
            self.final_value = self.modded_value;
        } else {
            self.final_value = if self.is_override {
                self.override_
            } else {
                self.internal_value
            };
        }
    }

    /// zombie.iso.weather.ClimateManager.ClimateBool.loadAdmin
    pub fn load_admin(
        &mut self,
        input: &mut ByteBuffer,
        _world_version: i32,
    ) -> Result<(), String> {
        self.is_admin_override = input.get()? != 0;
        self.admin_value = input.get()? != 0;
        Ok(())
    }
}

/// zombie.iso.weather.ClimateManager.DayInfo
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DayInfo {
    pub day: i32,
    pub month: i32,
    pub year: i32,
    pub hour: i32,
    pub minutes: i32,
    pub date_value: i64,
    pub calendar: Option<GregorianCalendar>,
    pub season: Option<ErosionSeason>,
}

impl DayInfo {
    /// zombie.iso.weather.ClimateManager.DayInfo.getSeason (set by setDayInfo before any read; the original would
    /// throw on a null season where this panics).
    pub fn season(&self) -> &ErosionSeason {
        self.season
            .as_ref()
            .expect("ClimateManager.DayInfo.season is set by setDayInfo")
    }
}

/// zombie.iso.weather.ClimateManager.setDayInfo: the date `day_offset` days from the given one (in UTC), its
/// fields and milliseconds, and the day's season (a clone of the manager's season on first use) set to it.
pub fn set_day_info(
    day_info: &mut DayInfo,
    day: i32,
    month: i32,
    year: i32,
    day_offset: i32,
    season: &ErosionSeason,
) {
    let mut calendar = GregorianCalendar::new(year, month, day, 0, 0);
    calendar.add_days(day_offset);
    let f = calendar.fields();
    day_info.day = f.day_of_month;
    day_info.month = f.month;
    day_info.year = f.year;
    day_info.date_value = calendar.time_in_millis();
    day_info.calendar = Some(calendar);
    let s = day_info.season.get_or_insert_with(|| season.clone_season());
    s.set_day(day_info.day, day_info.month, day_info.year);
}

/// zombie.iso.weather.ClimateManager.SeasonColor: a colour per temperature kind (warm, normal, cloudy) and
/// season (summer, fall, winter, spring), blended by the season's progress and the cloudiness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeasonColor {
    final_col: ClimateColorInfo,
    temp_col: [ClimateColorInfo; 3],
    pub colors: [[ClimateColorInfo; 4]; 3],
    ignore_normal: bool,
}

impl Default for SeasonColor {
    /// zombie.iso.weather.ClimateManager.SeasonColor(): new colour infos, ignoreNormal true.
    fn default() -> Self {
        SeasonColor {
            final_col: ClimateColorInfo::default(),
            temp_col: [ClimateColorInfo::default(); 3],
            colors: [[ClimateColorInfo::default(); 4]; 3],
            ignore_normal: true,
        }
    }
}

impl SeasonColor {
    /// zombie.iso.weather.ClimateManager.SeasonColor.setIgnoreNormal
    pub fn set_ignore_normal(&mut self, b: bool) {
        self.ignore_normal = b;
    }

    /// zombie.iso.weather.ClimateManager.SeasonColor.getColor
    pub fn get_color(&self, temperature: usize, season: usize) -> &ClimateColorInfo {
        &self.colors[temperature][season]
    }

    /// zombie.iso.weather.ClimateManager.SeasonColor.setColorInterior
    pub fn set_color_interior(
        &mut self,
        temperature: usize,
        season: usize,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
    ) {
        self.colors[temperature][season].interior = [r, g, b, a];
    }

    /// zombie.iso.weather.ClimateManager.SeasonColor.setColorExterior
    pub fn set_color_exterior(
        &mut self,
        temperature: usize,
        season: usize,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
    ) {
        self.colors[temperature][season].exterior = [r, g, b, a];
    }

    /// zombie.iso.weather.ClimateManager.SeasonColor.update: each temperature kind between the two seasons, then
    /// warm to cloudy (through normal unless it is ignored) by `temperature_lerp`. Returns the final colour (the
    /// original returns its own object, which the manager only reads).
    pub fn update(
        &mut self,
        temperature_lerp: f32,
        season_lerp: f32,
        season_from: usize,
        season_to: usize,
    ) -> ClimateColorInfo {
        for i in 0..3 {
            if !self.ignore_normal || i != 1 {
                self.temp_col[i] =
                    self.colors[i][season_from].interp(&self.colors[i][season_to], season_lerp);
            }
        }
        if !self.ignore_normal {
            if temperature_lerp < 0.5 {
                let cf = temperature_lerp * 2.0;
                self.final_col = self.temp_col[0].interp(&self.temp_col[1], cf);
            } else {
                let cf = 1.0 - (temperature_lerp - 0.5) * 2.0;
                self.final_col = self.temp_col[2].interp(&self.temp_col[1], cf);
            }
        } else {
            self.final_col = self.temp_col[0].interp(&self.temp_col[2], temperature_lerp);
        }
        self.final_col
    }
}

/// zombie.iso.weather.ClimateManager.clamp01
pub fn clamp01(val: f32) -> f32 {
    clamp(0.0, 1.0, val)
}

/// zombie.iso.weather.ClimateManager.clamp(float, float, float): Math.min then Math.max.
pub fn clamp(min: f32, max: f32, val: f32) -> f32 {
    let val = math_min_f32(max, val);
    math_max_f32(min, val)
}

/// zombie.iso.weather.ClimateManager.lerp
pub fn lerp(t: f32, a: f32, b: f32) -> f32 {
    a + t * (b - a)
}

/// zombie.iso.weather.ClimateManager.clerp: cosine interpolation, `(float)(1 - cos(t * PI)) / 2` (fdlibm cos).
pub fn clerp(t: f32, a: f32, b: f32) -> f32 {
    let t2 = (1.0 - java_fdlibm::cos(f64::from(t) * std::f64::consts::PI)) as f32 / 2.0;
    a * (1.0 - t2) + b * t2
}

/// zombie.iso.weather.ClimateManager.getTimeLerpHours: getTimeLerp over the day fractions (each clamped to 0..1).
pub fn get_time_lerp_hours(cur: f32, min: f32, max: f32, do_clerp: bool) -> f32 {
    get_time_lerp(
        clamp(0.0, 1.0, cur / 24.0),
        clamp(0.0, 1.0, min / 24.0),
        clamp(0.0, 1.0, max / 24.0),
        do_clerp,
    )
}

/// zombie.iso.weather.ClimateManager.getTimeLerp: 0 outside [min, max] (wrapping past 1 when min > max), rising
/// to 1 at the middle and back to 0.
pub fn get_time_lerp(cur: f32, min: f32, max: f32, do_clerp: bool) -> f32 {
    let up = |c: f32, mid: f32| {
        if c < mid {
            if do_clerp {
                clerp(c / mid, 0.0, 1.0)
            } else {
                lerp(c / mid, 0.0, 1.0)
            }
        } else if do_clerp {
            clerp((c - mid) / mid, 1.0, 0.0)
        } else {
            lerp((c - mid) / mid, 1.0, 0.0)
        }
    };
    let adjust = min > max;
    if !adjust {
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        let inside = !(cur < min) && !(cur > max);
        if inside {
            let c = cur - min;
            let len = max - min;
            let mid = len * 0.5;
            up(c, mid)
        } else {
            0.0
        }
    } else if cur < min && cur > max {
        0.0
    } else {
        let minoffset = 1.0 - min;
        let c = if cur >= min {
            cur - min
        } else {
            cur + minoffset
        };
        let len = max + minoffset;
        let mid = len * 0.5;
        up(c, mid)
    }
}

/// zombie.iso.weather.ClimateManager
#[derive(Clone, Debug, PartialEq)]
pub struct ClimateManager {
    pub disable_simulation: bool,
    pub disable_fx_update: bool,
    pub disable_weather_generation: bool,
    /// ErosionMain's season (`init` takes it; ErosionMain sets its day, with ErosionMain's 0-based day).
    pub season: ErosionSeason,
    pub last_minute_stamp: i64,
    pub mod_data_table: Option<KahluaTable>,
    pub air_mass: f32,
    pub air_mass_daily: f32,
    pub air_mass_temperature: f32,
    pub base_temperature: f32,
    pub snow_fall: f32,
    pub snow_strength: f32,
    pub snow_melt_strength: f32,
    pub snow_frac_now: f32,
    pub can_do_winter_sprites: bool,
    pub was_force_snow: bool,
    pub wind_power: f32,
    pub weather_period: WeatherPeriod,
    pub thunder_storm: ThunderStorm,
    pub simplex_offset_a: f64,
    pub simplex_offset_b: f64,
    pub simplex_offset_c: f64,
    pub simplex_offset_d: f64,
    pub day_do_fog: bool,
    pub day_fog_strength: f32,
    pub world_age_hours: f64,
    pub tick_is_climate_tick: bool,
    pub tick_is_day_change: bool,
    pub last_hour_stamp: i32,
    pub tick_is_hour_change: bool,
    pub tick_is_ten_mins: bool,
    pub current_front: AirFront,
    col_day: ClimateColorInfo,
    col_dusk: ClimateColorInfo,
    col_dawn: ClimateColorInfo,
    pub col_night: ClimateColorInfo,
    pub col_night_no_moon: ClimateColorInfo,
    pub col_night_moon: ClimateColorInfo,
    pub col_fog: ClimateColorInfo,
    pub col_fog_legacy: ClimateColorInfo,
    pub col_fog_new: ClimateColorInfo,
    pub fog_tint_storm: ClimateColorInfo,
    pub fog_tint_tropical: ClimateColorInfo,
    /// ClimateManager.winterIsComing, theDescendingFog, aStormIsComing (statics set by `init` from the game mode).
    pub winter_is_coming: bool,
    pub the_descending_fog: bool,
    pub a_storm_is_coming: bool,
    pub climate_values: ClimateValues,
    pub climate_forecaster: ClimateForecaster,
    pub day_light_lagged: f32,
    pub night_lagged: f32,
    pub climate_floats: [ClimateFloat; FLOAT_MAX],
    pub climate_colors: [ClimateColor; COLOR_MAX],
    pub climate_booleans: [ClimateBool; BOOL_MAX],
    pub weather_override: i32,
    pub fog_override: i32,
    /// ClimateManager.windNoiseOffset, windNoiseBase, windNoiseFinal, windTickFinal (statics).
    pub wind_noise_offset: f64,
    pub wind_noise_base: f64,
    pub wind_noise_final: f64,
    pub wind_tick_final: f64,
    pub fog_lerp_value: f32,
    pub season_color_dawn: SeasonColor,
    pub season_color_day: SeasonColor,
    pub season_color_dusk: SeasonColor,
    pub previous_day: Option<DayInfo>,
    pub current_day: Option<DayInfo>,
    pub next_day: Option<DayInfo>,
    pub climate_values_fronts: Option<ClimateValues>,
    /// ClimateMoon.getInstance()
    pub moon: ClimateMoon,
    /// Core.forceSnow, which the sandbox overrides set and the snow update reads.
    pub force_snow: bool,
}

impl Default for ClimateManager {
    /// The constructor's state with the simplex offsets at 0 and no draw from Rand (what `GameTime.load` fills
    /// from a save).
    fn default() -> Self {
        ClimateManager::construct([0.0; 4])
    }
}

impl ClimateManager {
    /// zombie.iso.weather.ClimateManager(): the colours, four simplex offsets drawn from Rand (A in [0, 8000),
    /// B in [8000, 16000), C in [-8000, 0), D in [-16000, -8000)), the season colours, the climate values and
    /// the rest. The original then triggers the Lua event OnClimateManagerInit with itself, whose handler (the
    /// server's ClimateMain.lua) sets the night, fog and cloud colours and the season colours: the host runs it
    /// right after (`zombie::lua::exposed_climate`).
    pub fn new<R: JavaRandom>(rand: &mut RandAbstract<R>) -> Self {
        let a = f64::from(rand.next_range(0, 8000));
        let b = f64::from(rand.next_range(8000, 16000));
        let c = f64::from(rand.next_range(0, -8000));
        let d = f64::from(rand.next_range(-8000, -16000));
        Self::construct([a, b, c, d])
    }

    /// The constructor after the offsets: initSeasonColors, setup, the climate values (copying the offsets), the
    /// thunder storm, the weather period (with its own copy of the climate values), an empty forecaster.
    fn construct(offsets: [f64; 4]) -> Self {
        let mut season_color_dawn = SeasonColor::default();
        season_color_dawn.set_ignore_normal(true);
        let mut season_color_day = SeasonColor::default();
        season_color_day.set_ignore_normal(true);
        let mut season_color_dusk = SeasonColor::default();
        season_color_dusk.set_ignore_normal(false);
        let climate_values = ClimateValues::new(offsets);
        let weather_period = WeatherPeriod::new(climate_values.get_copy(offsets));
        let mut cm = ClimateManager {
            disable_simulation: false,
            disable_fx_update: false,
            disable_weather_generation: false,
            season: ErosionSeason::default(),
            last_minute_stamp: -1,
            mod_data_table: None,
            air_mass: 0.0,
            air_mass_daily: 0.0,
            air_mass_temperature: 0.0,
            base_temperature: 0.0,
            snow_fall: 0.0,
            snow_strength: 0.0,
            snow_melt_strength: 0.0,
            snow_frac_now: 0.0,
            can_do_winter_sprites: false,
            was_force_snow: false,
            wind_power: 0.0,
            weather_period,
            thunder_storm: ThunderStorm::default(),
            simplex_offset_a: offsets[0],
            simplex_offset_b: offsets[1],
            simplex_offset_c: offsets[2],
            simplex_offset_d: offsets[3],
            day_do_fog: false,
            day_fog_strength: 0.0,
            world_age_hours: 0.0,
            tick_is_climate_tick: false,
            tick_is_day_change: false,
            last_hour_stamp: -1,
            tick_is_hour_change: false,
            tick_is_ten_mins: false,
            current_front: AirFront::default(),
            col_day: ClimateColorInfo::default(),
            col_dusk: ClimateColorInfo::default(),
            col_dawn: ClimateColorInfo::default(),
            col_night: ClimateColorInfo::new8(0.33, 0.33, 0.33, 0.4, 0.33, 0.33, 0.33, 0.4),
            col_night_no_moon: ClimateColorInfo::new8(0.33, 0.33, 0.33, 0.4, 0.33, 0.33, 0.33, 0.4),
            col_night_moon: ClimateColorInfo::new8(0.33, 0.33, 0.33, 0.4, 0.33, 0.33, 0.33, 0.4),
            col_fog: ClimateColorInfo::new8(0.4, 0.4, 0.4, 0.8, 0.4, 0.4, 0.4, 0.8),
            col_fog_legacy: ClimateColorInfo::new8(0.3, 0.3, 0.3, 0.8, 0.3, 0.3, 0.3, 0.8),
            col_fog_new: ClimateColorInfo::new8(0.5, 0.5, 0.55, 0.4, 0.5, 0.5, 0.55, 0.8),
            fog_tint_storm: ClimateColorInfo::new8(0.5, 0.45, 0.4, 1.0, 0.5, 0.45, 0.4, 1.0),
            fog_tint_tropical: ClimateColorInfo::new8(0.8, 0.75, 0.55, 1.0, 0.8, 0.75, 0.55, 1.0),
            winter_is_coming: false,
            the_descending_fog: false,
            a_storm_is_coming: false,
            climate_values,
            climate_forecaster: ClimateForecaster::default(),
            day_light_lagged: 0.0,
            night_lagged: 0.0,
            climate_floats: [ClimateFloat::default(); FLOAT_MAX],
            climate_colors: [ClimateColor::default(); COLOR_MAX],
            climate_booleans: [ClimateBool::default(); BOOL_MAX],
            weather_override: 0,
            fog_override: 0,
            wind_noise_offset: 0.0,
            wind_noise_base: 0.0,
            wind_noise_final: 0.0,
            wind_tick_final: 0.0,
            fog_lerp_value: 0.0,
            season_color_dawn,
            season_color_day,
            season_color_dusk,
            previous_day: None,
            current_day: None,
            next_day: None,
            climate_values_fronts: None,
            moon: ClimateMoon::default(),
            force_snow: false,
        };
        cm.setup();
        cm
    }

    /// zombie.iso.weather.ClimateManager.setup: names and ranges of the floats, colours and the bool; the new fog
    /// colour starts at (0.9, 0.9, 0.95, 1).
    fn setup(&mut self) {
        const FLOATS: [&str; FLOAT_MAX] = [
            "DESATURATION",
            "GLOBAL_LIGHT_INTENSITY",
            "NIGHT_STRENGTH",
            "PRECIPITATION_INTENSITY",
            "TEMPERATURE",
            "FOG_INTENSITY",
            "WIND_INTENSITY",
            "WIND_ANGLE_INTENSITY",
            "CLOUD_INTENSITY",
            "AMBIENT",
            "VIEW_DISTANCE",
            "DAYLIGHT_STRENGTH",
            "HUMIDITY",
        ];
        for (i, f) in self.climate_floats.iter_mut().enumerate() {
            *f = ClimateFloat::default();
            f.init(i as i32, FLOATS[i]);
        }
        for c in self.climate_colors.iter_mut() {
            *c = ClimateColor::default();
        }
        for b in self.climate_booleans.iter_mut() {
            *b = ClimateBool::default();
        }
        self.climate_floats[FLOAT_TEMPERATURE].min = -80.0;
        self.climate_floats[FLOAT_TEMPERATURE].max = 80.0;
        self.climate_floats[FLOAT_WIND_ANGLE_INTENSITY].min = -1.0;
        self.climate_floats[FLOAT_VIEW_DISTANCE].min = 0.0;
        self.climate_floats[FLOAT_VIEW_DISTANCE].max = 100.0;
        self.climate_colors[COLOR_GLOBAL_LIGHT].init(0, "GLOBAL_LIGHT");
        self.climate_colors[COLOR_NEW_FOG].init(1, "COLOR_NEW_FOG");
        self.climate_colors[COLOR_NEW_FOG]
            .internal_value
            .set_exterior(0.9, 0.9, 0.95, 1.0);
        self.climate_colors[COLOR_NEW_FOG]
            .internal_value
            .set_interior(0.9, 0.9, 0.95, 1.0);
        self.climate_booleans[BOOL_IS_SNOW].init(0, "IS_SNOW");
    }

    /// The four simplex offsets, as `new ClimateValues(this)` copies them.
    pub fn offsets(&self) -> [f64; 4] {
        [
            self.simplex_offset_a,
            self.simplex_offset_b,
            self.simplex_offset_c,
            self.simplex_offset_d,
        ]
    }

    /// What ClimateValues reads from the manager and the options.
    pub fn values_env(&self, env: &ClimateEnv) -> ValuesEnv {
        ValuesEnv {
            simplex_offset_c: self.simplex_offset_c,
            simplex_offset_d: self.simplex_offset_d,
            rain_modifier: env.rain_modifier,
            day_night_cycle: env.day_night_cycle,
            winter_is_coming: self.winter_is_coming,
        }
    }

    /// zombie.iso.weather.ClimateManager.getClimateFloat
    pub fn get_climate_float(&self, id: usize) -> &ClimateFloat {
        &self.climate_floats[id]
    }

    /// zombie.iso.weather.ClimateManager.getClimateColor
    pub fn get_climate_color(&self, id: usize) -> &ClimateColor {
        &self.climate_colors[id]
    }

    /// zombie.iso.weather.ClimateManager.getClimateBool
    pub fn get_climate_bool(&self, id: usize) -> &ClimateBool {
        &self.climate_booleans[id]
    }

    /// zombie.iso.weather.ClimateManager.getTemperature
    pub fn get_temperature(&self) -> f32 {
        self.climate_floats[FLOAT_TEMPERATURE].final_value
    }

    /// zombie.iso.weather.ClimateManager.getWindspeedKph
    pub fn get_windspeed_kph(&self) -> f32 {
        self.wind_power * 120.0
    }

    /// zombie.iso.weather.ClimateManager.getAirTemperatureForSquare for a square given as its facts: whether it is
    /// in a room (`isInARoom`), its floor (`getZ`), the world's hydro power (`IsoWorld.isHydroPowerOn`) and, in a
    /// vehicle, the vehicle's `getInsideTemperature`. The cell's heat sources are taken as none
    /// (`IsoCell.getHeatSourceHighestTemperature` then answers the air temperature).
    pub fn get_air_temperature_for_square(
        &self,
        in_room: bool,
        z: i32,
        hydro_power: bool,
        vehicle_inside_temperature: Option<f32>,
        do_wind_chill: bool,
    ) -> f32 {
        let mut temp = self.get_temperature();
        let is_inside = in_room;
        let vehicle = vehicle_inside_temperature;
        if is_inside || vehicle.is_some() {
            if temp <= 22.0 {
                if is_inside && hydro_power {
                    temp = 22.0;
                }
                let mut modv = 22.0 - temp;
                if z < 1 {
                    temp += modv * (0.4 + 0.2 * self.day_light_lagged);
                } else {
                    modv = (f64::from(modv) * 0.85) as f32;
                    temp += modv * (0.4 + 0.2 * self.day_light_lagged);
                }
            } else {
                if is_inside && hydro_power {
                    temp = 22.0;
                }
                let mut modv = temp - 22.0;
                if z < 1 {
                    modv = (f64::from(modv) * 0.85) as f32;
                    temp -= modv * (0.4 + 0.2 * self.day_light_lagged);
                } else {
                    temp -= modv * (0.4 + 0.2 * self.day_light_lagged + 0.2 * self.night_lagged);
                }
                if !is_inside && vehicle.is_some() {
                    temp = temp + modv + modv * self.day_light_lagged;
                }
            }
        } else if do_wind_chill {
            temp = super::temperature::windchill_celsius_kph(temp, self.get_windspeed_kph());
        }
        // no heat sources in the cell
        if let Some(v) = vehicle {
            if !is_inside {
                temp += v;
            } else {
                temp += if v > 0.0 { v } else { 0.0 };
            }
        }
        temp
    }

    /// zombie.iso.weather.ClimateManager.getSeason: the current day's season once there is one, else ErosionMain's.
    pub fn get_season(&self) -> &ErosionSeason {
        match self.current_day.as_ref().and_then(|d| d.season.as_ref()) {
            Some(s) => s,
            None => &self.season,
        }
    }

    /// zombie.iso.weather.ClimateManager.setEnabledWeatherGeneration
    pub fn set_enabled_weather_generation(&mut self, b: bool) {
        self.disable_weather_generation = !b;
    }

    /// zombie.iso.weather.ClimateManager.getAirMassNoiseFrequencyMod: the air mass noise period by the sandbox
    /// Rain option.
    pub fn get_air_mass_noise_frequency_mod(sandbox_rain: i32) -> f64 {
        match sandbox_rain {
            1 => 300.0,
            2 => 240.0,
            4 => 145.0,
            5 => 120.0,
            _ => 166.0,
        }
    }

    /// zombie.iso.weather.ClimateManager.getRainTimeMultiplierMod
    pub fn get_rain_time_multiplier_mod(sandbox_rain: i32) -> f32 {
        match sandbox_rain {
            1 => 0.5,
            2 => 0.75,
            4 => 1.25,
            5 => 1.5,
            _ => 1.0,
        }
    }

    /// zombie.iso.weather.ClimateManager.init: ErosionMain's season, the thunder clouds' map bounds from the meta
    /// grid (in cells, 256 squares each, plus 4000 around), the wind noise restarted, the game-mode flags, then the
    /// forecasts. WorldFlares.Clear is not modelled; ClimateHistory.init does nothing.
    #[allow(clippy::too_many_arguments)]
    pub fn init(
        &mut self,
        season: ErosionSeason,
        meta_min_x: i32,
        meta_max_x: i32,
        meta_min_y: i32,
        meta_max_y: i32,
        game_mode: &str,
        gt: &GameTime,
        env: &ClimateEnv,
    ) {
        self.season = season;
        self.thunder_storm.map_min_x = meta_min_x * 256 - 4000;
        self.thunder_storm.map_max_x = meta_max_x * 256 + 4000;
        self.thunder_storm.map_min_y = meta_min_y * 256 - 4000;
        self.thunder_storm.map_max_y = meta_max_y * 256 + 4000;
        self.wind_noise_offset = 0.0;
        self.winter_is_coming = game_mode == "Winter is Coming";
        self.the_descending_fog = game_mode == "The Descending Fog";
        self.a_storm_is_coming = game_mode == "A Storm is Coming";
        let mut fc = std::mem::take(&mut self.climate_forecaster);
        fc.init(self, gt, env);
        self.climate_forecaster = fc;
    }

    /// zombie.iso.weather.ClimateManager.updateEveryTenMins
    pub fn update_every_ten_mins(&mut self) {
        self.tick_is_ten_mins = true;
    }

    /// zombie.iso.weather.ClimateManager.update: once per game minute (a new minute stamp) the day info, hour and
    /// moon; then (simulation on) the forecasts on a new day, the sandbox overrides, values and weather period
    /// on the minute tick, every float, colour and bool calculated, the wind tick (not on a server), thunder, snow
    /// (on the minute tick) and the view distance. Not modelled: the client's network interpolation (wall clock),
    /// the Lua events OnClimateTick/OnClimateTickDebug (no handler in the game's Lua), world flares, the
    /// server's climate packet every ten minutes and puddle sync, and updateFx (IsoPuddles on a server, which
    /// reads the climate without changing it).
    pub fn update<R: JavaRandom>(
        &mut self,
        gt: &mut GameTime,
        env: &ClimateEnv,
        rand: &mut RandAbstract<R>,
    ) {
        self.tick_is_climate_tick = false;
        self.tick_is_hour_change = false;
        self.tick_is_day_change = false;
        self.world_age_hours = gt.get_world_age_hours();
        if self.last_minute_stamp != gt.minutes_stamp {
            self.last_minute_stamp = gt.minutes_stamp;
            self.tick_is_climate_tick = true;
            self.update_day_info(gt.get_day_plus_one(), gt.month, gt.year);
            let (y, m, d) = match self.current_day.as_mut() {
                Some(cur) => {
                    cur.hour = gt.get_hour();
                    cur.minutes = gt.get_minutes();
                    (cur.year, cur.month, cur.day)
                }
                None => (0, 0, 0),
            };
            if gt.get_hour() != self.last_hour_stamp {
                self.tick_is_hour_change = true;
                self.last_hour_stamp = gt.get_hour();
            }
            self.moon.update_phase(y, m, d);
        }
        if self.disable_simulation {
            return;
        }
        if self.tick_is_day_change && !env.client {
            let mut fc = std::mem::take(&mut self.climate_forecaster);
            fc.update_day_change(self, gt, env);
            self.climate_forecaster = fc;
        }
        if self.tick_is_climate_tick && !env.client {
            self.update_sandbox_overrides(gt, env, rand);
            self.update_values(gt, env, rand);
            let mut wp = std::mem::take(&mut self.weather_period);
            wp.update(self.world_age_hours, self, gt, env, rand);
            self.weather_period = wp;
        }
        for c in self.climate_colors.iter_mut() {
            c.calculate(env.client);
        }
        for f in self.climate_floats.iter_mut() {
            f.calculate(env.client);
        }
        for b in self.climate_booleans.iter_mut() {
            b.calculate(env.client);
        }
        self.update_wind_tick(gt, env);
        self.wind_power = self.climate_floats[FLOAT_WIND_INTENSITY].final_value;
        self.thunder_storm
            .update(self.world_age_hours, gt, env, rand);
        if env.client || self.tick_is_climate_tick {
            self.update_snow(env);
        }
        if !env.client {
            self.update_view_distance(gt);
        }
        if self.tick_is_climate_tick && env.server && self.tick_is_ten_mins {
            self.tick_is_ten_mins = false;
        }
    }

    /// zombie.iso.weather.ClimateManager.updateSandboxOverrides: the ClimateCycle option (2 no weather, 3 endless
    /// rain, 4 endless storm, 5 endless snow, 6 endless blizzard) and FogCycle option (2 no fog, 3 endless fog)
    /// as overrides, new random targets every hour in the endless modes.
    fn update_sandbox_overrides<R: JavaRandom>(
        &mut self,
        gt: &GameTime,
        env: &ClimateEnv,
        rand: &mut RandAbstract<R>,
    ) {
        let mut weather_setting_changed = false;
        let minute_lerp = gt.time_of_day % 1.0;
        let climate_cycle = env.climate_cycle;
        let is_overriding_weather = climate_cycle > 1;
        let is_endless_weather = climate_cycle > 2;
        let is_no_weather = climate_cycle == 2;
        let is_endless_rain = climate_cycle == 3;
        let is_endless_snow = climate_cycle == 5;
        let is_endless_blizzard = climate_cycle == 6;
        let is_freezing_temp = is_endless_snow || is_endless_blizzard;
        if self.weather_override != climate_cycle {
            self.weather_override = climate_cycle;
            self.set_enabled_weather_generation(self.weather_override == 1);
            self.climate_floats[FLOAT_PRECIPITATION_INTENSITY]
                .set_override_value(is_overriding_weather);
            self.climate_floats[FLOAT_WIND_INTENSITY].set_override_value(is_overriding_weather);
            self.climate_floats[FLOAT_TEMPERATURE].set_override_value(is_freezing_temp);
            self.force_snow = is_freezing_temp;
            if is_no_weather {
                self.force_snow = false;
                self.climate_floats[FLOAT_PRECIPITATION_INTENSITY].set_override(0.0, 1.0);
                self.climate_floats[FLOAT_WIND_INTENSITY].set_override(0.0, 1.0);
                self.climate_floats[FLOAT_TEMPERATURE].set_override_value(false);
            } else if is_endless_weather {
                self.climate_floats[FLOAT_PRECIPITATION_INTENSITY].override_internal = 0.5;
                self.climate_floats[FLOAT_WIND_INTENSITY].override_internal = 0.3;
                if is_freezing_temp {
                    self.climate_floats[FLOAT_TEMPERATURE].override_internal =
                        rand.next_float(-20.0, 0.0);
                }
            }
            weather_setting_changed = is_overriding_weather;
        }
        if is_endless_weather {
            self.climate_floats[FLOAT_PRECIPITATION_INTENSITY].interpolate = minute_lerp;
            self.climate_floats[FLOAT_WIND_INTENSITY].interpolate = minute_lerp;
            self.climate_floats[FLOAT_TEMPERATURE].interpolate = minute_lerp;
            if self.tick_is_hour_change || weather_setting_changed {
                self.force_snow = is_freezing_temp;
                self.climate_booleans[BOOL_IS_SNOW].set_override(is_freezing_temp);
                let is_calm_weather = is_endless_rain || is_endless_snow;
                let p = &mut self.climate_floats[FLOAT_PRECIPITATION_INTENSITY];
                p.override_internal = p.override_;
                let v = rand.next_float(
                    if is_calm_weather { 0.1 } else { 0.5 },
                    if is_calm_weather { 0.5 } else { 1.0 },
                );
                self.climate_floats[FLOAT_PRECIPITATION_INTENSITY].set_override(v, minute_lerp);
                let w = &mut self.climate_floats[FLOAT_WIND_INTENSITY];
                w.override_internal = w.override_;
                let v = rand.next_float(
                    if is_calm_weather { 0.0 } else { 0.3 },
                    if is_calm_weather { 0.3 } else { 1.0 },
                );
                self.climate_floats[FLOAT_WIND_INTENSITY].set_override(v, minute_lerp);
                if is_endless_snow || is_endless_blizzard {
                    let t = &mut self.climate_floats[FLOAT_TEMPERATURE];
                    t.override_internal = t.override_;
                    let v = rand.next_float(-20.0, 0.0);
                    self.climate_floats[FLOAT_TEMPERATURE].set_override(v, minute_lerp);
                }
            }
        }
        let blizzard_fog = is_endless_blizzard && env.fog_cycle != 2;
        let fog_cycle = if blizzard_fog { 4 } else { env.fog_cycle };
        let is_overriding_fog = fog_cycle > 1;
        let is_no_fog = fog_cycle == 2;
        let is_endless_fog = fog_cycle >= 3;
        if self.fog_override != fog_cycle {
            self.fog_override = fog_cycle;
            let fog = &mut self.climate_floats[FLOAT_FOG_INTENSITY];
            fog.set_enable_override(is_overriding_fog);
            fog.set_override_value(is_overriding_fog);
            if is_no_fog {
                fog.set_override(0.0, 1.0);
            } else if is_endless_fog {
                fog.override_internal = 0.5;
            }
            weather_setting_changed = is_overriding_fog;
        }
        if is_endless_fog {
            self.climate_floats[FLOAT_FOG_INTENSITY].interpolate = minute_lerp;
            if self.tick_is_hour_change || weather_setting_changed {
                let fog = &mut self.climate_floats[FLOAT_FOG_INTENSITY];
                fog.override_internal = fog.override_;
                let v = rand.next_float(0.1, 1.0);
                self.climate_floats[FLOAT_FOG_INTENSITY].set_override(v, minute_lerp);
            }
        }
    }

    /// zombie.iso.weather.ClimateManager.updateWindTick (not on a server): the wind with a noise gust.
    fn update_wind_tick(&mut self, gt: &GameTime, env: &ClimateEnv) {
        if !env.server {
            let mut wind = self.climate_floats[FLOAT_WIND_INTENSITY].final_value;
            self.wind_noise_offset +=
                (4.0E-4 + 6.0E-4 * f64::from(wind)) * f64::from(gt.get_multiplier());
            self.wind_noise_base = simplex_noise::noise(0.0, self.wind_noise_offset);
            self.wind_noise_final = self.wind_noise_base;
            if self.wind_noise_final > 0.0 {
                self.wind_noise_final *= 0.04 + 0.1 * f64::from(wind);
            } else {
                self.wind_noise_final *=
                    0.04 + 0.1 * f64::from(wind) + f64::from(0.05f32 * (wind * wind));
            }
            wind = clamp01(wind + self.wind_noise_final as f32);
            self.wind_tick_final = f64::from(wind);
        }
    }

    /// zombie.iso.weather.ClimateManager.updateSnow (not a client): forced snow, else per minute the snowfall and
    /// melt gathered and on the hour applied to the snow strength (0..10) and the fraction of snow shown. The
    /// cell's snow target and ErosionIceQueen's snow sprites are not modelled.
    fn update_snow(&mut self, env: &ClimateEnv) {
        if env.client {
            return;
        }
        if self.force_snow {
            self.snow_frac_now = 0.7;
            self.was_force_snow = true;
        } else {
            if self.was_force_snow {
                self.snow_frac_now = if self.snow_strength > 7.5 {
                    1.0
                } else {
                    self.snow_strength / 7.5
                };
                self.was_force_snow = false;
            }
            if !self.tick_is_hour_change {
                self.can_do_winter_sprites = self.season.is_season(5) || self.winter_is_coming;
                let precip = self.climate_floats[FLOAT_PRECIPITATION_INTENSITY].final_value;
                if self.climate_booleans[BOOL_IS_SNOW].final_value && precip > self.snow_fall {
                    self.snow_fall = precip;
                }
                let temp = self.climate_floats[FLOAT_TEMPERATURE].final_value;
                if temp > 0.0 {
                    let mut melt = temp / 10.0;
                    melt = melt * 0.2
                        + melt * 0.8 * self.climate_floats[FLOAT_DAYLIGHT_STRENGTH].final_value;
                    if melt > self.snow_melt_strength {
                        self.snow_melt_strength = melt;
                    }
                }
                if !self.climate_booleans[BOOL_IS_SNOW].final_value && precip > 0.0 {
                    self.snow_melt_strength += precip;
                }
            } else {
                self.snow_strength += self.snow_fall;
                self.snow_strength -= self.snow_melt_strength;
                self.snow_strength = clamp(0.0, 10.0, self.snow_strength);
                self.snow_frac_now = if self.snow_strength > 7.5 {
                    1.0
                } else {
                    self.snow_strength / 7.5
                };
                self.snow_fall = 0.0;
                self.snow_melt_strength = 0.0;
            }
        }
    }

    /// zombie.iso.weather.ClimateManager.resetOverrides
    pub fn reset_overrides(&mut self) {
        for c in self.climate_colors.iter_mut() {
            c.set_enable_override(false);
        }
        for f in self.climate_floats.iter_mut() {
            f.set_enable_override(false);
        }
        for b in self.climate_booleans.iter_mut() {
            b.set_enable_override(false);
        }
    }

    /// zombie.iso.weather.ClimateManager.updateValues: the climate values of now into the internal values; on the
    /// hour a change of air mass side starts a weather period (front strength from the last days); the day's fog;
    /// the season colours of dawn, day and dusk blended into the global light by the time of day, fogged and
    /// darkened by night (the night colour between no moon and moon by the phase). The players' grid-stack
    /// refresh is not modelled.
    fn update_values<R: JavaRandom>(
        &mut self,
        gt: &GameTime,
        env: &ClimateEnv,
        rand: &mut RandAbstract<R>,
    ) {
        let venv = self.values_env(env);
        if let (Some(cur), Some(next)) = (self.current_day.as_ref(), self.next_day.as_ref()) {
            self.climate_values.update_values(
                self.world_age_hours,
                gt.time_of_day,
                cur,
                next,
                &venv,
            );
        }
        self.air_mass = self.climate_values.get_noise_airmass();
        self.air_mass_temperature = self.climate_values.get_air_mass_temperature();
        if self.tick_is_hour_change {
            let air_type = if self.air_mass < 0.0 { -1 } else { 1 };
            if self.current_front.type_ != air_type {
                if !self.disable_weather_generation
                    && (!self.winter_is_coming || gt.get_world_age_hours() > 96.0)
                {
                    let mut front = self.current_front;
                    if self.the_descending_fog {
                        front.type_ = -1;
                        front.strength = rand.next_float(0.2, 0.45);
                    } else {
                        self.calculate_weather_front_strength(
                            gt.year,
                            gt.month,
                            gt.get_day_plus_one(),
                            &mut front,
                            gt,
                            env,
                        );
                    }
                    self.current_front = front;
                    let mut wp = std::mem::take(&mut self.weather_period);
                    {
                        let ctx = PeriodCtx {
                            offsets: self.offsets(),
                            season: &self.season,
                            current_season: self.get_season(),
                            values_env: self.values_env(env),
                            winter_is_coming: self.winter_is_coming,
                        };
                        wp.init(
                            &front,
                            self.world_age_hours,
                            gt.year,
                            gt.month,
                            gt.get_day_plus_one(),
                            &ctx,
                            gt,
                            env,
                        );
                    }
                    self.weather_period = wp;
                }
                self.current_front.set_front_type(air_type);
            }
            let age = gt.get_world_age_hours();
            if !self.winter_is_coming
                && !self.the_descending_fog
                && (72.0..=96.0).contains(&age)
                && !self.disable_weather_generation
                && !self.weather_period.is_running
            {
                // The original draws and ignores the result: `if (... && Rand.Next(0, 1000) < 50) {}`.
                let _ = rand.next_range(0, 1000);
            }
        }
        self.day_do_fog = self.climate_values.is_day_do_fog();
        self.day_fog_strength = self.climate_values.get_day_fog_strength();
        if env.fog_quality == 2 {
            self.day_fog_strength = 0.5 + 0.5 * self.day_fog_strength;
        } else {
            self.day_fog_strength = 0.2 + 0.8 * self.day_fog_strength;
        }
        let cv = &self.climate_values;
        self.base_temperature = cv.get_base_temperature();
        self.day_light_lagged = cv.get_day_light_lagged();
        self.night_lagged = cv.get_day_light_lagged();
        let fl = &mut self.climate_floats;
        fl[FLOAT_TEMPERATURE].internal_value = cv.get_temperature();
        self.climate_booleans[BOOL_IS_SNOW].internal_value = cv.is_temperature_is_snow();
        fl[FLOAT_HUMIDITY].internal_value = cv.get_humidity();
        fl[FLOAT_WIND_INTENSITY].internal_value = cv.get_wind_intensity();
        fl[FLOAT_WIND_ANGLE_INTENSITY].internal_value = cv.get_wind_angle_intensity();
        self.wind_power = fl[FLOAT_WIND_INTENSITY].internal_value;
        self.current_front
            .set_front_wind(cv.get_wind_angle_degrees());
        fl[FLOAT_CLOUD_INTENSITY].internal_value = cv.get_cloud_intensity();
        fl[FLOAT_PRECIPITATION_INTENSITY].internal_value = 0.0;
        fl[FLOAT_NIGHT_STRENGTH].internal_value = cv.get_night_strength();
        fl[FLOAT_DAYLIGHT_STRENGTH].internal_value = cv.get_day_light_strength();
        fl[FLOAT_AMBIENT].internal_value = cv.get_ambient();
        fl[FLOAT_DESATURATION].internal_value = cv.get_desaturation();
        let cur_season = self.season.get_season();
        let season_prog = self.season.get_season_progression();
        let mut tval = 0.0f32;
        let mut lerp_from_season = 0usize;
        let mut lerp_to_season = 0usize;
        if cur_season == 2 {
            lerp_from_season = 3;
            lerp_to_season = 0;
            tval = 0.5 + season_prog * 0.5;
        } else if cur_season == 3 {
            lerp_from_season = 0;
            lerp_to_season = 1;
            tval = season_prog * 0.5;
        } else if cur_season == 4 {
            if season_prog < 0.5 {
                lerp_from_season = 0;
                lerp_to_season = 1;
                tval = 0.5 + season_prog;
            } else {
                lerp_from_season = 1;
                lerp_to_season = 2;
                tval = season_prog - 0.5;
            }
        } else if cur_season == 5 {
            if season_prog < 0.5 {
                lerp_from_season = 1;
                lerp_to_season = 2;
                tval = 0.5 + season_prog;
            } else {
                lerp_from_season = 2;
                lerp_to_season = 3;
                tval = season_prog - 0.5;
            }
        } else if cur_season == 1 {
            if season_prog < 0.5 {
                lerp_from_season = 2;
                lerp_to_season = 3;
                tval = 0.5 + season_prog;
            } else {
                lerp_from_season = 3;
                lerp_to_season = 0;
                tval = season_prog - 0.5;
            }
        }
        let cloudy_t = cv.get_cloudy_t();
        self.col_dawn =
            self.season_color_dawn
                .update(cloudy_t, tval, lerp_from_season, lerp_to_season);
        self.col_day =
            self.season_color_day
                .update(cloudy_t, tval, lerp_from_season, lerp_to_season);
        self.col_dusk =
            self.season_color_dusk
                .update(cloudy_t, tval, lerp_from_season, lerp_to_season);
        let cv = &self.climate_values;
        let time = cv.get_time();
        let dawn = cv.get_dawn();
        let dusk = cv.get_dusk();
        let noon = cv.get_noon();
        let fog_duration = cv.get_day_fog_duration();
        let fl = &mut self.climate_floats;
        if !self.the_descending_fog {
            if self.day_do_fog
                && self.day_fog_strength > 0.0
                && time > dawn - 2.0
                && time < dawn + fog_duration
            {
                let mut lerpfog = get_time_lerp_hours(time, dawn - 2.0, dawn + fog_duration, true);
                lerpfog = clamp(0.0, 1.0, lerpfog * (fog_duration / 3.0));
                self.fog_lerp_value = lerpfog;
                fl[FLOAT_CLOUD_INTENSITY].internal_value =
                    lerp(lerpfog, fl[FLOAT_CLOUD_INTENSITY].internal_value, 0.0);
                let fog_val = self.day_fog_strength;
                fl[FLOAT_FOG_INTENSITY].internal_value = clerp(lerpfog, 0.0, fog_val);
                let desat = fl[FLOAT_DESATURATION].internal_value;
                fl[FLOAT_DESATURATION].internal_value =
                    if !env.weather_shader || env.fog_quality == 2 {
                        clerp(lerpfog, desat, 0.8 * fog_val)
                    } else {
                        clerp(lerpfog, desat, 0.65 * fog_val)
                    };
            } else {
                fl[FLOAT_FOG_INTENSITY].internal_value = 0.0;
            }
        } else {
            let age = gt.get_world_age_hours();
            fl[FLOAT_FOG_INTENSITY].internal_value =
                if age < 72.0 { age as f32 / 72.0 } else { 1.0 };
            fl[FLOAT_CLOUD_INTENSITY].internal_value = math_min_f32(
                fl[FLOAT_CLOUD_INTENSITY].internal_value,
                1.0 - fl[FLOAT_FOG_INTENSITY].internal_value,
            );
            if self.weather_period.is_running {
                fl[FLOAT_FOG_INTENSITY].internal_value =
                    math_min_f32(fl[FLOAT_FOG_INTENSITY].internal_value, 0.6);
            }
            if env.fog_quality == 2 {
                fl[FLOAT_FOG_INTENSITY].internal_value *= 0.93;
                fl[FLOAT_DESATURATION].internal_value =
                    0.8 * fl[FLOAT_FOG_INTENSITY].internal_value;
            } else {
                fl[FLOAT_DESATURATION].internal_value =
                    0.65 * fl[FLOAT_FOG_INTENSITY].internal_value;
            }
        }
        fl[FLOAT_HUMIDITY].internal_value = clamp01(
            fl[FLOAT_HUMIDITY].internal_value + fl[FLOAT_FOG_INTENSITY].internal_value * 0.6,
        );
        let day_max = cv.get_day_light_strength_base();
        let mut dusk_dawn_min = 0.25 * cv.get_day_light_strength_base();
        if env.weather_shader {
            dusk_dawn_min = 0.8 * cv.get_day_light_strength_base();
        }
        let gl = &mut self.climate_colors[COLOR_GLOBAL_LIGHT];
        if time < dawn || time > dusk {
            let total = 24.0 - dusk + dawn;
            let t = if time > dusk {
                (time - dusk) / total
            } else {
                (24.0 - dusk + time) / total
            };
            gl.internal_value = self.col_dusk.interp(&self.col_dawn, t);
            fl[FLOAT_GLOBAL_LIGHT_INTENSITY].internal_value =
                lerp(cv.get_lerp_night(), dusk_dawn_min, 0.4);
        } else if time < noon + 2.0 {
            let t = (time - dawn) / (noon + 2.0 - dawn);
            gl.internal_value = self.col_dawn.interp(&self.col_day, t);
            fl[FLOAT_GLOBAL_LIGHT_INTENSITY].internal_value = lerp(t, dusk_dawn_min, day_max);
        } else {
            let t = (time - (noon + 2.0)) / (dusk - (noon + 2.0));
            gl.internal_value = self.col_day.interp(&self.col_dusk, t);
            fl[FLOAT_GLOBAL_LIGHT_INTENSITY].internal_value = lerp(t, day_max, dusk_dawn_min);
        }
        let fog = fl[FLOAT_FOG_INTENSITY].internal_value;
        if fog > 0.0 {
            let target = if !env.weather_shader {
                &self.col_fog_legacy
            } else if env.fog_quality == 2 {
                &self.col_fog
            } else {
                &self.col_fog_new
            };
            gl.internal_value = gl.internal_value.interp(target, fog);
            fl[FLOAT_GLOBAL_LIGHT_INTENSITY].internal_value = clerp(
                self.fog_lerp_value,
                fl[FLOAT_GLOBAL_LIGHT_INTENSITY].internal_value,
                0.8,
            );
        }
        self.col_night = self
            .col_night_no_moon
            .interp(&self.col_night_moon, self.moon.get_moon_float());
        gl.internal_value = gl
            .internal_value
            .interp(&self.col_night, fl[FLOAT_NIGHT_STRENGTH].internal_value);
    }

    /// zombie.iso.weather.ClimateManager.updateViewDistance: the game time's view distance range from daylight
    /// and fog, and the view distance value (set as its final value too).
    fn update_view_distance(&mut self, gt: &mut GameTime) {
        let view_mod = self.climate_floats[FLOAT_DAYLIGHT_STRENGTH].final_value;
        let fog_mod = self.climate_floats[FLOAT_FOG_INTENSITY].final_value;
        let mut min = 19.0 - fog_mod * 8.0;
        let mut max = min + 4.0 + 7.0 * view_mod * (1.0 - fog_mod);
        min *= 3.0;
        max *= 3.0;
        gt.view_dist_min = min;
        gt.view_dist_max = max;
        let vd = &mut self.climate_floats[FLOAT_VIEW_DISTANCE];
        vd.internal_value = min + (max - min) * view_mod;
        vd.final_value = vd.internal_value;
    }

    /// zombie.iso.weather.ClimateManager.setSeasonColorDawn
    #[allow(clippy::too_many_arguments)]
    pub fn set_season_color_dawn(
        &mut self,
        temperature: usize,
        season: usize,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
        exterior: bool,
    ) {
        Self::set_season_color(
            &mut self.season_color_dawn,
            temperature,
            season,
            [r, g, b, a],
            exterior,
        );
    }

    /// zombie.iso.weather.ClimateManager.setSeasonColorDay
    #[allow(clippy::too_many_arguments)]
    pub fn set_season_color_day(
        &mut self,
        temperature: usize,
        season: usize,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
        exterior: bool,
    ) {
        Self::set_season_color(
            &mut self.season_color_day,
            temperature,
            season,
            [r, g, b, a],
            exterior,
        );
    }

    /// zombie.iso.weather.ClimateManager.setSeasonColorDusk
    #[allow(clippy::too_many_arguments)]
    pub fn set_season_color_dusk(
        &mut self,
        temperature: usize,
        season: usize,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
        exterior: bool,
    ) {
        Self::set_season_color(
            &mut self.season_color_dusk,
            temperature,
            season,
            [r, g, b, a],
            exterior,
        );
    }

    fn set_season_color(
        s: &mut SeasonColor,
        temperature: usize,
        season: usize,
        c: [f32; 4],
        exterior: bool,
    ) {
        if exterior {
            s.set_color_exterior(temperature, season, c[0], c[1], c[2], c[3]);
        } else {
            s.set_color_interior(temperature, season, c[0], c[1], c[2], c[3]);
        }
    }

    /// zombie.iso.weather.ClimateManager.getSeasonColor: segment 0 dawn, 1 day, 2 dusk.
    pub fn get_season_color(
        &self,
        segment: i32,
        temperature: usize,
        season: usize,
    ) -> Option<&ClimateColorInfo> {
        let s = match segment {
            0 => &self.season_color_dawn,
            1 => &self.season_color_day,
            2 => &self.season_color_dusk,
            _ => return None,
        };
        Some(s.get_color(temperature, season))
    }

    /// zombie.iso.weather.ClimateManager.updateDayInfo: on a new date (day 1-based), the current, previous and
    /// next day infos; a day change is only flagged once there was a current day.
    fn update_day_info(&mut self, day: i32, month: i32, year: i32) {
        self.tick_is_day_change = false;
        let changed = match &self.current_day {
            None => true,
            Some(c) => c.day != day || c.month != month || c.year != year,
        };
        if changed {
            self.tick_is_day_change = self.current_day.is_some();
            let season = &self.season;
            let cur = self.current_day.get_or_insert_with(DayInfo::default);
            set_day_info(cur, day, month, year, 0, season);
            let prev = self.previous_day.get_or_insert_with(|| DayInfo {
                season: Some(season.clone_season()),
                ..DayInfo::default()
            });
            set_day_info(prev, day, month, year, -1, season);
            let next = self.next_day.get_or_insert_with(|| DayInfo {
                season: Some(season.clone_season()),
                ..DayInfo::default()
            });
            set_day_info(next, day, month, year, 1, season);
        }
    }

    /// zombie.iso.weather.ClimateManager.getClimateValuesCopy
    pub fn get_climate_values_copy(&self) -> ClimateValues {
        self.climate_values.get_copy(self.offsets())
    }

    /// zombie.iso.weather.ClimateManager.CalculateWeatherFrontStrength: from three days before the date, four
    /// daily samples (a separate copy of the climate values, made on first use) of the air mass at midday; the
    /// samples on the front's side feed the front.
    pub fn calculate_weather_front_strength(
        &mut self,
        year: i32,
        month: i32,
        day: i32,
        front: &mut AirFront,
        gt: &GameTime,
        env: &ClimateEnv,
    ) {
        let mut calendar = GregorianCalendar::new(year, month, day, 0, 0);
        calendar.add_days(-3);
        if self.climate_values_fronts.is_none() {
            self.climate_values_fronts = Some(self.get_climate_values_copy());
        }
        let venv = self.values_env(env);
        let target_type = front.type_;
        if let Some(cvf) = self.climate_values_fronts.as_mut() {
            for _ in 0..4 {
                cvf.poll_date(&calendar, gt, &venv, &self.season);
                let airmass = cvf.get_air_front_airmass();
                let type_ = if airmass < 0.0 { -1 } else { 1 };
                if type_ == target_type {
                    front.add_day_sample(airmass);
                }
                calendar.add_days(1);
            }
        }
    }

    /// zombie.iso.weather.ClimateManager.load(DataInputStream, int): when the first byte is 1, the simplex offsets,
    /// the current air front, snow, winter sprites and fog; then the weather period and the thunder storm; on a
    /// server the admin overrides (a single-player or client save has none); the mod data table when its byte is 1
    /// (loaded into the existing table without wiping it); on a server the humidity override; then new climate
    /// values (with the loaded offsets).
    pub fn load(
        &mut self,
        input: &mut ByteBuffer,
        world_version: i32,
        side: MetaSide,
        log: &mut Vec<String>,
    ) -> Result<(), String> {
        let server = side == MetaSide::Server;
        if input.get()? == 1 {
            self.simplex_offset_a = input.get_double()?;
            self.simplex_offset_b = input.get_double()?;
            self.simplex_offset_c = input.get_double()?;
            self.simplex_offset_d = input.get_double()?;
            self.current_front.load(input)?;
            self.snow_frac_now = input.get_float()?;
            self.snow_strength = input.get_float()?;
            self.can_do_winter_sprites = input.get()? != 0;
            self.day_do_fog = input.get()? != 0;
            self.day_fog_strength = input.get_float()?;
        }
        self.weather_period.load(input, world_version, log)?;
        self.thunder_storm.load(input)?;
        if server {
            for i in [
                FLOAT_DESATURATION,
                FLOAT_GLOBAL_LIGHT_INTENSITY,
                FLOAT_NIGHT_STRENGTH,
                FLOAT_PRECIPITATION_INTENSITY,
                FLOAT_TEMPERATURE,
                FLOAT_FOG_INTENSITY,
                FLOAT_WIND_INTENSITY,
                FLOAT_WIND_ANGLE_INTENSITY,
                FLOAT_CLOUD_INTENSITY,
                FLOAT_AMBIENT,
                FLOAT_VIEW_DISTANCE,
                FLOAT_DAYLIGHT_STRENGTH,
            ] {
                self.climate_floats[i].load_admin(input, world_version)?;
            }
            self.climate_colors[COLOR_GLOBAL_LIGHT].load_admin(input, world_version)?;
            self.climate_booleans[BOOL_IS_SNOW].load_admin(input, world_version)?;
        }
        if input.get()? == 1 {
            self.mod_data_table
                .get_or_insert_with(KahluaTable::default)
                .load_stream(input, world_version)?;
        }
        if server {
            self.climate_floats[FLOAT_HUMIDITY].load_admin(input, world_version)?;
        }
        self.climate_values = ClimateValues::new(self.offsets());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A saved noise cache of three values keeps the first two; one of a single value leaves the second at -1.
    #[test]
    fn air_front_noise_cache_keeps_two_values() {
        let front = |cache: &[f32]| {
            let mut d = Vec::new();
            for v in [1.0f32, 2.0, 3.0] {
                d.extend(v.to_be_bytes());
            }
            d.extend(1i32.to_be_bytes());
            for v in [4.0f32, 5.0] {
                d.extend(v.to_be_bytes());
            }
            d.extend((cache.len() as i32).to_be_bytes());
            for v in cache {
                d.extend(v.to_be_bytes());
            }
            d.push(9);
            let mut f = AirFront {
                noise_cache: [7.0, 7.0],
                ..AirFront::default()
            };
            let mut b = ByteBuffer::wrap(&d);
            f.load(&mut b).unwrap();
            (f, b.get().unwrap())
        };
        let (f, next) = front(&[0.5, 0.25, 0.125]);
        assert_eq!(
            (f.noise_cache, f.type_, f.front_wind_angle_degrees, next),
            ([0.5, 0.25], 1, 5.0, 9)
        );
        assert_eq!(front(&[0.5]).0.noise_cache, [0.5, -1.0]);
    }
}
