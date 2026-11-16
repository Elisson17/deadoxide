//! zombie.iso.weather.ClimateForecaster: 40 day forecasts (10 past days, today and 29 coming ones), each sampled
//! hour by hour from a copy of the climate values and, where an air front passes, from a dummy weather period.
//!
//! The Java holds the forecasts as 40 objects in an array that shifts on a new day (the first object is reset and
//! becomes the last), and a forecast's overlap is a reference to another forecast object. Here the objects live in
//! an arena (`days`) and the array (`forecasts`) holds arena indices, so the same aliasing happens. The forecast
//! name (`Translator.getText("IGUI_Forecaster_Day")` and the date) is not kept as text: the date is in `name_date`
//! and the translated label is not ported (UNKNOWN there, nothing in the climate reads it).
use super::climate_manager::{AirFront, ClimateEnv, ClimateManager};
use super::climate_values::ClimateValues;
use super::weather_period::{PeriodCtx, WeatherPeriod, MAX_TEMPERATURE_INFLUENCE};
use crate::java_calendar::GregorianCalendar;
use crate::zombie::game_time::GameTime;

/// zombie.iso.weather.ClimateForecaster.OffsetToday: the index of today in the 40 forecasts.
const OFFSET_TODAY: i32 = 10;
/// The number of forecasts (`forecasts.length`).
pub const FORECAST_COUNT: usize = 40;

/// zombie.iso.weather.ClimateForecaster.ForecastValue
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ForecastValue {
    pub day_min: f32,
    pub day_max: f32,
    pub day_mean: f32,
    pub day_mean_ticks: i32,
    pub night_min: f32,
    pub night_max: f32,
    pub night_mean: f32,
    pub night_mean_ticks: i32,
    pub total_min: f32,
    pub total_max: f32,
    pub total_mean: f32,
    pub total_mean_ticks: i32,
}

impl Default for ForecastValue {
    /// zombie.iso.weather.ClimateForecaster.ForecastValue(): `reset()`.
    fn default() -> Self {
        ForecastValue {
            day_min: 10000.0,
            day_max: -10000.0,
            day_mean: 0.0,
            day_mean_ticks: 0,
            night_min: 10000.0,
            night_max: -10000.0,
            night_mean: 0.0,
            night_mean_ticks: 0,
            total_min: 10000.0,
            total_max: -10000.0,
            total_mean: 0.0,
            total_mean_ticks: 0,
        }
    }
}

impl ForecastValue {
    /// zombie.iso.weather.ClimateForecaster.ForecastValue.reset
    pub fn reset(&mut self) {
        *self = ForecastValue::default();
    }

    /// zombie.iso.weather.ClimateForecaster.ForecastValue.add: the sample into the day or night and the total
    /// minimum, maximum and running sum (a float sum, as in the original).
    pub fn add(&mut self, val: f32, is_day: bool) {
        if is_day {
            if val < self.day_min {
                self.day_min = val;
            }
            if val > self.day_max {
                self.day_max = val;
            }
            self.day_mean += val;
            self.day_mean_ticks += 1;
        } else {
            if val < self.night_min {
                self.night_min = val;
            }
            if val > self.night_max {
                self.night_max = val;
            }
            self.night_mean += val;
            self.night_mean_ticks += 1;
        }
        if val < self.total_min {
            self.total_min = val;
        }
        if val > self.total_max {
            self.total_max = val;
        }
        self.total_mean += val;
        self.total_mean_ticks += 1;
    }

    /// zombie.iso.weather.ClimateForecaster.ForecastValue.calculate: the sums into means; a part with no samples
    /// takes the total's values.
    pub fn calculate(&mut self) {
        if self.total_mean_ticks <= 0 {
            self.total_mean = 0.0;
        } else {
            self.total_mean /= self.total_mean_ticks as f32;
        }
        if self.day_mean_ticks <= 0 {
            self.day_min = self.total_min;
            self.day_max = self.total_max;
            self.day_mean = self.total_mean;
        } else {
            self.day_mean /= self.day_mean_ticks as f32;
        }
        if self.night_mean_ticks <= 0 {
            self.night_min = self.total_min;
            self.night_max = self.total_max;
            self.night_mean = self.total_mean;
        } else {
            self.night_mean /= self.night_mean_ticks as f32;
        }
    }
}

