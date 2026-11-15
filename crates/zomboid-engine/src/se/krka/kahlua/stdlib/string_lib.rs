//! se.krka.kahlua.stdlib.StringLib: the `string` table, also the class metatable of strings (`("x"):sub(1)`).
//! All 14 functions are ported. `split` is Java's `String.split`, a regular expression; only literal separators
//! are ported (a real regular expression throws "not ported", PLACEHOLDER).
use std::rc::Rc;

use crate::java_chars;
use crate::se::krka::kahlua::j2se::j2se_platform::J2SEPlatform;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
use crate::se::krka::kahlua::vm::kahlua_util::{
    bool_eval, fail, get_arg, get_class_metatables, get_number_arg, get_optional_arg,
    get_optional_number_arg, get_string_arg, is_negative, lua_assert, number_to_string,
    raw_tostring, round, to_double, tostring,
};
use crate::se::krka::kahlua::vm::object::{
    classes, lstr, rstr, JavaClass, JavaFunction, LuaStr, Object, TableRef, Throwable, R,
};

pub static CLASS: JavaClass = JavaClass {
    name: "se.krka.kahlua.stdlib.StringLib",
    superclass: Some(&classes::OBJECT),
    interfaces: &[&classes::JAVA_FUNCTION],
};

const NAMES: [&str; 14] = [
    "sub", "char", "byte", "lower", "upper", "reverse", "format", "find", "match", "gsub", "trim",
    "split", "sort", "contains",
];

pub struct StringLib {
    method_id: usize,
}

/// se.krka.kahlua.stdlib.StringLib.register
pub fn register(platform: &J2SEPlatform, env: &TableRef) -> R<()> {
    let string = KahluaTableImpl::new_ref();
    for (i, name) in NAMES.iter().enumerate() {
        KahluaTableImpl::rawset(
            &string,
            Object::str(name),
            Object::Function(Rc::new(StringLib { method_id: i })),
        );
    }
    KahluaTableImpl::rawset(
        &string,
        Object::str("__index"),
        Object::Table(string.clone()),
    );
    let metatables = get_class_metatables(env)?;
    KahluaTableImpl::rawset(
        &metatables,
        platform.class_object(&classes::STRING),
        Object::Table(string.clone()),
    );
    KahluaTableImpl::rawset(env, Object::str("string"), Object::Table(string));
    Ok(())
}

impl JavaFunction for StringLib {
    /// se.krka.kahlua.stdlib.StringLib.call
    fn call(&self, thread: &mut KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
        match self.method_id {
            0 => sub(cf, n),
            1 => string_char(cf, n),
            2 => string_byte(cf, n),
            3 => {
                lua_assert(n >= 1, "not enough arguments")?;
                let s = get_string_arg(cf, 1, NAMES[3])?;
                cf.push(same_or_new(
                    s.clone(),
                    &crate::java::to_lower_case(&rstr(&s)),
                ))?;
                Ok(1)
            }
            4 => {
                lua_assert(n >= 1, "not enough arguments")?;
                let s = get_string_arg(cf, 1, NAMES[4])?;
                cf.push(same_or_new(
                    s.clone(),
                    &crate::java::to_upper_case(&rstr(&s)),
                ))?;
                Ok(1)
            }
            5 => {
                lua_assert(n >= 1, "not enough arguments")?;
                let s = get_string_arg(cf, 1, NAMES[5])?;
                cf.push(Object::String(string_builder_reverse(&s)))?;
                Ok(1)
            }
            10 => {
                let s = get_string_arg(cf, 1, NAMES[10])?;
                cf.push(Object::String(java_trim(&s)))
            }
            12 => {
                let a = get_string_arg(cf, 1, NAMES[12])?;
                let b = get_string_arg(cf, 2, NAMES[12])?;
                cf.push(Object::Boolean(a[..].cmp(&b[..]).is_gt()))
            }
            13 => {
                lua_assert(n >= 2, "not enough arguments")?;
                let s = get_string_arg(cf, 1, NAMES[13])?;
                let s2 = get_string_arg(cf, 2, NAMES[13])?;
                let found = s2.is_empty() || s.windows(s2.len()).any(|w| w == &s2[..]);
                cf.push(Object::Boolean(found))?;
                Ok(1)
            }
            6 => format(cf),
            7 => find_aux(cf, true),
            8 => find_aux(cf, false),
            9 => gsub(thread, cf),
            11 => split(cf),
            _ => Ok(0),
        }
    }
    /// se.krka.kahlua.stdlib.StringLib.toString
    fn java_to_string(&self) -> String {
        NAMES[self.method_id].to_string()
    }
    fn class(&self) -> &'static JavaClass {
        &CLASS
    }
}

/// `toLowerCase`/`toUpperCase` return the string itself when nothing changes.
fn same_or_new(orig: LuaStr, changed: &str) -> Object {
    let units: Vec<u16> = changed.encode_utf16().collect();
    if units[..] == orig[..] {
        Object::String(orig)
    } else {
        Object::String(units.into())
    }
}

/// `String.trim`: drops leading and trailing units up to U+0020.
pub fn java_trim(s: &LuaStr) -> LuaStr {
    let start = s.iter().position(|&c| c > 0x20).unwrap_or(s.len());
    let end = s.iter().rposition(|&c| c > 0x20).map_or(start, |e| e + 1);
    if start == 0 && end == s.len() {
        s.clone()
    } else {
        s[start..end].into()
    }
}

/// `new StringBuilder(s).reverse()`: units reversed, valid surrogate pairs kept in order.
pub fn string_builder_reverse(s: &[u16]) -> LuaStr {
    let mut v: Vec<u16> = s.iter().rev().copied().collect();
    let mut i = 0;
    while i + 1 < v.len() {
        if (0xDC00..0xE000).contains(&v[i]) && (0xD800..0xDC00).contains(&v[i + 1]) {
            v.swap(i, i + 1);
            i += 2;
        } else {
            i += 1;
        }
    }
    v.into()
}

/// se.krka.kahlua.stdlib.StringLib.stringByte
fn string_byte(cf: &FrameRef, n: i32) -> R<i32> {
    lua_assert(n >= 1, "not enough arguments")?;
    let s = get_string_arg(cf, 1, NAMES[2])?;
    let mut i = get_optional_number_arg(cf, 2)?.map_or(1, |d| d as i32);
    let mut j = get_optional_number_arg(cf, 3)?.map_or(i, |d| d as i32);
    let len = s.len() as i32;
    if i < 0 {
        i += len + 1;
    }
    if i <= 0 {
        i = 1;
    }
    if j < 0 {
        j += len + 1;
    } else if j > len {
        j = len;
    }
    let n_returns = 1 + j - i;
    if n_returns <= 0 {
        return Ok(0);
    }
    cf.set_top(n_returns)?;
    let offset = i - 1;
    for i2 in 0..n_returns {
        let c = s
            .get((offset + i2) as usize)
            .copied()
            .ok_or_else(|| Throwable::Java {
                class: "java.lang.StringIndexOutOfBoundsException",
                message: Some(format!(
                    "Index {} out of bounds for length {len}",
                    offset + i2
                )),
            })?;
        cf.set(i2, Object::Number(f64::from(c)));
    }
    Ok(n_returns)
}

