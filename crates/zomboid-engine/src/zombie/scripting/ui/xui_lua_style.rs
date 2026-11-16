//! zombie.scripting.ui.XuiLuaStyle: one Lua class's style in a skin (a list of typed vars read from the skin script)
//! and the static var registry the xuiConfig scripts fill.
use std::collections::HashSet;

use super::xui_auto_apply::XuiAutoApply;
use super::xui_skin;
use super::xui_var_type::XuiVarType;
use super::JHashMap;
use crate::java;
use crate::zombie::core::color::Color;
use crate::zombie::core::colors::Colors;
use crate::zombie::scripting::objects::xui_config_script::XuiConfigScript;
use crate::zombie::ui::ui_font::UIFont;

/// zombie.scripting.ui.XuiLuaStyle.ALLOWED_VAR_TYPES (an EnumSet: ordinal order).
pub const ALLOWED_VAR_TYPES: [XuiVarType; 8] = [
    XuiVarType::String,
    XuiVarType::StringList,
    XuiVarType::TranslateString,
    XuiVarType::Double,
    XuiVarType::Boolean,
    XuiVarType::FontType,
    XuiVarType::Color,
    XuiVarType::Texture,
];

/// ALLOWED_VAR_TYPES.contains
pub fn is_allowed(t: XuiVarType) -> bool {
    ALLOWED_VAR_TYPES.contains(&t)
}

/// The XuiVar subclass of a var: XuiLuaStyle.XuiBoolean, XuiColor, XuiDouble, XuiFontType, XuiString,
/// XuiStringList, XuiTexture, XuiTranslateString.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XuiVarClass {
    Boolean,
    Color,
    Double,
    FontType,
    String,
    StringList,
    Texture,
    TranslateString,
}

impl XuiVarClass {
    /// Class.getSimpleName
    pub fn simple_name(self) -> &'static str {
        match self {
            XuiVarClass::Boolean => "XuiBoolean",
            XuiVarClass::Color => "XuiColor",
            XuiVarClass::Double => "XuiDouble",
            XuiVarClass::FontType => "XuiFontType",
            XuiVarClass::String => "XuiString",
            XuiVarClass::StringList => "XuiStringList",
            XuiVarClass::Texture => "XuiTexture",
            XuiVarClass::TranslateString => "XuiTranslateString",
        }
    }

    /// The XuiVarType each subclass passes to the XuiVar constructor.
    pub fn var_type(self) -> XuiVarType {
        match self {
            XuiVarClass::Boolean => XuiVarType::Boolean,
            XuiVarClass::Color => XuiVarType::Color,
            XuiVarClass::Double => XuiVarType::Double,
            XuiVarClass::FontType => XuiVarType::FontType,
            XuiVarClass::String => XuiVarType::String,
            XuiVarClass::StringList => XuiVarType::StringList,
            XuiVarClass::Texture => XuiVarType::Texture,
            XuiVarClass::TranslateString => XuiVarType::TranslateString,
        }
    }
}

/// The value of a var (the `T` of XuiVar<T, C>).
#[derive(Clone, Debug, PartialEq)]
pub enum XuiValue {
    Boolean(bool),
    Double(f64),
    Font(UIFont),
    /// XuiString, XuiTexture (the texture's name) and XuiTranslateString (the untranslated key).
    Str(String),
    StrList(Vec<String>),
    Color(Color),
}

/// zombie.scripting.ui.XuiLuaStyle.XuiVar and its subclasses.
#[derive(Clone, Debug, PartialEq)]
pub struct XuiVar {
    pub class: XuiVarClass,
    pub ui_order: i32,
    pub r#type: XuiVarType,
    /// XuiVar.parent: whether the var belongs to a style (the registry's own vars have a null parent).
    pub has_parent: bool,
    pub value_set: bool,
    pub auto_apply: XuiAutoApply,
    pub default_value: Option<XuiValue>,
    pub value: Option<XuiValue>,
    pub lua_table_key: String,
}

/// What XuiColor.fromString looks a colour name up in: `parent.xuiSkin.color` (the skin's colour map, then
/// zombie.core.Colors) when the style has a skin, then Colors.GetColorByName.
pub struct ColorLookup<'a> {
    pub skin_colors: Option<&'a JHashMap<Color>>,
    pub colors: &'a Colors,
}

