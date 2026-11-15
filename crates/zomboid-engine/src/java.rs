//! JDK behaviours the game's code relies on, reproduced where Rust's std differs.

/// java.lang.String.trim: strips code units `<= U+0020` from both ends (not Unicode whitespace).
pub fn trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

/// java.lang.Integer.parseInt(String): optional sign then decimal digits, no whitespace; `None` is NumberFormatException.
/// Non-ASCII digits, which Java also accepts, are rejected here.
pub fn parse_int(s: &str) -> Option<i32> {
    if s.is_empty()
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'+' || b == b'-')
    {
        return None;
    }
    s.parse::<i32>().ok()
}

/// java.lang.Character.isWhitespace(char).
pub fn is_whitespace(c: char) -> bool {
    matches!(c, '\t'..='\r' | '\u{1c}'..='\u{1f}')
        || (matches!(
            c,
            ' ' | '\u{1680}' | '\u{2000}'
                ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{205f}' | '\u{3000}'
        ) && c != '\u{2007}')
}

/// java.lang.String.compareTo: UTF-16 code units, then length.
pub fn string_compare_to(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// The key under which `String.CASE_INSENSITIVE_ORDER` sees two strings as equal: each char upper-cased, then
/// lower-cased (two strings compare as 0 exactly when these keys are equal).
pub fn case_insensitive_key(s: &str) -> String {
    s.chars().map(|c| lower(upper(c))).collect()
}

/// java.lang.String.equalsIgnoreCase: per-char comparison after toUpperCase, then toLowerCase.
pub fn equals_ignore_case(a: &str, b: &str) -> bool {
    let (mut x, mut y) = (a.chars(), b.chars());
    loop {
        match (x.next(), y.next()) {
            (None, None) => return true,
            (Some(p), Some(q)) => {
                if p == q {
                    continue;
                }
                let (pu, qu) = (upper(p), upper(q));
                if pu != qu && lower(pu) != lower(qu) {
                    return false;
                }
            }
            _ => return false,
        }
    }
}
fn upper(c: char) -> char {
    let mut u = c.to_uppercase();
    match (u.next(), u.next()) {
        (Some(x), None) => x,
        _ => c,
    }
}
fn lower(c: char) -> char {
    let mut u = c.to_lowercase();
    match (u.next(), u.next()) {
        (Some(x), None) => x,
        _ => c,
    }
}

/// java.lang.Math.min(float, float): NaN if either is NaN, and -0.0 below 0.0.
pub fn math_min_f32(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && b.to_bits() == (-0.0f32).to_bits() {
        return b;
    }
    if a <= b {
        a
    } else {
        b
    }
}

/// java.util.Random: the 48-bit linear congruential generator, bit-for-bit.
#[derive(Clone, Debug)]
pub struct Random {
    seed: i64,
}
const MULTIPLIER: i64 = 0x5DEECE66D;
const MASK: i64 = (1 << 48) - 1;
impl Random {
    /// java.util.Random(long seed).
    pub fn new(seed: i64) -> Self {
        Random {
            seed: (seed ^ MULTIPLIER) & MASK,
        }
    }
    /// java.util.Random.next(int bits).
    pub fn next(&mut self, bits: u32) -> i32 {
        self.seed = (self.seed.wrapping_mul(MULTIPLIER).wrapping_add(0xB)) & MASK;
        (self.seed >> (48 - bits)) as i32
    }
    /// java.util.Random.nextInt().
    pub fn next_int(&mut self) -> i32 {
        self.next(32)
    }
    /// java.util.Random.nextInt(int bound), the legacy algorithm kept by JDK 17+ for java.util.Random itself. `bound` must be positive.
    pub fn next_int_bound(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "bound must be positive");
        let mut r = self.next(31);
        let m = bound.wrapping_sub(1);
        if bound & m == 0 {
            ((bound as i64).wrapping_mul(r as i64) >> 31) as i32
        } else {
            let mut u = r;
            loop {
                r = u % bound;
                if u.wrapping_sub(r).wrapping_add(m) >= 0 {
                    break;
                }
                u = self.next(31);
            }
            r
        }
    }
    /// java.util.Random.nextLong().
    pub fn next_long(&mut self) -> i64 {
        ((self.next(32) as i64) << 32).wrapping_add(self.next(32) as i64)
    }
    /// java.util.Random.nextFloat().
    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1 << 24) as f32
    }
    /// java.util.Random.nextDouble().
    pub fn next_double(&mut self) -> f64 {
        (((self.next(26) as i64) << 27) + self.next(27) as i64) as f64 * (1.0 / (1u64 << 53) as f64)
    }
    /// java.util.Random.nextBoolean().
    pub fn next_boolean(&mut self) -> bool {
        self.next(1) != 0
    }
}

