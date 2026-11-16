//! zombie.config.{Integer,Boolean,Double,String,Array}ConfigOption as used by zombie.core.Core's options
//! (`newOption` / `newFakeOption`), and the option lists of Core.
//!
//! The change callbacks (`ConfigOption.invokeOnChangeEvent`) are not run here: `set_value` returns whether the value
//! changed and the Core method that owns the option runs its callback (only `controllerButtonStyle` and
//! `gamepadBindingPreset` have one). Messages that the original sends to `DebugLog.log` / `DebugType.General.println`
//! are pushed on the `log` the caller passes.
use std::io::Write;
use std::path::Path;

use super::core_options_table::declare;
use super::performance_settings::PerformanceSettings;
use crate::java::{compare_to_ignore_case, double_to_string, parse_double, parse_int};

/// A value as `setValueFromObject(Object)` sees it: a `Double` (Lua number), a `String`, a `Boolean`, or null.
#[derive(Clone, Debug)]
pub enum Arg {
    Null,
    Double(f64),
    Str(String),
    Bool(bool),
}

impl Arg {
    /// `String.valueOf(o)` / `"" + o` of the object.
    pub fn to_java_string(&self) -> String {
        match self {
            Arg::Null => "null".into(),
            Arg::Double(d) => double_to_string(*d),
            Arg::Str(s) => s.clone(),
            Arg::Bool(b) => b.to_string(),
        }
    }
}

/// zombie.config.IntegerConfigOption.
#[derive(Clone, Debug)]
pub struct IntOpt {
    pub name: String,
    pub value: i32,
    pub default: i32,
    pub min: i32,
    pub max: i32,
}

/// zombie.config.BooleanConfigOption.
#[derive(Clone, Debug)]
pub struct BoolOpt {
    pub name: String,
    pub value: bool,
    pub default: bool,
}

/// zombie.config.DoubleConfigOption.
#[derive(Clone, Debug)]
pub struct DoubleOpt {
    pub name: String,
    pub value: f64,
    pub default: f64,
    pub min: f64,
    pub max: f64,
}

/// zombie.config.StringConfigOption.
#[derive(Clone, Debug)]
pub struct StrOpt {
    pub name: String,
    pub value: String,
    pub default: String,
    pub max_length: i32,
    pub values: Option<Vec<String>>,
}

/// zombie.config.ArrayConfigOption.
#[derive(Clone, Debug)]
pub struct ArrayOpt {
    pub name: String,
    pub value: Vec<Opt>,
    pub default: Vec<Opt>,
    pub separator: String,
    pub handler: Opt,
    pub fixed_size: i32,
    pub multi_line: bool,
}

/// zombie.config.ConfigOption of one of the five classes.
#[derive(Clone, Debug)]
pub enum Opt {
    Int(IntOpt),
    Bool(BoolOpt),
    Double(DoubleOpt),
    Str(StrOpt),
    Array(Box<ArrayOpt>),
}