/// se.krka.kahlua.stdlib.StringLib.stringChar: each number cast to a UTF-16 unit.
fn string_char(cf: &FrameRef, n: i32) -> R<i32> {
    let mut sb: Vec<u16> = Vec::with_capacity(n.max(0) as usize);
    for i in 0..n {
        let num = get_number_arg(cf, i + 1, NAMES[1])? as i32;
        sb.push(num as u16);
    }
    cf.push(Object::String(sb.into()))
}

/// se.krka.kahlua.stdlib.StringLib.sub
fn sub(cf: &FrameRef, n: i32) -> R<i32> {
    let s = get_string_arg(cf, 1, NAMES[0])?;
    let start = get_number_arg(cf, 2, NAMES[0])?;
    let end = if n >= 3 {
        get_number_arg(cf, 3, NAMES[0])?
    } else {
        -1.0
    };
    let mut istart = start as i32;
    let mut iend = end as i32;
    let len = s.len() as i32;
    if istart < 0 {
        istart = (len + istart + 1).max(1);
    } else if istart == 0 {
        istart = 1;
    }
    if iend < 0 {
        iend = (iend + len + 1).max(0);
    } else if iend > len {
        iend = len;
    }
    if istart > iend {
        return cf.push(Object::str(""));
    }
    // String.substring returns the string itself for the whole range.
    if istart == 1 && iend == len {
        return cf.push(Object::String(s));
    }
    let res: LuaStr = s[(istart - 1) as usize..iend as usize].into();
    cf.push(Object::String(res))
}

// ---------------------------------------------------------------------------------------------------------------
// string.format
// ---------------------------------------------------------------------------------------------------------------

const DIGITS: [u16; 16] = [
    b'0' as u16,
    b'1' as u16,
    b'2' as u16,
    b'3' as u16,
    b'4' as u16,
    b'5' as u16,
    b'6' as u16,
    b'7' as u16,
    b'8' as u16,
    b'9' as u16,
    b'a' as u16,
    b'b' as u16,
    b'c' as u16,
    b'd' as u16,
    b'e' as u16,
    b'f' as u16,
];

const fn u(c: char) -> u16 {
    c as u16
}

/// `StringBuilder.charAt` out of range.
fn sb_index_error(index: i32, length: usize) -> Throwable {
    Throwable::Java {
        class: "java.lang.StringIndexOutOfBoundsException",
        message: Some(format!("Index {index} out of bounds for length {length}")),
    }
}

/// se.krka.kahlua.vm.KahluaUtil.ipow (long arithmetic wraps).
pub fn ipow(mut base: i64, exponent: i32) -> i64 {
    if exponent <= 0 {
        return 1;
    }
    let mut b = if exponent & 1 != 0 { base } else { 1 };
    let mut e = exponent >> 1;
    while e != 0 {
        base = base.wrapping_mul(base);
        if e & 1 != 0 {
            b = b.wrapping_mul(base);
        }
        e >>= 1;
    }
    b
}

/// se.krka.kahlua.stdlib.StringLib.unsigned
fn unsigned(v: i64) -> i64 {
    if v < 0 {
        v.wrapping_add(4294967296)
    } else {
        v
    }
}

/// se.krka.kahlua.stdlib.StringLib.extend: `setLength(len + extra)` (a negative `extra` truncates), then the
/// new units set to `pad`.
fn extend(buffer: &mut Vec<u16>, extra_width: i32, pad: u16) -> R<()> {
    let pre = buffer.len() as i64;
    let new_len = pre + i64::from(extra_width);
    if new_len < 0 {
        return Err(Throwable::Java {
            class: "java.lang.StringIndexOutOfBoundsException",
            message: Some(format!("String index out of range: {new_len}")),
        });
    }
    buffer.resize(new_len as usize, 0);
    for i in (0..extra_width.max(0)).rev() {
        buffer[(pre + i64::from(i)) as usize] = pad;
    }
    Ok(())
}

/// se.krka.kahlua.stdlib.StringLib.stringBufferAppend: digits of `value` (least significant first, then
/// reversed), at least `min_digits` of them.
fn string_buffer_append(
    sb: &mut Vec<u16>,
    mut value: f64,
    base: i32,
    print_zero: bool,
    mut min_digits: i32,
) -> R<()> {
    let start_pos = sb.len() as i64;
    let b = f64::from(base);
    while value > 0.0 || min_digits > 0 {
        let new_value = (value / b).floor();
        let idx = (value - new_value * b) as i32;
        let d = usize::try_from(idx)
            .ok()
            .and_then(|i| DIGITS.get(i))
            .ok_or_else(|| Throwable::Java {
                class: "java.lang.ArrayIndexOutOfBoundsException",
                message: Some(format!("Index {idx} out of bounds for length 16")),
            })?;
        sb.push(*d);
        value = new_value;
        min_digits -= 1;
    }
    let end_pos = sb.len() as i64 - 1;
    if start_pos > end_pos && print_zero {
        sb.push(u('0'));
    } else {
        let swap_count = (1 + end_pos - start_pos) / 2;
        for i in (0..swap_count).rev() {
            sb.swap((start_pos + i) as usize, (end_pos - i) as usize);
        }
    }
    Ok(())
}

/// se.krka.kahlua.stdlib.StringLib.roundToPrecision
pub fn round_to_precision(x: f64, precision: i32) -> f64 {
    let rounding_offset = ipow(10, precision) as f64;
    round(x * rounding_offset) / rounding_offset
}

/// se.krka.kahlua.stdlib.StringLib.roundToSignificantNumbers
pub fn round_to_significant_numbers(x: f64, precision: i32) -> f64 {
    if x == 0.0 {
        return 0.0;
    }
    if x < 0.0 {
        return -round_to_significant_numbers(-x, precision);
    }
    let lower_limit = ipow(10, precision.wrapping_sub(1)) as f64;
    let upper_limit = lower_limit * 10.0;
    let mut multiplier = 1.0;
    while multiplier * x < lower_limit {
        multiplier *= 10.0;
    }
    while multiplier * x >= upper_limit {
        multiplier /= 10.0;
    }
    round(x * multiplier) / multiplier
}

/// se.krka.kahlua.stdlib.StringLib.appendPrecisionNumber
fn append_precision_number(
    buffer: &mut Vec<u16>,
    number: f64,
    precision: i32,
    require_period: bool,
) -> R<()> {
    let number = round_to_precision(number, precision);
    let i_part = number.floor();
    let mut f_part = number - i_part;
    for _ in 0..precision.max(0) {
        f_part *= 10.0;
    }
    f_part = round(i_part + f_part) - i_part;
    string_buffer_append(buffer, i_part, 10, true, 0)?;
    if require_period || precision > 0 {
        buffer.push(u('.'));
    }
    string_buffer_append(buffer, f_part, 10, false, precision)
}

