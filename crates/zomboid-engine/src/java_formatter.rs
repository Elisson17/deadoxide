//! java.util.Formatter as `String.formatted(Object...)` runs it (ported from the JDK 27 source, the JVM `tools/oracle.sh`
//! runs the probes on; `TranslatorProbe` on the game's own JRE 25.0.5 printed the same 119451 lines, so the two agree
//! on `tests/translator_cases.txt`, beyond it UNKNOWN): `Formatter.parse` with `FormatSpecifierParser`, the
//! checks of each `FormatSpecifier`, then `Formatter.format`'s argument indexing (ordinary, explicit `n$`, relative
//! `<`) and printing. The whole format string is parsed (and checked) before anything is printed; any exception
//! discards the output. Printed in full: text, `%%` and `%n`, the general conversions `b B h H s S` with flags, width
//! and precision, and every conversion of a null argument or of an argument of the wrong class (the
//! IllegalFormatConversionException). Numbers, characters and dates of an argument of their own class are not
//! ported: `FormatError::NotPorted` (PLACEHOLDER). The default locale is taken as English (`%S` upper-cases with
//! `java::to_upper_case`).

/// An argument of `String.formatted`.
#[derive(Clone, Debug, PartialEq)]
pub enum FormatArg {
    /// null
    Null,
    /// java.lang.String
    Str(String),
    /// java.lang.Boolean
    Bool(bool),
    /// java.lang.Double
    Double(f64),
    /// any other object (not `Formattable`): its class name, `toString()` and `hashCode()`
    Object {
        class: String,
        string: String,
        hash_code: i32,
    },
}

impl FormatArg {
    fn class_name(&self) -> &str {
        match self {
            FormatArg::Null => "null",
            FormatArg::Str(_) => "java.lang.String",
            FormatArg::Bool(_) => "java.lang.Boolean",
            FormatArg::Double(_) => "java.lang.Double",
            FormatArg::Object { class, .. } => class,
        }
    }
}

/// What `String.formatted` throws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatError {
    /// java.util.MissingFormatArgumentException (its specifier)
    Missing(String),
    /// any other java.util.IllegalFormatException: class and message
    Illegal(&'static str, String),
    /// a conversion this port does not print (PLACEHOLDER)
    NotPorted(String),
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatError::Missing(s) => {
                write!(
                    f,
                    "java.util.MissingFormatArgumentException: Format specifier '{s}'"
                )
            }
            FormatError::Illegal(class, m) => write!(f, "{class}: {m}"),
            FormatError::NotPorted(m) => write!(f, "not ported: {m}"),
        }
    }
}

// java.util.Formatter.Flags
const LEFT_JUSTIFY: i32 = 1;
const UPPERCASE: i32 = 1 << 1;
const ALTERNATE: i32 = 1 << 2;
const PLUS: i32 = 1 << 3;
const LEADING_SPACE: i32 = 1 << 4;
const ZERO_PAD: i32 = 1 << 5;
const GROUP: i32 = 1 << 6;
const PARENTHESES: i32 = 1 << 7;
const PREVIOUS: i32 = 1 << 8;

fn unknown_conversion(s: &str) -> FormatError {
    FormatError::Illegal(
        "java.util.UnknownFormatConversionException",
        format!("Conversion = '{s}'"),
    )
}

fn unit_str(c: u16) -> String {
    String::from_utf16_lossy(&[c])
}

/// java.util.Formatter.Flags.toString.
fn flags_to_string(f: i32) -> String {
    let mut sb = String::new();
    for (bit, ch) in [
        (LEFT_JUSTIFY, '-'),
        (UPPERCASE, '^'),
        (ALTERNATE, '#'),
        (PLUS, '+'),
        (LEADING_SPACE, ' '),
        (ZERO_PAD, '0'),
        (GROUP, ','),
        (PARENTHESES, '('),
        (PREVIOUS, '<'),
    ] {
        if f & bit == bit {
            sb.push(ch);
        }
    }
    sb
}

/// java.util.Formatter.Flags.parse(char) (only flag characters reach it).
fn flag_of(c: u16) -> i32 {
    match c {
        45 => LEFT_JUSTIFY,
        35 => ALTERNATE,
        43 => PLUS,
        32 => LEADING_SPACE,
        48 => ZERO_PAD,
        44 => GROUP,
        40 => PARENTHESES,
        _ => PREVIOUS,
    }
}

