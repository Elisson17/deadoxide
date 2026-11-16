//! org.json.JSONObject (org.json 20250517): the tokener constructor, `stringToValue`/`stringToNumber`, the `opt*`
//! accessors `LanguageFile` uses, and the values `toMap()` hands to Java code.
use super::json_tokener::JsonTokener;
use super::JsonException;

/// A JSON value as org.json holds it: `String`, `Boolean`, `JSONObject.NULL`, a `Number`, a `JSONObject` or a
/// `JSONArray`.
#[derive(Clone, Debug, PartialEq)]
pub enum JsonValue {
    Str(String),
    Bool(bool),
    Null,
    Number(JsonNumber),
    Object(JsonObject),
    Array(Vec<JsonValue>),
}

/// The `Number` classes `JSONObject.stringToNumber` returns.
#[derive(Clone, Debug, PartialEq)]
pub enum JsonNumber {
    /// java.lang.Integer
    Integer(i32),
    /// java.lang.Long
    Long(i64),
    /// java.math.BigInteger: its decimal digits with an optional '-'
    BigInteger(String),
    /// java.math.BigDecimal: unscaled value (digits without leading zeros, optional '-') and scale
    BigDecimal { unscaled: String, scale: i32 },
    /// java.lang.Double
    Double(f64),
}

/// org.json.JSONObject: the key/value pairs in the order the text gives them (the original keeps a HashMap; keys are
/// unique, a duplicate key is an error).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JsonObject {
    entries: Vec<(String, JsonValue)>,
    /// key -> position in `entries` (the HashMap lookup of the original)
    index: std::collections::HashMap<String, usize>,
}

impl JsonObject {
    /// org.json.JSONObject(String): `new JSONObject(new JSONTokener(source))` with the default configuration (not
    /// strict, duplicate keys an error); text after the closing brace is ignored.
    pub fn parse_text(source: &str) -> Result<JsonObject, JsonException> {
        parse(&mut JsonTokener::new(source))
    }

    /// The pairs in the text's order.
    pub fn entries(&self) -> &[(String, JsonValue)] {
        &self.entries
    }

    /// The pairs in the text's order, by value.
    pub fn into_entries(self) -> Vec<(String, JsonValue)> {
        self.entries
    }

    /// org.json.JSONObject.opt: the value of `key` (`None` where Java returns null).
    pub fn opt(&self, key: &str) -> Option<&JsonValue> {
        self.index.get(key).map(|&i| &self.entries[i].1)
    }

    /// org.json.JSONObject.optInt(key, default): `optNumber(key, null)`, then `intValue()`.
    pub fn opt_int(&self, key: &str, default: i32) -> i32 {
        match self.opt_number(key) {
            Some(n) => int_value(&n),
            None => default,
        }
    }

    /// org.json.JSONObject.optNumber(key, null): a number as it is, anything else (but null) through
    /// `stringToNumber(val.toString())`, `None` when that fails.
    pub fn opt_number(&self, key: &str) -> Option<JsonNumber> {
        match self.opt(key) {
            None | Some(JsonValue::Null) => None,
            Some(JsonValue::Number(n)) => Some(n.clone()),
            Some(v) => string_to_number(&to_java_string(v)?).ok(),
        }
    }

    /// org.json.JSONObject.optString(key, default): `toString()` of anything but null.
    pub fn opt_string(&self, key: &str, default: Option<&str>) -> Option<String> {
        match self.opt(key) {
            None | Some(JsonValue::Null) => default.map(str::to_string),
            Some(v) => to_java_string(v),
        }
    }

    /// org.json.JSONObject.optBoolean(key, default): a Boolean, else `getBoolean` (the strings "true"/"false" in any
    /// case), else the default.
    pub fn opt_boolean(&self, key: &str, default: bool) -> bool {
        match self.opt(key) {
            None | Some(JsonValue::Null) => default,
            Some(JsonValue::Bool(b)) => *b,
            Some(JsonValue::Str(s)) if crate::java::equals_ignore_case(s, "false") => false,
            Some(JsonValue::Str(s)) if crate::java::equals_ignore_case(s, "true") => true,
            Some(_) => default,
        }
    }
}

