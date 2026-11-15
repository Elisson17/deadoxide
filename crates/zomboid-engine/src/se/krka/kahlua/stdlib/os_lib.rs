//! se.krka.kahlua.stdlib.OsLib: `os.date`, `os.difftime`, `os.time`. The original reads the wall clock
//! (`System.currentTimeMillis`); here the host sets it on the platform (`J2SEPlatform::current_time_millis`), so the
//! simulation stays reproducible. Every calendar is UTC, as in the original; `%c` is `Date.toString` in the JVM's
//! default zone, taken to be UTC (UNKNOWN on a machine in another zone).
use std::rc::Rc;

use crate::java_calendar::{compute_fields, compute_time, date_to_string, Fields};
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
use crate::se::krka::kahlua::vm::kahlua_util::{
    from_double, get_arg, get_double_arg, get_string_arg, to_double,
};
use crate::se::krka::kahlua::vm::object::{
    cast_table, classes, lstr, JavaClass, JavaFunction, Object, TableRef, Throwable, R,
};

pub static CLASS: JavaClass = JavaClass {
    name: "se.krka.kahlua.stdlib.OsLib",
    superclass: Some(&classes::OBJECT),
    interfaces: &[&classes::JAVA_FUNCTION],
};

const FUNCNAMES: [&str; 3] = ["date", "difftime", "time"];
/// OsLib.MILLISECOND
const MILLISECOND: &str = "milli";
const SHORT_DAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const LONG_DAY_NAMES: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
const SHORT_MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const LONG_MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

pub struct OsLib {
    method_id: usize,
}

/// se.krka.kahlua.stdlib.OsLib.register
pub fn register(env: &TableRef) {
    let os = KahluaTableImpl::new_ref();
    for (i, name) in FUNCNAMES.iter().enumerate() {
        KahluaTableImpl::rawset(
            &os,
            Object::str(name),
            Object::Function(Rc::new(OsLib { method_id: i })),
        );
    }
    KahluaTableImpl::rawset(env, Object::str("os"), Object::Table(os));
}

impl JavaFunction for OsLib {
    /// se.krka.kahlua.stdlib.OsLib.call
    fn call(&self, thread: &mut KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
        match self.method_id {
            0 => date(thread, cf, n),
            1 => difftime(cf),
            _ => time(thread, cf, n),
        }
    }
    fn java_to_string(&self) -> String {
        format!("{}@0", CLASS.name)
    }
    fn class(&self) -> &'static JavaClass {
        &CLASS
    }
}

/// se.krka.kahlua.stdlib.OsLib.time
fn time(thread: &KahluaThread, cf: &FrameRef, nargs: i32) -> R<i32> {
    let t = if nargs == 0 {
        thread.platform.current_time_millis.get() as f64 * 0.001
    } else {
        // getArg never returns null, so the cast either fails or gives a table.
        let o = get_arg(cf, 1, "time")?;
        let table = cast_table(&o)?.unwrap_or_else(KahluaTableImpl::new_ref);
        get_date_from_table(&table)? as f64 * 0.001
    };
    cf.push(Object::Number(to_double(t)))?;
    Ok(1)
}

/// se.krka.kahlua.stdlib.OsLib.difftime
fn difftime(cf: &FrameRef) -> R<i32> {
    let t2 = get_double_arg(cf, 1, "difftime")?;
    let t1 = get_double_arg(cf, 2, "difftime")?;
    cf.push(Object::Number(to_double(t2 - t1)))?;
    Ok(1)
}

/// se.krka.kahlua.stdlib.OsLib.date
fn date(thread: &KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
    let now = thread.platform.current_time_millis.get();
    let r = if n == 0 {
        getdate(&lstr("%c"), now)?
    } else {
        let format = get_string_arg(cf, 1, "date")?;
        if n == 1 {
            getdate(&format, now)?
        } else {
            let t = get_double_arg(cf, 2, "date")?;
            getdate(&format, (t * 1000.0) as i64)?
        }
    };
    cf.push(r)?;
    Ok(1)
}