impl XuiVar {
    /// The (parent, key) constructors of the XuiVar subclasses: XuiBoolean defaults to false, XuiDouble to 0.0,
    /// XuiFontType to UIFont.Small, XuiStringList to an empty list, the others to null. uiOrder 1000, autoApply
    /// IfSet.
    pub fn new(class: XuiVarClass, has_parent: bool, key: &str) -> XuiVar {
        let default_value = match class {
            XuiVarClass::Boolean => Some(XuiValue::Boolean(false)),
            XuiVarClass::Double => Some(XuiValue::Double(0.0)),
            XuiVarClass::FontType => Some(XuiValue::Font(UIFont::Small)),
            XuiVarClass::StringList => Some(XuiValue::StrList(Vec::new())),
            _ => None,
        };
        XuiVar {
            class,
            ui_order: 1000,
            r#type: class.var_type(),
            has_parent,
            value_set: false,
            auto_apply: XuiAutoApply::IfSet,
            default_value,
            value: None,
            lua_table_key: key.to_string(),
        }
    }

    /// XuiVar.copy(parent) of every subclass: the (parent, key, defaultValue) constructor, then copyValuesTo
    /// (uiOrder, valueSet, autoApply, defaultValue, value). XuiColor's copy gets its own Color, which a value type is
    /// here already.
    pub fn copy(&self, has_parent: bool) -> XuiVar {
        let mut c = XuiVar::new(self.class, has_parent, &self.lua_table_key);
        c.default_value = self.default_value.clone();
        c.ui_order = self.ui_order;
        c.value_set = self.value_set;
        c.auto_apply = self.auto_apply;
        c.value = self.value.clone();
        c
    }

    /// XuiVar.setValue: the value, marked as set.
    pub fn set_value(&mut self, value: Option<XuiValue>) {
        self.value = value;
        self.value_set = true;
    }

    /// XuiVar.value: the value when set, else the default.
    pub fn value(&self) -> Option<&XuiValue> {
        if self.value_set {
            self.value.as_ref()
        } else {
            self.default_value.as_ref()
        }
    }

    /// XuiVar.acceptsKey
    pub fn accepts_key(&self, key: &str) -> bool {
        self.lua_table_key == key
    }

    /// XuiVar.getValueString and its XuiColor and XuiTranslateString overrides.
    pub fn get_value_string(&self) -> String {
        match self.class {
            XuiVarClass::Color => {
                let c = match self.value() {
                    Some(XuiValue::Color(c)) => Some(*c),
                    _ => None,
                };
                let ch = |f: fn(&Color) -> f32| c.as_ref().map_or(1.0, f);
                format!(
                    "{}, {}, {}, {}",
                    java::float_to_string(ch(|c| c.r)),
                    java::float_to_string(ch(|c| c.g)),
                    java::float_to_string(ch(|c| c.b)),
                    java::float_to_string(ch(|c| c.a))
                )
            }
            _ => match self.value() {
                None => "null".to_string(),
                Some(XuiValue::Boolean(b)) => b.to_string(),
                Some(XuiValue::Double(d)) => java::double_to_string(*d),
                Some(XuiValue::Font(f)) => f.name().to_string(),
                Some(XuiValue::Str(s)) => s.clone(),
                // ArrayList.toString
                Some(XuiValue::StrList(l)) => format!("[{}]", l.join(", ")),
                Some(XuiValue::Color(_)) => "null".to_string(),
            },
        }
    }

    /// XuiVar.fromString of each subclass. A value that does not parse leaves the var as it was (the original
    /// catches the exception and logs it).
    fn set_from_string(&mut self, val: &str, lookup: &ColorLookup, log: &mut Vec<String>) {
        match self.class {
            // Boolean.parseBoolean
            XuiVarClass::Boolean => self.set_value(Some(XuiValue::Boolean(
                java::equals_ignore_case(val, "true"),
            ))),
            XuiVarClass::Double => match java::parse_double(val) {
                Some(d) => self.set_value(Some(XuiValue::Double(d))),
                None => log.push(format!(
                    "java.lang.NumberFormatException: {}",
                    java::number_format_message(val)
                )),
            },
            XuiVarClass::FontType => {
                let v = if val.starts_with("UIFont.") {
                    match val.find('.') {
                        Some(p) => &val[p + 1..],
                        None => val,
                    }
                } else {
                    val
                };
                match UIFont::from_string(v) {
                    Some(f) => self.set_value(Some(XuiValue::Font(f))),
                    None => log.push(format!(
                        "java.lang.IllegalArgumentException: No enum constant zombie.ui.UIFont.{v}"
                    )),
                }
            }
            XuiVarClass::String | XuiVarClass::Texture | XuiVarClass::TranslateString => {
                self.set_value(Some(XuiValue::Str(val.to_string())))
            }
            XuiVarClass::StringList => {
                let list = java::split_char(val, ':')
                    .into_iter()
                    .map(|s| java::trim(s).to_string())
                    .collect();
                self.set_value(Some(XuiValue::StrList(list)));
            }
            XuiVarClass::Color => match read_color(val, lookup, log) {
                Ok(c) => self.set_value(Some(XuiValue::Color(c))),
                // Core.debug is false: a warning only.
                Err(_) => log.push(format!("Could not read color: {val}")),
            },
        }
    }

