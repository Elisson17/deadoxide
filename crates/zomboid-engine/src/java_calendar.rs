//! java.util.GregorianCalendar and java.util.Date as Kahlua's OsLib uses them: the UTC zone, the default cutover
//! (Julian dates before 1582-10-15), lenient field setting. Days are "fixed dates" (day 1 = Gregorian 0001-01-01), as
//! in sun.util.calendar.BaseCalendar; integer widths and the order of the cutover tests follow the JDK.

const ONE_DAY: i64 = 86_400_000;
/// GregorianCalendar.EPOCH_OFFSET: the fixed date of 1970-01-01.
const EPOCH_OFFSET: i64 = 719_163;
/// GregorianCalendar.gregorianCutoverDate for the default cutover (-12219292800000 ms).
const CUTOVER_DATE: i64 = 577_736;
const CUTOVER_YEAR: i32 = 1582;
/// sun.util.calendar.JulianCalendar.JULIAN_EPOCH
const JULIAN_EPOCH: i64 = -1;
/// sun.util.calendar.BaseCalendar.ACCUMULATED_DAYS_IN_MONTH
const ACCUMULATED_DAYS_IN_MONTH: [i64; 13] =
    [-30, 0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];

/// The calendar fields OsLib reads (`Calendar.get`), plus the calendar system of the date (`calsys`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fields {
    /// ERA: false = BC.
    pub ad: bool,
    /// YEAR (year of the era, 1 or more).
    pub year: i32,
    /// MONTH, 0 = January.
    pub month: i32,
    pub day_of_month: i32,
    /// DAY_OF_WEEK, 1 = Sunday.
    pub day_of_week: i32,
    pub hour_of_day: i32,
    pub minute: i32,
    pub second: i32,
    pub millisecond: i32,
    /// The date is in the Julian calendar (GregorianCalendar.calsys == jcal).
    pub julian: bool,
}

impl Fields {
    /// Calendar.HOUR
    pub fn hour(&self) -> i32 {
        self.hour_of_day % 12
    }
    /// Calendar.AM_PM
    pub fn am_pm(&self) -> i32 {
        self.hour_of_day / 12
    }
    /// The normalized year (1 BC = 0), what GregorianCalendar.computeTime starts from.
    pub fn normalized_year(&self) -> i32 {
        if self.ad {
            self.year
        } else {
            1i32.wrapping_sub(self.year)
        }
    }
}

/// sun.util.calendar.CalendarUtils.isGregorianLeapYear
fn is_gregorian_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// sun.util.calendar.CalendarUtils.isJulianLeapYear
fn is_julian_leap_year(year: i32) -> bool {
    year.rem_euclid(4) == 0
}

/// sun.util.calendar.BaseCalendar.getFixedDate (Gregorian; `month` 1 to 12).
fn gregorian_fixed_date(year: i32, month: i32, day_of_month: i64) -> i64 {
    let prevyear = i64::from(year) - 1;
    let mut days = day_of_month;
    days += 365 * prevyear + prevyear.div_euclid(4) - prevyear.div_euclid(100)
        + prevyear.div_euclid(400)
        + (367 * i64::from(month) - 362).div_euclid(12);
    if month > 2 {
        days -= if is_gregorian_leap_year(year) { 1 } else { 2 };
    }
    days
}

/// sun.util.calendar.JulianCalendar.getFixedDate (`jyear` normalized, `month` 1 to 12).
fn julian_fixed_date(jyear: i32, month: i32, day_of_month: i64) -> i64 {
    let y = i64::from(jyear);
    let mut days = JULIAN_EPOCH - 1 + 365 * (y - 1) + day_of_month;
    days += (y - 1).div_euclid(4);
    days += (367 * i64::from(month) - 362).div_euclid(12);
    if month > 2 {
        days -= if is_julian_leap_year(jyear) { 1 } else { 2 };
    }
    days
}

/// sun.util.calendar.BaseCalendar.getGregorianYearFromFixedDate
fn gregorian_year_from_fixed_date(fixed_date: i64) -> i32 {
    let d0 = fixed_date - 1;
    let n400 = d0.div_euclid(146_097) as i32;
    let d1 = d0.rem_euclid(146_097) as i32;
    let n100 = d1 / 36524;
    let d2 = d1 % 36524;
    let n4 = d2 / 1461;
    let d3 = d2 % 1461;
    let n1 = d3 / 365;
    let mut year = 400i32
        .wrapping_mul(n400)
        .wrapping_add(100 * n100)
        .wrapping_add(4 * n4)
        .wrapping_add(n1);
    if !(n100 == 4 || n1 == 4) {
        year = year.wrapping_add(1);
    }
    year
}

/// sun.util.calendar.BaseCalendar.getDayOfWeekFromFixedDate (1 = Sunday)
fn day_of_week_from_fixed_date(fixed_date: i64) -> i32 {
    fixed_date.rem_euclid(7) as i32 + 1
}