/// `Object.toString()` of a value: `None` for objects and arrays (their JSON text is not ported).
pub fn to_java_string(v: &JsonValue) -> Option<String> {
    match v {
        JsonValue::Str(s) => Some(s.clone()),
        JsonValue::Bool(b) => Some(b.to_string()),
        JsonValue::Null => Some("null".into()),
        JsonValue::Number(n) => Some(number_to_string(n)),
        JsonValue::Object(_) | JsonValue::Array(_) => None,
    }
}

/// `toString()` of the Number classes: Integer/Long/BigInteger in decimal, `BigDecimal.toString` (scientific when
/// the scale is negative or the adjusted exponent below -6), `Double.toString`.
pub fn number_to_string(n: &JsonNumber) -> String {
    match n {
        JsonNumber::Integer(v) => v.to_string(),
        JsonNumber::Long(v) => v.to_string(),
        JsonNumber::BigInteger(s) => s.clone(),
        JsonNumber::BigDecimal { unscaled, scale } => big_decimal_to_string(unscaled, *scale),
        JsonNumber::Double(d) => crate::java::double_to_string(*d),
    }
}

/// java.math.BigDecimal.toString (`layoutChars(true)`).
fn big_decimal_to_string(unscaled: &str, scale: i32) -> String {
    let (neg, coeff) = match unscaled.strip_prefix('-') {
        Some(c) => (true, c),
        None => (false, unscaled),
    };
    let mut buf = String::new();
    if neg {
        buf.push('-');
    }
    if scale == 0 {
        buf.push_str(coeff);
        return buf;
    }
    let len = coeff.len() as i64;
    let adjusted = -i64::from(scale) + (len - 1);
    if scale >= 0 && adjusted >= -6 {
        let pad = i64::from(scale) - len;
        if pad >= 0 {
            buf.push_str("0.");
            for _ in 0..pad {
                buf.push('0');
            }
            buf.push_str(coeff);
        } else {
            let cut = (-pad) as usize;
            buf.push_str(&coeff[..cut]);
            buf.push('.');
            buf.push_str(&coeff[cut..]);
        }
    } else {
        buf.push_str(&coeff[..1]);
        if coeff.len() > 1 {
            buf.push('.');
            buf.push_str(&coeff[1..]);
        }
        if adjusted != 0 {
            buf.push('E');
            if adjusted > 0 {
                buf.push('+');
            }
            buf.push_str(&adjusted.to_string());
        }
    }
    buf
}

/// `Number.intValue()`: Integer as it is, Long and BigInteger keep their low 32 bits, BigDecimal drops its fraction
/// first, Double is cast (saturating, NaN 0).
pub fn int_value(n: &JsonNumber) -> i32 {
    fn low_bits(digits: &str, extra_zeros: i64) -> i32 {
        let (neg, d) = match digits.strip_prefix('-') {
            Some(d) => (true, d),
            None => (false, digits),
        };
        let mut v: u32 = 0;
        for b in d.bytes() {
            v = v.wrapping_mul(10).wrapping_add(u32::from(b - b'0'));
        }
        for _ in 0..extra_zeros.clamp(0, 64) {
            v = v.wrapping_mul(10);
        }
        let v = v as i32;
        if neg {
            v.wrapping_neg()
        } else {
            v
        }
    }
    match n {
        JsonNumber::Integer(v) => *v,
        JsonNumber::Long(v) => *v as i32,
        JsonNumber::BigInteger(s) => low_bits(s, 0),
        JsonNumber::BigDecimal { unscaled, scale } => {
            let digits = unscaled.trim_start_matches('-');
            let keep = digits.len() as i64 - i64::from(*scale);
            if keep <= 0 {
                0
            } else if *scale >= 0 {
                let cut = keep as usize;
                let sign = if unscaled.starts_with('-') { "-" } else { "" };
                low_bits(&format!("{sign}{}", &digits[..cut]), 0)
            } else {
                low_bits(unscaled, -i64::from(*scale))
            }
        }
        JsonNumber::Double(d) => *d as i32,
    }
}

