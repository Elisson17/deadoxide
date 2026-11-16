//! zombie.iso.weather.ClimateValues: the climate of a moment from noise over the world age and the day's season
//! data (air mass, temperature, humidity, wind, daylight, clouds, the day's fog).
use super::climate_manager::{self, ClimateManager, DayInfo};
use super::simplex_noise;
use crate::java::Random;
use crate::java_calendar::GregorianCalendar;
use crate::zombie::erosion::season::erosion_season::ErosionSeason;
use crate::zombie::game_time::GameTime;

/// What ClimateValues reads from the climate manager and the global options while it updates: the manager's
/// live simplex offsets C and D (the day's fog seed), the sandbox Rain (`getAirMassNoiseFrequencyMod`) and
/// DayNightCycle (`GameTime.isEndlessDay/Night`) options, and the static `ClimateManager.winterIsComing`.
#[derive(Clone, Copy, Debug)]
pub struct ValuesEnv {
    pub simplex_offset_c: f64,
    pub simplex_offset_d: f64,
    pub rain_modifier: i32,
    pub day_night_cycle: i32,
    pub winter_is_coming: bool,
}

/// zombie.iso.weather.ClimateValues
#[derive(Clone, Debug, PartialEq)]
pub struct ClimateValues {
    simplex_offset_a: f64,
    simplex_offset_b: f64,
    simplex_offset_c: f64,
    simplex_offset_d: f64,
    time: f32,
    dawn: f32,
    dusk: f32,
    noon: f32,
    day_mean_temperature: f32,
    air_mass_noise_frequency_mod: f64,
    noise_airmass: f32,
    air_mass_temperature: f32,
    base_temperature: f32,
    day_light_lagged: f32,
    night_lagged: f32,
    temperature: f32,
    temperature_is_snow: bool,
    humidity: f32,
    wind_intensity: f32,
    wind_angle_intensity: f32,
    wind_angle_degrees: f32,
    night_strength: f32,
    day_light_strength: f32,
    ambient: f32,
    desaturation: f32,
    day_light_strength_base: f32,
    lerp_night: f32,
    cloudy_t: f32,
    cloud_intensity: f32,
    air_front_airmass: f32,
    day_do_fog: bool,
    day_fog_strength: f32,
    day_fog_duration: f32,
    test_current_day: Option<DayInfo>,
    test_next_day: Option<DayInfo>,
    cache_world_age_hours: f64,
    cache_year: i32,
    cache_month: i32,
    cache_day: i32,
    seeded_random: Random,
}

impl ClimateValues {
    /// zombie.iso.weather.ClimateValues(ClimateManager): the manager's simplex offsets, a `Random(1984)`.
    pub fn new(offsets: [f64; 4]) -> Self {
        ClimateValues {
            simplex_offset_a: offsets[0],
            simplex_offset_b: offsets[1],
            simplex_offset_c: offsets[2],
            simplex_offset_d: offsets[3],
            time: 0.0,
            dawn: 0.0,
            dusk: 0.0,
            noon: 0.0,
            day_mean_temperature: 0.0,
            air_mass_noise_frequency_mod: 0.0,
            noise_airmass: 0.0,
            air_mass_temperature: 0.0,
            base_temperature: 0.0,
            day_light_lagged: 0.0,
            night_lagged: 0.0,
            temperature: 0.0,
            temperature_is_snow: false,
            humidity: 0.0,
            wind_intensity: 0.0,
            wind_angle_intensity: 0.0,
            wind_angle_degrees: 0.0,
            night_strength: 0.0,
            day_light_strength: 0.0,
            ambient: 0.0,
            desaturation: 0.0,
            day_light_strength_base: 0.0,
            lerp_night: 0.0,
            cloudy_t: 0.0,
            cloud_intensity: 0.0,
            air_front_airmass: 0.0,
            day_do_fog: false,
            day_fog_strength: 0.0,
            day_fog_duration: 0.0,
            test_current_day: None,
            test_next_day: None,
            cache_world_age_hours: 0.0,
            cache_year: 0,
            cache_month: 0,
            cache_day: 0,
            seeded_random: Random::new(1984),
        }
    }