/// java.util.Formatter.Flags.isFlag.
fn is_flag(c: u16) -> bool {
    matches!(c, 45 | 35 | 43 | 32 | 48 | 44 | 40 | 60)
}

fn is_digit(c: u16) -> bool {
    (48..=57).contains(&c)
}

/// java.util.Formatter.isConversion.
fn is_conversion(c: u16) -> bool {
    (97..=122).contains(&c) || (65..=90).contains(&c) || c == 37
}

/// java.util.Formatter.Conversion.isValid.
fn conversion_is_valid(c: u16) -> bool {
    matches!(
        c as u8 as char,
        'b' | 'B'
            | 's'
            | 'S'
            | 'h'
            | 'H'
            | 'c'
            | 'C'
            | 'd'
            | 'o'
            | 'x'
            | 'X'
            | 'e'
            | 'E'
            | 'g'
            | 'G'
            | 'f'
            | 'a'
            | 'A'
            | 'n'
            | '%'
    ) && c < 128
}

fn is_general(c: u16) -> bool {
    matches!(c, 98 | 66 | 115 | 83 | 104 | 72)
}
fn is_character(c: u16) -> bool {
    matches!(c, 99 | 67)
}
fn is_integer(c: u16) -> bool {
    matches!(c, 100 | 111 | 120 | 88)
}
fn is_float(c: u16) -> bool {
    matches!(c, 101 | 69 | 103 | 71 | 102 | 97 | 65)
}
fn is_text(c: u16) -> bool {
    matches!(c, 110 | 37)
}

/// java.util.Formatter.DateTime.isValid.
fn date_time_is_valid(c: u16) -> bool {
    c < 128 && "HIklMNLQpsSTzZaAbBCdehjmyYrRcDF".contains(c as u8 as char)
}

/// java.lang.Integer.parseInt(CharSequence, begin, end, 10) over ASCII digits: `None` on overflow.
fn parse_int(s: &[u16], begin: usize, end: usize) -> Option<i32> {
    let mut v: i64 = 0;
    for &c in &s[begin..end] {
        v = v * 10 + i64::from(c - 48);
        if v > i64::from(i32::MAX) {
            return None;
        }
    }
    Some(v as i32)
}

/// java.util.Formatter.FormatSpecifier.
#[derive(Clone, Debug)]
struct FormatSpecifier {
    index: i32,
    flags: i32,
    width: i32,
    precision: i32,
    dt: bool,
    c: u16,
}

enum FormatString {
    /// java.util.Formatter.FixedString: a range of the format string
    Fixed(usize, usize),
    Spec(FormatSpecifier),
}

impl FormatSpecifier {
    /// java.util.Formatter.FormatSpecifier(char): the one-character fast path, no checks.
    fn from_char(conv: u16) -> Self {
        let mut fs = FormatSpecifier {
            index: 0,
            flags: 0,
            width: -1,
            precision: -1,
            dt: false,
            c: conv,
        };
        if (65..=90).contains(&conv) {
            fs.flags = UPPERCASE;
            fs.c = conv + 32;
        }
        if is_text(conv) {
            fs.index = -2;
        }
        fs
    }

    /// java.util.Formatter.FormatSpecifier(String, int, int, int, int, int, char, char): index, flags, width and
    /// precision from their ranges, the conversion, then `check()`.
    #[allow(clippy::too_many_arguments)]
    fn new(
        s: &[u16],
        i: usize,
        arg_size: usize,
        flag_size: usize,
        width_size: usize,
        precision_size: usize,
        t: u16,
        conversion: u16,
    ) -> Result<Self, FormatError> {
        let mut fs = FormatSpecifier {
            index: 0,
            flags: 0,
            width: -1,
            precision: -1,
            dt: false,
            c: 0,
        };
        let arg_end = i + arg_size;
        let flag_end = arg_end + flag_size;
        let width_end = flag_end + width_size;
        let precision_end = width_end + precision_size;
        if arg_size > 0 {
            // index(): skip the trailing '$'
            match parse_int(s, i, arg_end - 1) {
                Some(index) => {
                    fs.index = index;
                    if index <= 0 {
                        return Err(FormatError::Illegal(
                            "java.util.IllegalFormatArgumentIndexException",
                            format!("Illegal format argument index = {index}"),
                        ));
                    }
                }
                None => {
                    return Err(FormatError::Illegal(
                        "java.util.IllegalFormatArgumentIndexException",
                        format!("Illegal format argument index = {}", i32::MIN),
                    ))
                }
            }
        }
        if flag_size > 0 {
            // Flags.parse: a repeated flag is DuplicateFormatFlagsException
            let mut f = 0;
            for &c in &s[arg_end..flag_end] {
                let v = flag_of(c);
                if f & v == v {
                    return Err(FormatError::Illegal(
                        "java.util.DuplicateFormatFlagsException",
                        format!("Flags = '{}'", flags_to_string(v)),
                    ));
                }
                f |= v;
            }
            fs.flags = f;
            if fs.flags & PREVIOUS == PREVIOUS {
                fs.index = -1;
            }
        }
        if width_size > 0 {
            match parse_int(s, flag_end, width_end) {
                Some(w) => fs.width = w,
                None => {
                    return Err(FormatError::Illegal(
                        "java.util.IllegalFormatWidthException",
                        i32::MIN.to_string(),
                    ))
                }
            }
        }
        if precision_size > 0 {
            // skip the leading '.'
            match parse_int(s, width_end + 1, precision_end) {
                Some(p) => fs.precision = p,
                None => {
                    return Err(FormatError::Illegal(
                        "java.util.IllegalFormatPrecisionException",
                        i32::MIN.to_string(),
                    ))
                }
            }
        }
        if t != 0 {
            fs.dt = true;
            if t == 84 {
                fs.flags |= UPPERCASE;
            }
        }
        fs.conversion(conversion)?;
        fs.check()?;
        Ok(fs)
    }

