//! zombie.erosion.season.ErosionSeason: the season, day temperatures and daylight of a date, from the latitude,
//! the temperature range and layered noise.
use crate::java_calendar::GregorianCalendar;
use crate::java_fdlibm;
use crate::zombie::erosion::utils::noise2d::Noise2D;

pub const SEASON_DEFAULT: i32 = 0;
pub const SEASON_SPRING: i32 = 1;
pub const SEASON_SUMMER: i32 = 2;
pub const SEASON_SUMMER2: i32 = 3;
pub const SEASON_AUTUMN: i32 = 4;
pub const SEASON_WINTER: i32 = 5;

/// zombie.erosion.season.ErosionSeason.names
const NAMES: [&str; 6] = [
    "Default",
    "Spring",
    "Early Summer",
    "Late Summer",
    "Autumn",
    "Winter",
];

/// zombie.erosion.season.ErosionSeason.YearData: calendars kept as their UTC milliseconds.
#[derive(Clone, Copy, Debug, PartialEq)]
struct YearData {
    year: i32,
    win_sols: GregorianCalendar,
    sum_sols: GregorianCalendar,
    hottest_day: GregorianCalendar,
    coldest_day: GregorianCalendar,
    winter_s: f32,
    winter_e: f32,
    winter_start_day: GregorianCalendar,
    winter_end_day: GregorianCalendar,
    summer_s: f32,
    summer_e: f32,
    summer_start_day: GregorianCalendar,
    summer_end_day: GregorianCalendar,
    last_summer_str: f32,
    last_winter_str: f32,
    summer_str: f32,
    winter_str: f32,
    next_summer_str: f32,
    next_winter_str: f32,
}

impl Default for YearData {
    /// zombie.erosion.season.ErosionSeason.YearData(): year Integer.MIN_VALUE, calendars null (here the epoch,
    /// never read before setYearData fills them).
    fn default() -> Self {
        let zero = GregorianCalendar::new(1970, 0, 1, 0, 0);
        YearData {
            year: i32::MIN,
            win_sols: zero,
            sum_sols: zero,
            hottest_day: zero,
            coldest_day: zero,
            winter_s: 0.0,
            winter_e: 0.0,
            winter_start_day: zero,
            winter_end_day: zero,
            summer_s: 0.0,
            summer_e: 0.0,
            summer_start_day: zero,
            summer_end_day: zero,
            last_summer_str: 0.0,
            last_winter_str: 0.0,
            summer_str: 0.0,
            winter_str: 0.0,
            next_summer_str: 0.0,
            next_winter_str: 0.0,
        }
    }
}

/// zombie.erosion.season.ErosionSeason. The sandbox's day/night cycle (`SandboxOptions.dayNightCycle`, 2 endless
/// day, 3 endless night), which the original reads from the global options, is a field.
#[derive(Clone, Debug, PartialEq)]
pub struct ErosionSeason {
    lat: i32,
    temp_max: i32,
    temp_min: i32,
    temp_diff: i32,
    high_noon: f32,
    high_noon_current: f32,
    season_lag: i32,
    rain: [f32; 12],
    su_sol: f64,
    wi_sol: f64,
    zero_day: GregorianCalendar,
    is_h1: bool,
    year_data: [YearData; 3],
    cur_season: i32,
    cur_season_day: f32,
    cur_season_days: f32,
    cur_season_strength: f32,
    cur_season_progression: f32,
    day_mean_temperature: f32,
    day_temperature: f32,
    day_noise_val: f32,
    is_rain_day: bool,
    rain_year_average: f32,
    rain_day_strength: f32,
    is_thunder_day: bool,
    is_sunny_day: bool,
    day_dusk: f32,
    day_dawn: f32,
    day_daylight: f32,
    winter_mod: f32,
    summer_mod: f32,
    summer_tilt: f32,
    cur_day_percent: f32,
    per: Noise2D,
    seed_a: i32,
    seed_b: i32,
    seed_c: i32,
    /// SandboxOptions.dayNightCycle
    pub day_night_cycle: i32,
}

