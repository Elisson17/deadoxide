//! zombie.SandboxOptions: the world's rule settings. The option list is generated from the original class
//! (`sandbox_options_table.rs`); this module holds values, parsing and the `map_sand.bin` reader.
use std::collections::HashMap;

use super::core::translator::Translator;
use super::sandbox_options_table::OPTIONS;
use crate::java;
use crate::java_formatter::{self, FormatArg};
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::object::{rstr, Object, TableRef, R};

/// zombie.SandboxOptions.parseName: "Group.Name" is (group, name); anything else (no dot, or more than one) is the
/// short name alone.
pub fn parse_name(name: &str) -> (Option<&str>, &str) {
    let mut ss: Vec<&str> = name.split('.').collect();
    // String.split drops trailing empty strings
    while ss.len() > 1 && ss.last() == Some(&"") {
        ss.pop();
    }
    if name.contains('.') && ss.len() == 2 {
        (Some(ss[0]), ss[1])
    } else {
        (None, name)
    }
}

/// Definition of one option, as its `new...Option(...)` call declares it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Def {
    pub name: &'static str,
    pub kind: Kind,
    /// `.setTranslation(...)` of the declaration
    pub translation: Option<&'static str>,
    /// `.setValueTranslation(...)` of the declaration (enum options)
    pub value_translation: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Boolean(bool),
    Double {
        min: f64,
        max: f64,
        default: f64,
    },
    Integer {
        min: i32,
        max: i32,
        default: i32,
    },
    /// Values 1..=num_values.
    Enum {
        num_values: i32,
        default: i32,
    },
    Str {
        default: &'static str,
        max_length: i32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Boolean(bool),
    Double(f64),
    Integer(i32),
    Str(String),
}

/// java.lang.Double.parseDouble for the forms the game writes (decimal and exponent notation, surrounding blanks,
/// an optional f/d suffix); `None` is NumberFormatException. Hexadecimal floats, "NaN" and "Infinity" are not handled.
fn parse_double(s: &str) -> Option<f64> {
    let t = java::trim(s);
    let t = t.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(t);
    if t.is_empty()
        || !t
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.' | b'e' | b'E'))
    {
        return None;
    }
    t.parse::<f64>().ok()
}

/// java.lang.Double.toString: shortest digits that round-trip, plain notation for 1e-3 <= |v| < 1e7, otherwise
/// "d.dddE±n".
pub fn double_to_string(v: f64) -> String {
    if v == 0.0 {
        return if v.is_sign_negative() {
            "-0.0".into()
        } else {
            "0.0".into()
        };
    }
    let a = v.abs();
    if (1.0e-3..1.0e7).contains(&a) {
        let s = format!("{v}");
        return if s.contains('.') { s } else { format!("{s}.0") };
    }
    // Rust's {:e} prints the shortest round-trip digits as d.ddde±n
    let s = format!("{v:e}");
    let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
    let m = if m.contains('.') {
        m.to_string()
    } else {
        format!("{m}.0")
    };
    format!("{m}E{e}")
}

/// zombie.SandboxOptions
#[derive(Clone, Debug, PartialEq)]
pub struct SandboxOptions {
    values: Vec<Value>,
    /// ConfigOption.defaultValue of every option.
    default_values: Vec<Value>,
    by_name: HashMap<&'static str, usize>,
    /// `IsoWorld.getPreset()` as stored at the end of `map_sand.bin`.
    pub preset: String,
    /// "ERROR ..." lines the original would log while loading.
    pub log: Vec<String>,
}

impl Default for SandboxOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl SandboxOptions {
    /// Every option at its default value.
    pub fn new() -> Self {
        let values: Vec<Value> = OPTIONS
            .iter()
            .map(|d| match d.kind {
                Kind::Boolean(b) => Value::Boolean(b),
                Kind::Double { default, .. } => Value::Double(default),
                Kind::Integer { default, .. } | Kind::Enum { default, .. } => {
                    Value::Integer(default)
                }
                Kind::Str { default, .. } => Value::Str(default.to_string()),
            })
            .collect();
        // A repeated name keeps its last registration, as `optionByName.put` does.
        let by_name = OPTIONS
            .iter()
            .enumerate()
            .map(|(i, d)| (d.name, i))
            .collect();
        SandboxOptions {
            default_values: values.clone(),
            values,
            by_name,
            preset: String::new(),
            log: Vec::new(),
        }
    }