/// se.krka.kahlua.stdlib.StringLib.appendSignificantNumber
fn append_significant_number(
    buffer: &mut Vec<u16>,
    number: f64,
    mut significant_decimals: i32,
    include_trailing_zeros: bool,
) -> R<()> {
    let i_part = number.floor();
    string_buffer_append(buffer, i_part, 10, true, 0)?;
    let mut f_part = round_to_significant_numbers(number - i_part, significant_decimals);
    let has_not_started = i_part == 0.0 && f_part != 0.0;
    let mut zero_padding_before = 0;
    let mut scan_length = significant_decimals;
    let mut i = 0;
    while i < scan_length {
        f_part *= 10.0;
        if f_part.floor() == 0.0 && f_part != 0.0 {
            zero_padding_before += 1;
            if has_not_started {
                scan_length += 1;
            }
        }
        i += 1;
    }
    f_part = round(f_part);
    if !include_trailing_zeros {
        while f_part > 0.0 && f_part % 10.0 == 0.0 {
            f_part /= 10.0;
            significant_decimals -= 1;
        }
    }
    buffer.push(u('.'));
    let period_pos = buffer.len();
    extend(buffer, zero_padding_before, u('0'))?;
    let pre_pos = buffer.len();
    string_buffer_append(buffer, f_part, 10, false, 0)?;
    let post_pos = buffer.len();
    let len = (post_pos - pre_pos) as i32;
    if include_trailing_zeros && len < significant_decimals {
        let pad_right_size = significant_decimals - len - zero_padding_before;
        extend(buffer, pad_right_size, u('0'))?;
    }
    if !include_trailing_zeros && period_pos == buffer.len() {
        buffer.truncate(period_pos - 1);
    }
    Ok(())
}

/// se.krka.kahlua.stdlib.StringLib.appendScientificNumber
fn append_scientific_number(
    buffer: &mut Vec<u16>,
    mut x: f64,
    precision: i32,
    repr: bool,
    use_significant_numbers: bool,
) -> R<()> {
    let mut exponent: i32 = 0;
    for _ in 0..2 {
        if x >= 1.0 {
            while x >= 10.0 {
                x /= 10.0;
                exponent += 1;
            }
        } else {
            while x > 0.0 && x < 1.0 {
                x *= 10.0;
                exponent -= 1;
            }
        }
        x = round_to_precision(x, precision);
    }
    let abs_exponent = exponent.wrapping_abs();
    let exp_sign = if exponent >= 0 { u('+') } else { u('-') };
    if use_significant_numbers {
        append_significant_number(buffer, x, precision, repr)?;
    } else {
        append_precision_number(buffer, x, precision, repr)?;
    }
    buffer.push(u('e'));
    buffer.push(exp_sign);
    string_buffer_append(buffer, f64::from(abs_exponent), 10, true, 2)
}

/// The sign handling shared by `%e`, `%f` and `%g`: returns the magnitude and whether it is infinite or NaN.
fn push_sign(result: &mut Vec<u16>, v: f64, show_plus: bool, space_for_sign: bool) -> (f64, bool) {
    let is_nan = v.is_infinite() || v.is_nan();
    let mut v_double = v;
    if is_negative(v_double) {
        if !is_nan {
            result.push(u('-'));
        }
        v_double = -v_double;
    } else if show_plus {
        result.push(u('+'));
    } else if space_for_sign {
        result.push(u(' '));
    }
    (v_double, is_nan)
}