/// se.krka.kahlua.stdlib.OsLib.getdate(String, long, Platform): a table for "*t", else the formatted text ("!" in
/// front changes nothing: both calendars are UTC).
pub fn getdate(format: &[u16], time: i64) -> R<Object> {
    let mut si = 0;
    match format.first() {
        None => {
            return Err(Throwable::Java {
                class: "java.lang.StringIndexOutOfBoundsException",
                message: Some("Index 0 out of bounds for length 0".into()),
            })
        }
        Some(&c) if c == u16::from(b'!') => si += 1,
        Some(_) => {}
    }
    let cal = compute_fields(time);
    if format[si..].starts_with(&[u16::from(b'*'), u16::from(b't')]) {
        Ok(Object::Table(get_table_from_date(&cal, time)))
    } else {
        Ok(Object::String(Rc::from(format_time(
            &format[si..],
            &cal,
            time,
        ))))
    }
}

/// se.krka.kahlua.stdlib.OsLib.formatTime
fn format_time(format: &[u16], cal: &Fields, time: i64) -> Vec<u16> {
    let mut buffer = Vec::new();
    let mut i = 0;
    while i < format.len() {
        if format[i] == u16::from(b'%') && i + 1 != format.len() {
            i += 1;
            match strftime(format[i], cal, time) {
                Some(s) => buffer.extend(s),
                None => buffer.extend("null".encode_utf16()),
            }
        } else {
            buffer.push(format[i]);
        }
        i += 1;
    }
    buffer
}