/// zombie.iso.weather.ClimateForecaster.DayForecast
#[derive(Clone, Debug, PartialEq)]
pub struct DayForecast {
    pub index_offset: i32,
    /// The year, month (0-based) and day of month the name `"<label>: y - m+1 - d"` is made from.
    pub name_date: (i32, i32, i32),
    pub weather_period: WeatherPeriod,
    pub temperature: ForecastValue,
    pub humidity: ForecastValue,
    pub wind_direction: ForecastValue,
    pub wind_power: ForecastValue,
    pub cloudiness: ForecastValue,
    pub weather_starts: bool,
    pub weather_start_time: f32,
    pub weather_end_time: f32,
    pub chance_on_snow: bool,
    pub air_front_string: &'static str,
    pub has_fog: bool,
    pub fog_strength: f32,
    pub fog_duration: f32,
    pub air_front: AirFront,
    /// Arena index of the forecast this one's weather overlaps with (`weatherOverlap`).
    pub weather_overlap: Option<usize>,
    pub has_heavy_rain: bool,
    pub has_storm: bool,
    pub has_tropical_storm: bool,
    pub has_blizzard: bool,
    pub dawn: f32,
    pub dusk: f32,
    pub day_light_hours: f32,
    pub weather_stages: Vec<i32>,
}

impl Default for DayForecast {
    /// zombie.iso.weather.ClimateForecaster.DayForecast(): no weather, the end of weather at hour 24.
    fn default() -> Self {
        DayForecast {
            index_offset: 0,
            name_date: (0, 0, 0),
            weather_period: WeatherPeriod::default(),
            temperature: ForecastValue::default(),
            humidity: ForecastValue::default(),
            wind_direction: ForecastValue::default(),
            wind_power: ForecastValue::default(),
            cloudiness: ForecastValue::default(),
            weather_starts: false,
            weather_start_time: 0.0,
            weather_end_time: 24.0,
            chance_on_snow: false,
            air_front_string: "",
            has_fog: false,
            fog_strength: 0.0,
            fog_duration: 0.0,
            air_front: AirFront::default(),
            weather_overlap: None,
            has_heavy_rain: false,
            has_storm: false,
            has_tropical_storm: false,
            has_blizzard: false,
            dawn: 0.0,
            dusk: 0.0,
            day_light_hours: 0.0,
            weather_stages: Vec::new(),
        }
    }
}

impl DayForecast {
    /// zombie.iso.weather.ClimateForecaster.DayForecast.reset: the weather period stopped (a dummy period resets
    /// nothing of the climate), the values and flags back to their start (the air front, the front string, the
    /// dawn, dusk and daylight hours are not reset).
    pub fn reset(&mut self) {
        self.weather_period.stop_weather_period();
        self.temperature.reset();
        self.humidity.reset();
        self.wind_direction.reset();
        self.wind_power.reset();
        self.cloudiness.reset();
        self.weather_starts = false;
        self.weather_start_time = 0.0;
        self.weather_end_time = 24.0;
        self.chance_on_snow = false;
        self.has_fog = false;
        self.fog_strength = 0.0;
        self.fog_duration = 0.0;
        self.weather_overlap = None;
        self.has_heavy_rain = false;
        self.has_storm = false;
        self.has_tropical_storm = false;
        self.has_blizzard = false;
        self.weather_stages.clear();
    }
}

/// zombie.iso.weather.ClimateForecaster
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClimateForecaster {
    pub climate_values: Option<ClimateValues>,
    /// The forecast objects.
    pub days: Vec<DayForecast>,
    /// `forecasts`: arena indices in day order (empty until `init`).
    pub forecasts: Vec<usize>,
}