/// se.krka.kahlua.stdlib.StringLib.format: Kahlua's own printf (not Java's Formatter).
fn format(cf: &FrameRef) -> R<i32> {
    let f = get_string_arg(cf, 1, NAMES[6])?;
    let len = f.len() as i32;
    let at = |i: i32| f[i as usize];
    let incomplete = "incomplete option to 'format'";
    let mut argc = 2;
    let mut result: Vec<u16> = Vec::new();
    let mut i = 0;
    while i < len {
        let mut c = at(i);
        if c != u('%') {
            result.push(c);
            i += 1;
            continue;
        }
        i += 1;
        lua_assert(i < len, incomplete)?;
        c = at(i);
        if c == u('%') {
            result.push(u('%'));
            i += 1;
            continue;
        }
        let mut repr = false;
        let mut zero_padding = false;
        let mut left_justify = false;
        let mut show_plus = false;
        let mut space_for_sign = false;
        loop {
            match c {
                0x20 => space_for_sign = true,
                0x23 => repr = true,
                0x2B => show_plus = true,
                0x2D => left_justify = true,
                0x30 => zero_padding = true,
                _ => break,
            }
            i += 1;
            lua_assert(i < len, incomplete)?;
            c = at(i);
        }
        let mut width: i32 = 0;
        while (u('0')..=u('9')).contains(&c) {
            width = width
                .wrapping_mul(10)
                .wrapping_add(i32::from(c))
                .wrapping_sub(48);
            i += 1;
            lua_assert(i < len, incomplete)?;
            c = at(i);
        }
        let mut precision: i32 = 0;
        let mut has_precision = false;
        if c == u('.') {
            has_precision = true;
            i += 1;
            lua_assert(i < len, incomplete)?;
            c = at(i);
            while (u('0')..=u('9')).contains(&c) {
                precision = precision
                    .wrapping_mul(10)
                    .wrapping_add(i32::from(c))
                    .wrapping_sub(48);
                i += 1;
                lua_assert(i < len, incomplete)?;
                c = at(i);
            }
        }
        if left_justify {
            zero_padding = false;
        }
        let mut base = 10;
        let mut upper_case = false;
        let mut default_precision = 6;
        let mut base_prepend: &[u16] = &[];
        const UPPER_X: &[u16] = &[b'0' as u16, b'X' as u16];
        const LOWER_X: &[u16] = &[b'0' as u16, b'x' as u16];
        const OCT: &[u16] = &[b'0' as u16];
        match char::from_u32(u32::from(c)).unwrap_or('\0') {
            'E' | 'G' => upper_case = true,
            'X' => {
                base = 16;
                default_precision = 1;
                upper_case = true;
                base_prepend = UPPER_X;
            }
            'c' | 's' => zero_padding = false,
            'd' | 'i' | 'u' => default_precision = 1,
            'e' | 'f' | 'g' => {}
            'o' => {
                base = 8;
                default_precision = 1;
                base_prepend = OCT;
            }
            'q' => width = 0,
            'x' => {
                base = 16;
                default_precision = 1;
                base_prepend = LOWER_X;
            }
            _ => {
                let mut msg = String::from("invalid option '%");
                msg.push_str(&String::from_utf16_lossy(&[c]));
                msg.push_str("' to 'format'");
                return Err(Throwable::runtime(msg));
            }
        }
        if !has_precision {
            precision = default_precision;
        }
        if has_precision && base != 10 {
            zero_padding = false;
        }
        let pad_character = if zero_padding { u('0') } else { u(' ') };
        let result_start_length = result.len();
        if !left_justify {
            extend(&mut result, width, pad_character)?;
        }
        match char::from_u32(u32::from(c)).unwrap_or('\0') {
            'E' | 'e' | 'f' => {
                let v = get_number_arg(cf, argc, NAMES[6])?;
                let (v_double, is_nan) = push_sign(&mut result, v, show_plus, space_for_sign);
                if is_nan {
                    result.extend(number_to_string(v).encode_utf16());
                } else if c == u('f') {
                    append_precision_number(&mut result, v_double, precision, repr)?;
                } else {
                    append_scientific_number(&mut result, v_double, precision, repr, false)?;
                }
            }
            'G' | 'g' => {
                if precision <= 0 {
                    precision = 1;
                }
                let v = get_number_arg(cf, argc, NAMES[6])?;
                let (v_double, is_nan) = push_sign(&mut result, v, show_plus, space_for_sign);
                if is_nan {
                    result.extend(number_to_string(v).encode_utf16());
                } else {
                    let x = round_to_significant_numbers(v_double, precision);
                    if x == 0.0 || x >= 1.0E-4 && x < ipow(10, precision) as f64 {
                        let i_part_size = if x == 0.0 {
                            1
                        } else if x.floor() == 0.0 {
                            0
                        } else {
                            let mut long_value = x;
                            let mut n = 1;
                            while long_value >= 10.0 {
                                long_value /= 10.0;
                                n += 1;
                            }
                            n
                        };
                        append_significant_number(&mut result, x, precision - i_part_size, repr)?;
                    } else {
                        append_scientific_number(&mut result, x, precision - 1, repr, true)?;
                    }
                }
            }
            'X' | 'o' | 'u' | 'x' => {
                let v_long = unsigned(get_number_arg(cf, argc, NAMES[6])? as i64);
                if repr {
                    if base == 8 {
                        let mut digits = 0;
                        let mut v2 = v_long;
                        while v2 > 0 {
                            v2 /= 8;
                            digits += 1;
                        }
                        if precision <= digits {
                            result.extend_from_slice(base_prepend);
                        }
                    } else if base == 16 && v_long != 0 {
                        result.extend_from_slice(base_prepend);
                    }
                }
                if v_long != 0 || precision > 0 {
                    string_buffer_append(&mut result, v_long as f64, base, false, precision)?;
                }
            }
            'c' => {
                // (char) Double.shortValue(): (short)(int)d.
                let v = get_number_arg(cf, argc, NAMES[6])?;
                result.push((v as i32) as i16 as u16);
            }
            'd' | 'i' => {
                let v = get_number_arg(cf, argc, NAMES[6])?;
                let mut v_long = v as i64;
                if v_long < 0 {
                    result.push(u('-'));
                    v_long = v_long.wrapping_neg();
                } else if show_plus {
                    result.push(u('+'));
                } else if space_for_sign {
                    result.push(u(' '));
                }
                if v_long != 0 || precision > 0 {
                    string_buffer_append(&mut result, v_long as f64, base, false, precision)?;
                }
            }
            'q' => {
                let q = get_string_arg(cf, argc, NAMES[6])?;
                result.push(u('"'));
                for &d in q.iter() {
                    match d {
                        0x0A => result.extend_from_slice(&[u('\\'), 0x0A]),
                        0x0D => result.extend_from_slice(&[u('\\'), u('r')]),
                        0x22 => result.extend_from_slice(&[u('\\'), u('"')]),
                        // The original appends "\\", a single backslash.
                        0x5C => result.push(u('\\')),
                        _ => result.push(d),
                    }
                }
                result.push(u('"'));
            }
            's' => {
                let s = get_string_arg(cf, argc, NAMES[6])?;
                let mut n = s.len() as i32;
                if has_precision {
                    n = precision.min(n);
                }
                result.extend_from_slice(&s[..n.max(0) as usize]);
            }
            _ => return Err(Throwable::runtime("Internal error")),
        }
        if left_justify {
            let d = width - (result.len() - result_start_length) as i32;
            if d > 0 {
                extend(&mut result, d, u(' '))?;
            }
        } else {
            let mut d = (result.len() - result_start_length) as i32 - width;
            d = d.min(width);
            if d > 0 {
                result.drain(result_start_length..result_start_length + d as usize);
            }
            if zero_padding {
                let sign_pos = result_start_length as i64 + i64::from(width) - i64::from(d);
                let ch = usize::try_from(sign_pos)
                    .ok()
                    .and_then(|p| result.get(p).copied())
                    .ok_or_else(|| sb_index_error(sign_pos as i32, result.len()))?;
                if ch == u('+') || ch == u('-') || ch == u(' ') {
                    result[sign_pos as usize] = u('0');
                    result[result_start_length] = ch;
                }
            }
        }
        if upper_case {
            for x in &mut result[result_start_length..] {
                if (u('a')..=u('z')).contains(x) {
                    *x -= 32;
                }
            }
        }
        argc += 1;
        i += 1;
    }
    cf.push(Object::String(result.into()))
}

// ---------------------------------------------------------------------------------------------------------------
// Lua patterns: find, match, gsub
// ---------------------------------------------------------------------------------------------------------------

/// se.krka.kahlua.stdlib.StringLib.StringPointer: a position in a string. The Java one is a pooled mutable
/// object that every method clones before changing it, so a copy is the same thing.
#[derive(Clone, Copy)]
struct StringPointer<'a> {
    string: &'a [u16],
    index: i32,
}

/// `String.charAt` out of range.
fn char_at_error(index: i64, length: usize) -> Throwable {
    Throwable::Java {
        class: "java.lang.StringIndexOutOfBoundsException",
        message: Some(format!("Index {index} out of bounds for length {length}")),
    }
}

/// `String.substring(begin, end)`.
fn substring(s: &[u16], begin: i32, end: i32) -> R<LuaStr> {
    if begin < 0 || end > s.len() as i32 || begin > end {
        return Err(Throwable::Java {
            class: "java.lang.StringIndexOutOfBoundsException",
            message: Some(format!("begin {begin}, end {end}, length {}", s.len())),
        });
    }
    Ok(s[begin as usize..end as usize].into())
}

impl<'a> StringPointer<'a> {
    fn new(string: &'a [u16]) -> Self {
        StringPointer { string, index: 0 }
    }

    /// StringPointer.getChar(int): NUL past the end, `charAt` (which throws) before the start.
    fn ch(&self, str_index: i32) -> R<u16> {
        let i = i64::from(self.index) + i64::from(str_index);
        if i >= self.string.len() as i64 {
            Ok(0)
        } else if i < 0 {
            Err(char_at_error(i, self.string.len()))
        } else {
            Ok(self.string[i as usize])
        }
    }

    /// StringPointer.length / getStringLength
    fn length(&self) -> i32 {
        self.string.len() as i32 - self.index
    }

    /// StringPointer.postIncrString: reads the current unit, then moves.
    fn post_incr(&mut self, num: i32) -> R<u16> {
        let c = self.ch(0)?;
        self.index += num;
        Ok(c)
    }