/// (normalized year, month 1-12, day) of a Gregorian fixed date: BaseCalendar.getCalendarDateFromFixedDate.
fn gregorian_date(fixed_date: i64) -> (i32, i32, i32) {
    let year = gregorian_year_from_fixed_date(fixed_date);
    let jan1 = gregorian_fixed_date(year, 1, 1);
    let is_leap = is_gregorian_leap_year(year);
    let mut prior_days = (fixed_date - jan1) as i32;
    let mut mar1 = jan1 + ACCUMULATED_DAYS_IN_MONTH[3];
    if is_leap {
        mar1 += 1;
    }
    if fixed_date >= mar1 {
        prior_days = prior_days.wrapping_add(if is_leap { 1 } else { 2 });
    }
    let month = 12i32
        .wrapping_mul(prior_days)
        .wrapping_add(373)
        .div_euclid(367);
    let mut month1 = jan1 + ACCUMULATED_DAYS_IN_MONTH[month.clamp(0, 12) as usize];
    if is_leap && month >= 3 {
        month1 += 1;
    }
    (year, month, (fixed_date - month1) as i32 + 1)
}

/// (normalized year, month 1-12, day) of a Julian fixed date: JulianCalendar.getCalendarDateFromFixedDate.
fn julian_date(fixed_date: i64) -> (i32, i32, i32) {
    let fd = 4 * (fixed_date - JULIAN_EPOCH) + 1464;
    let year = fd.div_euclid(1461) as i32;
    let mut prior_days = (fixed_date - julian_fixed_date(year, 1, 1)) as i32;
    let is_leap = is_julian_leap_year(year);
    if fixed_date >= julian_fixed_date(year, 3, 1) {
        prior_days = prior_days.wrapping_add(if is_leap { 1 } else { 2 });
    }
    let month = 12i32
        .wrapping_mul(prior_days)
        .wrapping_add(373)
        .div_euclid(367);
    let day = (fixed_date - julian_fixed_date(year, month, 1)) as i32 + 1;
    (year, month, day)
}

/// GregorianCalendar.computeFields for a UTC time in milliseconds (`Calendar.setTime`).
pub fn compute_fields(time: i64) -> Fields {
    let fixed_date = time.div_euclid(ONE_DAY) + EPOCH_OFFSET;
    let time_of_day = time.rem_euclid(ONE_DAY) as i32;
    let julian = fixed_date < CUTOVER_DATE;
    let (year, month, day) = if julian {
        julian_date(fixed_date)
    } else {
        gregorian_date(fixed_date)
    };
    let (ad, year) = if year <= 0 {
        (false, 1i32.wrapping_sub(year))
    } else {
        (true, year)
    };
    let hours = time_of_day / 3_600_000;
    let r = time_of_day % 3_600_000;
    Fields {
        ad,
        year,
        month: month - 1,
        day_of_month: day,
        day_of_week: day_of_week_from_fixed_date(fixed_date),
        hour_of_day: hours,
        minute: r / 60_000,
        second: r % 60_000 / 1000,
        millisecond: r % 1000,
        julian,
    }
}

/// GregorianCalendar.getFixedDate(cal, year, fieldMask) for MONTH and DAY_OF_MONTH set: the month is brought
/// into range (moving the year), then the day is added to the first of the month.
fn fixed_date_of(julian: bool, year: i32, month: i32, day_of_month: i32) -> i64 {
    let mut year = year;
    let mut month = month;
    if month > 11 {
        year = year.wrapping_add(month / 12);
        month %= 12;
    } else if month < 0 {
        year = year.wrapping_add(month.div_euclid(12));
        month = month.rem_euclid(12);
    }
    let first = if julian {
        julian_fixed_date(year, month + 1, 1)
    } else {
        gregorian_fixed_date(year, month + 1, 1)
    };
    first + i64::from(day_of_month) - 1
}