impl ClimateForecaster {
    /// zombie.iso.weather.ClimateForecaster.getForecast(int): the forecast `offset` days from today.
    pub fn get_forecast(&self, offset: i32) -> Option<&DayForecast> {
        let target = OFFSET_TODAY + offset;
        if target >= 0 && (target as usize) < self.forecasts.len() {
            Some(&self.days[self.forecasts[target as usize]])
        } else {
            None
        }
    }

    /// The position in `forecasts` of an arena index (`ArrayList.indexOf` by identity), -1 when absent.
    pub fn index_of(&self, arena: Option<usize>) -> i32 {
        match arena {
            Some(a) => self
                .forecasts
                .iter()
                .position(|&x| x == a)
                .map_or(-1, |p| p as i32),
            None => -1,
        }
    }

    /// zombie.iso.weather.ClimateForecaster.init: a climate values copy, then the 40 forecasts, each with a dummy
    /// weather period (own climate values copy) and sampled for its offset, from -10 to 29 days from today.
    pub fn init(&mut self, cm: &mut ClimateManager, gt: &GameTime, env: &ClimateEnv) {
        self.climate_values = Some(cm.get_climate_values_copy());
        self.days.clear();
        self.forecasts.clear();
        for i in 0..FORECAST_COUNT {
            let offset = i as i32 - OFFSET_TODAY;
            let mut day_forecast = DayForecast {
                weather_period: WeatherPeriod::new(cm.get_climate_values_copy()),
                ..DayForecast::default()
            };
            day_forecast.weather_period.set_dummy(true);
            day_forecast.index_offset = offset;
            day_forecast.air_front = AirFront::default();
            self.sample_day(cm, &mut day_forecast, offset, gt, env);
            self.days.push(day_forecast);
            self.forecasts.push(i);
        }
    }

    /// zombie.iso.weather.ClimateForecaster.updateDayChange: every forecast moves one place back (its index offset
    /// one lower), the first one is reset, sampled for 29 days from today and becomes the last.
    pub fn update_day_change(&mut self, cm: &mut ClimateManager, gt: &GameTime, env: &ClimateEnv) {
        if self.forecasts.len() != FORECAST_COUNT {
            return;
        }
        let first_id = self.forecasts[0];
        for i in 1..FORECAST_COUNT {
            let id = self.forecasts[i];
            self.days[id].index_offset = i as i32 - 1 - OFFSET_TODAY;
            self.forecasts[i - 1] = id;
        }
        let mut first = std::mem::take(&mut self.days[first_id]);
        first.reset();
        let last_offset = FORECAST_COUNT as i32 - 1 - OFFSET_TODAY;
        self.sample_day(cm, &mut first, last_offset, gt, env);
        first.index_offset = last_offset;
        self.days[first_id] = first;
        self.forecasts[FORECAST_COUNT - 1] = first_id;
    }