    /// StringPointer.getStringSubString
    fn sub_string(&self, len: i32) -> R<LuaStr> {
        substring(self.string, self.index, self.index.wrapping_add(len))
    }

    /// StringPointer.getString
    fn rest(&self) -> R<LuaStr> {
        substring(self.string, self.index, self.string.len() as i32)
    }

    /// StringPointer.compareTo
    fn compare_to(&self, cmp: &StringPointer, len: i32) -> R<i32> {
        for i in 0..len.max(0) {
            let val = i32::from(self.ch(i)?) - i32::from(cmp.ch(i)?);
            if val != 0 {
                return Ok(val);
            }
        }
        Ok(0)
    }
}

const CAP_UNFINISHED: i32 = -1;
const CAP_POSITION: i32 = -2;
const LUA_MAXCAPTURES: usize = 32;

/// se.krka.kahlua.stdlib.StringLib.MatchState.Capture
#[derive(Clone, Copy)]
struct Capture<'a> {
    init: Option<StringPointer<'a>>,
    len: i32,
}

/// se.krka.kahlua.stdlib.StringLib.MatchState. The original takes it from a static pool and never clears the
/// captures, so a back reference to a capture slot this call never opened reads what an earlier call left
/// there; here the slots start empty (that case is UNKNOWN).
struct MatchState<'a> {
    src_init: StringPointer<'a>,
    end_index: i32,
    capture: [Capture<'a>; LUA_MAXCAPTURES],
    level: i32,
}

impl<'a> MatchState<'a> {
    fn new(src_init: StringPointer<'a>, end_index: i32) -> Self {
        MatchState {
            src_init,
            end_index,
            capture: [Capture { init: None, len: 0 }; LUA_MAXCAPTURES],
            level: 0,
        }
    }

    /// `ms.capture[i]` (Java array bounds).
    fn cap(&self, i: i32) -> R<&Capture<'a>> {
        usize::try_from(i)
            .ok()
            .and_then(|i| self.capture.get(i))
            .ok_or_else(|| aioobe32(i))
    }

    fn cap_init(&self, i: i32) -> R<StringPointer<'a>> {
        self.cap(i)?.init.ok_or_else(|| {
            Throwable::npe(
                "Cannot invoke \"se.krka.kahlua.stdlib.StringLib$StringPointer.length()\" because \
                 \"this.capture[i].init\" is null",
            )
        })
    }

    /// MatchState.getCapture: a string, or the position for `()`; None past the open captures.
    fn get_capture(&self, i: i32) -> R<Option<Object>> {
        if i >= self.level {
            return Ok(None);
        }
        let c = self.cap(i)?;
        let init = self.cap_init(i)?;
        Ok(Some(if c.len == CAP_POSITION {
            Object::Number(f64::from(self.src_init.length() - init.length() + 1))
        } else {
            Object::String(init.sub_string(c.len)?)
        }))
    }
}

fn aioobe32(i: i32) -> Throwable {
    Throwable::Java {
        class: "java.lang.ArrayIndexOutOfBoundsException",
        message: Some(format!("Index {i} out of bounds for length 32")),
    }
}

/// se.krka.kahlua.stdlib.StringLib.push_onecapture
fn push_onecapture(
    cf: &FrameRef,
    ms: &MatchState,
    i: i32,
    s: Option<StringPointer>,
    e: Option<StringPointer>,
) -> R<()> {
    if i >= ms.level {
        if i == 0 {
            // s and e are set whenever level 0 gets here (push_captures).
            let (Some(s), Some(e)) = (s, e) else {
                return Err(Throwable::npe(
                    "Cannot read field \"string\" because \"s\" is null",
                ));
            };
            let res = substring(s.string, s.index, e.index)?;
            cf.push(Object::String(res))?;
            Ok(())
        } else {
            Err(Throwable::runtime("invalid capture index"))
        }
    } else {
        let l = ms.cap(i)?.len;
        if l == CAP_UNFINISHED {
            Err(Throwable::runtime("unfinished capture"))
        } else if l == CAP_POSITION {
            let init = ms.cap_init(i)?;
            cf.push(Object::Number(to_double(f64::from(
                ms.src_init.length() - init.length() + 1,
            ))))?;
            Ok(())
        } else {
            let init = ms.cap_init(i)?;
            cf.push(Object::String(substring(
                init.string,
                init.index,
                init.index.wrapping_add(l),
            )?))?;
            Ok(())
        }
    }
}

/// se.krka.kahlua.stdlib.StringLib.push_captures
fn push_captures(
    cf: &FrameRef,
    ms: &MatchState,
    s: Option<StringPointer>,
    e: Option<StringPointer>,
) -> R<i32> {
    let nlevels = if ms.level == 0 && s.is_some() {
        1
    } else {
        ms.level
    };
    lua_assert(nlevels <= 32, "too many captures")?;
    for i in 0..nlevels {
        push_onecapture(cf, ms, i, s, e)?;
    }
    Ok(nlevels)
}

/// se.krka.kahlua.stdlib.StringLib.noSpecialChars
fn no_special_chars(pattern: &[u16]) -> bool {
    !pattern
        .iter()
        .any(|&c| c < 256 && b"^$*+?.([%-".contains(&(c as u8)))
}

/// What `catch (Exception e) { throw new RuntimeException(e); }` makes of an exception (Errors pass).
fn wrap_runtime(e: Throwable) -> Throwable {
    let is_error = matches!(&e, Throwable::Java { class, .. } if class.ends_with("Error"));
    if is_error {
        return e;
    }
    let text = match e.get_message() {
        Some(m) => format!("{}: {m}", e.class_name()),
        None => e.class_name().to_string(),
    };
    Throwable::runtime(text)
}

/// se.krka.kahlua.stdlib.StringLib.findAux: find and match.
fn find_aux(cf: &FrameRef, find: bool) -> R<i32> {
    find_aux_inner(cf, find).map_err(wrap_runtime)
}

/// se.krka.kahlua.stdlib.StringLib.findAuxInner
fn find_aux_inner(cf: &FrameRef, find: bool) -> R<i32> {
    let f = if find { NAMES[7] } else { NAMES[8] };
    let source = get_string_arg(cf, 1, f)?;
    let pattern = get_string_arg(cf, 2, f)?;
    let i = get_optional_number_arg(cf, 3)?;
    let plain = bool_eval(&get_optional_arg(cf, 4));
    let len = source.len() as i32;
    let mut init = match i {
        None => 0,
        Some(d) => (d as i32).wrapping_sub(1),
    };
    if init < 0 {
        init += len;
        if init < 0 {
            init = 0;
        }
    } else if init > len {
        init = len;
    }
    if !find || !plain && !no_special_chars(&pattern) {
        let s = StringPointer::new(&source);
        let mut p = StringPointer::new(&pattern);
        let mut anchor = false;
        if p.ch(0)? == u('^') {
            anchor = true;
            p.post_incr(1)?;
        }
        let mut s1 = s;
        s1.post_incr(init)?;
        let mut ms = MatchState::new(s, s.length());
        loop {
            ms.level = 0;
            if let Some(res) = do_match(&mut ms, s1, p)? {
                if find {
                    let n = cf.push2(
                        Object::Number(f64::from(s.length() - s1.length() + 1)),
                        Object::Number(f64::from(s.length() - res.length())),
                    )?;
                    return Ok(n + push_captures(cf, &ms, None, None)?);
                }
                return push_captures(cf, &ms, Some(s1), Some(res));
            }
            let old = s1.index;
            s1.index += 1;
            if old >= ms.end_index || anchor {
                break;
            }
        }
    } else if let Some(pos) = index_of(&source, &pattern, init) {
        return cf.push2(
            Object::Number(to_double((pos + 1) as f64)),
            Object::Number(to_double((pos + pattern.len()) as f64)),
        );
    }
    cf.push_nil()
}