    /// XuiVar.load: fromString when the key is this var's.
    pub fn load(
        &mut self,
        key: &str,
        val: &str,
        lookup: &ColorLookup,
        log: &mut Vec<String>,
    ) -> bool {
        if self.accepts_key(key) {
            self.set_from_string(val, lookup, log);
            true
        } else {
            false
        }
    }
}

/// XuiLuaStyle.XuiColor.fromString without its catch: a colour of the skin or of Colors by that name, else
/// "r:g:b[:a]" (0..1) or "rgb:r:g:b[:a]" (0..255). `Err` is the exception the original catches.
fn read_color(val: &str, lookup: &ColorLookup, log: &mut Vec<String>) -> Result<Color, String> {
    let mut color = None;
    if let Some(skin) = lookup.skin_colors {
        color = xui_skin::color_internal(skin, lookup.colors, Some(val), true, true);
    }
    if color.is_none() {
        color = lookup.colors.get_color_by_name(val);
    }
    if color.is_none() && val.contains(':') {
        let mut c = Color::default();
        let split = java::split_char(val, ':');
        if split.len() < 3 {
            log.push(format!("Warning color has <3 values. color: {val}"));
        }
        let parse = |s: &str| {
            java::parse_float(java::trim(s)).ok_or_else(|| {
                format!(
                    "java.lang.NumberFormatException: {}",
                    java::number_format_message(java::trim(s))
                )
            })
        };
        if split.len() > 1 && java::equals_ignore_case(java::trim(split[0]), "rgb") {
            for (i, s) in split.iter().enumerate().skip(1) {
                match i {
                    1 => c.r = parse(s)? / 255.0,
                    2 => c.g = parse(s)? / 255.0,
                    3 => c.b = parse(s)? / 255.0,
                    4 => c.a = parse(s)? / 255.0,
                    _ => {}
                }
            }
        } else {
            for (i, s) in split.iter().enumerate() {
                match i {
                    0 => c.r = parse(s)?,
                    1 => c.g = parse(s)?,
                    2 => c.b = parse(s)?,
                    3 => c.a = parse(s)?,
                    _ => {}
                }
            }
        }
        color = Some(c);
    }
    color.ok_or_else(|| format!("java.lang.Exception: Could not read color: {val}"))
}

/// zombie.scripting.ui.XuiLuaStyle
#[derive(Clone, Debug, PartialEq)]
pub struct XuiLuaStyle {
    pub xui_lua_class: String,
    pub xui_style_name: String,
    /// XuiLuaStyle.xuiSkin, by name.
    pub xui_skin: Option<String>,
    /// XuiLuaStyle.varsMap: key -> index in `vars` (the same objects in the original).
    pub vars_map: JHashMap<usize>,
    pub vars: Vec<XuiVar>,
}

impl XuiLuaStyle {
    /// zombie.scripting.ui.XuiLuaStyle(String, String)
    pub fn new(xui_lua_class: &str, xui_style_name: &str) -> XuiLuaStyle {
        XuiLuaStyle {
            xui_lua_class: xui_lua_class.to_string(),
            xui_style_name: xui_style_name.to_string(),
            xui_skin: None,
            vars_map: JHashMap::new(),
            vars: Vec::new(),
        }
    }

    /// zombie.scripting.ui.XuiLuaStyle.getVar
    pub fn get_var(&self, key: &str) -> Option<&XuiVar> {
        self.vars_map.get(key).map(|&i| &self.vars[i])
    }

    /// zombie.scripting.ui.XuiLuaStyle.addVar: `Err` is the RuntimeException for a key already added (a new copy is
    /// never in `vars` already, so the identity test of the original is the key test).
    fn add_var(&mut self, key: &str, var: XuiVar) -> Result<(), String> {
        if !self.vars_map.contains_key(&var.lua_table_key) {
            self.vars_map.put(key, self.vars.len());
            self.vars.push(var);
            Ok(())
        } else {
            Err(format!(
                "java.lang.RuntimeException: Var already added: {}",
                var.lua_table_key
            ))
        }
    }