    /// zombie.iso.weather.ClimateValues.getCopy: a new instance for the manager (its offsets now) with this one's
    /// values copied; the test days and the random generator are new.
    pub fn get_copy(&self, offsets: [f64; 4]) -> ClimateValues {
        let mut copy = ClimateValues::new(offsets);
        self.copy_values(&mut copy);
        copy
    }

    /// zombie.iso.weather.ClimateValues.CopyValues: every computed value and the cached date.
    pub fn copy_values(&self, copy: &mut ClimateValues) {
        copy.time = self.time;
        copy.dawn = self.dawn;
        copy.dusk = self.dusk;
        copy.noon = self.noon;
        copy.day_mean_temperature = self.day_mean_temperature;
        copy.air_mass_noise_frequency_mod = self.air_mass_noise_frequency_mod;
        copy.noise_airmass = self.noise_airmass;
        copy.air_mass_temperature = self.air_mass_temperature;
        copy.base_temperature = self.base_temperature;
        copy.day_light_lagged = self.day_light_lagged;
        copy.night_lagged = self.night_lagged;
        copy.temperature = self.temperature;
        copy.temperature_is_snow = self.temperature_is_snow;
        copy.humidity = self.humidity;
        copy.wind_intensity = self.wind_intensity;
        copy.wind_angle_intensity = self.wind_angle_intensity;
        copy.wind_angle_degrees = self.wind_angle_degrees;
        copy.night_strength = self.night_strength;
        copy.day_light_strength = self.day_light_strength;
        copy.ambient = self.ambient;
        copy.desaturation = self.desaturation;
        copy.day_light_strength_base = self.day_light_strength_base;
        copy.lerp_night = self.lerp_night;
        copy.cloudy_t = self.cloudy_t;
        copy.cloud_intensity = self.cloud_intensity;
        copy.air_front_airmass = self.air_front_airmass;
        copy.day_do_fog = self.day_do_fog;
        copy.day_fog_strength = self.day_fog_strength;
        copy.day_fog_duration = self.day_fog_duration;
        copy.cache_world_age_hours = self.cache_world_age_hours;
        copy.cache_year = self.cache_year;
        copy.cache_month = self.cache_month;
        copy.cache_day = self.cache_day;
    }

    /// zombie.iso.weather.ClimateValues.pollDate(int year, int month, int dayOfMonth): midnight of that date.
    pub fn poll_date_ymd(
        &mut self,
        year: i32,
        month: i32,
        day_of_month: i32,
        gt: &GameTime,
        env: &ValuesEnv,
        season: &ErosionSeason,
    ) {
        self.poll_date(
            &GregorianCalendar::new(year, month, day_of_month, 0, 0),
            gt,
            env,
            season,
        );
    }

    /// zombie.iso.weather.ClimateValues.pollDate(GregorianCalendar): the values at that date and hour, with the
    /// world age moved by the whole minutes between now (the game clock's date, hour and minute) and then
    /// (`ChronoUnit.MINUTES.between`, in UTC). `season` is the manager's, which new test days clone.
    pub fn poll_date(
        &mut self,
        calendar: &GregorianCalendar,
        gt: &GameTime,
        env: &ValuesEnv,
        season: &ErosionSeason,
    ) {
        let f = calendar.fields();
        let mut current = self.test_current_day.take().unwrap_or_default();
        let mut next = self.test_next_day.take().unwrap_or_default();
        let current_world_age_hours = gt.get_world_age_hours();
        climate_manager::set_day_info(&mut current, f.day_of_month, f.month, f.year, 0, season);
        climate_manager::set_day_info(&mut next, f.day_of_month, f.month, f.year, 1, season);
        let now = GregorianCalendar::new(
            gt.year,
            gt.month,
            gt.get_day_plus_one(),
            gt.get_hour(),
            gt.get_minutes(),
        );
        let mut difference = ((calendar.time_in_millis() - now.time_in_millis()) / 60_000) as f64;
        difference /= 60.0;
        let poll_world_age_hours = current_world_age_hours + difference;
        let poll_time = f.hour_of_day as f32 + f.minute as f32 / 60.0;
        self.update_values(poll_world_age_hours, poll_time, &current, &next, env);
        self.test_current_day = Some(current);
        self.test_next_day = Some(next);
    }