/// The source of randomness behind `RandAbstract`: the two calls the game makes on a `java.util.Random`.
pub trait JavaRandom {
    fn next_int_bound(&mut self, bound: i32) -> i32;
    fn next_float(&mut self) -> f32;
}
impl JavaRandom for Random {
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        Random::next_int_bound(self, bound)
    }
    fn next_float(&mut self) -> f32 {
        Random::next_float(self)
    }
}

/// java.lang.String.hashCode over UTF-16 code units.
pub fn string_hash_code(s: &str) -> i32 {
    s.encode_utf16()
        .fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
}

/// Iteration order of a `java.util.HashMap<String, _>` filled by `put` in the given order (no removals): indices into
/// `keys_in_insertion_order`, duplicates keeping their first position. Buckets ascend, entries of one bucket keep
/// insertion order. Bins that Java would turn into trees (8+ entries with 64+ buckets) may iterate differently there.
pub fn hash_map_order(keys_in_insertion_order: &[&str]) -> Vec<usize> {
    let mut seen = std::collections::HashSet::new();
    let unique: Vec<usize> = (0..keys_in_insertion_order.len())
        .filter(|i| seen.insert(keys_in_insertion_order[*i]))
        .collect();
    let mut cap: usize = 16;
    while unique.len() as f64 > cap as f64 * 0.75 {
        cap *= 2;
    }
    let bucket = |i: usize| {
        let h = string_hash_code(keys_in_insertion_order[i]);
        ((h ^ ((h as u32) >> 16) as i32) as u32 as usize) & (cap - 1)
    };
    let mut order = unique;
    order.sort_by_key(|i| bucket(*i)); // stable: insertion order within a bucket
    order
}

/// java.util.zip.CRC32 over `data` (the IEEE polynomial, reflected, as `getValue()` returns it).
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// java.lang.String(byte[], "UTF-8") as the game's runtime decodes (`String.decodeUTF8_UTF16`, JDK 21+): each malformed
/// sequence becomes U+FFFD with the JDK's own rules for how many bytes it swallows, a truncated sequence at the end
/// becomes one U+FFFD (a 4-byte lead's second byte is consumed with it), and an encoded surrogate is malformed.
/// The result is UTF-16 code units.
pub fn utf8_decode_jdk(src: &[u8]) -> Vec<u16> {
    const REPL: u16 = 0xFFFD;
    let not_cont = |b: i32| (b & 0xc0) != 0x80;
    let is_malformed3 = |b1: i32, b2: i32, b3: i32| {
        (b1 == 0xe0_u8 as i8 as i32 && (b2 & 0xe0) == 0x80)
            || (b2 & 0xc0) != 0x80
            || (b3 & 0xc0) != 0x80
    };
    let is_malformed3_2 = |b1: i32, b2: i32| {
        (b1 == 0xe0_u8 as i8 as i32 && (b2 & 0xe0) == 0x80) || (b2 & 0xc0) != 0x80
    };
    let is_malformed4 = |b2: i32, b3: i32, b4: i32| {
        (b2 & 0xc0) != 0x80 || (b3 & 0xc0) != 0x80 || (b4 & 0xc0) != 0x80
    };
    let is_malformed4_2 = |b1: i32, b2: i32| {
        (b1 == 0xf0 && !(0x90..=0xbf).contains(&b2))
            || (b1 == 0xf4 && (b2 & 0xf0) != 0x80)
            || (b2 & 0xc0) != 0x80
    };
    let byte = |i: usize| i32::from(src[i] as i8);
    let sl = src.len();
    let mut dst = Vec::with_capacity(sl);
    let mut sp = 0usize;
    while sp < sl {
        let b1 = byte(sp);
        sp += 1;
        if b1 >= 0 {
            dst.push(b1 as u16);
        } else if (b1 >> 5) == -2 && (b1 & 0x1e) != 0 {
            if sp < sl {
                let b2 = byte(sp);
                sp += 1;
                if not_cont(b2) {
                    dst.push(REPL);
                    sp -= 1;
                } else {
                    dst.push(
                        (((b1 << 6) ^ b2) ^ ((0xC0_u8 as i8 as i32) << 6 ^ (0x80_u8 as i8 as i32)))
                            as u16,
                    );
                }
                continue;
            }
            dst.push(REPL);
            break;
        } else if (b1 >> 4) == -2 {
            if sp + 1 < sl {
                let b2 = byte(sp);
                let b3 = byte(sp + 1);
                sp += 2;
                if is_malformed3(b1, b2, b3) {
                    dst.push(REPL);
                    sp -= 3;
                    // String.malformed3: one byte when the second cannot follow the first, else two.
                    let m1 = byte(sp);
                    let m2 = byte(sp + 1);
                    sp += if (m1 == 0xe0_u8 as i8 as i32 && (m2 & 0xe0) == 0x80) || not_cont(m2) {
                        1
                    } else {
                        2
                    };
                } else {
                    let c = ((b1 << 12)
                        ^ (b2 << 6)
                        ^ (b3
                            ^ ((0xE0_u8 as i8 as i32) << 12
                                ^ (0x80_u8 as i8 as i32) << 6
                                ^ (0x80_u8 as i8 as i32)))) as u16;
                    dst.push(if (0xD800..=0xDFFF).contains(&c) {
                        REPL
                    } else {
                        c
                    });
                }
                continue;
            }
            if sp < sl && is_malformed3_2(b1, byte(sp)) {
                dst.push(REPL);
                continue;
            }
            dst.push(REPL);
            break;
        } else if (b1 >> 3) == -2 {
            if sp + 2 < sl {
                let b2 = byte(sp);
                let b3 = byte(sp + 1);
                let b4 = byte(sp + 2);
                sp += 3;
                let uc = (b1 << 18)
                    ^ (b2 << 12)
                    ^ (b3 << 6)
                    ^ (b4
                        ^ ((0xF0_u8 as i8 as i32) << 18
                            ^ (0x80_u8 as i8 as i32) << 12
                            ^ (0x80_u8 as i8 as i32) << 6
                            ^ (0x80_u8 as i8 as i32)));
                if is_malformed4(b2, b3, b4) || !(0x10000..=0x10FFFF).contains(&uc) {
                    dst.push(REPL);
                    sp -= 4;
                    // String.malformed4
                    let m1 = i32::from(src[sp]);
                    let m2 = i32::from(src[sp + 1]);
                    sp += if m1 > 0xf4
                        || (m1 == 0xf0 && !(0x90..=0xbf).contains(&m2))
                        || (m1 == 0xf4 && (m2 & 0xf0) != 0x80)
                        || not_cont(m2)
                    {
                        1
                    } else if not_cont(byte(sp + 2)) {
                        2
                    } else {
                        3
                    };
                } else {
                    let v = uc - 0x10000;
                    dst.push((0xD800 + (v >> 10)) as u16);
                    dst.push((0xDC00 + (v & 0x3FF)) as u16);
                }
                continue;
            }
            let b1u = b1 & 0xff;
            if b1u > 0xf4 || (sp < sl && is_malformed4_2(b1u, i32::from(src[sp]))) {
                dst.push(REPL);
                continue;
            }
            sp += 1;
            dst.push(REPL);
            if sp < sl && not_cont(byte(sp)) {
                continue;
            }
            break;
        } else {
            dst.push(REPL);
        }
    }
    dst
}