    /// zombie.iso.weather.ClimateForecaster.sampleDay: 24 hourly polls of the climate values from midnight of the
    /// day `day_offset` from today (its dawn and dusk and its air front's side taken at hour 0). When the air
    /// mass noise changes sign and no earlier weather overlaps the hour, a front of 0.1 strength or more starts the
    /// day's weather (a dummy period initialised on the front); each hour is then sampled from the day's own or the
    /// overlapping period's stage for that time (temperature, wind and cloudiness by stage kind). The values are
    /// added to the day's and the night's statistics, and the day's fog is read at the end.
    fn sample_day(
        &mut self,
        cm: &mut ClimateManager,
        day_forecast: &mut DayForecast,
        day_offset: i32,
        gt: &GameTime,
        env: &ClimateEnv,
    ) {
        let year = gt.year;
        let month = gt.month;
        let day = gt.get_day_plus_one();
        let mut calendar = GregorianCalendar::new(year, month, day, 0, 0);
        calendar.add_days(day_offset);
        let mut last_front_warm = true;
        day_forecast.weather_overlap = self.get_weather_overlap(day_offset + OFFSET_TODAY, 0.0);
        day_forecast.weather_period.stop_weather_period();
        let f0 = calendar.fields();
        day_forecast.name_date = (f0.year, f0.month, f0.day_of_month);
        let venv = cm.values_env(env);
        let mut cv = self
            .climate_values
            .take()
            .unwrap_or_else(|| ClimateValues::new(cm.offsets()));
        for i in 0..24i32 {
            if i != 0 {
                calendar.add_hours(1);
            }
            cv.poll_date(&calendar, gt, &venv, &cm.season);
            let fields = calendar.fields();
            if i == 0 {
                last_front_warm = cv.get_noise_airmass() >= 0.0;
                day_forecast.air_front_string = if last_front_warm { "WARM" } else { "COLD" };
                day_forecast.dawn = cv.get_dawn();
                day_forecast.dusk = cv.get_dusk();
                day_forecast.day_light_hours = day_forecast.dusk - day_forecast.dawn;
            }
            if !day_forecast.weather_starts
                && (last_front_warm && cv.get_noise_airmass() < 0.0
                    || !last_front_warm && cv.get_noise_airmass() >= 0.0)
            {
                let last_type = if cv.get_noise_airmass() >= 0.0 { -1 } else { 1 };
                day_forecast.air_front.set_front_type(last_type);
                cm.calculate_weather_front_strength(
                    fields.year,
                    fields.month,
                    fields.day_of_month,
                    &mut day_forecast.air_front,
                    gt,
                    env,
                );
                day_forecast
                    .air_front
                    .set_front_wind(cv.get_wind_angle_degrees());
                if day_forecast.air_front.strength >= 0.1 {
                    let overlap = self.get_weather_overlap(day_offset + OFFSET_TODAY, i as f32);
                    let overlap_strength = match overlap {
                        Some(o) => self.days[o].weather_period.get_total_strength(),
                        None => -1.0,
                    };
                    if overlap_strength < 0.1 {
                        day_forecast.weather_starts = true;
                        day_forecast.weather_start_time = i as f32;
                        let front = day_forecast.air_front;
                        let ctx = PeriodCtx {
                            offsets: cm.offsets(),
                            season: &cm.season,
                            current_season: cm.get_season(),
                            values_env: venv,
                            winter_is_coming: cm.winter_is_coming,
                        };
                        day_forecast.weather_period.init(
                            &front,
                            cv.get_cache_world_age_hours(),
                            fields.year,
                            fields.month,
                            fields.day_of_month,
                            &ctx,
                            gt,
                            env,
                        );
                    }
                }
                if !day_forecast.weather_starts {
                    last_front_warm = !last_front_warm;
                }
            }
            let is_day_time = (i as f32) > cv.get_dawn() && (i as f32) <= cv.get_dusk();
            let mut temperature = cv.get_temperature();
            let humidity = cv.get_humidity();
            let mut wind_direction = cv.get_wind_angle_degrees();
            let mut wind_power = cv.get_wind_intensity();
            let mut cloudiness = cv.get_cloud_intensity();
            if day_forecast.weather_starts || day_forecast.weather_overlap.is_some() {
                let wp: &WeatherPeriod = if day_forecast.weather_starts {
                    &day_forecast.weather_period
                } else {
                    // weatherOverlap is non-null here (the condition above).
                    &self.days[day_forecast.weather_overlap.unwrap_or(0)].weather_period
                };
                wind_direction = wp.get_wind_angle_degrees();
                let total_strength = wp.get_total_strength();
                let stage_id = wp
                    .get_stage_for_world_age(cv.get_cache_world_age_hours())
                    .map(|s| s.stage_id);
                match stage_id {
                    Some(id) => {
                        if !day_forecast.weather_stages.contains(&id) {
                            day_forecast.weather_stages.push(id);
                        }
                        match id {
                            2 => {
                                wind_power = 0.5 * total_strength;
                                temperature -= MAX_TEMPERATURE_INFLUENCE * wind_power;
                                cloudiness = 0.5 + 0.5 * wind_power;
                                day_forecast.has_heavy_rain = true;
                            }
                            3 => {
                                wind_power = 0.2 + 0.5 * total_strength;
                                temperature -= MAX_TEMPERATURE_INFLUENCE * wind_power;
                                cloudiness = 0.5 + 0.5 * wind_power;
                                day_forecast.has_storm = true;
                            }
                            7 => {
                                day_forecast.chance_on_snow = true;
                                wind_power = 0.75 + 0.25 * total_strength;
                                temperature -= MAX_TEMPERATURE_INFLUENCE * wind_power;
                                cloudiness = 0.5 + 0.5 * wind_power;
                                day_forecast.has_blizzard = true;
                            }
                            8 => {
                                wind_power = 0.4 + 0.6 * total_strength;
                                temperature -= MAX_TEMPERATURE_INFLUENCE * wind_power;
                                cloudiness = 0.5 + 0.5 * wind_power;
                                day_forecast.has_tropical_storm = true;
                            }
                            other => {
                                // case 1 sets the heavy rain flag, then like 4, 5, 6 and the default.
                                if other == 1 {
                                    day_forecast.has_heavy_rain = true;
                                }
                                temperature -= MAX_TEMPERATURE_INFLUENCE * 0.25;
                                cloudiness = 0.35 + 0.5 * total_strength;
                            }
                        }
                    }
                    None => {
                        if day_forecast.weather_overlap.is_some()
                            && (i as f32) < day_forecast.weather_end_time
                        {
                            day_forecast.weather_end_time = i as f32;
                        }
                    }
                }
                if temperature < 0.0 {
                    day_forecast.chance_on_snow = true;
                }
            }
            day_forecast.temperature.add(temperature, is_day_time);
            day_forecast.humidity.add(humidity, is_day_time);
            day_forecast.wind_direction.add(wind_direction, is_day_time);
            day_forecast.wind_power.add(wind_power, is_day_time);
            day_forecast.cloudiness.add(cloudiness, is_day_time);
        }
        day_forecast.temperature.calculate();
        day_forecast.humidity.calculate();
        day_forecast.wind_direction.calculate();
        day_forecast.wind_power.calculate();
        day_forecast.cloudiness.calculate();
        day_forecast.has_fog = cv.is_day_do_fog();
        day_forecast.fog_strength = cv.get_day_fog_strength();
        day_forecast.fog_duration = cv.get_day_fog_duration();
        self.climate_values = Some(cv);
    }