/// org.json.JSONObject(JSONTokener, JSONParserConfiguration), not strict: `{`, then `key : value` pairs separated
/// by `,` or `;` (a separator right before `}` is accepted), keys through `nextSimpleValue(c).toString()`, a key
/// seen before is "Duplicate key".
pub fn parse(x: &mut JsonTokener) -> Result<JsonObject, JsonException> {
    let mut obj = JsonObject::default();
    if x.next_clean() != u16::from(b'{') {
        return Err(x.syntax_error("A JSONObject text must begin with '{'"));
    }
    loop {
        let c = x.next_clean();
        match c {
            0 => return Err(x.syntax_error("A JSONObject text must end with '}'")),
            125 => return Ok(obj),
            _ => {}
        }
        let key_value = x.next_simple_value(c)?;
        let key = to_java_string(&key_value).unwrap_or_default();
        if x.next_clean() != u16::from(b':') {
            return Err(x.syntax_error("Expected a ':' after a key"));
        }
        if obj.opt(&key).is_some() {
            return Err(x.syntax_error(&format!("Duplicate key \"{key}\"")));
        }
        let value = x.next_value()?;
        obj.index.insert(key.clone(), obj.entries.len());
        obj.entries.push((key, value));
        match x.next_clean() {
            59 | 44 => {
                if x.next_clean() == u16::from(b'}') {
                    return Ok(obj);
                }
                if x.end() {
                    return Err(x.syntax_error("A JSONObject text must end with '}'"));
                }
                x.back()?;
            }
            125 => return Ok(obj),
            _ => return Err(x.syntax_error("Expected a ',' or '}'")),
        }
    }
}

/// org.json.JSONObject.stringToValue: "true"/"false"/"null" in any case, a number when the text starts with a
/// digit or '-' and parses as one, else the text itself.
pub fn string_to_value(string: &str) -> JsonValue {
    if string.is_empty() {
        return JsonValue::Str(String::new());
    }
    if crate::java::equals_ignore_case(string, "true") {
        return JsonValue::Bool(true);
    }
    if crate::java::equals_ignore_case(string, "false") {
        return JsonValue::Bool(false);
    }
    if crate::java::equals_ignore_case(string, "null") {
        return JsonValue::Null;
    }
    let initial = string.as_bytes()[0];
    if initial.is_ascii_digit() || initial == b'-' {
        if let Ok(n) = string_to_number(string) {
            return JsonValue::Number(n);
        }
    }
    JsonValue::Str(string.to_string())
}

/// org.json.JSONObject.isDecimalNotation.
fn is_decimal_notation(val: &str) -> bool {
    val.contains('.') || val.contains('e') || val.contains('E') || val == "-0"
}

/// java.lang.NumberFormatException out of `stringToNumber`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumberFormatException;

