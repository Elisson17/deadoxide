//! zombie.GameTime: the world clock and calendar, and `map_t.bin`, the save file that holds them together with the
//! climate (`IsoWorld.init` loads it before the meta grid).
use crate::java_nio::{read_string_stream, ByteBuffer};
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTable;
use crate::zombie::iso::map_meta::MetaSide;
use crate::zombie::iso::weather::climate_manager::ClimateManager;

/// The two names `GameTime.load` stores into `zombie.core.Core` (which is not ported): the poisonous berry and
/// mushroom item types of this world.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CorePoisonous {
    pub poisonous_berry: Option<String>,
    pub poisonous_mushroom: Option<String>,
}

/// zombie.GameTime: the fields a save stores and the minute stamp derived from them.
#[derive(Clone, Debug, PartialEq)]
pub struct GameTime {
    pub time_of_day: f32,
    pub nights_survived: i32,
    pub multiplier: f32,
    pub last_time_of_day: f32,
    pub target_zombies: i32,
    pub day: i32,
    pub month: i32,
    pub year: i32,
    pub helicopter_day1: i32,
    pub helicopter_time1_start: i32,
    pub helicopter_time1_end: i32,
    /// zombie.GameTime.table (`getModData`): null until made.
    pub table: Option<KahluaTable>,
    pub minutes_stamp: i64,
    /// The clock's speed: real minutes per game day (60 for a one-hour day; the field initialiser is 30).
    pub minutes_per_day: f32,
    pub fps_multiplier: f32,
    pub multiplier_bias: f32,
    pub per_object_multiplier: f32,
    pub view_dist_min: f32,
    pub view_dist_max: f32,
}

impl Default for GameTime {
    /// zombie.GameTime(): the field initialisers.
    fn default() -> Self {
        GameTime {
            time_of_day: 9.0,
            nights_survived: 0,
            multiplier: 1.0,
            last_time_of_day: 0.0,
            target_zombies: 750,
            day: 22,
            month: 7,
            year: 2012,
            helicopter_day1: 0,
            helicopter_time1_start: 0,
            helicopter_time1_end: 0,
            table: None,
            minutes_stamp: 0,
            minutes_per_day: 30.0,
            fps_multiplier: 1.0,
            multiplier_bias: 1.0,
            per_object_multiplier: 1.0,
            view_dist_min: 19.0,
            view_dist_max: 42.0,
        }
    }
}

impl GameTime {
    /// zombie.GameTime.getWorldAgeHours: nights times 24 plus the hours since 7:00, in float.
    pub fn get_world_age_hours(&self) -> f64 {
        let mut elapsed_hours = self.nights_survived as f32 * 24.0;
        if self.time_of_day >= 7.0 {
            elapsed_hours += self.time_of_day - 7.0;
        } else {
            elapsed_hours += self.time_of_day + 17.0;
        }
        f64::from(elapsed_hours)
    }

    /// zombie.GameTime.getMinutes
    pub fn get_minutes(&self) -> i32 {
        ((self.time_of_day - self.time_of_day as i32 as f32) * 60.0) as i32
    }

    /// zombie.GameTime.getHour
    pub fn get_hour(&self) -> i32 {
        let sec = f64::from(self.time_of_day * 3600.0).floor();
        (sec / 3600.0).floor() as i32
    }

    /// zombie.GameTime.setMinutesStamp
    pub fn set_minutes_stamp(&mut self) {
        self.minutes_stamp = (self.get_world_age_hours() as i64)
            .wrapping_mul(60)
            .wrapping_add(i64::from(self.get_minutes()));
    }

    /// zombie.GameTime.getDayPlusOne
    pub fn get_day_plus_one(&self) -> i32 {
        self.day + 1
    }

    /// zombie.GameTime.getMultiplier on a dedicated server that is not fast-forwarding: multiplier, fps
    /// multiplier, bias, per-object multiplier and the debug slow-motion multiplier (1 unless a debug option sets
    /// it), times 0.8. The single-player sleeping branch (200 * 30 / lock FPS) is not modelled.
    pub fn get_multiplier(&self) -> f32 {
        let mut multiplier = 1.0f32;
        multiplier *= self.multiplier;
        multiplier *= self.fps_multiplier;
        multiplier *= self.multiplier_bias;
        multiplier *= self.per_object_multiplier;
        multiplier *= 1.0;
        multiplier * 0.8
    }