/// `String.indexOf(String, int)`.
fn index_of(s: &[u16], pat: &[u16], from: i32) -> Option<usize> {
    let from = from.max(0) as usize;
    if pat.is_empty() {
        return (from <= s.len()).then_some(from.min(s.len()));
    }
    if from >= s.len() {
        return None;
    }
    s[from..]
        .windows(pat.len())
        .position(|w| w == pat)
        .map(|p| p + from)
}

/// se.krka.kahlua.stdlib.StringLib.startCapture
fn start_capture<'a>(
    ms: &mut MatchState<'a>,
    s: StringPointer<'a>,
    p: StringPointer<'a>,
    what: i32,
) -> R<Option<StringPointer<'a>>> {
    let level = ms.level;
    lua_assert(level < 32, "too many captures")?;
    ms.capture[level as usize].init = Some(s);
    ms.capture[level as usize].len = what;
    ms.level = level + 1;
    let res = do_match(ms, s, p)?;
    if res.is_none() {
        ms.level -= 1;
    }
    Ok(res)
}

/// se.krka.kahlua.stdlib.StringLib.captureToClose
fn capture_to_close(ms: &MatchState) -> R<i32> {
    let mut level = ms.level - 1;
    while level >= 0 {
        if ms.capture[level as usize].len == CAP_UNFINISHED {
            return Ok(level);
        }
        level -= 1;
    }
    Err(Throwable::runtime("invalid pattern capture"))
}

/// se.krka.kahlua.stdlib.StringLib.endCapture
fn end_capture<'a>(
    ms: &mut MatchState<'a>,
    s: StringPointer<'a>,
    p: StringPointer<'a>,
) -> R<Option<StringPointer<'a>>> {
    let l = capture_to_close(ms)? as usize;
    let init = ms.cap_init(l as i32)?;
    ms.capture[l].len = init.length() - s.length();
    let res = do_match(ms, s, p)?;
    if res.is_none() {
        ms.capture[l].len = CAP_UNFINISHED;
    }
    Ok(res)
}

/// se.krka.kahlua.stdlib.StringLib.checkCapture. The original's assertion is the reverse of Lua's: it
/// rejects a closed capture and lets an unfinished, absent or `%0` one through.
fn check_capture(ms: &MatchState, l: u16) -> R<i32> {
    let l = i32::from(l) - 49;
    let ok = l < 0 || l >= ms.level || ms.cap(l)?.len == CAP_UNFINISHED;
    lua_assert(ok, "invalid capture index")?;
    Ok(l)
}

/// se.krka.kahlua.stdlib.StringLib.matchCapture
fn match_capture<'a>(
    ms: &MatchState<'a>,
    s: StringPointer<'a>,
    l: u16,
) -> R<Option<StringPointer<'a>>> {
    let l = check_capture(ms, l)?;
    let len = ms.cap(l)?.len;
    if ms.end_index - s.length() >= len {
        let init = ms.cap(l)?.init.ok_or_else(|| {
            Throwable::npe(
                "Cannot invoke \"se.krka.kahlua.stdlib.StringLib$StringPointer.compareTo(\
                 se.krka.kahlua.stdlib.StringLib$StringPointer, int)\" because \"ms.capture[l].init\" is null",
            )
        })?;
        if init.compare_to(&s, len)? == 0 {
            let mut sp = s;
            sp.post_incr(len)?;
            return Ok(Some(sp));
        }
    }
    Ok(None)
}

/// se.krka.kahlua.stdlib.StringLib.matchBalance
fn match_balance<'a>(
    ms: &MatchState<'a>,
    ss: StringPointer<'a>,
    p: StringPointer<'a>,
) -> R<Option<StringPointer<'a>>> {
    lua_assert(p.ch(0)? != 0 && p.ch(1)? != 0, "unbalanced pattern")?;
    let mut s = ss;
    if s.ch(0)? != p.ch(0)? {
        return Ok(None);
    }
    let b = p.ch(0)?;
    let e = p.ch(1)?;
    let mut cont = 1;
    loop {
        s.index += 1;
        if s.index >= ms.end_index {
            break;
        }
        if s.ch(0)? == e {
            cont -= 1;
            if cont == 0 {
                let mut sp = s;
                sp.post_incr(1)?;
                return Ok(Some(sp));
            }
        } else if s.ch(0)? == b {
            cont += 1;
        }
    }
    Ok(None)
}

/// se.krka.kahlua.stdlib.StringLib.classEnd
fn class_end(pp: StringPointer) -> R<StringPointer> {
    let mut p = pp;
    match p.post_incr(1)? {
        0x25 => {
            lua_assert(p.ch(0)? != 0, "malformed pattern (ends with '%')")?;
            p.post_incr(1)?;
            Ok(p)
        }
        0x5B => {
            if p.ch(0)? == u('^') {
                p.post_incr(1)?;
            }
            loop {
                lua_assert(p.ch(0)? != 0, "malformed pattern (missing ']')")?;
                if p.post_incr(1)? == u('%') && p.ch(0)? != 0 {
                    p.post_incr(1)?;
                }
                if p.ch(0)? == u(']') {
                    break;
                }
            }
            p.post_incr(1)?;
            Ok(p)
        }
        _ => Ok(p),
    }
}

/// se.krka.kahlua.stdlib.StringLib.singleMatch
fn single_match(c: u16, p: StringPointer, ep: StringPointer) -> R<bool> {
    match p.ch(0)? {
        0x25 => Ok(match_class(p.ch(1)?, c)),
        0x2E => Ok(true),
        0x5B => {
            let mut sp = ep;
            sp.post_incr(-1)?;
            match_bracket_class(c, p, sp)
        }
        pc => Ok(pc == c),
    }
}

/// se.krka.kahlua.stdlib.StringLib.minExpand
fn min_expand<'a>(
    ms: &mut MatchState<'a>,
    ss: StringPointer<'a>,
    p: StringPointer<'a>,
    ep: StringPointer<'a>,
) -> R<Option<StringPointer<'a>>> {
    let mut sp = ep;
    let mut s = ss;
    sp.post_incr(1)?;
    loop {
        if let Some(res) = do_match(ms, s, sp)? {
            return Ok(Some(res));
        }
        if s.index >= ms.end_index || !single_match(s.ch(0)?, p, ep)? {
            return Ok(None);
        }
        s.post_incr(1)?;
    }
}