/// `String.split(separator)` with a one-character separator that is not a regex metacharacter (",", ";"): no match
/// gives the input itself, and trailing empty strings are dropped.
fn java_split(s: &str, sep: &str) -> Vec<String> {
    if !s.contains(sep) {
        return vec![s.to_string()];
    }
    let mut parts: Vec<String> = s.split(sep).map(str::to_string).collect();
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

impl Opt {
    /// `new IntegerConfigOption(name, min, max, defaultValue)`.
    pub fn int(name: &str, min: i32, max: i32, default: i32) -> Opt {
        debug_assert!(default >= min && default <= max);
        Opt::Int(IntOpt {
            name: name.into(),
            value: default,
            default,
            min,
            max,
        })
    }

    /// `new BooleanConfigOption(name, defaultValue)`.
    pub fn boolean(name: &str, default: bool) -> Opt {
        Opt::Bool(BoolOpt {
            name: name.into(),
            value: default,
            default,
        })
    }

    /// `new DoubleConfigOption(name, min, max, defaultValue)`.
    pub fn double(name: &str, min: f64, max: f64, default: f64) -> Opt {
        debug_assert!(default >= min && default <= max);
        Opt::Double(DoubleOpt {
            name: name.into(),
            value: default,
            default,
            min,
            max,
        })
    }

    /// `new StringConfigOption(name, defaultValue, maxLength)`.
    pub fn string_max(name: &str, default: &str, max_length: i32) -> Opt {
        Opt::Str(StrOpt {
            name: name.into(),
            value: default.into(),
            default: default.into(),
            max_length,
            values: None,
        })
    }

    /// `new StringConfigOption(name, defaultValue, onChange)`: no maximum length.
    pub fn string_callback(name: &str, default: &str) -> Opt {
        Opt::string_max(name, default, 0)
    }

    /// `new StringConfigOption(name, defaultValue, values)`: the default when it is one of the values, else the
    /// first value.
    pub fn string_values(name: &str, default: &str, values: &[&str]) -> Opt {
        let value = if values.contains(&default) {
            default
        } else {
            values[0]
        };
        Opt::Str(StrOpt {
            name: name.into(),
            value: value.into(),
            default: value.into(),
            max_length: 0,
            values: Some(values.iter().map(|v| v.to_string()).collect()),
        })
    }

    /// `new ArrayConfigOption(name, elementHandler, separator, defaultValue)`.
    pub fn array(name: &str, handler: Opt, separator: &str, default: &str) -> Opt {
        let mut a = Opt::Array(Box::new(ArrayOpt {
            name: name.into(),
            value: Vec::new(),
            default: Vec::new(),
            separator: separator.into(),
            handler,
            fixed_size: -1,
            multi_line: false,
        }));
        let mut log = Vec::new();
        a.parse(default, &mut log);
        a.set_default_to_current_value();
        a
    }

    /// `ArrayConfigOption.setFixedSize(size)`.
    pub fn fixed_size(mut self, size: i32) -> Opt {
        if let Opt::Array(a) = &mut self {
            if size > 0 {
                a.resize(false, size);
                a.resize(true, size);
            } else {
                a.fixed_size = -1;
            }
        }
        self
    }

    /// `ArrayConfigOption.setMultiLine(true)`.
    pub fn multi_line(mut self) -> Opt {
        if let Opt::Array(a) = &mut self {
            a.multi_line = true;
        }
        self
    }

    /// `ConfigOption.getName`.
    pub fn name(&self) -> &str {
        match self {
            Opt::Int(o) => &o.name,
            Opt::Bool(o) => &o.name,
            Opt::Double(o) => &o.name,
            Opt::Str(o) => &o.name,
            Opt::Array(o) => &o.name,
        }
    }

    /// `ConfigOption.getType`.
    pub fn get_type(&self) -> &'static str {
        match self {
            Opt::Int(_) => "integer",
            Opt::Bool(_) => "boolean",
            Opt::Double(_) => "double",
            Opt::Str(_) => "string",
            Opt::Array(_) => "array",
        }
    }

    /// `*ConfigOption.resetToDefault`. Returns whether the value changed.
    pub fn reset_to_default(&mut self, log: &mut Vec<String>) -> bool {
        match self {
            Opt::Int(o) => {
                let d = o.default;
                o.set_value(d, log)
            }
            Opt::Bool(o) => {
                o.value = o.default;
                false
            }
            Opt::Double(o) => {
                let d = o.default;
                o.set_value(d, log);
                false
            }
            Opt::Str(o) => {
                let d = o.default.clone();
                o.set_value_internal(d)
            }
            Opt::Array(o) => {
                o.value = o.default.clone();
                false
            }
        }
    }

    /// `*ConfigOption.setDefaultToCurrentValue`.
    pub fn set_default_to_current_value(&mut self) {
        match self {
            Opt::Int(o) => o.default = o.value,
            Opt::Bool(o) => o.default = o.value,
            Opt::Double(o) => o.default = o.value,
            Opt::Str(o) => o.default = o.value.clone(),
            Opt::Array(o) => o.default = o.value.clone(),
        }
    }

    /// `*ConfigOption.parse`.
    pub fn parse(&mut self, s: &str, log: &mut Vec<String>) {
        match self {
            Opt::Int(o) => match parse_double(s) {
                Some(v) => {
                    o.set_value(v as i32, log);
                }
                None => log.push(format!(
                    "ERROR IntegerConfigOption.parse() \"{}\" string=\"{s}\"",
                    o.name
                )),
            },
            Opt::Bool(o) => {
                if Opt::bool_is_valid(s) {
                    o.value = s.eq_ignore_ascii_case("true") || s == "1";
                } else {
                    log.push(format!(
                        "ERROR BooleanConfigOption.parse() \"{}\" string={s}\"",
                        o.name
                    ));
                }
            }
            Opt::Double(o) => match parse_double(s) {
                Some(v) => o.set_value(v, log),
                None => log.push(format!(
                    "ERROR DoubleConfigOption.parse() \"{}\" string=\"{s}\"",
                    o.name
                )),
            },
            Opt::Str(_) | Opt::Array(_) => {
                self.set_value_from_object(&Arg::Str(s.to_string()), log);
            }
        }
    }

    fn bool_is_valid(s: &str) -> bool {
        s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("false") || s == "1" || s == "0"
    }

    /// `*ConfigOption.getValueAsString`.
    pub fn value_as_string(&self) -> String {
        match self {
            Opt::Int(o) => o.value.to_string(),
            Opt::Bool(o) => o.value.to_string(),
            Opt::Double(o) => double_to_string(o.value),
            Opt::Str(o) => o.value.clone(),
            Opt::Array(o) => o.value_as_string(false),
        }
    }

    /// `*ConfigOption.setValueFromObject`.
    pub fn set_value_from_object(&mut self, a: &Arg, log: &mut Vec<String>) {
        match self {
            Opt::Int(o) => match a {
                Arg::Double(d) => {
                    o.set_value(*d as i32, log);
                }
                Arg::Str(s) => {
                    let s = s.clone();
                    self.parse(&s, log);
                }
                _ => {}
            },
            Opt::Bool(o) => match a {
                Arg::Bool(b) => o.value = *b,
                Arg::Double(d) => o.value = *d != 0.0,
                Arg::Str(s) => {
                    let s = s.clone();
                    self.parse(&s, log);
                }
                Arg::Null => {}
            },
            Opt::Double(o) => match a {
                Arg::Double(d) => o.set_value(*d, log),
                Arg::Str(s) => {
                    let s = s.clone();
                    self.parse(&s, log);
                }
                _ => {}
            },
            Opt::Str(o) => {
                if let Some(values) = &o.values {
                    if let Arg::Str(s) = a {
                        if let Some(v) = values.iter().find(|v| *v == s) {
                            let v = v.clone();
                            o.set_value_internal(v);
                            return;
                        }
                    }
                    log.push(format!(
                        "General: ERROR: StringConfigOption.setValueFromObject() \"{}\" value \"{}\" is unknown",
                        o.name,
                        a.to_java_string()
                    ));
                } else {
                    match a {
                        Arg::Null => o.set_value_internal(String::new()),
                        _ => o.set_value_internal(a.to_java_string()),
                    };
                }
            }
            Opt::Array(o) => o.set_value_from_object(a, log),
        }
    }

    /// `*ConfigOption.isValidString`.
    pub fn is_valid_string(&self, s: &str) -> bool {
        match self {
            Opt::Int(o) => parse_int(s).is_some_and(|v| v >= o.min && v <= o.max),
            Opt::Bool(_) => Opt::bool_is_valid(s),
            Opt::Double(o) => parse_double(s).is_some_and(|v| v >= o.min && v <= o.max),
            Opt::Str(o) => match &o.values {
                Some(values) => values.iter().any(|v| v == s),
                None => true,
            },
            Opt::Array(o) => {
                if crate::java::trim(s).is_empty() {
                    return true;
                }
                java_split(s, &o.separator)
                    .iter()
                    .all(|part| o.handler.is_valid_string(part))
            }
        }
    }

    /// `ConfigOption.makeCopy` (the copy has no change callback).
    pub fn make_copy(&self) -> Opt {
        match self {
            Opt::Array(o) => {
                // new ArrayConfigOption(name, elementHandler, separator, defaultValueStr), then copyValue(value, copy.value)
                let default_str = o.value_as_string(true);
                let mut copy = Opt::array(&o.name, o.handler.clone(), &o.separator, &default_str);
                if let Opt::Array(c) = &mut copy {
                    c.value = o.value.iter().map(Opt::make_copy).collect();
                }
                copy
            }
            o => o.clone(),
        }
    }

    // ---- typed reads: the Core option getters only ask for the kind they declared ----

    /// `IntegerConfigOption.getValue`.
    pub fn int_value(&self) -> i32 {
        match self {
            Opt::Int(o) => o.value,
            _ => unreachable!("not an IntegerConfigOption: {}", self.name()),
        }
    }

    /// `BooleanConfigOption.getValue`.
    pub fn bool_value(&self) -> bool {
        match self {
            Opt::Bool(o) => o.value,
            _ => unreachable!("not a BooleanConfigOption: {}", self.name()),
        }
    }

    /// `DoubleConfigOption.getValue`.
    pub fn double_value(&self) -> f64 {
        match self {
            Opt::Double(o) => o.value,
            _ => unreachable!("not a DoubleConfigOption: {}", self.name()),
        }
    }

    /// `StringConfigOption.getValue`.
    pub fn str_value(&self) -> &str {
        match self {
            Opt::Str(o) => &o.value,
            _ => unreachable!("not a StringConfigOption: {}", self.name()),
        }
    }

    /// The array behind an `ArrayConfigOption`.
    pub fn array_value(&self) -> &ArrayOpt {
        match self {
            Opt::Array(a) => a,
            _ => unreachable!("not an ArrayConfigOption: {}", self.name()),
        }
    }

    /// `ArrayConfigOption`, mutable.
    pub fn array_mut(&mut self) -> &mut ArrayOpt {
        match self {
            Opt::Array(a) => a,
            _ => unreachable!("not an ArrayConfigOption"),
        }
    }
}