/// se.krka.kahlua.stdlib.OsLib.format2Digits
fn format2_digits(value: i32) -> String {
    if value < 10 {
        format!("0{value}")
    } else {
        value.to_string()
    }
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// se.krka.kahlua.stdlib.OsLib.strftime: `None` is the original's null (an unknown conversion).
fn strftime(format: u16, cal: &Fields, time: i64) -> Option<Vec<u16>> {
    let c = char::from_u32(u32::from(format))?;
    let s = match c {
        'A' => LONG_DAY_NAMES[(cal.day_of_week - 1) as usize].to_string(),
        'B' => LONG_MONTH_NAMES[cal.month as usize].to_string(),
        'C' => (cal.year / 100).to_string(),
        'D' => return Some(format_time(&utf16("%m/%d/%y"), cal, time)),
        'H' => format2_digits(cal.hour_of_day),
        'I' => format2_digits(cal.hour()),
        'M' => format2_digits(cal.minute),
        'R' => return Some(format_time(&utf16("%H:%M"), cal, time)),
        'S' => format2_digits(cal.second),
        'U' => get_week_of_year(cal, time, true, false).to_string(),
        'V' => get_week_of_year(cal, time, false, true).to_string(),
        'W' => get_week_of_year(cal, time, false, false).to_string(),
        'Y' => cal.year.to_string(),
        'Z' => "UTC".to_string(),
        'a' => SHORT_DAY_NAMES[(cal.day_of_week - 1) as usize].to_string(),
        'b' | 'h' => SHORT_MONTH_NAMES[cal.month as usize].to_string(),
        'c' => date_to_string(time),
        'd' => format2_digits(cal.day_of_month),
        'e' => {
            let d = format2_digits(cal.day_of_month);
            if cal.day_of_month < 10 {
                format!(" {d}")
            } else {
                d
            }
        }
        'j' => get_day_of_year(cal, time).to_string(),
        'm' => format2_digits(cal.month + 1),
        'n' => "\n".to_string(),
        'p' => if cal.am_pm() == 0 { "AM" } else { "PM" }.to_string(),
        'r' => return Some(format_time(&utf16("%I:%M:%S %p"), cal, time)),
        'w' => (cal.day_of_week - 1).to_string(),
        'y' => (cal.year % 100).to_string(),
        _ => return None,
    };
    Some(utf16(&s))
}

fn number(d: i64) -> Object {
    Object::Number(to_double(d as f64))
}

/// se.krka.kahlua.stdlib.OsLib.getTableFromDate
fn get_table_from_date(c: &Fields, time: i64) -> TableRef {
    let t = KahluaTableImpl::new_ref();
    let set = |k: &str, v: i64| KahluaTableImpl::rawset(&t, Object::str(k), number(v));
    set("year", i64::from(c.year));
    set("month", i64::from(c.month + 1));
    set("day", i64::from(c.day_of_month));
    set("hour", i64::from(c.hour_of_day));
    set("min", i64::from(c.minute));
    set("sec", i64::from(c.second));
    set("wday", i64::from(c.day_of_week));
    set("yday", i64::from(get_day_of_year(c, time)));
    set(MILLISECOND, i64::from(c.millisecond));
    t
}

/// `(int) KahluaUtil.fromDouble(o)`
fn int_of(o: &Object) -> R<i32> {
    Ok(from_double(o)? as i32)
}

/// se.krka.kahlua.stdlib.OsLib.getDateFromTable: a lenient UTC calendar (era AD, as of the current time).
pub fn get_date_from_table(time: &TableRef) -> R<i64> {
    let get = |k: &str| KahluaTableImpl::rawget(time, &Object::str(k));
    let year = int_of(&get("year")?)?;
    let month = int_of(&get("month")?)?.wrapping_sub(1);
    let day = int_of(&get("day")?)?;
    let opt = |o: Object| -> R<i32> {
        if o.is_nil() {
            Ok(0)
        } else {
            int_of(&o)
        }
    };
    let hour = opt(get("hour")?)?;
    let minute = opt(get("min")?)?;
    let seconds = opt(get("sec")?)?;
    let milliseconds = opt(get(MILLISECOND)?)?;
    Ok(compute_time(
        year,
        month,
        day,
        hour,
        minute,
        seconds,
        milliseconds,
        false,
    ))
}

/// Jan 1 of `c`'s year at `c`'s time of day (`c2.setTime(c.getTime()); c2.set(MONTH, 0); c2.set(DAY_OF_MONTH, 1)`)
/// moved to day `day`.
fn same_year_on(c: &Fields, day: i32) -> i64 {
    compute_time(
        c.normalized_year(),
        0,
        day,
        c.hour_of_day,
        c.minute,
        c.second,
        c.millisecond,
        c.julian,
    )
}

/// se.krka.kahlua.stdlib.OsLib.getDayOfYear: whole days since Jan 1 rounded up (0 at midnight on Jan 1).
fn get_day_of_year(c: &Fields, time: i64) -> i32 {
    let diff = time.wrapping_sub(same_year_on(c, 1));
    (diff as f64 / 8.64E7).ceil() as i32
}

/// se.krka.kahlua.stdlib.OsLib.getWeekOfYear (a Sunday Jan 1 with weekStartsSunday takes the Monday branch, as in
/// the original).
fn get_week_of_year(c: &Fields, time: i64, week_starts_sunday: bool, jan1midweek: bool) -> i32 {
    let jan1 = same_year_on(c, 1);
    let c2 = compute_fields(jan1);
    let day_of_week = c2.day_of_week;
    let start = if week_starts_sunday && day_of_week != 1 {
        compute_time(
            c2.normalized_year(),
            c2.month,
            7 - day_of_week + 1,
            c2.hour_of_day,
            c2.minute,
            c2.second,
            c2.millisecond,
            c2.julian,
        )
    } else if day_of_week != 2 {
        compute_time(
            c2.normalized_year(),
            c2.month,
            7 - day_of_week + 1 + 1,
            c2.hour_of_day,
            c2.minute,
            c2.second,
            c2.millisecond,
            c2.julian,
        )
    } else {
        jan1
    };
    let diff = time.wrapping_sub(start);
    let mut w = (diff / 604_800_000) as i32;
    if jan1midweek && 7 - day_of_week >= 4 {
        w += 1;
    }
    w
}