    /// java.util.Formatter.FormatSpecifier.conversion.
    fn conversion(&mut self, conv: u16) -> Result<(), FormatError> {
        self.c = conv;
        if !self.dt {
            if !conversion_is_valid(self.c) {
                return Err(unknown_conversion(&unit_str(self.c)));
            }
            if (65..=90).contains(&self.c) {
                self.flags |= UPPERCASE;
                self.c += 32;
            }
            if is_text(self.c) {
                self.index = -2;
            }
        }
        Ok(())
    }

    /// java.util.Formatter.FormatSpecifier.toString.
    fn to_java_string(&self) -> String {
        let mut sb = String::from("%");
        sb.push_str(&flags_to_string(self.flags & !UPPERCASE));
        if self.index > 0 {
            sb.push_str(&format!("{}$", self.index));
        }
        if self.width != -1 {
            sb.push_str(&self.width.to_string());
        }
        if self.precision != -1 {
            sb.push_str(&format!(".{}", self.precision));
        }
        let upper = self.flags & UPPERCASE == UPPERCASE;
        if self.dt {
            sb.push(if upper { 'T' } else { 't' });
        }
        let c = unit_str(self.c);
        sb.push_str(&if upper { c.to_uppercase() } else { c });
        sb
    }

    fn fail_mismatch(&self, f: i32) -> FormatError {
        FormatError::Illegal(
            "java.util.FormatFlagsConversionMismatchException",
            format!(
                "Conversion = {}, Flags = {}",
                unit_str(self.c),
                flags_to_string(f)
            ),
        )
    }

    fn missing_width(&self) -> FormatError {
        FormatError::Illegal(
            "java.util.MissingFormatWidthException",
            self.to_java_string(),
        )
    }

    fn precision_error(&self) -> FormatError {
        FormatError::Illegal(
            "java.util.IllegalFormatPrecisionException",
            self.precision.to_string(),
        )
    }

    /// java.util.Formatter.FormatSpecifier.checkBadFlags.
    fn check_bad_flags(&self, bad: i32) -> Result<(), FormatError> {
        if self.flags & bad != 0 {
            return Err(self.fail_mismatch(self.flags & bad));
        }
        Ok(())
    }

