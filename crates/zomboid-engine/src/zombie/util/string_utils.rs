use crate::java;

/// zombie.util.StringUtils.isNullOrWhitespace: `None` is Java null. The original's two-ended scan is kept as written.
pub fn is_null_or_whitespace(s: Option<&str>) -> bool {
    match s {
        None => true,
        Some("") => true,
        Some(s) => is_whitespace(s),
    }
}

/// zombie.util.StringUtils.isWhitespace.
fn is_whitespace(s: &str) -> bool {
    let c: Vec<char> = s.chars().collect();
    let length = c.len();
    if length == 0 {
        return false;
    }
    let (mut start, middle, mut end) = (0usize, length / 2, length - 1);
    while start <= middle {
        if !java::is_whitespace(c[start]) || !java::is_whitespace(c[end]) {
            return false;
        }
        start += 1;
        end = end.wrapping_sub(1);
    }
    true
}

/// zombie.util.StringUtils.equalsIgnoreCase: null and empty compare equal to each other.
pub fn equals_ignore_case(a: Option<&str>, b: Option<&str>) -> bool {
    let empty = |s: Option<&str>| s.is_none_or(str::is_empty);
    if empty(a) && empty(b) {
        return true;
    }
    match (a, b) {
        (Some(a), Some(b)) => java::equals_ignore_case(a, b),
        _ => false,
    }
}

/// zombie.util.StringUtils.isValidVariableName: at least one character, each '_' or an ASCII letter or digit
/// (`isValidVariableChar`).
pub fn is_valid_variable_name(var_name: &str) -> bool {
    let mut valid = false;
    for ch in var_name.encode_utf16() {
        let ok = ch == u16::from(b'_')
            || (u16::from(b'A')..=u16::from(b'Z')).contains(&ch)
            || (u16::from(b'a')..=u16::from(b'z')).contains(&ch)
            || (u16::from(b'0')..=u16::from(b'9')).contains(&ch);
        if !ok {
            return false;
        }
        valid = true;
    }
    valid
}

/// zombie.util.StringUtils.tryParseBoolean: trimmed "true" (any case), "1" or "1.0"; false otherwise and for blank.
pub fn try_parse_boolean(var_str: Option<&str>) -> bool {
    if is_null_or_whitespace(var_str) {
        return false;
    }
    let processed = java::trim(var_str.unwrap_or(""));
    java::equals_ignore_case(processed, "true") || processed == "1" || processed == "1.0"
}

/// zombie.util.StringUtils.tryParseFloat: `Float.parseFloat`, 0 for blank or unparsable text.
pub fn try_parse_float(value_str: Option<&str>) -> f32 {
    if is_null_or_whitespace(value_str) {
        return 0.0;
    }
    java::parse_float(value_str.unwrap_or("")).unwrap_or(0.0)
}
