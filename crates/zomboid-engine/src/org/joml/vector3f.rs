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

    /// org.joml.Vector3f.mul(float): each component times the scalar.
    pub fn mul(&mut self, scalar: f32) {
        self.x *= scalar;
        self.y *= scalar;
        self.z *= scalar;
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

#[cfg(test)]
mod tests {
    use super::Vector3f;

    /// `new Vector3f(x, y, z).toString()` of the original (JOML in the game's jar), measured with an ad hoc probe
    /// on 2026-11-16: zeros of both signs, rounding ties and carries, subnormals, NaN and infinities.
    #[test]
    fn to_string_matches_original() {
        let cases: [(u32, u32, u32, &str); 10] = [
            (
                0x0,
                0x80000000,
                0x3f800000,
                "( 0.000E+0 -0.000E+0  1.000E+0)",
            ),
            (
                0x3f800000,
                0xbf800000,
                0x42f6e979,
                "( 1.000E+0 -1.000E+0  1.235E+2)",
            ),
            (
                0x42f6e979,
                0x3aa1be2b,
                0x116c2,
                "( 1.235E+2  1.234E-3  1.000E-40)",
            ),
            (
                0x116c2,
                0x7fc00000,
                0x7f800000,
                "( 1.000E-40       NaN      +Inf)",
            ),
            (
                0x7f800000,
                0xff800000,
                0x3f880000,
                "(     +Inf      -Inf  1.062E+0)",
            ),
            (
                0x3f880000,
                0x411ffdf4,
                0x47c34fc0,
                "( 1.062E+0  1.000E+1  1.000E+5)",
            ),
            (
                0x47c34fc0,
                0xff7fc99e,
                0x1,
                "( 1.000E+5 -3.400E+38  1.401E-45)",
            ),
            (
                0x1,
                0x3f000000,
                0x40000831,
                "( 1.401E-45  5.000E-1  2.000E+0)",
            ),
            (
                0x40000831,
                0x4996b438,
                0xba82f0e1,
                "( 2.000E+0  1.235E+6 -9.990E-4)",
            ),
            (
                0xba82f0e1,
                0x6228a74c,
                0x0,
                "(-9.990E-4  7.778E+20  0.000E+0)",
            ),
        ];
        for (x, y, z, want) in cases {
            let v = Vector3f::new(f32::from_bits(x), f32::from_bits(y), f32::from_bits(z));
            assert_eq!(v.java_to_string(), want);
        }
    }
}