impl IntOpt {
    /// zombie.config.IntegerConfigOption.setValue. Returns whether the value changed (the original then runs
    /// `invokeOnChangeEvent`).
    pub fn set_value(&mut self, value: i32, log: &mut Vec<String>) -> bool {
        if value < self.min {
            log.push(format!(
                "ERROR: IntegerConfigOption.setValue() \"{}\" {value} is less than min={}",
                self.name, self.min
            ));
            false
        } else if value > self.max {
            log.push(format!(
                "ERROR: IntegerConfigOption.setValue() \"{}\" {value} is greater than max={}",
                self.name, self.max
            ));
            false
        } else if self.value != value {
            self.value = value;
            true
        } else {
            false
        }
    }
}

impl DoubleOpt {
    /// zombie.config.DoubleConfigOption.setValue.
    pub fn set_value(&mut self, value: f64, log: &mut Vec<String>) {
        if value < self.min {
            log.push(format!(
                "ERROR: DoubleConfigOption.setValue() \"{}\" {} is less than min={}",
                self.name,
                double_to_string(value),
                double_to_string(self.min)
            ));
        } else if value > self.max {
            log.push(format!(
                "ERROR: DoubleConfigOption.setValue() \"{}\" {} is greater than max={}",
                self.name,
                double_to_string(value),
                double_to_string(self.max)
            ));
        } else {
            self.value = value;
        }
    }
}