    pub fn defs(&self) -> &'static [Def] {
        &OPTIONS
    }
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.by_name.get(name).map(|i| &self.values[*i])
    }
    /// Value of an integer or enum option.
    pub fn get_int(&self, name: &str) -> Option<i32> {
        match self.get(name)? {
            Value::Integer(v) => Some(*v),
            _ => None,
        }
    }
    /// zombie.config.DoubleConfigOption.getValueAsString (and its siblings) of option `i`.
    pub fn value_as_string(&self, i: usize) -> String {
        match &self.values[i] {
            Value::Boolean(b) => b.to_string(),
            Value::Double(d) => double_to_string(*d),
            Value::Integer(v) => v.to_string(),
            Value::Str(s) => s.clone(),
        }
    }

    /// zombie.SandboxOptions.set(name, o): `setValueFromObject` of the named option; false when there is none (the
    /// original throws IllegalArgumentException).
    pub fn set_by_name(&mut self, name: &str, o: &Object) -> bool {
        match self.by_name.get(name).copied() {
            Some(i) => {
                self.set_value_from_object(i, o);
                true
            }
            None => false,
        }
    }

    /// SandboxOptions.getNumOptions
    pub fn get_num_options(&self) -> usize {
        OPTIONS.len()
    }

    /// SandboxOptions.getOptionByName: the option's index.
    pub fn get_option_by_name(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }

    /// ConfigOption.getType of option `i`'s class.
    pub fn get_type(&self, i: usize) -> &'static str {
        match OPTIONS[i].kind {
            Kind::Boolean(_) => "boolean",
            Kind::Double { .. } => "double",
            Kind::Integer { .. } => "integer",
            Kind::Enum { .. } => "enum",
            Kind::Str { .. } => "string",
        }
    }

    /// EnumConfigOption.getNumValues of option `i` (`None`: not an enum option).
    pub fn get_num_values(&self, i: usize) -> Option<i32> {
        match OPTIONS[i].kind {
            Kind::Enum { num_values, .. } => Some(num_values),
            _ => None,
        }
    }

    /// `translation == null ? getShortName() : translation`, the key part of every *SandboxOption text.
    fn translation_key(&self, i: usize) -> &'static str {
        let d = &OPTIONS[i];
        d.translation.unwrap_or_else(|| parse_name(d.name).1)
    }

    /// SandboxOptions$*SandboxOption.getTranslatedName: `Translator.getText("Sandbox_" + key)`.
    pub fn get_translated_name(
        &self,
        i: usize,
        translator: &mut Translator,
    ) -> Result<String, String> {
        translator.get_text(&format!("Sandbox_{}", self.translation_key(i)), &[])
    }

    /// EnumSandboxOption.getValueTranslation: `valueTranslation`, else the translation key.
    fn value_translation(&self, i: usize) -> &'static str {
        OPTIONS[i]
            .value_translation
            .unwrap_or_else(|| self.translation_key(i))
    }

    /// EnumSandboxOption.getValueTranslationByIndex (`or_null`: getValueTranslationByIndexOrNull):
    /// `Sandbox_<valueTranslation>_option<index>`; `Err` is the ArrayIndexOutOfBoundsException outside
    /// 1..=getNumValues (and a translator failure the port has not written).
    pub fn get_value_translation_by_index(
        &self,
        i: usize,
        index: i32,
        or_null: bool,
        translator: &mut Translator,
    ) -> Result<Option<String>, String> {
        let n = self.get_num_values(i).unwrap_or(0);
        if index < 1 || index > n {
            return Err("java.lang.ArrayIndexOutOfBoundsException".into());
        }
        let key = format!("Sandbox_{}_option{index}", self.value_translation(i));
        if or_null {
            translator.get_text_or_null(&key, &[])
        } else {
            translator.get_text(&key, &[]).map(Some)
        }
    }

    /// SandboxOptions$*SandboxOption.getTooltip: `Sandbox_<key>_tooltip` (`_help` for the integer and double
    /// options of ZombieConfig) or null, then for an enum `Sandbox_Default` with the default's value text and for
    /// a number `Sandbox_MinMaxDefault` (doubles as `%.02f`, ints as Integers), joined by a backslash and n.
    pub fn get_tooltip(
        &self,
        i: usize,
        translator: &mut Translator,
    ) -> Result<Option<String>, String> {
        let d = &OPTIONS[i];
        let key = self.translation_key(i);
        let zombie_config = parse_name(d.name).0 == Some("ZombieConfig");
        let suffix = match d.kind {
            Kind::Integer { .. } | Kind::Double { .. } if zombie_config => "_help",
            _ => "_tooltip",
        };
        let s1 = translator.get_text_or_null(&format!("Sandbox_{key}{suffix}"), &[])?;
        let s2 = match d.kind {
            Kind::Boolean(_) | Kind::Str { .. } => return Ok(s1),
            Kind::Enum { .. } => {
                let default = match self.default_value(i) {
                    Value::Integer(v) => *v,
                    _ => 0,
                };
                match self.get_value_translation_by_index(i, default, true, translator)? {
                    None => None,
                    Some(value) => {
                        Some(translator.get_text("Sandbox_Default", &[FormatArg::Str(value)])?)
                    }
                }
            }
            Kind::Double { min, max, .. } => {
                let default = match self.default_value(i) {
                    Value::Double(v) => *v,
                    _ => 0.0,
                };
                let f = |v: f64| {
                    java_formatter::format("%.02f", &[FormatArg::Double(v)])
                        .map(FormatArg::Str)
                        .map_err(|_| "not ported: String.format(\"%.02f\")".to_string())
                };
                Some(
                    translator
                        .get_text("Sandbox_MinMaxDefault", &[f(min)?, f(max)?, f(default)?])?,
                )
            }
            Kind::Integer { min, max, .. } => {
                let default = match self.default_value(i) {
                    Value::Integer(v) => *v,
                    _ => 0,
                };
                let int = |v: i32| FormatArg::Object {
                    class: "java.lang.Integer".into(),
                    string: v.to_string(),
                    hash_code: v,
                };
                Some(
                    translator
                        .get_text("Sandbox_MinMaxDefault", &[int(min), int(max), int(default)])?,
                )
            }
        };
        Ok(match (s1, s2) {
            (None, s2) => s2,
            (Some(s1), None) => Some(s1),
            (Some(s1), Some(s2)) => Some(format!("{s1}\\n{s2}")),
        })
    }

    /// zombie.config.IntegerConfigOption.parse (and the Boolean/Double/String ones) for option `i`: an unparsable or out-of-range value is logged and the
    /// option keeps its value.
    pub fn parse(&mut self, i: usize, s: &str) {
        let d = &OPTIONS[i];
        match d.kind {
            Kind::Boolean(_) => {
                let is = |x: &str| s.eq_ignore_ascii_case(x);
                if is("true") || is("false") || is("1") || is("0") {
                    self.values[i] = Value::Boolean(is("true") || is("1"));
                } else {
                    self.log.push(format!(
                        "ERROR BooleanConfigOption.parse() \"{}\" string={s}\"",
                        d.name
                    ));
                }
            }
            Kind::Double { min, max, .. } => match parse_double(s) {
                Some(v) if v < min => self.log.push(format!(
                    "ERROR: DoubleConfigOption.setValue() \"{}\" {} is less than min={}",
                    d.name,
                    double_to_string(v),
                    double_to_string(min)
                )),
                Some(v) if v > max => self.log.push(format!(
                    "ERROR: DoubleConfigOption.setValue() \"{}\" {} is greater than max={}",
                    d.name,
                    double_to_string(v),
                    double_to_string(max)
                )),
                Some(v) => self.values[i] = Value::Double(v),
                None => self.log.push(format!(
                    "ERROR DoubleConfigOption.parse() \"{}\" string=\"{s}\"",
                    d.name
                )),
            },
            Kind::Integer { .. } | Kind::Enum { .. } => {
                let (min, max) = match d.kind {
                    Kind::Integer { min, max, .. } => (min, max),
                    Kind::Enum { num_values, .. } => (1, num_values),
                    _ => (0, 0),
                };
                match parse_double(s) {
                    // (int) of a double saturates in Java as `as i32` does in Rust.
                    Some(v) if (v as i32) < min => self.log.push(format!(
                        "ERROR: IntegerConfigOption.setValue() \"{}\" {} is less than min={min}",
                        d.name, v as i32
                    )),
                    Some(v) if (v as i32) > max => self.log.push(format!(
                        "ERROR: IntegerConfigOption.setValue() \"{}\" {} is greater than max={max}",
                        d.name, v as i32
                    )),
                    Some(v) => self.values[i] = Value::Integer(v as i32),
                    None => self.log.push(format!(
                        "ERROR IntegerConfigOption.parse() \"{}\" string=\"{s}\"",
                        d.name
                    )),
                }
            }
            // Length limits of string options are not applied yet.
            Kind::Str { .. } => self.values[i] = Value::Str(s.to_string()),
        }
    }

    /// zombie.config.DoubleConfigOption.setValue / IntegerConfigOption.setValue (EnumConfigOption is an integer
    /// option with min 1 and max `numValues`) / BooleanConfigOption.setValue for option `i`: an out-of-range value
    /// is logged and the option keeps its value.
    fn set_number(&mut self, i: usize, v: f64) {
        let d = &OPTIONS[i];
        match d.kind {
            Kind::Boolean(_) => self.values[i] = Value::Boolean(v != 0.0),
            Kind::Double { min, max, .. } => {
                if v < min {
                    self.log.push(format!(
                        "ERROR: DoubleConfigOption.setValue() \"{}\" {} is less than min={}",
                        d.name,
                        double_to_string(v),
                        double_to_string(min)
                    ));
                } else if v > max {
                    self.log.push(format!(
                        "ERROR: DoubleConfigOption.setValue() \"{}\" {} is greater than max={}",
                        d.name,
                        double_to_string(v),
                        double_to_string(max)
                    ));
                } else {
                    self.values[i] = Value::Double(v);
                }
            }
            Kind::Integer { .. } | Kind::Enum { .. } => {
                let (min, max) = match d.kind {
                    Kind::Integer { min, max, .. } => (min, max),
                    Kind::Enum { num_values, .. } => (1, num_values),
                    _ => (0, 0),
                };
                // Double.intValue(): saturating, NaN -> 0, as `as i32` is.
                let v = v as i32;
                if v < min {
                    self.log.push(format!(
                        "ERROR: IntegerConfigOption.setValue() \"{}\" {v} is less than min={min}",
                        d.name
                    ));
                } else if v > max {
                    self.log.push(format!(
                        "ERROR: IntegerConfigOption.setValue() \"{}\" {v} is greater than max={max}",
                        d.name
                    ));
                } else {
                    self.values[i] = Value::Integer(v);
                }
            }
            Kind::Str { .. } => {
                self.values[i] = Value::Str(crate::java::double_to_string(v));
            }
        }
    }

    /// zombie.config.BooleanConfigOption.setValueFromObject (and the Double/Integer/String ones) for option `i`
    /// with a Lua value: a boolean or number sets it, a string is parsed; anything else is ignored, except by a
    /// string option, which takes `toString()` of it.
    pub fn set_value_from_object(&mut self, i: usize, o: &Object) {
        let string_option = matches!(OPTIONS[i].kind, Kind::Str { .. });
        match o {
            Object::Boolean(b) if matches!(OPTIONS[i].kind, Kind::Boolean(_)) => {
                self.values[i] = Value::Boolean(*b)
            }
            Object::Number(d) if !string_option => self.set_number(i, *d),
            Object::String(s) => self.parse(i, &rstr(s)),
            Object::Nil => {
                if string_option {
                    self.values[i] = Value::Str(String::new());
                }
            }
            other => {
                if string_option {
                    self.values[i] = Value::Str(other.java_to_string());
                }
            }
        }
    }

    /// zombie.config.BooleanConfigOption.getValueAsObject (and the others; an integer or enum option gives a
    /// double) of option `i`.
    pub fn value_as_object(&self, i: usize) -> Object {
        match &self.values[i] {
            Value::Boolean(b) => Object::Boolean(*b),
            Value::Double(d) => Object::Number(*d),
            Value::Integer(v) => Object::Number(f64::from(*v)),
            Value::Str(s) => Object::str(s),
        }
    }

    /// zombie.SandboxOptions.BooleanSandboxOption.fromTable (the same in every option class): the value under the
    /// option's short name, inside its group table when the name has one ("ZombieLore.Speed"); a missing group or
    /// value leaves the option as it is.
    pub fn from_table(&mut self, i: usize, table: &TableRef) -> R<()> {
        let (table_name, short_name) = parse_name(OPTIONS[i].name);
        let mut table = table.clone();
        if let Some(t) = table_name {
            match KahluaTableImpl::rawget(&table, &Object::str(t))? {
                Object::Table(sub) => table = sub,
                _ => return Ok(()),
            }
        }
        let o = KahluaTableImpl::rawget(&table, &Object::str(short_name))?;
        if !o.is_nil() {
            self.set_value_from_object(i, &o);
        }
        Ok(())
    }

    /// zombie.SandboxOptions.BooleanSandboxOption.toTable (the same in every option class): stores the value under
    /// the short name, creating the group table when it is missing.
    pub fn to_table(&self, i: usize, table: &TableRef) -> R<()> {
        let (table_name, short_name) = parse_name(OPTIONS[i].name);
        let mut table = table.clone();
        if let Some(t) = table_name {
            match KahluaTableImpl::rawget(&table, &Object::str(t))? {
                Object::Table(sub) => table = sub,
                _ => {
                    let sub = KahluaTableImpl::new_ref();
                    KahluaTableImpl::rawset(&table, Object::str(t), Object::Table(sub.clone()));
                    table = sub;
                }
            }
        }
        KahluaTableImpl::rawset(&table, Object::str(short_name), self.value_as_object(i));
        Ok(())
    }

    /// zombie.SandboxOptions.initSandboxVars: every option reads `vars`, then writes its value back.
    pub fn init_sandbox_vars(&mut self, vars: &TableRef) -> R<()> {
        for i in 0..OPTIONS.len() {
            self.from_table(i, vars)?;
            self.to_table(i, vars)?;
        }
        Ok(())
    }

    /// zombie.SandboxOptions.setDefaultsToCurrentValues
    pub fn set_defaults_to_current_values(&mut self) {
        self.default_values = self.values.clone();
    }

    /// The value `resetToDefault` would restore for option `i`.
    pub fn default_value(&self, i: usize) -> &Value {
        &self.default_values[i]
    }

    /// zombie.SandboxOptions.loadGameFile: applies a preset table (`media/lua/shared/Sandbox/<name>.lua`, a Lua file
    /// that returns a table of options, with one level of group tables such as `ZombieLore = { ... }`). The file is
    /// read as data here, not executed. The original constructor applies "Apocalypse" and takes the result as its
    /// defaults.
    pub fn load_game_file(&mut self, text: &str) {
        let mut group = String::new();
        for line in text.lines() {
            let l = java::trim(line);
            let l = l.strip_suffix(',').unwrap_or(l);
            if l == "}" {
                group.clear();
                continue;
            }
            let Some((k, v)) = l.split_once('=') else {
                continue;
            };
            let (k, v) = (java::trim(k), java::trim(v));
            if v == "{" {
                group = format!("{k}.");
                continue;
            }
            let name = format!("{group}{k}");
            if let Some(i) = self.by_name.get(name.as_str()).copied() {
                // A Lua string value arrives without its quotes.
                let v = v
                    .strip_prefix('"')
                    .and_then(|x| x.strip_suffix('"'))
                    .unwrap_or(v);
                self.parse(i, v);
            }
        }
    }

    /// zombie.SandboxOptions.load (ByteBuffer): the contents of a save's `map_sand.bin` ("SAND", world version, options
    /// version, count, then name/value strings, then the preset name). Strings are `GameWindow.ReadString`: a
    /// big-endian short length and UTF-8 bytes. The value upgrades for option versions below 6 are not ported; such a
    /// file is refused.
    pub fn load(&mut self, input: &[u8]) -> Result<(), String> {
        let mut p = 4usize;
        let eof = "java.nio.BufferUnderflowException";
        let int = |p: &mut usize| -> Result<i32, String> {
            let b = input.get(*p..*p + 4).ok_or(eof)?;
            *p += 4;
            Ok(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        };
        let _world_version = int(&mut p)?;
        let version = int(&mut p)?;
        if version < 6 {
            return Err(format!(
                "sandbox options version {version}: upgrades from versions below 6 are not ported"
            ));
        }
        let count = int(&mut p)?;
        let string = |p: &mut usize| -> Result<String, String> {
            let b = input.get(*p..*p + 2).ok_or(eof)?;
            let n = i16::from_be_bytes([b[0], b[1]]).max(0) as usize;
            *p += 2;
            let s = input.get(*p..*p + n).ok_or(eof)?;
            *p += n;
            Ok(String::from_utf8_lossy(s).into_owned())
        };
        for _ in 0..count {
            let name = string(&mut p)?;
            let value = string(&mut p)?;
            match self.by_name.get(name.as_str()).copied() {
                Some(i) => self.parse(i, &value),
                None => self
                    .log
                    .push(format!("ERROR unknown SandboxOption \"{name}\"")),
            }
        }
        self.preset = string(&mut p)?;
        Ok(())
    }
}