/// org.json.JSONObject.stringToNumber: decimal notation through `new BigDecimal` (a negative zero becomes the
/// Double -0.0), falling back to `Double.valueOf` (finite only); integers (no leading zeros) through `new
/// BigInteger`, narrowed to Integer or Long by bit length. `Err` is the NumberFormatException. Digits are ASCII
/// here (Java's BigDecimal/BigInteger also accept other Unicode digits: UNKNOWN); hexadecimal `Double.valueOf`
/// input ("0x1.8p1") is not ported (`java::parse_double`), so such a text stays a string (UNKNOWN).
pub fn string_to_number(val: &str) -> Result<JsonNumber, NumberFormatException> {
    let b = val.as_bytes();
    let initial = *b.first().ok_or(NumberFormatException)?;
    if !initial.is_ascii_digit() && initial != b'-' {
        return Err(NumberFormatException);
    }
    if is_decimal_notation(val) {
        if let Some((unscaled, scale)) = parse_big_decimal(val) {
            let zero = unscaled.trim_start_matches('-') == "0";
            if initial == b'-' && zero {
                return Ok(JsonNumber::Double(-0.0));
            }
            return Ok(JsonNumber::BigDecimal { unscaled, scale });
        }
        return match crate::java::parse_double(val) {
            Some(d) if d.is_finite() => Ok(JsonNumber::Double(d)),
            _ => Err(NumberFormatException),
        };
    }
    if initial == b'0' && b.len() > 1 {
        if b[1].is_ascii_digit() {
            return Err(NumberFormatException);
        }
    } else if initial == b'-' && b.len() > 2 && b[1] == b'0' && b[2].is_ascii_digit() {
        return Err(NumberFormatException);
    }
    let (neg, digits) = match val.strip_prefix('-') {
        Some(d) => (true, d),
        None => (false, val.strip_prefix('+').unwrap_or(val)),
    };
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return Err(NumberFormatException);
    }
    let trimmed = digits.trim_start_matches('0');
    let magnitude = if trimmed.is_empty() { "0" } else { trimmed };
    let text = if neg && magnitude != "0" {
        format!("-{magnitude}")
    } else {
        magnitude.to_string()
    };
    // bitLength() <= 31: Integer; <= 63: Long; else BigInteger.
    match text.parse::<i64>() {
        Ok(v) if i32::try_from(v).is_ok() => Ok(JsonNumber::Integer(v as i32)),
        Ok(v) => Ok(JsonNumber::Long(v)),
        Err(_) if text == "-9223372036854775808" => Ok(JsonNumber::Long(i64::MIN)),
        Err(_) => Ok(JsonNumber::BigInteger(text)),
    }
}

/// java.math.BigDecimal(String): optional sign, digits with at most one point (at least one digit), optional
/// exponent `e|E[sign]digits`; the unscaled value (leading zeros dropped) and the scale (fraction digits minus the
/// exponent, within int range). `None` is the NumberFormatException.
fn parse_big_decimal(val: &str) -> Option<(String, i32)> {
    let b = val.as_bytes();
    let mut i = 0;
    let mut neg = false;
    if let Some(&s) = b.first() {
        if s == b'-' || s == b'+' {
            neg = s == b'-';
            i = 1;
        }
    }
    let mut digits = String::new();
    let mut frac: i64 = 0;
    let mut point = false;
    while i < b.len() {
        match b[i] {
            c @ b'0'..=b'9' => {
                digits.push(c as char);
                if point {
                    frac += 1;
                }
            }
            b'.' if !point => point = true,
            _ => break,
        }
        i += 1;
    }
    if digits.is_empty() {
        return None;
    }
    let mut exp: i64 = 0;
    if i < b.len() {
        if b[i] != b'e' && b[i] != b'E' {
            return None;
        }
        i += 1;
        let mut eneg = false;
        if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
            eneg = b[i] == b'-';
            i += 1;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            exp = exp
                .saturating_mul(10)
                .saturating_add(i64::from(b[i] - b'0'));
            i += 1;
        }
        if i == start || i != b.len() {
            return None;
        }
        if eneg {
            exp = -exp;
        }
    }
    let scale = frac - exp;
    let scale = i32::try_from(scale).ok()?;
    let trimmed = digits.trim_start_matches('0');
    let magnitude = if trimmed.is_empty() { "0" } else { trimmed };
    let unscaled = if neg && magnitude != "0" {
        format!("-{magnitude}")
    } else {
        magnitude.to_string()
    };
    Some((unscaled, scale))
}