impl StrOpt {
    /// zombie.config.StringConfigOption.setValue.
    pub fn set_value(&mut self, value: Option<&str>, log: &mut Vec<String>) -> bool {
        if let Some(values) = &self.values {
            match value {
                Some(v) if values.iter().any(|x| x == v) => self.set_value_internal(v.to_string()),
                _ => {
                    log.push(format!(
                        "General: ERROR StringConfigOption.setValue() \"{}\" string=\"{}\"",
                        self.name,
                        value.unwrap_or("null")
                    ));
                    false
                }
            }
        } else {
            let mut v = value.unwrap_or("").to_string();
            if self.max_length > 0 && v.encode_utf16().count() > self.max_length as usize {
                let units: Vec<u16> = v.encode_utf16().take(self.max_length as usize).collect();
                v = String::from_utf16_lossy(&units);
            }
            self.set_value_internal(v)
        }
    }

    /// zombie.config.StringConfigOption.setValueInternal (the split-values cache is not kept). Returns whether the
    /// value changed.
    fn set_value_internal(&mut self, value: String) -> bool {
        if self.value != value {
            self.value = value;
            true
        } else {
            false
        }
    }
}

impl ArrayOpt {
    /// ArrayConfigOption.getValueAsString(value) of the current (`default == false`) or the default values.
    fn value_as_string(&self, default: bool) -> String {
        let list = if default { &self.default } else { &self.value };
        list.iter()
            .map(Opt::value_as_string)
            .collect::<Vec<_>>()
            .join(&self.separator)
    }

    /// ArrayConfigOption.resize(value, newSize): sets `fixedSize` and pads/truncates the current or default list.
    fn resize(&mut self, default: bool, new_size: i32) {
        self.fixed_size = new_size;
        let mut log = Vec::new();
        self.handler.reset_to_default(&mut log);
        let handler = self.handler.clone();
        let list = if default {
            &mut self.default
        } else {
            &mut self.value
        };
        while (list.len() as i32) < new_size {
            list.push(handler.make_copy());
        }
        while (list.len() as i32) > new_size {
            list.pop();
        }
    }