    /// zombie.GameTime.getTimeDelta: `getTimeDeltaFromMultiplier(getMultiplier())`.
    pub fn get_time_delta(&self) -> f32 {
        self.get_multiplier() / 0.8 / self.multiplier_bias / 60.0
    }

    /// zombie.GameTime.getGameWorldSecondsSinceLastUpdate: the time delta times the game seconds per real second.
    pub fn get_game_world_seconds_since_last_update(&self) -> f32 {
        let dif = 1440.0f32 / self.minutes_per_day;
        self.get_time_delta() * dif
    }

    /// zombie.GameTime.getThirtyFPSMultiplier
    pub fn get_thirty_fps_multiplier(&self) -> f32 {
        self.get_multiplier() / 1.6
    }

    /// zombie.GameTime.getInvMultiplier
    pub fn get_inv_multiplier(&self) -> f32 {
        1.0 / self.get_multiplier()
    }

    /// zombie.GameTime.getDeltaMinutesPerDay
    pub fn get_delta_minutes_per_day(&self) -> f32 {
        30.0 / self.minutes_per_day
    }

    /// zombie.GameTime.getUnmoddedMultiplier (no player asleep).
    pub fn get_unmodded_multiplier(&self) -> f32 {
        self.multiplier * self.fps_multiplier * self.per_object_multiplier
    }

    /// zombie.GameTime.getMultipliedSecondsSinceLastUpdate
    pub fn get_multiplied_seconds_since_last_update(&self) -> f32 {
        0.016666668 * self.get_unmodded_multiplier()
    }

    /// zombie.GameTime.daysInMonth: the Gregorian month length (leap years by GregorianCalendar.isLeapYear).
    pub fn days_in_month(&self, year: i32, month: i32) -> i32 {
        let mut days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let leap = if year > 1582 {
            year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
        } else {
            year % 4 == 0
        };
        days[1] += i32::from(leap);
        days[month as usize]
    }

    /// The clock part of zombie.GameTime.update on a dedicated server (not paused, no Last Stand, the debug
    /// freeze-time option off): the time of day moves by `1 / minutesPerDay / 60 * getMultiplier() / 2`, a night
    /// is counted when 7:00 is passed, midnight advances the calendar (advanceOneDay), the minute stamp is set.
    /// Returns true when the ten-minute mark changed (`now / 10 != minutesMod`), where the original runs
    /// ErosionMain.EveryTenMinutes and ClimateManager.updateEveryTenMins; `minutes_mod` is GameTime.minutesMod.
    /// The helicopter, gunfire, ambient and meta events of update (which draw from Rand) are not modelled.
    pub fn update_clock(&mut self, minutes_mod: &mut i32) -> bool {
        let last_time_of_day = self.time_of_day;
        let time = 1.0f32 / self.minutes_per_day / 60.0 * self.get_multiplier() / 2.0;
        self.time_of_day += time;
        if last_time_of_day <= 7.0 && self.time_of_day > 7.0 {
            self.nights_survived += 1;
        }
        if self.time_of_day >= 24.0 {
            self.time_of_day -= 24.0;
            self.advance_one_day();
        }
        self.set_minutes_stamp();
        let now = ((self.time_of_day - self.time_of_day as i32 as f32) * 60.0) as i32;
        if now / 10 != *minutes_mod {
            *minutes_mod = now / 10;
            return true;
        }
        false
    }

    /// zombie.GameTime.advanceOneDay
    pub fn advance_one_day(&mut self) {
        self.day += 1;
        if self.day >= self.days_in_month(self.year, self.month) {
            self.day = 0;
            self.month += 1;
            if self.month >= 12 {
                self.month = 0;
                self.year += 1;
            }
        }
    }

