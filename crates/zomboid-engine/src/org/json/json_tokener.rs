//! org.json.JSONTokener (org.json 20250517, bundled in the game's jar) over a `java.io.StringReader`: the text is a
//! sequence of UTF-16 code units, and a unit 0 reads like the end of the input (the reader's `read() <= 0`).
use super::json_object::{string_to_value, JsonValue};
use super::JsonException;

/// org.json.JSONTokener: the reader position and the index/character/line counters its error messages print.
pub struct JsonTokener {
    src: Vec<u16>,
    pos: usize,
    character: i64,
    eof: bool,
    index: i64,
    line: i64,
    previous: u16,
    use_previous: bool,
    character_previous_line: i64,
}

impl JsonTokener {
    /// org.json.JSONTokener(String): `new StringReader(source)`, counters at index 0, character 1, line 1.
    pub fn new(source: &str) -> Self {
        JsonTokener {
            src: source.encode_utf16().collect(),
            pos: 0,
            character: 1,
            eof: false,
            index: 0,
            line: 1,
            previous: 0,
            use_previous: false,
            character_previous_line: 0,
        }
    }

    /// java.io.StringReader.read: the next unit, or -1 at the end.
    fn read(&mut self) -> i32 {
        match self.src.get(self.pos) {
            Some(&c) => {
                self.pos += 1;
                i32::from(c)
            }
            None => -1,
        }
    }

    /// org.json.JSONTokener.getPrevious.
    pub fn get_previous(&self) -> u16 {
        self.previous
    }

    /// org.json.JSONTokener.back: one step back, never two in a row.
    pub fn back(&mut self) -> Result<(), JsonException> {
        if !self.use_previous && self.index > 0 {
            self.decrement_indexes();
            self.use_previous = true;
            self.eof = false;
            Ok(())
        } else {
            Err(JsonException(
                "Stepping back two steps is not supported".into(),
            ))
        }
    }

    /// org.json.JSONTokener.decrementIndexes.
    fn decrement_indexes(&mut self) {
        self.index -= 1;
        if self.previous == u16::from(b'\r') || self.previous == u16::from(b'\n') {
            self.line -= 1;
            self.character = self.character_previous_line;
        } else if self.character > 0 {
            self.character -= 1;
        }
    }

    /// org.json.JSONTokener.end.
    pub fn end(&self) -> bool {
        self.eof && !self.use_previous
    }

    /// org.json.JSONTokener.next(): the next unit, 0 at the end (or at a unit 0).
    pub fn next(&mut self) -> u16 {
        let c = if self.use_previous {
            self.use_previous = false;
            i32::from(self.previous)
        } else {
            self.read()
        };
        if c <= 0 {
            self.eof = true;
            return 0;
        }
        self.increment_indexes(c);
        self.previous = c as u16;
        self.previous
    }

    /// org.json.JSONTokener.incrementIndexes.
    fn increment_indexes(&mut self, c: i32) {
        if c > 0 {
            self.index += 1;
            if c == 13 {
                self.line += 1;
                self.character_previous_line = self.character;
                self.character = 0;
            } else if c == 10 {
                if self.previous != u16::from(b'\r') {
                    self.line += 1;
                    self.character_previous_line = self.character;
                }
                self.character = 0;
            } else {
                self.character += 1;
            }
        }
    }

    /// org.json.JSONTokener.next(int): `n` units; running out is "Substring bounds error".
    pub fn next_n(&mut self, n: usize) -> Result<Vec<u16>, JsonException> {
        let mut chars = Vec::with_capacity(n);
        for _ in 0..n {
            chars.push(self.next());
            if self.end() {
                return Err(self.syntax_error("Substring bounds error"));
            }
        }
        Ok(chars)
    }

    /// org.json.JSONTokener.nextClean: skips units up to U+0020.
    pub fn next_clean(&mut self) -> u16 {
        loop {
            let c = self.next();
            if c == 0 || c > u16::from(b' ') {
                return c;
            }
        }
    }