    /// ArrayConfigOption.setValueFromObject.
    fn set_value_from_object(&mut self, a: &Arg, log: &mut Vec<String>) {
        let Arg::Str(s) = a else {
            log.push(format!(
                "ERROR ArrayConfigOption.setValueFromObject() \"{}\" string={}\"",
                self.name,
                a.to_java_string()
            ));
            return;
        };
        if !self.is_valid(s) {
            log.push(format!(
                "ERROR ArrayConfigOption.setValueFromObject() \"{}\" string={s}\"",
                self.name
            ));
            return;
        }
        if !self.multi_line {
            self.value.clear();
        }
        if !crate::java::trim(s).is_empty() {
            for part in java_split(s, &self.separator) {
                self.handler.reset_to_default(log);
                self.handler.parse(&part, log);
                self.value.push(self.handler.make_copy());
            }
        }
        if self.fixed_size > 0 {
            self.resize(false, self.fixed_size);
        }
    }

    fn is_valid(&self, s: &str) -> bool {
        if crate::java::trim(s).is_empty() {
            return true;
        }
        java_split(s, &self.separator)
            .iter()
            .all(|part| self.handler.is_valid_string(part))
    }

    /// ArrayConfigOption.setValueVarArgs.
    pub fn set_value_var_args(&mut self, args: &[Arg], log: &mut Vec<String>) {
        let mut size = args.len();
        if self.fixed_size > 0 {
            size = size.min(self.fixed_size as usize);
        } else {
            self.resize(false, args.len() as i32);
        }
        for (i, a) in args.iter().enumerate().take(size) {
            self.value[i].set_value_from_object(a, log);
        }
    }

    /// ArrayConfigOption.size.
    pub fn size(&self) -> usize {
        self.value.len()
    }

    /// ArrayConfigOption.getElementAsDouble.
    pub fn element_as_double(&self, index: i32, default: f32) -> f64 {
        if index >= 0
            && (index as usize) < self.value.len()
            && matches!(self.handler, Opt::Double(_))
        {
            self.value[index as usize].double_value()
        } else {
            default as f64
        }
    }

    /// ArrayConfigOption.getValueAsColorInfo: the four elements as floats into `color`.
    pub fn value_as_color_info(
        &self,
        color: &crate::zombie::core::textures::color_info::ColorInfo,
    ) {
        color.set(
            self.element_as_double(0, 1.0) as f32,
            self.element_as_double(1, 1.0) as f32,
            self.element_as_double(2, 1.0) as f32,
            self.element_as_double(3, 1.0) as f32,
        );
    }
}

struct Item {
    field: &'static str,
    fake: bool,
    opt: Opt,
}

/// The `options` / `fakeOptions` lists of zombie.core.Core (and their by-name maps), kept by the Java field name.
pub struct CoreOptions {
    items: Vec<Item>,
}

impl Default for CoreOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl CoreOptions {
    /// Core's field initialisers (`core_options_table::declare`).
    pub fn new() -> CoreOptions {
        let mut o = CoreOptions { items: Vec::new() };
        declare(&mut o);
        o
    }

    /// `newOption` / `newFakeOption`.
    pub fn add(&mut self, field: &'static str, fake: bool, opt: Opt) {
        self.items.push(Item { field, fake, opt });
    }

    /// The option held by Core field `field`.
    pub fn get(&self, field: &str) -> &Opt {
        match self.items.iter().find(|i| i.field == field) {
            Some(i) => &i.opt,
            None => unreachable!("no Core option field {field}"),
        }
    }

    /// The option held by Core field `field`, mutable.
    pub fn get_mut(&mut self, field: &str) -> &mut Opt {
        match self.items.iter_mut().find(|i| i.field == field) {
            Some(i) => &mut i.opt,
            None => unreachable!("no Core option field {field}"),
        }
    }

    /// Core.getOptionCount.
    pub fn option_count(&self) -> usize {
        self.items.iter().filter(|i| !i.fake).count()
    }

    /// Core.getOptionByIndex.
    pub fn option_by_index(&self, index: usize) -> Option<&Opt> {
        self.items
            .iter()
            .filter(|i| !i.fake)
            .nth(index)
            .map(|i| &i.opt)
    }

    /// Core.getOptionByName (`optionByName.get(name)`: the last registration of a name wins, as in a HashMap).
    pub fn option_by_name(&self, name: &str) -> Option<&Opt> {
        self.items
            .iter()
            .rev()
            .find(|i| !i.fake && i.opt.name() == name)
            .map(|i| &i.opt)
    }
}