/// se.krka.kahlua.stdlib.StringLib.maxExpand
fn max_expand<'a>(
    ms: &mut MatchState<'a>,
    s: StringPointer<'a>,
    p: StringPointer<'a>,
    ep: StringPointer<'a>,
) -> R<Option<StringPointer<'a>>> {
    let mut i = 0;
    while s.index + i < ms.end_index && single_match(s.ch(i)?, p, ep)? {
        i += 1;
    }
    while i >= 0 {
        let mut sp1 = s;
        sp1.post_incr(i)?;
        let mut sp2 = ep;
        sp2.post_incr(1)?;
        if let Some(res) = do_match(ms, sp1, sp2)? {
            return Ok(Some(res));
        }
        i -= 1;
    }
    Ok(None)
}

/// se.krka.kahlua.stdlib.StringLib.matchBracketClass
fn match_bracket_class(c: u16, pp: StringPointer, ecc: StringPointer) -> R<bool> {
    let mut p = pp;
    let ec = ecc;
    let mut sig = true;
    if p.ch(1)? == u('^') {
        sig = false;
        p.post_incr(1)?;
    }
    loop {
        p.index += 1;
        if p.index >= ec.index {
            break;
        }
        if p.ch(0)? == u('%') {
            p.post_incr(1)?;
            if match_class(p.ch(0)?, c) {
                return Ok(sig);
            }
        } else if p.ch(1)? == u('-') && p.index + 2 < ec.index {
            p.post_incr(2)?;
            if p.ch(-2)? <= c && c <= p.ch(0)? {
                return Ok(sig);
            }
        } else if p.ch(0)? == c {
            return Ok(sig);
        }
    }
    Ok(!sig)
}

/// se.krka.kahlua.stdlib.StringLib.match
fn do_match<'a>(
    ms: &mut MatchState<'a>,
    ss: StringPointer<'a>,
    pp: StringPointer<'a>,
) -> R<Option<StringPointer<'a>>> {
    let mut s = ss;
    let mut p = pp;
    loop {
        match p.ch(0)? {
            0 => return Ok(Some(s)),
            0x24 if p.ch(1)? == 0 => {
                return Ok(if s.index == ms.end_index {
                    Some(s)
                } else {
                    None
                });
            }
            0x25 => match p.ch(1)? {
                0x62 => {
                    let mut p1 = p;
                    p1.post_incr(2)?;
                    match match_balance(ms, s, p1)? {
                        None => return Ok(None),
                        Some(x) => s = x,
                    }
                    p.post_incr(4)?;
                    continue;
                }
                0x66 => {
                    p.post_incr(2)?;
                    lua_assert(p.ch(0)? == u('['), "missing '[' after '%%f' in pattern")?;
                    let ep = class_end(p)?;
                    let previous = if s.index == ms.src_init.index {
                        0
                    } else {
                        s.ch(-1)?
                    };
                    let mut ep1 = ep;
                    ep1.post_incr(-1)?;
                    if !match_bracket_class(previous, p, ep1)?
                        && match_bracket_class(s.ch(0)?, p, ep1)?
                    {
                        p = ep;
                        continue;
                    }
                    return Ok(None);
                }
                d if java_chars::is_digit(d) => {
                    match match_capture(ms, s, d)? {
                        None => return Ok(None),
                        Some(x) => s = x,
                    }
                    p.post_incr(2)?;
                    continue;
                }
                _ => {}
            },
            0x28 => {
                let mut p1 = p;
                if p.ch(1)? == u(')') {
                    p1.post_incr(2)?;
                    return start_capture(ms, s, p1, CAP_POSITION);
                }
                p1.post_incr(1)?;
                return start_capture(ms, s, p1, CAP_UNFINISHED);
            }
            0x29 => {
                let mut p1 = p;
                p1.post_incr(1)?;
                return end_capture(ms, s, p1);
            }
            _ => {}
        }
        // The default case: a single character class and its repetition suffix.
        let mut ep = class_end(p)?;
        let m = s.index < ms.end_index && single_match(s.ch(0)?, p, ep)?;
        match ep.ch(0)? {
            0x2A => return max_expand(ms, s, p, ep),
            0x2B => {
                let mut s1 = s;
                s1.post_incr(1)?;
                return if m {
                    max_expand(ms, s1, p, ep)
                } else {
                    Ok(None)
                };
            }
            0x2D => return min_expand(ms, s, p, ep),
            0x3F => {
                let mut s1 = s;
                s1.post_incr(1)?;
                let mut ep1 = ep;
                ep1.post_incr(1)?;
                if m {
                    if let Some(res) = do_match(ms, s1, ep1)? {
                        return Ok(Some(res));
                    }
                }
                // `p = ep; ep.postIncrString(1)`: the same object, so p moves too.
                ep.post_incr(1)?;
                p = ep;
            }
            _ => {
                if !m {
                    return Ok(None);
                }
                s.post_incr(1)?;
                p = ep;
            }
        }
    }
}

/// se.krka.kahlua.stdlib.StringLib.matchClass. Only an ASCII letter lower-cases to a class letter (the two
/// non-ASCII units that lower-case to ASCII, U+0130 and U+212A, give `i` and `k`, which are not classes).
fn match_class(class_identifier: u16, c: u16) -> bool {
    let lower = if (u('A')..=u('Z')).contains(&class_identifier) {
        class_identifier + 32
    } else {
        class_identifier
    };
    let res = match char::from_u32(u32::from(lower)).unwrap_or('\0') {
        'a' => java_chars::is_lower_case(c) || java_chars::is_upper_case(c),
        'c' => c <= 31 || c == 127,
        'd' => java_chars::is_digit(c),
        'l' => java_chars::is_lower_case(c),
        'p' => {
            (u('!')..=u('/')).contains(&c)
                || (u(':')..=u('@')).contains(&c)
                || (u('[')..=u('`')).contains(&c)
                || (u('{')..=u('~')).contains(&c)
        }
        's' => (0x09..=0x0D).contains(&c) || c == u(' '),
        'u' => java_chars::is_upper_case(c),
        'w' => {
            java_chars::is_lower_case(c) || java_chars::is_upper_case(c) || java_chars::is_digit(c)
        }
        'x' => c < 128 && (c as u8).is_ascii_hexdigit(),
        'z' => c == 0,
        _ => return class_identifier == c,
    };
    (lower == class_identifier) == res
}

/// se.krka.kahlua.stdlib.StringLib.gsub
fn gsub(thread: &mut KahluaThread, cf: &FrameRef) -> R<i32> {
    gsub_inner(thread, cf).map_err(wrap_runtime)
}