    /// zombie.iso.weather.ClimateValues.updateValues: on a new date the day's fog from a generator seeded by the
    /// date and the manager's offsets C/D; then the noise samples and everything derived from them.
    pub fn update_values(
        &mut self,
        world_age_hours: f64,
        time: f32,
        current_day: &DayInfo,
        next_day: &DayInfo,
        env: &ValuesEnv,
    ) {
        let cur_season = current_day.season();
        let next_season = next_day.season();
        if current_day.year != self.cache_year
            || current_day.month != self.cache_month
            || current_day.day != self.cache_day
        {
            let base_seed_a = env.simplex_offset_c as i32;
            let base_seed_b = env.simplex_offset_d as i32;
            let mut seed = i64::from((current_day.year - 1990).wrapping_mul(100_000));
            seed += i64::from(
                current_day
                    .month
                    .wrapping_mul(current_day.day)
                    .wrapping_mul(1234),
            );
            seed += i64::from(
                (current_day.year - 1990)
                    .wrapping_mul(current_day.month)
                    .wrapping_mul(10_000),
            );
            seed += i64::from(
                base_seed_b
                    .wrapping_sub(base_seed_a)
                    .wrapping_mul(current_day.day),
            );
            self.seeded_random.set_seed(seed);
            self.day_fog_strength = 0.0;
            self.day_do_fog = false;
            self.day_fog_duration = 0.0;
            let r = self.seeded_random.next_int_bound(1000) as f32;
            self.day_do_fog = r < 200.0;
            if self.day_do_fog {
                self.day_fog_duration = 4.0;
                if r < 25.0 {
                    self.day_fog_strength = 1.0;
                    self.day_fog_duration += 2.0;
                } else {
                    self.day_fog_strength = self.seeded_random.next_float();
                }
                let mut mean_temp = cur_season.get_day_mean_temperature();
                let airmass_sample = simplex_noise::noise(
                    self.simplex_offset_a,
                    (world_age_hours + 12.0 - 48.0)
                        / ClimateManager::get_air_mass_noise_frequency_mod(env.rain_modifier),
                ) as f32;
                mean_temp += airmass_sample * 8.0;
                let randy = self.seeded_random.next_float();
                if mean_temp < 0.0 {
                    self.day_fog_duration += 5.0 * self.day_fog_strength;
                    self.day_fog_duration += 8.0 * randy;
                } else if mean_temp < 10.0 {
                    self.day_fog_duration += 2.5 * self.day_fog_strength;
                    self.day_fog_duration += 5.0 * randy;
                } else if mean_temp < 20.0 {
                    self.day_fog_duration += 1.5 * self.day_fog_strength;
                    self.day_fog_duration += 2.5 * randy;
                } else {
                    self.day_fog_duration += 1.0 * self.day_fog_strength;
                    self.day_fog_duration += 1.0 * randy;
                }
                if self.day_fog_duration > 24.0 - cur_season.get_dawn() {
                    self.day_fog_duration = 24.0 - cur_season.get_dawn() - 1.0;
                }
            }
        }
        self.cache_world_age_hours = world_age_hours;
        self.cache_year = current_day.year;
        self.cache_month = current_day.month;
        self.cache_day = current_day.day;
        self.time = time;
        self.dawn = cur_season.get_dawn();
        self.dusk = cur_season.get_dusk();
        self.noon = cur_season.get_day_high_noon();
        self.day_mean_temperature = cur_season.get_day_mean_temperature();
        let day_lerp_val = time / 24.0;
        let day_perc_time_now = climate_manager::lerp(
            day_lerp_val,
            cur_season.get_cur_day_percent(),
            next_season.get_cur_day_percent(),
        );
        self.air_mass_noise_frequency_mod =
            ClimateManager::get_air_mass_noise_frequency_mod(env.rain_modifier);
        self.noise_airmass = simplex_noise::noise(
            self.simplex_offset_a,
            world_age_hours / self.air_mass_noise_frequency_mod,
        ) as f32;
        let noise_humidity = simplex_noise::noise(
            self.simplex_offset_c,
            world_age_hours / self.air_mass_noise_frequency_mod,
        ) as f32;
        self.air_mass_temperature = simplex_noise::noise(
            self.simplex_offset_a,
            (world_age_hours - 48.0) / self.air_mass_noise_frequency_mod,
        ) as f32;
        let daymidpoint = world_age_hours.floor() + 12.0;
        self.air_front_airmass = simplex_noise::noise(
            self.simplex_offset_a,
            daymidpoint / self.air_mass_noise_frequency_mod,
        ) as f32;
        let cur_temperature = climate_manager::clerp(
            day_lerp_val,
            cur_season.get_day_temperature(),
            next_season.get_day_temperature(),
        );
        let cur_mean_temperature = climate_manager::clerp(
            day_lerp_val,
            cur_season.get_day_mean_temperature(),
            next_season.get_day_mean_temperature(),
        );
        let _below_mean = cur_temperature < cur_mean_temperature;
        self.base_temperature = cur_mean_temperature + self.air_mass_temperature * 8.0;
        let mut ddusk = self.dusk + 4.0;
        if ddusk >= 24.0 {
            ddusk -= 24.0;
        }
        self.day_light_lagged =
            climate_manager::get_time_lerp_hours(time, self.dawn + 4.0, ddusk, true);
        let mut temp_mod = 5.0 * (1.0 - self.day_light_lagged);
        self.night_lagged =
            climate_manager::get_time_lerp_hours(time, ddusk, self.dawn + 4.0, true);
        temp_mod += 5.0 * self.night_lagged;
        self.temperature = self.base_temperature + 1.0 - temp_mod;
        self.temperature_is_snow = self.temperature < 0.0 || env.winter_is_coming;
        let mut cur_temp_offset = self.temperature;
        cur_temp_offset = (45.0 - cur_temp_offset) / 90.0;
        cur_temp_offset = climate_manager::clamp01(1.0 - cur_temp_offset);
        let noise_hum = (1.0 + noise_humidity) * 0.5;
        self.humidity = noise_hum * cur_temp_offset;
        let wind_mod = 1.0 - (self.air_mass_temperature + 1.0) * 0.5;
        let wind_mod2 = 1.0 - day_perc_time_now * 0.4;
        let noise_wind_base =
            simplex_noise::noise(world_age_hours / 40.0, self.simplex_offset_a) as f32;
        let mut wind_base = (noise_wind_base + 1.0) * 0.5;
        wind_base *= wind_mod * wind_mod2;
        wind_base *= 0.65;
        self.wind_intensity = wind_base;
        self.wind_angle_intensity =
            simplex_noise::noise(world_age_hours / 80.0, self.simplex_offset_b) as f32;
        let mut noise_wind_angle_base =
            simplex_noise::noise(world_age_hours / 40.0, self.simplex_offset_d) as f32;
        noise_wind_angle_base = (noise_wind_angle_base + 1.0) * 0.5;
        self.wind_angle_degrees = 360.0 * noise_wind_angle_base;
        if env.day_night_cycle == 3 {
            self.day_light_strength = 0.0;
            self.night_strength = 1.0;
        } else {
            self.lerp_night =
                climate_manager::get_time_lerp_hours(time, self.dusk, self.dawn, true);
            self.lerp_night = climate_manager::clamp(0.0, 1.0, self.lerp_night * 2.0);
            self.night_strength = self.lerp_night;
        }
        if env.day_night_cycle == 2 {
            self.day_light_strength =
                1.0 - 0.15 * day_perc_time_now - 0.2 * self.wind_intensity;
            self.night_strength = 0.0;
        } else {
            self.day_light_strength_base = 1.0 - self.night_strength;
            let modd = 1.0 - 0.15 * day_perc_time_now - 0.2 * self.wind_intensity;
            self.day_light_strength_base *= modd;
            self.day_light_strength = self.day_light_strength_base;
        }
        self.ambient = self.day_light_strength;
        let desat_curr_day = (1.0 - cur_season.get_cur_day_percent()) * 0.4;
        let desat_next_day = (1.0 - next_season.get_cur_day_percent()) * 0.4;
        self.desaturation = climate_manager::lerp(day_lerp_val, desat_curr_day, desat_next_day);
        self.cloudy_t =
            1.0 - climate_manager::clamp01((self.air_mass_temperature + 0.8) * 0.625);
        self.cloudy_t *= 0.8;
        self.cloudy_t = climate_manager::clamp01(self.cloudy_t + self.wind_intensity);
        self.cloud_intensity = climate_manager::clamp01(self.wind_intensity * 2.0);
        self.cloud_intensity -= self.cloud_intensity * 0.5 * self.night_strength;
    }