/// What `Core.saveOptions` reads besides its options.
pub struct SaveContext<'a> {
    /// `Core.fileversion` (8).
    pub file_version: i32,
    /// `Core.getScreenWidth()` / `getScreenHeight()`.
    pub screen_width: i32,
    pub screen_height: i32,
    /// The PerformanceSettings statics.
    pub performance: &'a PerformanceSettings,
    /// `System.getProperty("os.name").contains("OS X")`.
    pub os_x: bool,
    /// `JoypadManager.instance.activeControllerGuids` (a HashSet: the order is its own).
    pub active_controller_guids: &'a [String],
}

impl CoreOptions {
    /// zombie.core.Core.saveOptions: a `ConfigFile` (version string "version", no tooltips) writes the options
    /// sorted by name (case-insensitive, stable) after `addFakeOptionsForWriting`. Returns what `ConfigFile.write`
    /// returns. Messages that the setters log go to `log`.
    pub fn save_options(&mut self, ctx: &SaveContext, path: &Path, log: &mut Vec<String>) -> bool {
        // addFakeOptionsForWriting(options)
        let p = ctx.performance;
        let set_int = |o: &mut CoreOptions, field: &str, v: i32, log: &mut Vec<String>| {
            if let Opt::Int(x) = o.get_mut(field) {
                x.set_value(v, log);
            }
        };
        set_int(self, "optionFogQuality", p.fog_quality, log);
        set_int(self, "optionViewConeOpacity", p.view_cone_opacity, log);
        set_int(self, "optionLightingFps", p.lighting_fps, log);
        set_int(self, "optionLockFps", p.lock_fps, log);
        set_int(self, "optionPuddlesQuality", p.puddles_quality, log);
        set_int(self, "optionScreenHeight", ctx.screen_height, log);
        set_int(self, "optionScreenWidth", ctx.screen_width, log);
        // optionTieredZombieUpdates.setValue(getOptionTieredZombieUpdates()): its own value
        if let Opt::Bool(x) = self.get_mut("optionUncappedFps") {
            x.value = p.uncapped_fps;
        }
        set_int(self, "optionWaterQuality", p.water_quality, log);
        let guids = self.get_mut("optionActiveControllerGuids").array_mut();
        guids.value.clear();
        for guid in ctx.active_controller_guids {
            self.get_mut("optionActiveControllerGuids").parse(guid, log);
        }
        // `new ArrayList<>(this.options)`, minus the macOS-only options elsewhere, plus the fake options
        let mut list: Vec<&Item> = self
            .items
            .iter()
            .filter(|i| !i.fake)
            .filter(|i| {
                ctx.os_x
                    || !matches!(
                        i.field,
                        "optionMacosIgnoreMouseWheelAcceleration"
                            | "optionMacosMapHorizontalMouseWheelToVertical"
                    )
            })
            .collect();
        list.extend(self.items.iter().filter(|i| i.fake));
        list.sort_by(|a, b| compare_to_ignore_case(a.opt.name(), b.opt.name()));
        let options: Vec<&Opt> = list.iter().map(|i| &i.opt).collect();
        config_file_write(path, "version", ctx.file_version, &options)
    }
}

/// zombie.config.ConfigFile.write with `writeTooltips == false`: `<versionString>=<version>` (when the version is not
/// 0), then `name=value` per option, the options separated (not terminated) by `System.lineSeparator()`; a
/// multi-line array option writes one `name=element` per element.
pub fn config_file_write(
    path: &Path,
    version_string: &str,
    version: i32,
    options: &[&Opt],
) -> bool {
    let sep = if cfg!(windows) { "\r\n" } else { "\n" };
    let mut out = String::new();
    if version != 0 {
        out.push_str(&format!("{version_string}={version}{sep}"));
    }
    for (i, option) in options.iter().enumerate() {
        match option {
            Opt::Array(a) if a.multi_line => {
                for (j, element) in a.value.iter().enumerate() {
                    out.push_str(&format!("{}={}", a.name, element.value_as_string()));
                    if j + 1 < a.value.len() || i + 1 < options.len() {
                        out.push_str(sep);
                    }
                }
            }
            _ => {
                out.push_str(&format!("{}={}", option.name(), option.value_as_string()));
                if i + 1 < options.len() {
                    out.push_str(sep);
                }
            }
        }
    }
    // FileWriter(file, false): an exception (no folder, no permission) is logged and false returned
    match std::fs::File::create(path) {
        Ok(mut f) => f.write_all(out.as_bytes()).is_ok(),
        Err(_) => false,
    }
}