    /// java.util.Formatter.FormatSpecifier.check (with checkGeneral, checkDateTime, checkCharacter, checkInteger,
    /// checkFloat, checkNumeric and checkText).
    fn check(&self) -> Result<(), FormatError> {
        let left_without_width = self.width == -1 && self.flags & LEFT_JUSTIFY == LEFT_JUSTIFY;
        if self.dt {
            if self.precision != -1 {
                return Err(self.precision_error());
            }
            if !date_time_is_valid(self.c) {
                return Err(unknown_conversion(&format!("t{}", unit_str(self.c))));
            }
            self.check_bad_flags(
                ALTERNATE | PLUS | LEADING_SPACE | ZERO_PAD | GROUP | PARENTHESES,
            )?;
            if left_without_width {
                return Err(self.missing_width());
            }
        } else if is_general(self.c) {
            if (self.c == 98 || self.c == 104) && self.flags & ALTERNATE == ALTERNATE {
                return Err(self.fail_mismatch(ALTERNATE));
            }
            if left_without_width {
                return Err(self.missing_width());
            }
            self.check_bad_flags(PLUS | LEADING_SPACE | ZERO_PAD | GROUP | PARENTHESES)?;
        } else if is_character(self.c) {
            if self.precision != -1 {
                return Err(self.precision_error());
            }
            self.check_bad_flags(
                ALTERNATE | PLUS | LEADING_SPACE | ZERO_PAD | GROUP | PARENTHESES,
            )?;
            if left_without_width {
                return Err(self.missing_width());
            }
        } else if is_integer(self.c) {
            self.check_numeric()?;
            if self.precision != -1 {
                return Err(self.precision_error());
            }
            if self.c == 100 {
                self.check_bad_flags(ALTERNATE)?;
            } else {
                self.check_bad_flags(GROUP)?;
            }
        } else if is_float(self.c) {
            self.check_numeric()?;
            match self.c {
                97 => self.check_bad_flags(PARENTHESES | GROUP)?,
                101 => self.check_bad_flags(GROUP)?,
                103 => self.check_bad_flags(ALTERNATE)?,
                _ => {}
            }
        } else if is_text(self.c) {
            if self.precision != -1 {
                return Err(self.precision_error());
            }
            if self.c == 37 {
                if self.flags != LEFT_JUSTIFY && self.flags != 0 {
                    return Err(FormatError::Illegal(
                        "java.util.IllegalFormatFlagsException",
                        format!("Flags = '{}'", flags_to_string(self.flags)),
                    ));
                }
                if left_without_width {
                    return Err(self.missing_width());
                }
            } else {
                if self.width != -1 {
                    return Err(FormatError::Illegal(
                        "java.util.IllegalFormatWidthException",
                        self.width.to_string(),
                    ));
                }
                if self.flags != 0 {
                    return Err(FormatError::Illegal(
                        "java.util.IllegalFormatFlagsException",
                        format!("Flags = '{}'", flags_to_string(self.flags)),
                    ));
                }
            }
        } else {
            return Err(unknown_conversion(&unit_str(self.c)));
        }
        Ok(())
    }

    /// java.util.Formatter.FormatSpecifier.checkNumeric.
    fn check_numeric(&self) -> Result<(), FormatError> {
        if self.width == -1 && self.flags & (LEFT_JUSTIFY | ZERO_PAD) != 0 {
            return Err(self.missing_width());
        }
        if self.flags & (PLUS | LEADING_SPACE) == PLUS | LEADING_SPACE
            || self.flags & (LEFT_JUSTIFY | ZERO_PAD) == LEFT_JUSTIFY | ZERO_PAD
        {
            return Err(FormatError::Illegal(
                "java.util.IllegalFormatFlagsException",
                format!("Flags = '{}'", flags_to_string(self.flags)),
            ));
        }
        Ok(())
    }

    fn fail_conversion(&self, arg: &FormatArg) -> FormatError {
        FormatError::Illegal(
            "java.util.IllegalFormatConversionException",
            format!("{} != {}", unit_str(self.c), arg.class_name()),
        )
    }