    pub fn get_time(&self) -> f32 {
        self.time
    }
    pub fn get_dawn(&self) -> f32 {
        self.dawn
    }
    pub fn get_dusk(&self) -> f32 {
        self.dusk
    }
    pub fn get_noon(&self) -> f32 {
        self.noon
    }
    pub fn get_air_mass_noise_frequency_mod(&self) -> f64 {
        self.air_mass_noise_frequency_mod
    }
    pub fn get_noise_airmass(&self) -> f32 {
        self.noise_airmass
    }
    pub fn get_air_mass_temperature(&self) -> f32 {
        self.air_mass_temperature
    }
    pub fn get_base_temperature(&self) -> f32 {
        self.base_temperature
    }
    pub fn get_day_light_lagged(&self) -> f32 {
        self.day_light_lagged
    }
    pub fn get_night_lagged(&self) -> f32 {
        self.night_lagged
    }
    pub fn get_temperature(&self) -> f32 {
        self.temperature
    }
    pub fn is_temperature_is_snow(&self) -> bool {
        self.temperature_is_snow
    }
    pub fn get_humidity(&self) -> f32 {
        self.humidity
    }
    pub fn get_wind_intensity(&self) -> f32 {
        self.wind_intensity
    }
    pub fn get_wind_angle_intensity(&self) -> f32 {
        self.wind_angle_intensity
    }
    pub fn get_wind_angle_degrees(&self) -> f32 {
        self.wind_angle_degrees
    }
    pub fn get_night_strength(&self) -> f32 {
        self.night_strength
    }
    pub fn get_day_light_strength(&self) -> f32 {
        self.day_light_strength
    }
    pub fn get_ambient(&self) -> f32 {
        self.ambient
    }
    pub fn get_desaturation(&self) -> f32 {
        self.desaturation
    }
    pub fn get_day_light_strength_base(&self) -> f32 {
        self.day_light_strength_base
    }
    pub fn get_lerp_night(&self) -> f32 {
        self.lerp_night
    }
    pub fn get_cloudy_t(&self) -> f32 {
        self.cloudy_t
    }
    pub fn get_cloud_intensity(&self) -> f32 {
        self.cloud_intensity
    }
    pub fn get_air_front_airmass(&self) -> f32 {
        self.air_front_airmass
    }
    pub fn get_cache_world_age_hours(&self) -> f64 {
        self.cache_world_age_hours
    }
    pub fn get_day_mean_temperature(&self) -> f32 {
        self.day_mean_temperature
    }
    pub fn is_day_do_fog(&self) -> bool {
        self.day_do_fog
    }
    pub fn get_day_fog_strength(&self) -> f32 {
        self.day_fog_strength
    }
    pub fn get_day_fog_duration(&self) -> f32 {
        self.day_fog_duration
    }
}