/// java.lang.Long.parseLong(String, int) for radix 2..=36 over ASCII digits: an optional sign, at least one digit,
/// no overflow; `None` is the original's NumberFormatException.
pub fn parse_long(s: &str, radix: u32) -> Option<i64> {
    let b = s.as_bytes();
    let (neg, digits) = match b.first()? {
        b'-' => (true, &b[1..]),
        b'+' => (false, &b[1..]),
        _ => (false, b),
    };
    if digits.is_empty() {
        return None;
    }
    // Accumulate negatively, as the JDK does, so Long.MIN_VALUE parses.
    let mut result: i64 = 0;
    for &c in digits {
        let d = i64::from((c as char).to_digit(radix)?);
        result = result.checked_mul(i64::from(radix))?.checked_sub(d)?;
    }
    if neg {
        Some(result)
    } else {
        result.checked_neg()
    }
}

/// java.lang.Double.parseDouble (`FloatingDecimal.readJavaFormatString`) for decimal input: leading and trailing
/// characters up to U+0020 trimmed, an optional sign, "NaN", "Infinity", digits with at most one point and at least
/// one digit, an optional exponent with at least one digit, and one optional trailing f/F/d/D; correctly rounded.
/// Hexadecimal floating-point input ("0x1p4") is not ported (`None`, the NumberFormatException). `None` is also the
/// original's NumberFormatException for anything else.
pub fn parse_double(s: &str) -> Option<f64> {
    let s = trim(s);
    let b = s.as_bytes();
    let mut i = 0usize;
    let mut neg = false;
    match b.first()? {
        b'-' => {
            neg = true;
            i = 1;
        }
        b'+' => i = 1,
        _ => {}
    }
    let rest = &s[i..];
    if rest.starts_with('N') || rest.starts_with('I') {
        let v = if rest == "NaN" {
            f64::NAN
        } else if rest == "Infinity" {
            f64::INFINITY
        } else {
            return None;
        };
        return Some(if neg { -v } else { v });
    }
    if rest.starts_with("0x") || rest.starts_with("0X") {
        return None;
    }
    let mut mantissa = String::new();
    let mut digits = 0usize;
    let mut point = false;
    while i < b.len() {
        match b[i] {
            c @ b'0'..=b'9' => {
                mantissa.push(c as char);
                digits += 1;
            }
            b'.' if !point => {
                point = true;
                mantissa.push('.');
            }
            b'.' => return None,
            _ => break,
        }
        i += 1;
    }
    if digits == 0 {
        return None;
    }
    let mut exponent = String::new();
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            exponent.push(b[i] as char);
            i += 1;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            exponent.push(b[i] as char);
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    if i < b.len() && (i != b.len() - 1 || !matches!(b[i], b'f' | b'F' | b'd' | b'D')) {
        return None;
    }
    let text = if exponent.is_empty() {
        mantissa
    } else {
        format!("{mantissa}e{exponent}")
    };
    let v: f64 = text.parse().ok()?;
    Some(if neg { -v } else { v })
}