    /// java.util.Formatter.FormatSpecifier.print(Formatter, Object, Locale).
    fn print(&self, out: &mut Vec<u16>, arg: Option<&FormatArg>) -> Result<(), FormatError> {
        let arg = arg.unwrap_or(&FormatArg::Null);
        if self.dt {
            // printDateTime: Long, Date, Calendar and TemporalAccessor print a date (not ported)
            return match arg {
                FormatArg::Null => self.print_string(out, "null"),
                FormatArg::Object { class, .. } => Err(FormatError::NotPorted(format!(
                    "Formatter date/time conversion of a {class}"
                ))),
                _ => Err(self.fail_conversion(arg)),
            };
        }
        match self.c {
            // printInteger, printFloat, printCharacter
            100 | 111 | 120 | 101 | 103 | 102 | 97 | 99 => match arg {
                FormatArg::Null => self.print_string(out, "null"),
                FormatArg::Double(_) if is_float(self.c) => Err(FormatError::NotPorted(
                    "Formatter floating-point conversion of a Double".into(),
                )),
                FormatArg::Object { class, .. } => Err(FormatError::NotPorted(format!(
                    "Formatter conversion '{}' of a {class}",
                    unit_str(self.c)
                ))),
                _ => Err(self.fail_conversion(arg)),
            },
            // printBoolean
            98 => {
                let s = match arg {
                    FormatArg::Null => "false",
                    FormatArg::Bool(false) => "false",
                    _ => "true",
                };
                self.print_string(out, s)
            }
            // printString (no argument class here is Formattable)
            115 => {
                if self.flags & ALTERNATE == ALTERNATE {
                    return Err(self.fail_mismatch(ALTERNATE));
                }
                match arg {
                    FormatArg::Null => self.print_string(out, "null"),
                    FormatArg::Str(s) => self.print_string(out, s),
                    FormatArg::Bool(b) => self.print_string(out, if *b { "true" } else { "false" }),
                    FormatArg::Double(d) => {
                        self.print_string(out, &crate::java::double_to_string(*d))
                    }
                    FormatArg::Object { string, .. } => self.print_string(out, string),
                }
            }
            // printHashCode: Integer.toHexString(arg.hashCode())
            104 => {
                let h = match arg {
                    FormatArg::Null => return self.print_string(out, "null"),
                    FormatArg::Str(s) => crate::java::string_hash_code(s),
                    FormatArg::Bool(b) => {
                        if *b {
                            1231
                        } else {
                            1237
                        }
                    }
                    FormatArg::Double(d) => {
                        let bits = if d.is_nan() {
                            0x7ff8_0000_0000_0000u64
                        } else {
                            d.to_bits()
                        };
                        (bits ^ (bits >> 32)) as i32
                    }
                    FormatArg::Object { hash_code, .. } => *hash_code,
                };
                self.print_string(out, &format!("{:x}", h as u32))
            }
            // System.lineSeparator()
            110 => {
                out.extend(LINE_SEPARATOR.encode_utf16());
                Ok(())
            }
            _ => self.print_string(out, "%"),
        }
    }

    /// java.util.Formatter.FormatSpecifier.print(Formatter, String, Locale): precision cuts UTF-16 units, then the
    /// upper case, then `appendJustified` pads with spaces to the width (in UTF-16 units). A cut through a surrogate
    /// pair keeps a lone surrogate in Java; here it becomes U+FFFD when the result is turned into a Rust string.
    fn print_string(&self, out: &mut Vec<u16>, s: &str) -> Result<(), FormatError> {
        let mut units: Vec<u16> = s.encode_utf16().collect();
        if self.precision != -1 && (self.precision as usize) < units.len() {
            units.truncate(self.precision as usize);
        }
        if self.flags & UPPERCASE == UPPERCASE {
            let upper = crate::java::to_upper_case(&String::from_utf16_lossy(&units));
            units = upper.encode_utf16().collect();
        }
        if self.width == -1 {
            out.extend_from_slice(&units);
            return Ok(());
        }
        let pad_right = self.flags & LEFT_JUSTIFY == LEFT_JUSTIFY;
        let sp = i64::from(self.width) - units.len() as i64;
        if pad_right {
            out.extend_from_slice(&units);
        }
        for _ in 0..sp.max(0) {
            out.push(32);
        }
        if !pad_right {
            out.extend_from_slice(&units);
        }
        Ok(())
    }
}

/// System.lineSeparator().
const LINE_SEPARATOR: &str = if cfg!(windows) { "\r\n" } else { "\n" };

/// java.util.Formatter.FormatSpecifierParser.parse: the length of the specifier at `start` (0: none), pushing it.
fn parse_specifier(
    al: &mut Vec<FormatString>,
    first: u16,
    start: usize,
    s: &[u16],
) -> Result<usize, FormatError> {
    let max = s.len();
    let mut off = start;
    let mut c = first;
    let mut arg_size = 0;
    let mut precision_size = 0;
    // parseArgument: (\d+\$)?
    {
        let mut i = off;
        while i < max && {
            c = s[i];
            is_digit(c)
        } {
            i += 1;
        }
        if i == off || c != 36 {
            c = first;
        } else {
            i += 1;
            if i < max {
                c = s[i];
            }
            arg_size = i - off;
            off = i;
        }
    }
    // parseFlag: ([-#+ 0,(\<]*)?
    let flag_size = {
        let mut i = off;
        while i < max && {
            c = s[i];
            is_flag(c)
        } {
            i += 1;
        }
        let n = i - off;
        off = i;
        n
    };
    // parseWidth: (\d+)?
    let width_size = {
        let mut i = off;
        while i < max && {
            c = s[i];
            is_digit(c)
        } {
            i += 1;
        }
        let n = i - off;
        off = i;
        n
    };
    if c == 46 {
        // parsePrecision: (\.\d+)?
        off += 1;
        let mut i = off;
        while i < max && {
            c = s[i];
            is_digit(c)
        } {
            i += 1;
        }
        if i != off {
            precision_size = i - off + 1;
            off = i;
        } else {
            return Ok(0);
        }
    }
    // ([tT])?([a-zA-Z%])
    let mut t: u16 = 0;
    let mut conversion: u16 = 0;
    if (c == 116 || c == 84) && off + 1 < max {
        let c1 = s[off + 1];
        if is_conversion(c1) {
            t = c;
            conversion = c1;
            off += 2;
        }
    } else if is_conversion(c) {
        conversion = c;
        off += 1;
    } else {
        return Ok(0);
    }
    if arg_size + flag_size + width_size + precision_size + t as usize + conversion as usize != 0 {
        al.push(FormatString::Spec(FormatSpecifier::new(
            s,
            start,
            arg_size,
            flag_size,
            width_size,
            precision_size,
            t,
            conversion,
        )?));
        return Ok(off - start);
    }
    Ok(0)
}