impl Default for ErosionSeason {
    /// zombie.erosion.season.ErosionSeason(): the field initialisers (not yet init'ed), day/night cycle 1.
    fn default() -> Self {
        ErosionSeason {
            lat: 38,
            temp_max: 25,
            temp_min: 0,
            temp_diff: 7,
            high_noon: 12.5,
            high_noon_current: 12.5,
            season_lag: 31,
            rain: [0.0; 12],
            su_sol: 0.0,
            wi_sol: 0.0,
            zero_day: GregorianCalendar::new(1970, 0, 1, 0, 0),
            is_h1: false,
            year_data: [YearData::default(); 3],
            cur_season: 0,
            cur_season_day: 0.0,
            cur_season_days: 0.0,
            cur_season_strength: 0.0,
            cur_season_progression: 0.0,
            day_mean_temperature: 0.0,
            day_temperature: 0.0,
            day_noise_val: 0.0,
            is_rain_day: false,
            rain_year_average: 0.0,
            rain_day_strength: 0.0,
            is_thunder_day: false,
            is_sunny_day: false,
            day_dusk: 0.0,
            day_dawn: 0.0,
            day_daylight: 0.0,
            winter_mod: 0.0,
            summer_mod: 0.0,
            summer_tilt: 0.0,
            cur_day_percent: 0.0,
            per: Noise2D::default(),
            seed_a: 64,
            seed_b: 128,
            seed_c: 255,
            day_night_cycle: 1,
        }
    }
}

impl ErosionSeason {
    /// zombie.erosion.season.ErosionSeason.init: the solstice day lengths (fdlibm acos/tan, as StrictMath; the
    /// original's Math intrinsics are taken to agree, which the climate oracle checks), three noise layers and
    /// empty year data.
    #[allow(clippy::too_many_arguments)]
    pub fn init(
        &mut self,
        lat: i32,
        temp_max: i32,
        temp_min: i32,
        temp_diff: i32,
        season_lag: i32,
        noon: f32,
        seed_a: i32,
        seed_b: i32,
        seed_c: i32,
    ) {
        self.lat = lat;
        self.temp_max = temp_max;
        self.temp_min = temp_min;
        self.temp_diff = temp_diff;
        self.season_lag = season_lag;
        self.high_noon = noon;
        self.high_noon_current = noon;
        self.seed_a = seed_a;
        self.seed_b = seed_b;
        self.seed_c = seed_c;
        self.summer_tilt = 2.0;
        self.winter_mod = if self.temp_min < 0 {
            0.05 * -(self.temp_min as f32)
        } else {
            0.02 * -(self.temp_min as f32)
        };
        self.summer_mod = if self.temp_max < 0 {
            0.05 * self.temp_max as f32
        } else {
            0.02 * self.temp_max as f32
        };
        let tan_lat = java_fdlibm::tan(Self::radian(f64::from(self.lat)));
        let tan_tilt = java_fdlibm::tan(Self::radian(23.44));
        self.su_sol = 2.0 * Self::degree(java_fdlibm::acos(-tan_lat * tan_tilt)) / 15.0;
        self.wi_sol = 2.0 * Self::degree(java_fdlibm::acos(tan_lat * tan_tilt)) / 15.0;
        self.per.reset();
        self.per.add_layer(seed_a, 8.0, 2.0);
        self.per.add_layer(seed_b, 6.0, 4.0);
        self.per.add_layer(seed_c, 4.0, 6.0);
        self.year_data = [YearData::default(); 3];
    }

    pub fn get_lat(&self) -> i32 {
        self.lat
    }
    pub fn get_temp_max(&self) -> i32 {
        self.temp_max
    }
    pub fn get_temp_min(&self) -> i32 {
        self.temp_min
    }
    pub fn get_temp_diff(&self) -> i32 {
        self.temp_diff
    }
    pub fn get_season_lag(&self) -> i32 {
        self.season_lag
    }
    pub fn get_high_noon(&self) -> f32 {
        self.high_noon
    }
    pub fn get_seed_a(&self) -> i32 {
        self.seed_a
    }
    pub fn get_seed_b(&self) -> i32 {
        self.seed_b
    }
    pub fn get_seed_c(&self) -> i32 {
        self.seed_c
    }