    /// org.json.JSONTokener.nextString: the text up to `quote` with the backslash escapes; a raw line break or the
    /// end is "Unterminated string". `\u` takes four units through `Integer.parseInt(next, 16)` (a sign is allowed;
    /// non-ASCII digits, which Java also accepts, are an error here: UNKNOWN), cast to a char.
    pub fn next_string(&mut self, quote: u16) -> Result<Vec<u16>, JsonException> {
        let mut sb: Vec<u16> = Vec::new();
        loop {
            let c = self.next();
            match c {
                0 | 10 | 13 => {
                    return Err(self.syntax_error(&format!(
                        "Unterminated string. Character with int code {c} is not allowed within a quoted string."
                    )))
                }
                92 => {
                    let c = self.next();
                    match c {
                        34 | 39 | 47 | 92 => sb.push(c),
                        98 => sb.push(8),
                        102 => sb.push(12),
                        110 => sb.push(10),
                        114 => sb.push(13),
                        116 => sb.push(9),
                        117 => {
                            let next = self.next_n(4)?;
                            match parse_int_radix16(&next) {
                                Some(v) => sb.push(v as u16),
                                None => {
                                    return Err(self.syntax_error(&format!(
                                        "Illegal escape. \\u must be followed by a 4 digit hexadecimal number. \\{} is not valid.",
                                        String::from_utf16_lossy(&next)
                                    )))
                                }
                            }
                        }
                        _ => {
                            let mut m: Vec<u16> = "Illegal escape. Escape sequence  \\".encode_utf16().collect();
                            m.push(c);
                            m.extend(" is not valid.".encode_utf16());
                            return Err(self.syntax_error(&String::from_utf16_lossy(&m)));
                        }
                    }
                }
                _ => {
                    if c == quote {
                        return Ok(sb);
                    }
                    sb.push(c);
                }
            }
        }
    }

    /// org.json.JSONTokener.nextValue: an object, an array or a simple value.
    pub fn next_value(&mut self) -> Result<JsonValue, JsonException> {
        let c = self.next_clean();
        match c {
            91 => {
                self.back()?;
                super::json_array::parse(self).map(JsonValue::Array)
            }
            123 => {
                self.back()?;
                super::json_object::parse(self).map(JsonValue::Object)
            }
            _ => self.next_simple_value(c),
        }
    }

    /// org.json.JSONTokener.nextSimpleValue (not strict, the parser configuration of Translator.tryFillMapFromFile
    /// outside Core.IS_DEV): a quoted string, or the unquoted text up to `,:]}/\"[{;=#` or a control unit, trimmed,
    /// through `JSONObject.stringToValue`.
    pub fn next_simple_value(&mut self, mut c: u16) -> Result<JsonValue, JsonException> {
        match c {
            34 | 39 => Ok(JsonValue::Str(units_to_string(&self.next_string(c)?))),
            _ => {
                const STOP: &[u8] = b",:]}/\\\"[{;=#";
                let mut sb: Vec<u16> = Vec::new();
                while c >= u16::from(b' ') && !(c < 128 && STOP.contains(&(c as u8))) {
                    sb.push(c);
                    c = self.next();
                }
                if !self.eof {
                    self.back()?;
                }
                let s = units_to_string(&sb);
                let string = crate::java::trim(&s);
                if string.is_empty() {
                    return Err(self.syntax_error("Missing value"));
                }
                Ok(string_to_value(string))
            }
        }
    }

    /// org.json.JSONTokener.syntaxError: the message with " at <index> [character <c> line <l>]".
    pub fn syntax_error(&self, message: &str) -> JsonException {
        JsonException(format!(
            "{message} at {} [character {} line {}]",
            self.index, self.character, self.line
        ))
    }
}

/// java.lang.Integer.parseInt(String, 16) over ASCII digits: an optional sign, at least one digit.
fn parse_int_radix16(units: &[u16]) -> Option<i32> {
    let s = String::from_utf16(units).ok()?;
    let v = crate::java::parse_long(&s, 16)?;
    i32::try_from(v).ok()
}

/// A Java string's UTF-16 units as a Rust string. A lone surrogate (from a `\u` escape) becomes U+FFFD: Java keeps
/// it; not reproduced (UNKNOWN, none in the game's files).
pub fn units_to_string(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}