/// GregorianCalendar.computeTime (lenient, UTC) when YEAR, MONTH, DAY_OF_MONTH, HOUR_OF_DAY, MINUTE, SECOND and
/// MILLISECOND decide the time: `year` is normalized (1 BC = 0), `calsys_julian` the calendar system of the
/// calendar's previous `computeFields` (it settles a date of the ten days both calendars could mean).
#[allow(clippy::too_many_arguments)]
pub fn compute_time(
    year: i32,
    month: i32,
    day_of_month: i32,
    hour_of_day: i32,
    minute: i32,
    second: i32,
    millisecond: i32,
    calsys_julian: bool,
) -> i64 {
    let mut time_of_day = i64::from(hour_of_day);
    time_of_day = time_of_day * 60 + i64::from(minute);
    time_of_day = time_of_day * 60 + i64::from(second);
    time_of_day = time_of_day * 1000 + i64::from(millisecond);
    let mut fixed_date = time_of_day / ONE_DAY;
    time_of_day %= ONE_DAY;
    while time_of_day < 0 {
        time_of_day += ONE_DAY;
        fixed_date -= 1;
    }
    let gfd;
    let jfd;
    if year > CUTOVER_YEAR {
        let g = fixed_date + fixed_date_of(false, year, month, day_of_month);
        if g >= CUTOVER_DATE {
            return (g - EPOCH_OFFSET)
                .wrapping_mul(ONE_DAY)
                .wrapping_add(time_of_day);
        }
        gfd = g;
        jfd = fixed_date + fixed_date_of(true, year, month, day_of_month);
    } else if year < CUTOVER_YEAR {
        let j = fixed_date + fixed_date_of(true, year, month, day_of_month);
        if j < CUTOVER_DATE {
            return (j - EPOCH_OFFSET)
                .wrapping_mul(ONE_DAY)
                .wrapping_add(time_of_day);
        }
        jfd = j;
        gfd = j;
    } else {
        jfd = fixed_date + fixed_date_of(true, year, month, day_of_month);
        gfd = fixed_date + fixed_date_of(false, year, month, day_of_month);
    }
    let fixed = if gfd >= CUTOVER_DATE {
        if jfd >= CUTOVER_DATE || !calsys_julian {
            gfd
        } else {
            jfd
        }
    } else {
        jfd
    };
    (fixed - EPOCH_OFFSET)
        .wrapping_mul(ONE_DAY)
        .wrapping_add(time_of_day)
}

const DAY_ABBR: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTH_ABBR: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// java.util.Date.toString in the JVM's default zone, taken to be UTC (the oracle runs the original with UTC; on
/// a player's machine it is their zone: UNKNOWN). "EEE MMM dd HH:mm:ss zzz yyyy".
pub fn date_to_string(time: i64) -> String {
    let f = compute_fields(time);
    format!(
        "{} {} {:02} {:02}:{:02}:{:02} UTC {}",
        DAY_ABBR[(f.day_of_week - 1) as usize],
        MONTH_ABBR[f.month as usize],
        f.day_of_month,
        f.hour_of_day,
        f.minute,
        f.second,
        f.year
    )
}

/// A java.util.GregorianCalendar in the UTC zone, as the climate code uses one: made from date fields (lenient: a
/// day of 0 is the last day of the month before), moved by whole days or hours, read back by field or as
/// `getTime().getTime()`. The oracle pins the JVM's default zone to UTC; on a player's machine the original uses
/// their zone, whose daylight-saving hours this does not model (UNKNOWN).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GregorianCalendar {
    time: i64,
}

impl GregorianCalendar {
    /// GregorianCalendar(int year, int month, int dayOfMonth, int hourOfDay, int minute); also the three-argument
    /// constructor (hour and minute 0).
    pub fn new(year: i32, month: i32, day_of_month: i32, hour_of_day: i32, minute: i32) -> Self {
        GregorianCalendar {
            time: compute_time(year, month, day_of_month, hour_of_day, minute, 0, 0, false),
        }
    }

    /// Calendar.add(Calendar.DAY_OF_MONTH, amount): whole days, the time of day kept.
    pub fn add_days(&mut self, amount: i32) {
        self.time = self
            .time
            .wrapping_add(i64::from(amount).wrapping_mul(ONE_DAY));
    }

    /// Calendar.add(Calendar.HOUR_OF_DAY, amount)
    pub fn add_hours(&mut self, amount: i32) {
        self.time = self
            .time
            .wrapping_add(i64::from(amount).wrapping_mul(3_600_000));
    }

    /// Calendar.getTime().getTime() (and toInstant().toEpochMilli()).
    pub fn time_in_millis(&self) -> i64 {
        self.time
    }

    /// Calendar.get for the fields: YEAR, MONTH, DAY_OF_MONTH, HOUR_OF_DAY, MINUTE...
    pub fn fields(&self) -> Fields {
        compute_fields(self.time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Independent values: the Unix epoch, the Gregorian cutover (Julian 1582-10-04 is followed by Gregorian
    /// 1582-10-15) and 2000-02-29.
    #[test]
    fn known_dates() {
        let e = compute_fields(0);
        assert_eq!(
            (e.year, e.month, e.day_of_month, e.day_of_week),
            (1970, 0, 1, 5)
        );
        let c = compute_fields(-12_219_292_800_000);
        assert_eq!(
            (c.year, c.month, c.day_of_month, c.julian),
            (1582, 9, 15, false)
        );
        let j = compute_fields(-12_219_292_800_001);
        assert_eq!(
            (j.year, j.month, j.day_of_month, j.julian),
            (1582, 9, 4, true)
        );
        assert_eq!(
            compute_time(2000, 1, 29, 0, 0, 0, 0, false),
            951_782_400_000
        );
        assert_eq!(date_to_string(0), "Thu Jan 01 00:00:00 UTC 1970");
    }
}
