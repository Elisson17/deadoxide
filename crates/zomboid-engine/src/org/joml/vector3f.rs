//! org.joml.Vector3f (JOML, bundled in the game's jar; not decompiled, read from its bytecode with `javap -c`).

/// org.joml.Vector3f
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3f {
    /// org.joml.Vector3f.<init>(float, float, float)
    pub fn new(x: f32, y: f32, z: f32) -> Vector3f {
        Vector3f { x, y, z }
    }

    /// org.joml.Vector3f.toString: `Runtime.formatNumbers(toString(Options.NUMBER_FORMAT))`, where
    /// `toString(NumberFormat)` is "(" + x + " " + y + " " + z + ")", each through `Runtime.format(double, ...)`.
    pub fn java_to_string(&self) -> String {
        format_numbers(&format!(
            "({} {} {})",
            runtime_format(f64::from(self.x)),
            runtime_format(f64::from(self.y)),
            runtime_format(f64::from(self.z))
        ))
    }
}

/// org.joml.Runtime.format(double, NumberFormat) with Options.NUMBER_FORMAT (`new DecimalFormat(" 0.000E0;-")`:
/// joml.format on, joml.format.decimals 3): NaN and infinities padded by `padLeft`, the rest through the
/// DecimalFormat.
fn runtime_format(number: f64) -> String {
    if number.is_nan() {
        return pad_left(" NaN");
    }
    if number.is_infinite() {
        return pad_left(if number > 0.0 { " +Inf" } else { " -Inf" });
    }
    decimal_format(number)
}

/// org.joml.Runtime.padLeft: `format.format(0.0).length() - s.length() + 1` spaces, then `s`.
fn pad_left(s: &str) -> String {
    let len = decimal_format(0.0).len();
    let mut b = String::new();
    let mut i = 0usize;
    while i + s.len() < len + 1 {
        b.push(' ');
        i += 1;
    }
    b + s
}

/// java.text.DecimalFormat(" 0.000E0;-").format(double): prefix " " (positive) or "-" (negative, -0.0 included),
/// one integer digit, three fraction digits rounded HALF_EVEN on the exact binary value, "E", the exponent with
/// "-" when negative. Rust's `{:.3e}` rounds the exact value half to even as well.
fn decimal_format(number: f64) -> String {
    let negative = number < 0.0 || (number == 0.0 && number.is_sign_negative());
    let s = format!("{:.3e}", number.abs());
    let (mantissa, exponent) = s.split_once('e').unwrap_or((&s, "0"));
    format!("{}{mantissa}E{exponent}", if negative { "-" } else { " " })
}

/// org.joml.Runtime.formatNumbers: a "+" before an exponent's first digit, and in place of a space right after
/// "E".
fn format_numbers(s: &str) -> String {
    let mut res = String::new();
    let mut e_index = i64::from(i32::MIN);
    for (i, c) in s.chars().enumerate() {
        let i = i as i64;
        if c == 'E' {
            e_index = i;
        } else if c == ' ' && e_index == i - 1 {
            res.push('+');
            continue;
        } else if c.is_ascii_digit() && e_index == i - 1 {
            res.push('+');
        }
        res.push(c);
    }
    res
}
