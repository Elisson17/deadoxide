//! The JSON reader of the glTF2 importer: rapidjson (`contrib/rapidjson`, bundled with assimp) as `glTF2::Asset::Load`
//! calls it, `Document::ParseInsitu` with the default flags. What matters for the importer is the number type
//! rapidjson gives a literal (`Uint`, `Int`, `Uint64`, `Int64`, `Double`) and how a fractional literal is converted
//! (`ParseNumber` without `kParseFullPrecisionFlag`: `StrtodNormalPrecision`, a significand of at most 17 digits scaled by a
//! power of ten in double arithmetic).

/// A rapidjson number with the type the parser chose for the literal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Num {
    Uint(u32),
    Int(i32),
    Uint64(u64),
    Int64(i64),
    Double(f64),
}

/// A rapidjson `Value`.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(Num),
    Str(String),
    Arr(Vec<Json>),
    /// members in document order
    Obj(Vec<(String, Json)>),
}

impl Json {
    /// `FindMember(name)` of an object (the last member of that name, like `GenericValue::FindMember` after a
    /// duplicate key is added by the parser: rapidjson keeps both and returns the first; objects written by the glTF
    /// exporters never repeat a key).
    pub fn get(&self, name: &str) -> Option<&Json> {
        match self {
            Json::Obj(m) => m.iter().find(|(k, _)| k == name).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Obj(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// `IsUint()` + `GetUint()`: an `Uint` literal or a non-negative `Int`; a double never is a `Uint`.
    pub fn as_uint(&self) -> Option<u32> {
        match self {
            Json::Num(Num::Uint(u)) => Some(*u),
            Json::Num(Num::Int(i)) if *i >= 0 => Some(*i as u32),
            Json::Num(Num::Uint64(u)) => u32::try_from(*u).ok(),
            Json::Num(Num::Int64(i)) => u32::try_from(*i).ok(),
            _ => None,
        }
    }

    /// `IsInt()` + `GetInt()`
    pub fn as_int(&self) -> Option<i32> {
        match self {
            Json::Num(Num::Int(i)) => Some(*i),
            Json::Num(Num::Uint(u)) => i32::try_from(*u).ok(),
            Json::Num(Num::Int64(i)) => i32::try_from(*i).ok(),
            Json::Num(Num::Uint64(u)) => i32::try_from(*u).ok(),
            _ => None,
        }
    }

    /// `IsNumber()` + `GetDouble()`
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(Num::Uint(u)) => Some(f64::from(*u)),
            Json::Num(Num::Int(i)) => Some(f64::from(*i)),
            Json::Num(Num::Uint64(u)) => Some(*u as f64),
            Json::Num(Num::Int64(i)) => Some(*i as f64),
            Json::Num(Num::Double(d)) => Some(*d),
            _ => None,
        }
    }

    /// `static_cast<float>(GetDouble())`, the way the glTF2 reader converts a `float` member.
    pub fn as_f32(&self) -> Option<f32> {
        self.as_f64().map(|d| d as f32)
    }
}

struct Reader<'a> {
    s: &'a [u8],
    i: usize,
}

/// `Document::ParseInsitu` of the JSON chunk: the value of the text, or the parse error message.
pub fn parse(text: &[u8]) -> Result<Json, String> {
    let mut r = Reader { s: text, i: 0 };
    r.skip_ws();
    let v = r.value()?;
    r.skip_ws();
    if r.i != r.s.len() {
        return Err(format!(
            "GLTF: JSON parse error: the document root must be followed by the end of the text (offset {})",
            r.i
        ));
    }
    Ok(v)
}

/// `internal::Pow10(n)`: the table entry `1eN` (the double nearest to 10^n).
fn pow10(n: i32) -> f64 {
    format!("1e{n}").parse::<f64>().unwrap_or(f64::INFINITY)
}

/// `internal::StrtodNormalPrecision(d, p)` (`FastPath` twice for an exponent below -308)
fn strtod_normal_precision(d: f64, p: i32) -> f64 {
    fn fast_path(significand: f64, exp: i32) -> f64 {
        if exp < -308 {
            0.0
        } else if exp >= 0 {
            significand * pow10(exp)
        } else {
            significand / pow10(-exp)
        }
    }
    if p < -308 {
        fast_path(fast_path(d, -308), p + 308)
    } else {
        fast_path(d, p)
    }
}