    /// zombie.iso.weather.ClimateForecaster.getWeatherOverlap: among the forecasts of up to 10 days before `index`
    /// (oldest first) the first whose weather, started at its start hour and lasting its period's duration, still
    /// runs at day `index` and `hour`. Returns its arena index.
    fn get_weather_overlap(&self, index: i32, hour: f32) -> Option<usize> {
        let start = std::cmp::max(0, index - 10);
        if start == index {
            return None;
        }
        for i in start..index {
            let id = *self.forecasts.get(i as usize)?;
            let d = &self.days[id];
            if d.weather_starts {
                let days = d.weather_period.duration as f32 / 24.0;
                let mut end = i as f32 + d.weather_start_time / 24.0;
                end += days;
                let stamptoday = index as f32 + hour / 24.0;
                if end > stamptoday {
                    return Some(id);
                }
            }
        }
        None
    }

    /// zombie.iso.weather.ClimateForecaster.getDaysTillFirstWeather: the index of the first forecast from today on
    /// (not the last one) whose weather starts, or -1.
    pub fn get_days_till_first_weather(&self) -> i32 {
        let mut first = -1;
        for i in 10..self.forecasts.len().saturating_sub(1) {
            if self.days[self.forecasts[i]].weather_starts && first < 0 {
                first = i as i32;
            }
        }
        first
    }
}