    /// zombie.scripting.ui.XuiLuaStyle.loadVar: the style's var of that key, made from the registry's on first use;
    /// a value is read with fromString, a null value sets null. `Err` is the exception for a key the registry does
    /// not have.
    pub fn load_var(
        &mut self,
        key: &str,
        val: Option<&str>,
        registry: &JHashMap<XuiVar>,
        lookup: &ColorLookup,
        log: &mut Vec<String>,
    ) -> Result<bool, String> {
        let index = match self.vars_map.get(key) {
            Some(&i) => i,
            None => {
                let registered = registry.get(key);
                match registered {
                    Some(r) if r.accepts_key(key) => {
                        let var = r.copy(true);
                        self.add_var(key, var)?;
                        self.vars.len() - 1
                    }
                    _ => {
                        log.push(self.to_string());
                        return Err(format!(
                            "java.lang.Exception: Variable '{key}' is not registered or key typo. [registered={}]",
                            registered.map_or("null", |r| r.class.simple_name())
                        ));
                    }
                }
            }
        };
        let var = &mut self.vars[index];
        match val {
            Some(v) if var.accepts_key(key) => Ok(var.load(key, v, lookup, log)),
            None if var.accepts_key(key) => {
                var.set_value(None);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// zombie.scripting.ui.XuiLuaStyle.copyVarsFrom: a copy of every var of `other`, in its order.
    pub fn copy_vars_from(&mut self, other: &XuiLuaStyle) -> Result<(), String> {
        self.vars.clear();
        self.vars_map.clear();
        for var in &other.vars {
            let copy = var.copy(true);
            let key = copy.lua_table_key.clone();
            self.add_var(&key, copy)?;
        }
        Ok(())
    }
}

impl std::fmt::Display for XuiLuaStyle {
    /// zombie.scripting.ui.XuiLuaStyle.toString, without Object.toString's identity hash.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "XuiLuaStyle [class={}, styleName={}]",
            self.xui_lua_class, self.xui_style_name
        )
    }
}

/// zombie.scripting.ui.XuiLuaStyle.addStaticVar: `Err` is the RuntimeException for a key registered twice.
fn add_static_var(registry: &mut JHashMap<XuiVar>, var: XuiVar) -> Result<(), String> {
    if registry.contains_key(&var.lua_table_key) {
        return Err(format!(
            "java.lang.RuntimeException: Key already exists: {}",
            var.lua_table_key
        ));
    }
    let key = var.lua_table_key.clone();
    registry.put(&key, var);
    Ok(())
}

/// zombie.scripting.ui.XuiLuaStyle.ReadConfigs: every key of every xuiConfig script registered as a var of its type
/// (parent null). `Err` is the exception for a key of two types.
pub fn read_configs(
    registry: &mut JHashMap<XuiVar>,
    configs: &[XuiConfigScript],
) -> Result<(), String> {
    let mut parsed: Vec<(XuiVarType, HashSet<String>)> = ALLOWED_VAR_TYPES
        .iter()
        .map(|t| (*t, HashSet::new()))
        .collect();
    for config in configs {
        parse_config(registry, &mut parsed, config)?;
    }
    Ok(())
}

/// zombie.scripting.ui.XuiLuaStyle.parseConfig. The original walks a HashMap keyed by the enum (identity hash
/// order, UNKNOWN); here the config's own order. Which of two clashing keys is reported depends on it, the outcome
/// does not.
fn parse_config(
    registry: &mut JHashMap<XuiVar>,
    parsed: &mut [(XuiVarType, HashSet<String>)],
    config: &XuiConfigScript,
) -> Result<(), String> {
    for (var_type, keys) in &config.var_configs {
        let var_type = *var_type;
        if !is_allowed(var_type) {
            return Err(format!(
                "java.lang.Exception: Var type not allowed: {}",
                var_type.name()
            ));
        }
        for key in keys {
            if parsed
                .iter()
                .any(|(t, set)| *t != var_type && set.contains(key))
            {
                return Err(format!(
                    "java.lang.Exception: Duplicate key '{key}' in var type: {}, and type: {}",
                    var_type.name(),
                    var_type.name()
                ));
            }
            if let Some((_, set)) = parsed.iter_mut().find(|(t, _)| *t == var_type) {
                set.insert(key.clone());
            }
            if !registry.contains_key(key) {
                let class = match var_type {
                    XuiVarType::String => XuiVarClass::String,
                    XuiVarType::StringList => XuiVarClass::StringList,
                    XuiVarType::TranslateString => XuiVarClass::TranslateString,
                    XuiVarType::Double => XuiVarClass::Double,
                    XuiVarType::Boolean => XuiVarClass::Boolean,
                    XuiVarType::FontType => XuiVarClass::FontType,
                    XuiVarType::Color => XuiVarClass::Color,
                    XuiVarType::Texture => XuiVarClass::Texture,
                    other => {
                        return Err(format!(
                            "java.lang.Exception: No handler for: {}",
                            other.name()
                        ))
                    }
                };
                add_static_var(registry, XuiVar::new(class, false, key))?;
            }
        }
    }
    Ok(())
}