fn gsub_inner(thread: &mut KahluaThread, cf: &FrameRef) -> R<i32> {
    let src_temp = get_string_arg(cf, 1, NAMES[9])?;
    let p_temp = get_string_arg(cf, 2, NAMES[9])?;
    let mut repl = get_arg(cf, 3, NAMES[9])?;
    if let Some(tmp) = raw_tostring(&repl) {
        repl = Object::String(tmp);
    }
    let max_substitutions = get_optional_number_arg(cf, 4)?.map_or(i32::MAX, |d| d as i32);
    let mut pattern = StringPointer::new(&p_temp);
    let mut src = StringPointer::new(&src_temp);
    let mut anchor = false;
    if pattern.ch(0)? == u('^') {
        anchor = true;
        pattern.post_incr(1)?;
    }
    if !matches!(
        repl,
        Object::Number(_)
            | Object::String(_)
            | Object::Closure(_)
            | Object::Function(_)
            | Object::Table(_)
    ) {
        return Err(fail(&format!(
            "string/function/table expected, got {}",
            repl.java_to_string()
        )));
    }
    let mut ms = MatchState::new(src, src.length());
    let mut n = 0;
    let mut b: Vec<u16> = Vec::new();
    while n < max_substitutions {
        ms.level = 0;
        let e = do_match(&mut ms, src, pattern)?;
        if let Some(e) = e {
            n += 1;
            add_value(thread, &ms, &repl, &mut b, src, e)?;
        }
        match e {
            Some(e) if e.index > src.index => src.index = e.index,
            _ => {
                if src.index >= ms.end_index {
                    break;
                }
                b.push(src.post_incr(1)?);
            }
        }
        if anchor {
            break;
        }
    }
    b.extend_from_slice(&src.rest()?);
    cf.push2(Object::String(b.into()), Object::Number(f64::from(n)))
}

/// se.krka.kahlua.stdlib.StringLib.addValue
fn add_value(
    thread: &mut KahluaThread,
    ms: &MatchState,
    repl: &Object,
    b: &mut Vec<u16>,
    src: StringPointer,
    e: StringPointer,
) -> R<()> {
    if let Some(repl_string) = raw_tostring(repl) {
        let s = add_string(ms, &repl_string, src, e)?;
        b.extend_from_slice(&s);
        return Ok(());
    }
    let matched: LuaStr = match ms.get_capture(0)? {
        Some(captures) => raw_tostring(&captures).unwrap_or_else(|| lstr("null")),
        None => src.sub_string(e.index - src.index)?,
    };
    let mut res = match repl {
        Object::Table(t) => KahluaTableImpl::rawget(t, &Object::String(matched.clone()))?,
        _ => thread.call3(
            repl,
            Object::String(matched.clone()),
            Object::Nil,
            Object::Nil,
        )?,
    };
    if res.is_nil() {
        res = Object::String(matched);
    }
    // StringBuilder.append((String) null) appends "null".
    match raw_tostring(&res) {
        Some(s) => b.extend_from_slice(&s),
        None => b.extend("null".encode_utf16()),
    }
    Ok(())
}

/// se.krka.kahlua.stdlib.StringLib.addString: `%0`..`%9` in a replacement string.
fn add_string(ms: &MatchState, repl: &[u16], s: StringPointer, e: StringPointer) -> R<Vec<u16>> {
    let repl_str = StringPointer::new(repl);
    let mut buf = Vec::new();
    let mut i = 0;
    while i < repl.len() as i32 {
        let mut c = repl_str.ch(i)?;
        if c != u('%') {
            buf.push(c);
        } else {
            i += 1;
            c = repl_str.ch(i)?;
            if !java_chars::is_digit(c) {
                buf.push(c);
            } else if c == u('0') {
                let len = s.length() - e.length();
                buf.extend_from_slice(&s.sub_string(len)?);
            } else {
                match ms.get_capture(i32::from(c) - 49)? {
                    None => return Err(Throwable::Kahlua(Object::str("invalid capture index"))),
                    Some(o) => buf.extend_from_slice(&tostring(&o, None)?),
                }
            }
        }
        i += 1;
    }
    Ok(buf)
}

// ---------------------------------------------------------------------------------------------------------------
// string.split
// ---------------------------------------------------------------------------------------------------------------

/// The separator `String.split(regex)` finds when the regular expression is a literal: no metacharacter, or a
/// backslash before a non-alphanumeric unit or before one of Java's control escapes. None for a real regular
/// expression (PLACEHOLDER: java.util.regex is not ported).
fn literal_regex(regex: &[u16]) -> Option<Vec<u16>> {
    const META: &[u8] = b".$|()[{^?*+\\";
    if !regex.iter().any(|&c| c < 128 && META.contains(&(c as u8))) {
        return Some(regex.to_vec());
    }
    if regex.len() == 2 && regex[0] == u('\\') {
        let ch = regex[1];
        if ch < 128 && (ch as u8).is_ascii_alphanumeric() {
            let control = match ch as u8 {
                b't' => 0x09,
                b'n' => 0x0A,
                b'r' => 0x0D,
                b'f' => 0x0C,
                b'a' => 0x07,
                b'e' => 0x1B,
                _ => return None,
            };
            return Some(vec![control]);
        }
        if !(0xD800..=0xDFFF).contains(&ch) {
            return Some(vec![ch]);
        }
    }
    None
}

/// `String.split(regex)` (limit 0) for a literal separator: no leading empty piece for an empty match at the
/// start, trailing empty pieces dropped, the whole string when nothing matches.
fn split_literal(input: &[u16], sep: &[u16]) -> Vec<LuaStr> {
    let mut list: Vec<LuaStr> = Vec::new();
    let mut index = 0usize;
    let mut from = 0usize;
    let mut last_match: Option<usize> = None;
    loop {
        let start = if sep.is_empty() {
            let mut next = from;
            if last_match == Some(next) {
                next += 1;
            }
            if next > input.len() {
                None
            } else {
                Some(next)
            }
        } else if from > input.len() {
            None
        } else {
            input[from..]
                .windows(sep.len())
                .position(|w| w == sep)
                .map(|p| p + from)
        };
        let Some(start) = start else { break };
        let end = start + sep.len();
        last_match = Some(start);
        from = end;
        if index == 0 && start == 0 && start == end {
            continue;
        }
        list.push(input[index..start].into());
        index = end;
    }
    if index == 0 {
        return vec![input.into()];
    }
    list.push(input[index..].into());
    while list.last().is_some_and(|s| s.is_empty()) {
        list.pop();
    }
    list
}

/// se.krka.kahlua.stdlib.StringLib.split: `String.split` into a new table, pieces at 1..n.
fn split(cf: &FrameRef) -> R<i32> {
    let src_temp = get_string_arg(cf, 1, NAMES[11])?;
    let src_temp2 = get_string_arg(cf, 2, NAMES[11])?;
    let Some(sep) = literal_regex(&src_temp2) else {
        return Err(Throwable::runtime(format!(
            "string.split with the regular expression {} is not ported",
            rstr(&src_temp2)
        )));
    };
    let t = KahluaTableImpl::new_ref();
    for (n, piece) in split_literal(&src_temp, &sep).into_iter().enumerate() {
        KahluaTableImpl::rawset_int(&t, n as i32 + 1, Object::String(piece));
    }
    cf.push(Object::Table(t))
}