    /// zombie.erosion.season.ErosionSeason.setRain: the monthly rain chances and their yearly average, floored.
    #[allow(clippy::too_many_arguments)]
    pub fn set_rain(&mut self, months: [f32; 12]) {
        self.rain = months;
        let mut total = 0.0f32;
        for v in self.rain {
            total += v;
        }
        self.rain_year_average =
            f64::from(365.0f32 * (total / self.rain.len() as f32)).floor() as i32 as f32;
    }

    /// zombie.erosion.season.ErosionSeason.clone: a new season init'ed with the same settings (the noon is the
    /// configured one) and rain; no day set yet.
    pub fn clone_season(&self) -> ErosionSeason {
        let mut clone = ErosionSeason {
            day_night_cycle: self.day_night_cycle,
            ..ErosionSeason::default()
        };
        clone.init(
            self.lat,
            self.temp_max,
            self.temp_min,
            self.temp_diff,
            self.season_lag,
            self.high_noon,
            self.seed_a,
            self.seed_b,
            self.seed_c,
        );
        clone.set_rain(self.rain);
        clone
    }

    pub fn get_cur_day_percent(&self) -> f32 {
        self.cur_day_percent
    }
    pub fn get_max_daylight_winter(&self) -> f64 {
        self.wi_sol
    }
    pub fn get_max_daylight_summer(&self) -> f64 {
        self.su_sol
    }
    pub fn get_dusk(&self) -> f32 {
        self.day_dusk
    }
    pub fn get_dawn(&self) -> f32 {
        self.day_dawn
    }
    pub fn get_daylight(&self) -> f32 {
        self.day_daylight
    }
    pub fn get_day_temperature(&self) -> f32 {
        self.day_temperature
    }
    pub fn get_day_mean_temperature(&self) -> f32 {
        self.day_mean_temperature
    }
    pub fn get_season(&self) -> i32 {
        self.cur_season
    }
    pub fn get_day_high_noon(&self) -> f32 {
        self.high_noon_current
    }
    /// zombie.erosion.season.ErosionSeason.getSeasonName
    pub fn get_season_name(&self) -> &'static str {
        NAMES[self.cur_season as usize]
    }
    /// zombie.erosion.season.ErosionSeason.isSeason
    pub fn is_season(&self, season: i32) -> bool {
        season == self.cur_season
    }
    pub fn get_season_day(&self) -> f32 {
        self.cur_season_day
    }
    pub fn get_season_days(&self) -> f32 {
        self.cur_season_days
    }
    pub fn get_season_strength(&self) -> f32 {
        self.cur_season_strength
    }
    pub fn get_season_progression(&self) -> f32 {
        self.cur_season_progression
    }
    pub fn get_day_noise_val(&self) -> f32 {
        self.day_noise_val
    }
    pub fn is_rain_day(&self) -> bool {
        self.is_rain_day
    }
    pub fn get_rain_day_strength(&self) -> f32 {
        self.rain_day_strength
    }
    pub fn get_rain_year_average(&self) -> f32 {
        self.rain_year_average
    }
    pub fn is_thunder_day(&self) -> bool {
        self.is_thunder_day
    }
    pub fn is_sunny_day(&self) -> bool {
        self.is_sunny_day
    }

    /// zombie.erosion.season.ErosionSeason.setDay: the date at midnight (lenient: ErosionMain passes its 0-based
    /// day, which is the last day of the month before for 0), then the year, season and daylight data. The
    /// original's notice for year 0 is not modelled.
    pub fn set_day(&mut self, day: i32, month: i32, year: i32) {
        let day_date = GregorianCalendar::new(year, month, day, 0, 0);
        let day_value = day_date.time_in_millis();
        self.set_year_data(year);
        self.set_season_data(day_value as f32, &day_date, year, month);
        self.set_daylight_data(day_value, &day_date);
    }

    /// zombie.erosion.season.ErosionSeason.setYearData: solstices, hottest/coldest days and the noisy season
    /// bounds of the year before, this year and the next (recomputed only when the year changes).
    fn set_year_data(&mut self, year: i32) {
        if self.year_data[1].year == year {
            return;
        }
        for i in 0..3 {
            let cur_year = year + (i as i32 - 1);
            let mut yd = self.year_data[i];
            yd.year = cur_year;
            yd.win_sols = GregorianCalendar::new(cur_year, 11, 22, 0, 0);
            yd.sum_sols = GregorianCalendar::new(cur_year, 5, 22, 0, 0);
            yd.hottest_day = GregorianCalendar::new(cur_year, 5, 22, 0, 0);
            yd.coldest_day = GregorianCalendar::new(cur_year, 11, 22, 0, 0);
            yd.hottest_day.add_days(self.season_lag);
            yd.coldest_day.add_days(self.season_lag);
            yd.winter_s = self.per.layered_noise((64 + cur_year) as f32, 64.0);
            yd.winter_e = self.per.layered_noise(64.0, (64 + cur_year) as f32);
            yd.winter_start_day = GregorianCalendar::new(cur_year, 11, 22, 0, 0);
            yd.winter_end_day = GregorianCalendar::new(cur_year, 11, 22, 0, 0);
            let ws = 40.0f32 + 40.0 * self.winter_mod + 20.0 * yd.winter_s;
            yd.winter_start_day.add_days(-f64::from(ws).floor() as i32);
            let we = 40.0f32 + 40.0 * self.winter_mod + 20.0 * yd.winter_e;
            yd.winter_end_day.add_days(f64::from(we).floor() as i32);
            yd.summer_s = self.per.layered_noise((128 + cur_year) as f32, 128.0);
            yd.summer_e = self.per.layered_noise(128.0, (128 + cur_year) as f32);
            yd.summer_start_day = GregorianCalendar::new(cur_year, 5, 22, 0, 0);
            yd.summer_end_day = GregorianCalendar::new(cur_year, 5, 22, 0, 0);
            let ss = 40.0f32 + 40.0 * self.summer_mod + 20.0 * yd.summer_s;
            yd.summer_start_day.add_days(-f64::from(ss).floor() as i32);
            let se = 40.0f32 + 40.0 * self.summer_mod + 20.0 * yd.summer_e;
            yd.summer_end_day.add_days(f64::from(se).floor() as i32);
            self.year_data[i] = yd;
        }
        let (y0, y2) = (self.year_data[0], self.year_data[2]);
        let y1 = &mut self.year_data[1];
        y1.last_summer_str = y0.summer_s + y0.summer_e - 1.0;
        y1.last_winter_str = y0.winter_s + y0.winter_e - 1.0;
        y1.summer_str = y1.summer_s + y1.summer_e - 1.0;
        y1.winter_str = y1.winter_s + y1.winter_e - 1.0;
        y1.next_summer_str = y2.summer_s + y2.summer_e - 1.0;
        y1.next_winter_str = y2.winter_s + y2.winter_e - 1.0;
    }

    /// `(float)` of a calendar's milliseconds, as setSeasonData compares them.
    fn unx(c: &GregorianCalendar) -> f32 {
        c.time_in_millis() as f32
    }

    /// zombie.erosion.season.ErosionSeason.setSeasonData: which season the date is in (comparing milliseconds
    /// as floats, as the original does), its progress, the day's mean and noisy temperature, rain, thunder and sun.
    fn set_season_data(
        &mut self,
        day_value: f32,
        day_date: &GregorianCalendar,
        _year: i32,
        month: i32,
    ) {
        let [y0, y1, y2] = self.year_data;
        let (sd, ed);
        if day_value < Self::unx(&y0.winter_end_day) {
            self.cur_season = 5;
            sd = y0.winter_start_day;
            ed = y0.winter_end_day;
        } else if day_value < Self::unx(&y1.summer_start_day) {
            self.cur_season = 1;
            sd = y0.winter_end_day;
            ed = y1.summer_start_day;
        } else if day_value < Self::unx(&y1.summer_end_day) {
            self.cur_season = 2;
            sd = y1.summer_start_day;
            ed = y1.summer_end_day;
        } else if day_value < Self::unx(&y1.winter_start_day) {
            self.cur_season = 4;
            sd = y1.summer_end_day;
            ed = y1.winter_start_day;
        } else {
            self.cur_season = 5;
            sd = y1.winter_start_day;
            ed = y1.winter_end_day;
        }
        self.cur_season_day = Self::day_diff(day_date, &sd);
        self.cur_season_days = Self::day_diff(&sd, &ed);
        self.cur_season_strength = self.cur_season_days / 90.0 - 1.0;
        self.cur_season_progression = self.cur_season_day / self.cur_season_days;
        let half = (self.temp_diff / 2) as f32;
        let (temp_a, temp_b, day_perc);
        if day_value < Self::unx(&y0.coldest_day) && day_value >= Self::unx(&y0.hottest_day) {
            temp_a = self.temp_max as f32 + half * y1.last_summer_str;
            temp_b = self.temp_min as f32 + half * y1.last_winter_str;
            day_perc = Self::day_diff(day_date, &y0.hottest_day)
                / Self::day_diff(&y0.hottest_day, &y0.coldest_day);
        } else if day_value < Self::unx(&y1.hottest_day) && day_value >= Self::unx(&y0.coldest_day)
        {
            temp_a = self.temp_min as f32 + half * y1.last_winter_str;
            temp_b = self.temp_max as f32 + half * y1.summer_str;
            day_perc = Self::day_diff(day_date, &y0.coldest_day)
                / Self::day_diff(&y1.hottest_day, &y0.coldest_day);
        } else if day_value < Self::unx(&y1.coldest_day) && day_value >= Self::unx(&y1.hottest_day)
        {
            temp_a = self.temp_max as f32 + half * y1.summer_str;
            temp_b = self.temp_min as f32 + half * y1.winter_str;
            day_perc = Self::day_diff(day_date, &y1.hottest_day)
                / Self::day_diff(&y1.hottest_day, &y1.coldest_day);
        } else {
            temp_a = self.temp_min as f32 + half * y1.winter_str;
            temp_b = self.temp_max as f32 + half * y1.next_summer_str;
            day_perc = Self::day_diff(day_date, &y1.coldest_day)
                / Self::day_diff(&y1.coldest_day, &y2.hottest_day);
        }
        let day_average_temp =
            Self::clerp(f64::from(day_perc), f64::from(temp_a), f64::from(temp_b)) as f32;
        let super_offset = Self::day_diff(&self.zero_day, day_date) / 20.0;
        self.day_noise_val = self.per.layered_noise(super_offset, 0.0);
        let modifier = self.day_noise_val * 2.0 - 1.0;
        self.day_temperature = day_average_temp + self.temp_diff as f32 * modifier;
        self.day_mean_temperature = day_average_temp;
        self.is_thunder_day = false;
        self.is_rain_day = false;
        self.is_sunny_day = false;
        let r = self.rain[month as usize];
        let rain_mod = if 0.1f32 + r <= 1.0 { 0.1 + r } else { 1.0 };
        if rain_mod > 0.0 && self.day_noise_val < rain_mod {
            self.is_rain_day = true;
            self.rain_day_strength = 1.0 - self.day_noise_val / rain_mod;
            let tmp = self.per.layered_noise(0.0, super_offset);
            if f64::from(tmp) > 0.6 {
                self.is_thunder_day = true;
            }
        }
        if f64::from(self.day_noise_val) > 0.6 {
            self.is_sunny_day = true;
        }
    }

    /// zombie.erosion.season.ErosionSeason.setDaylightData: the day's length between the solstice lengths, the
    /// noon shifted by the summer tilt, dawn and dusk; endless night/day from the sandbox.
    fn set_daylight_data(&mut self, day_value: i64, day_date: &GregorianCalendar) {
        let y0 = self.year_data[0];
        let y1 = self.year_data[1];
        let y2 = self.year_data[2];
        let (cycle_start, cycle_end);
        if day_value < y1.win_sols.time_in_millis() && day_value >= y1.sum_sols.time_in_millis() {
            self.is_h1 = false;
            cycle_start = y1.sum_sols;
            cycle_end = y1.win_sols;
        } else {
            self.is_h1 = true;
            if day_value >= y1.win_sols.time_in_millis() {
                cycle_start = y1.win_sols;
                cycle_end = y2.sum_sols;
            } else {
                cycle_start = y0.win_sols;
                cycle_end = y1.sum_sols;
            }
        }
        let h_cycle_perc =
            Self::day_diff(day_date, &cycle_start) / Self::day_diff(&cycle_start, &cycle_end);
        let mut perc2 = h_cycle_perc;
        if self.is_h1 {
            self.day_daylight =
                Self::clerp(f64::from(h_cycle_perc), self.wi_sol, self.su_sol) as f32;
        } else {
            self.day_daylight =
                Self::clerp(f64::from(h_cycle_perc), self.su_sol, self.wi_sol) as f32;
            perc2 = 1.0 - h_cycle_perc;
        }
        self.cur_day_percent = perc2;
        if self.is_endless_night() {
            self.high_noon_current = 0.0;
            self.day_dawn = 0.0;
            self.day_dusk = 0.0;
            self.day_daylight = 0.0;
        } else if self.is_endless_day() {
            self.day_dawn = 0.0;
            self.day_dusk = 0.0;
            self.day_daylight = 24.0;
        } else {
            self.high_noon_current = self.high_noon + self.summer_tilt * perc2;
            self.day_dawn = self.high_noon_current - self.day_daylight / 2.0;
            self.day_dusk = self.high_noon_current + self.day_daylight / 2.0;
        }
    }

    /// zombie.erosion.season.ErosionSeason.dayDiff: whole days between two dates (long division), as a float.
    fn day_diff(date1: &GregorianCalendar, date2: &GregorianCalendar) -> f32 {
        let diff = date1.time_in_millis() - date2.time_in_millis();
        (diff / 86_400_000).wrapping_abs() as f32
    }

    /// zombie.erosion.season.ErosionSeason.clerp (double), with fdlibm cos.
    fn clerp(t: f64, a: f64, b: f64) -> f64 {
        let t2 = (1.0 - java_fdlibm::cos(t * std::f64::consts::PI)) / 2.0;
        a * (1.0 - t2) + b * t2
    }

    /// zombie.erosion.season.ErosionSeason.radian
    fn radian(a: f64) -> f64 {
        a * (std::f64::consts::PI / 180.0)
    }

    /// zombie.erosion.season.ErosionSeason.degree
    fn degree(a: f64) -> f64 {
        a * (180.0 / std::f64::consts::PI)
    }

    /// zombie.erosion.season.ErosionSeason.setCurSeason
    pub fn set_cur_season(&mut self, season: i32) {
        self.cur_season = season;
    }

    /// zombie.erosion.season.ErosionSeason.isEndlessDay
    pub fn is_endless_day(&self) -> bool {
        self.day_night_cycle == 2
    }

    /// zombie.erosion.season.ErosionSeason.isEndlessNight
    pub fn is_endless_night(&self) -> bool {
        self.day_night_cycle == 3
    }
}