/// java.lang.Double.toString (JDK 19 and later, the game's JRE 25): the shortest decimal that reads back as `d`;
/// when a single digit would do, the closest of the one- and two-digit decimals (so 2^-1074 is "4.9E-324").
/// Plain notation for 10^-3 <= |d| < 10^7, else "d.dddE<n>".
pub fn double_to_string(d: f64) -> String {
    if d.is_nan() {
        return "NaN".into();
    }
    if d.is_infinite() {
        return if d > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    if d == 0.0 {
        return if d.is_sign_negative() {
            "-0.0".into()
        } else {
            "0.0".into()
        };
    }
    let a = d.abs();
    let (mut digits, mut exp) = split_sci(&format!("{a:e}"));
    if digits.len() == 1 {
        let two = format!("{a:.1e}");
        if two.parse::<f64>().ok() == Some(a) {
            let (d2, e2) = split_sci(&two);
            digits = d2;
            exp = e2;
        }
    }
    let digits = digits.trim_end_matches('0').to_string();
    let digits = if digits.is_empty() {
        "0".to_string()
    } else {
        digits
    };
    let mut out = String::new();
    if d < 0.0 {
        out.push('-');
    }
    if (-3..7).contains(&exp) {
        if exp >= 0 {
            let e = exp as usize;
            if digits.len() > e + 1 {
                out.push_str(&digits[..=e]);
                out.push('.');
                out.push_str(&digits[e + 1..]);
            } else {
                out.push_str(&digits);
                out.push_str(&"0".repeat(e + 1 - digits.len()));
                out.push_str(".0");
            }
        } else {
            out.push_str("0.");
            out.push_str(&"0".repeat((-exp - 1) as usize));
            out.push_str(&digits);
        }
    } else {
        out.push_str(&digits[..1]);
        out.push('.');
        if digits.len() > 1 {
            out.push_str(&digits[1..]);
        } else {
            out.push('0');
        }
        out.push('E');
        out.push_str(&exp.to_string());
    }
    out
}

/// The significant digits and decimal exponent of Rust's `{:e}` text ("1.25e-3" -> ("125", -3)).
fn split_sci(s: &str) -> (String, i32) {
    let (m, e) = s.split_once('e').unwrap_or((s, "0"));
    let digits: String = m.chars().filter(char::is_ascii_digit).collect();
    (digits, e.parse().unwrap_or(0))
}

/// java.lang.Integer.toString(int, int): lowercase digits, a leading '-' for negatives.
pub fn integer_to_string_radix(i: i32, radix: u32) -> String {
    let radix = if (2..=36).contains(&radix) { radix } else { 10 };
    let mut n = i64::from(i).unsigned_abs();
    if n == 0 {
        return "0".into();
    }
    let mut buf = Vec::new();
    while n > 0 {
        let d = (n % u64::from(radix)) as u32;
        buf.push(std::char::from_digit(d, radix).unwrap_or('0'));
        n /= u64::from(radix);
    }
    if i < 0 {
        buf.push('-');
    }
    buf.iter().rev().collect()
}

/// java.lang.String.toLowerCase() (a non-Turkish default locale).
pub fn to_lower_case(s: &str) -> String {
    s.to_lowercase()
}

/// java.lang.String.toUpperCase() (a non-Turkish default locale).
pub fn to_upper_case(s: &str) -> String {
    s.to_uppercase()
}

/// java.lang.Math.pow: StrictMath.pow (FdLibm.Pow) on the game's JRE, bit for bit (oracle `fdlibm`).
pub fn math_pow(x: f64, y: f64) -> f64 {
    crate::java_fdlibm::pow(x, y)
}

#[cfg(test)]
mod crc32_tests {
    /// The CRC-32 check value of "123456789" (what `java.util.zip.CRC32` returns for it).
    #[test]
    fn check_value() {
        assert_eq!(super::crc32(b"123456789"), 0xCBF4_3926);
    }
}