impl Reader<'_> {
    fn peek(&self) -> u8 {
        self.s.get(self.i).copied().unwrap_or(0)
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), b' ' | b'\n' | b'\r' | b'\t') {
            self.i += 1;
        }
    }

    fn err<T>(&self, what: &str) -> Result<T, String> {
        Err(format!(
            "GLTF: JSON parse error: {what} (offset {})",
            self.i
        ))
    }

    fn value(&mut self) -> Result<Json, String> {
        match self.peek() {
            b'n' => self.literal("null", Json::Null),
            b't' => self.literal("true", Json::Bool(true)),
            b'f' => self.literal("false", Json::Bool(false)),
            b'"' => Ok(Json::Str(self.string()?)),
            b'{' => self.object(),
            b'[' => self.array(),
            _ => self.number(),
        }
    }

    fn literal(&mut self, word: &str, v: Json) -> Result<Json, String> {
        if self.s[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(v)
        } else {
            self.err("invalid value")
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.i += 1;
        let mut members = Vec::new();
        self.skip_ws();
        if self.peek() == b'}' {
            self.i += 1;
            return Ok(Json::Obj(members));
        }
        loop {
            self.skip_ws();
            if self.peek() != b'"' {
                return self.err("missing a name for object member");
            }
            let name = self.string()?;
            self.skip_ws();
            if self.peek() != b':' {
                return self.err("missing a colon after a name of object member");
            }
            self.i += 1;
            self.skip_ws();
            let v = self.value()?;
            members.push((name, v));
            self.skip_ws();
            match self.peek() {
                b',' => self.i += 1,
                b'}' => {
                    self.i += 1;
                    return Ok(Json::Obj(members));
                }
                _ => return self.err("missing a comma or '}' after an object member"),
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.i += 1;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == b']' {
            self.i += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value()?);
            self.skip_ws();
            match self.peek() {
                b',' => self.i += 1,
                b']' => {
                    self.i += 1;
                    return Ok(Json::Arr(items));
                }
                _ => return self.err("missing a comma or ']' after an array element"),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let mut v = 0u32;
        for _ in 0..4 {
            let c = self.peek();
            let d = match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 10,
                b'A'..=b'F' => c - b'A' + 10,
                _ => return self.err("incorrect hex digit after \\u escape in string"),
            };
            v = v * 16 + u32::from(d);
            self.i += 1;
        }
        Ok(v)
    }

    fn string(&mut self) -> Result<String, String> {
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = self.peek();
            if self.i >= self.s.len() {
                return self.err("missing a closing quotation mark in string");
            }
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = self.peek();
                    self.i += 1;
                    match e {
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'/' => out.push(b'/'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let mut cp = self.hex4()?;
                            if (0xD800..=0xDBFF).contains(&cp) {
                                if self.peek() != b'\\' || self.s.get(self.i + 1) != Some(&b'u') {
                                    return self.err("the surrogate pair in string is invalid");
                                }
                                self.i += 2;
                                let lo = self.hex4()?;
                                if !(0xDC00..=0xDFFF).contains(&lo) {
                                    return self.err("the surrogate pair in string is invalid");
                                }
                                cp = (((cp - 0xD800) << 10) | (lo - 0xDC00)) + 0x10000;
                            }
                            let ch = char::from_u32(cp).unwrap_or('\u{fffd}');
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        _ => return self.err("unknown escape character"),
                    }
                }
                c if c < 0x20 => return self.err("invalid encoding in string"),
                c => out.push(c),
            }
        }
        String::from_utf8(out)
            .map_err(|_| "GLTF: JSON parse error: invalid encoding in string".into())
    }

    fn digit(&self) -> bool {
        self.peek().is_ascii_digit()
    }

    fn take_digit(&mut self) -> u8 {
        let d = self.peek() - b'0';
        self.i += 1;
        d
    }

    /// `GenericReader::ParseNumber<kParseInsituFlag>` (no `kParseFullPrecisionFlag`)
    fn number(&mut self) -> Result<Json, String> {
        let minus = self.peek() == b'-';
        if minus {
            self.i += 1;
        }
        let mut i: u32 = 0;
        let mut i64v: u64 = 0;
        let mut use64 = false;
        let mut significand_digit = 0i32;
        if self.peek() == b'0' {
            self.i += 1;
        } else if (b'1'..=b'9').contains(&self.peek()) {
            i = u32::from(self.take_digit());
            if minus {
                while self.digit() {
                    if i >= 214_748_364 && (i != 214_748_364 || self.peek() > b'8') {
                        i64v = u64::from(i);
                        use64 = true;
                        break;
                    }
                    i = i * 10 + u32::from(self.take_digit());
                    significand_digit += 1;
                }
            } else {
                while self.digit() {
                    if i >= 429_496_729 && (i != 429_496_729 || self.peek() > b'5') {
                        i64v = u64::from(i);
                        use64 = true;
                        break;
                    }
                    i = i * 10 + u32::from(self.take_digit());
                    significand_digit += 1;
                }
            }
        } else {
            return self.err("invalid value");
        }
        let mut use_double = false;
        let mut d = 0.0f64;
        if use64 {
            if minus {
                while self.digit() {
                    if i64v >= 922_337_203_685_477_580
                        && (i64v != 922_337_203_685_477_580 || self.peek() > b'8')
                    {
                        d = i64v as f64;
                        use_double = true;
                        break;
                    }
                    i64v = i64v * 10 + u64::from(self.take_digit());
                    significand_digit += 1;
                }
            } else {
                while self.digit() {
                    if i64v >= 1_844_674_407_370_955_161
                        && (i64v != 1_844_674_407_370_955_161 || self.peek() > b'5')
                    {
                        d = i64v as f64;
                        use_double = true;
                        break;
                    }
                    i64v = i64v * 10 + u64::from(self.take_digit());
                    significand_digit += 1;
                }
            }
        }
        if use_double {
            while self.digit() {
                d = d * 10.0 + f64::from(self.take_digit());
            }
        }
        let mut exp_frac = 0i32;
        if self.peek() == b'.' {
            self.i += 1;
            if !self.digit() {
                return self.err("missing fraction part in number");
            }
            if !use_double {
                if !use64 {
                    i64v = u64::from(i);
                }
                while self.digit() {
                    if i64v > 0x1F_FFFF_FFFF_FFFF {
                        break;
                    }
                    i64v = i64v * 10 + u64::from(self.take_digit());
                    exp_frac -= 1;
                    if i64v != 0 {
                        significand_digit += 1;
                    }
                }
                d = i64v as f64;
                use_double = true;
            }
            while self.digit() {
                if significand_digit < 17 {
                    d = d * 10.0 + f64::from(self.take_digit());
                    exp_frac -= 1;
                    if d > 0.0 {
                        significand_digit += 1;
                    }
                } else {
                    self.i += 1;
                }
            }
        }
        let mut exp = 0i32;
        if matches!(self.peek(), b'e' | b'E') {
            self.i += 1;
            if !use_double {
                d = if use64 { i64v as f64 } else { f64::from(i) };
                use_double = true;
            }
            let mut exp_minus = false;
            if self.peek() == b'+' {
                self.i += 1;
            } else if self.peek() == b'-' {
                self.i += 1;
                exp_minus = true;
            }
            if !self.digit() {
                return self.err("missing exponent in number");
            }
            exp = i32::from(self.take_digit());
            if exp_minus {
                while self.digit() {
                    exp = exp * 10 + i32::from(self.take_digit());
                    if exp >= 214_748_364 {
                        while self.digit() {
                            self.i += 1;
                        }
                    }
                }
            } else {
                let max_exp = 308 - exp_frac;
                while self.digit() {
                    exp = exp * 10 + i32::from(self.take_digit());
                    if exp > max_exp {
                        return self.err("number too big to be stored in double");
                    }
                }
            }
            if exp_minus {
                exp = -exp;
            }
        }
        if use_double {
            let v = strtod_normal_precision(d, exp + exp_frac);
            Ok(Json::Num(Num::Double(if minus { -v } else { v })))
        } else if use64 {
            Ok(Json::Num(if minus {
                Num::Int64((!i64v).wrapping_add(1) as i64)
            } else {
                Num::Uint64(i64v)
            }))
        } else {
            Ok(Json::Num(if minus {
                Num::Int((!i).wrapping_add(1) as i32)
            } else {
                Num::Uint(i)
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_types_follow_rapidjson() {
        assert_eq!(parse(b"7").unwrap(), Json::Num(Num::Uint(7)));
        assert_eq!(parse(b"-7").unwrap(), Json::Num(Num::Int(-7)));
        assert_eq!(
            parse(b"4294967295").unwrap(),
            Json::Num(Num::Uint(4_294_967_295))
        );
        assert_eq!(
            parse(b"4294967296").unwrap(),
            Json::Num(Num::Uint64(4_294_967_296))
        );
        assert_eq!(
            parse(b"-2147483648").unwrap(),
            Json::Num(Num::Int(i32::MIN))
        );
        assert_eq!(parse(b"3.0").unwrap(), Json::Num(Num::Double(3.0)));
        assert_eq!(parse(b"0.5").unwrap(), Json::Num(Num::Double(0.5)));
        assert_eq!(parse(b"1e2").unwrap(), Json::Num(Num::Double(100.0)));
        assert_eq!(parse(b"-0.0").unwrap(), Json::Num(Num::Double(-0.0)));
    }

    #[test]
    fn normal_precision_is_not_correctly_rounded() {
        // 0.1 is 1 / 10 in double arithmetic: the nearest double
        assert_eq!(parse(b"0.1").unwrap().as_f64(), Some(0.1));
        // 17 digits: the significand 8.4512115e-4 is built as an integer and divided by a power of ten
        let v = parse(b"8.4512116E-4").unwrap().as_f64().unwrap();
        assert_eq!(v, 84_512_116.0 / 1e11);
    }

    #[test]
    fn structure() {
        let j = parse(br#" {"a":[1,2.5,"x\u00e9\n"],"b":{"c":null,"d":true}} "#).unwrap();
        let a = j.get("a").unwrap().as_array().unwrap();
        assert_eq!(a[0].as_uint(), Some(1));
        assert_eq!(a[1].as_f32(), Some(2.5));
        assert_eq!(a[2].as_str(), Some("x\u{e9}\n"));
        assert_eq!(j.get("b").unwrap().get("d").unwrap().as_bool(), Some(true));
        assert!(parse(b"{} x").is_err());
    }
}