/// The season ErosionMain makes (zombie.erosion.ErosionMain.init's season part): ErosionConfig.Season's defaults,
/// the minimum and maximum temperature moved by the sandbox's Temperature option (1 very cold .. 5 very hot), and
/// the monthly rain. The day is set separately (`set_day`, which ErosionMain does with its 0-based day).
pub fn erosion_main_season(temperature_modifier: i32, day_night_cycle: i32) -> ErosionSeason {
    let mut temp_min = 0i32;
    let mut temp_max = 25i32;
    match temperature_modifier {
        1 => {
            temp_min -= 10;
            temp_max -= 10;
        }
        2 => {
            temp_min -= 5;
            temp_max -= 5;
        }
        4 => {
            temp_min = (f64::from(temp_min) + 7.5) as i32;
            temp_max += 4;
        }
        5 => {
            temp_min += 15;
            temp_max += 8;
        }
        _ => {}
    }
    let mut season = ErosionSeason {
        day_night_cycle,
        ..ErosionSeason::default()
    };
    season.init(38, temp_max, temp_min, 7, 31, 12.5, 64, 128, 255);
    season.set_rain([
        0.39, 0.35, 0.39, 0.4, 0.35, 0.37, 0.29, 0.26, 0.23, 0.23, 0.3, 0.32,
    ]);
    season
}