    /// zombie.GameTime.load(DataInputStream) over the bytes of `map_t.bin`: the world version from the "GMTM" header
    /// (without it, `savedWorldVersion`, or 249 when that is -1, and the data starts at byte 0), the clock and
    /// calendar, three unused values, the mod data table when its byte is 1 (loaded into the existing table without
    /// wiping it), the poisonous berry and mushroom (not on a client), the first helicopter day and hours, then
    /// `ClimateManager.load`, and the minute stamp. `Err` where the original throws; what was read before stays, as
    /// in the original, whose caller logs the exception.
    pub fn load(
        &mut self,
        data: &[u8],
        saved_world_version: i32,
        side: MetaSide,
        core: &mut CorePoisonous,
        climate: &mut ClimateManager,
        log: &mut Vec<String>,
    ) -> Result<(), String> {
        let mut input = ByteBuffer::wrap(data);
        let mut world_version = saved_world_version;
        if world_version == -1 {
            world_version = 249;
        }
        let magic = [input.get()?, input.get()?, input.get()?, input.get()?];
        if magic == [71, 77, 84, 77] {
            world_version = input.get_int()?;
        } else {
            input.set_position(0)?;
        }
        self.multiplier = input.get_float()?;
        self.nights_survived = input.get_int()?;
        self.target_zombies = input.get_int()?;
        self.last_time_of_day = input.get_float()?;
        self.time_of_day = input.get_float()?;
        self.day = input.get_int()?;
        self.month = input.get_int()?;
        self.year = input.get_int()?;
        input.get_float()?;
        input.get_float()?;
        input.get_int()?;
        if input.get()? == 1 {
            self.table
                .get_or_insert_with(KahluaTable::default)
                .load_stream(&mut input, world_version)?;
        }
        if side != MetaSide::Client {
            core.poisonous_berry = Some(read_string_stream(&mut input)?);
            core.poisonous_mushroom = Some(read_string_stream(&mut input)?);
        }
        self.helicopter_day1 = input.get_int()?;
        self.helicopter_time1_start = input.get_int()?;
        self.helicopter_time1_end = input.get_int()?;
        climate.load(&mut input, world_version, side, log)?;
        self.set_minutes_stamp();
        Ok(())
    }
}

/// zombie.GameTime.AnimTimer: a countdown in thirtieths of a second of multiplied game time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimTimer {
    pub elapsed: f32,
    pub duration: f32,
    pub finished: bool,
    pub ticks: i32,
}

impl Default for AnimTimer {
    /// zombie.GameTime.AnimTimer(): finished.
    fn default() -> Self {
        AnimTimer {
            elapsed: 0.0,
            duration: 0.0,
            finished: true,
            ticks: 0,
        }
    }
}

impl AnimTimer {
    /// zombie.GameTime.AnimTimer.init
    pub fn init(&mut self, ticks: i32) {
        self.ticks = ticks;
        self.elapsed = 0.0;
        self.duration = ticks as f32 / 30.0;
        self.finished = false;
    }

    /// zombie.GameTime.AnimTimer.update
    pub fn update(&mut self, gt: &GameTime) {
        self.elapsed += gt.get_multiplied_seconds_since_last_update() * 60.0 / 30.0;
        if self.elapsed >= self.duration {
            self.elapsed = self.duration;
            self.finished = true;
        }
    }

    /// zombie.GameTime.AnimTimer.ratio
    pub fn ratio(&self) -> f32 {
        self.elapsed / self.duration
    }

    /// zombie.GameTime.AnimTimer.finished
    pub fn finished(&self) -> bool {
        self.finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 11:25 on the first day (no nights survived): 4.4 hours since 7:00, stamp 4 * 60 + 25.
    #[test]
    fn minute_stamp_counts_from_seven() {
        let mut t = GameTime {
            time_of_day: 11.42,
            ..GameTime::default()
        };
        t.set_minutes_stamp();
        assert_eq!(
            (t.get_hour(), t.get_minutes(), t.minutes_stamp),
            (11, 25, 265)
        );
        t.time_of_day = 3.5;
        t.nights_survived = 2;
        t.set_minutes_stamp();
        assert_eq!(t.minutes_stamp, (48 + 20) * 60 + 30);
    }
}