/// java.util.Formatter.parse: fixed text and format specifiers; a trailing `%` or an unparsable specifier is the
/// UnknownFormatConversionException.
fn parse(s: &[u16]) -> Result<Vec<FormatString>, FormatError> {
    let mut al = Vec::new();
    let mut i = 0;
    let max = s.len();
    while i < max {
        let Some(n) = s[i..].iter().position(|&u| u == 37).map(|p| p + i) else {
            al.push(FormatString::Fixed(i, max));
            break;
        };
        if i != n {
            al.push(FormatString::Fixed(i, n));
        }
        i = n + 1;
        if i >= max {
            return Err(unknown_conversion("%"));
        }
        let c = s[i];
        if conversion_is_valid(c) {
            al.push(FormatString::Spec(FormatSpecifier::from_char(c)));
            i += 1;
        } else {
            let off = parse_specifier(&mut al, c, i, s)?;
            if off > 0 {
                i += off;
            } else {
                return Err(unknown_conversion(&unit_str(c)));
            }
        }
    }
    Ok(al)
}

/// java.lang.String.formatted(Object...): `new Formatter().format(this, args).toString()` (java.util.Formatter.format:
/// ordinary specifiers take the next argument, `n$` the n-th, `<` the one used last; running out is the
/// MissingFormatArgumentException).
pub fn format(format: &str, args: &[FormatArg]) -> Result<String, FormatError> {
    let s: Vec<u16> = format.encode_utf16().collect();
    let fsa = parse(&s)?;
    let mut out: Vec<u16> = Vec::with_capacity(s.len());
    let mut last: i64 = -1;
    let mut lasto: i64 = -1;
    let n = args.len() as i64;
    for fs in &fsa {
        match fs {
            FormatString::Fixed(a, b) => out.extend_from_slice(&s[*a..*b]),
            FormatString::Spec(spec) => match spec.index {
                -2 => spec.print(&mut out, None)?,
                -1 => {
                    if last < 0 || last > n - 1 {
                        return Err(FormatError::Missing(spec.to_java_string()));
                    }
                    spec.print(&mut out, Some(&args[last as usize]))?;
                }
                0 => {
                    lasto += 1;
                    last = lasto;
                    if lasto > n - 1 {
                        return Err(FormatError::Missing(spec.to_java_string()));
                    }
                    spec.print(&mut out, Some(&args[lasto as usize]))?;
                }
                index => {
                    last = i64::from(index) - 1;
                    if last > n - 1 {
                        return Err(FormatError::Missing(spec.to_java_string()));
                    }
                    spec.print(&mut out, Some(&args[last as usize]))?;
                }
            },
        }
    }
    Ok(String::from_utf16_lossy(&out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> FormatArg {
        FormatArg::Str(v.into())
    }

    #[test]
    fn indexing_and_errors_follow_java() {
        assert_eq!(format("%2$s %s %<s", &[s("a"), s("b")]), Ok("b a a".into()));
        assert!(matches!(
            format("%s %s", &[s("a")]),
            Err(FormatError::Missing(_))
        ));
        assert!(matches!(format("50%", &[]), Err(FormatError::Illegal(..))));
        assert_eq!(
            format("[%-4s|%3.1S]", &[s("ab"), s("xyz")]),
            Ok("[ab  |  X]".into())
        );
    }
}
