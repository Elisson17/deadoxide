//! zombie.network.ServerOptions: the server's settings, as `new ServerOptions()` makes them (every option at its
//! default; the option list is generated from the original class, `server_options_table.rs`), with the methods of
//! the option classes (ServerOptions$*ServerOption over zombie.config.*ConfigOption) the scripts call.
use std::collections::HashMap;

use super::server_options_table::OPTIONS;
use crate::java;
use crate::java::JavaRandom;
use crate::java_formatter::{self, FormatArg};
use crate::zombie::core::random::rand_abstract::RandAbstract;
use crate::zombie::core::translator::Translator;

/// The default of an option, as its field initialiser computes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Default {
    Int(i32),
    /// `Rand.Next(bound)`
    Rand(i32),
    Str(&'static str),
    /// `Integer.toString(Rand.Next(bound))`
    RandString(i32),
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
        default: Default,
    },
    /// EnumConfigOption: an IntegerConfigOption over 1..=num_values.
    Enum {
        num_values: i32,
        default: i32,
    },
    Str {
        default: Default,
        max_length: i32,
    },
    /// TextServerOption: a StringConfigOption whose type is "text".
    Text {
        default: Default,
        max_length: i32,
    },
}

/// One `new ServerOptions.*ServerOption(this, name, ...)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Def {
    pub name: &'static str,
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Boolean(bool),
    Double(f64),
    Integer(i32),
    Str(String),
}

/// A server option: its definition, value and default (the constructor sets both to the default).
#[derive(Clone, Debug, PartialEq)]
pub struct ServerOption {
    pub def: &'static Def,
    pub value: Value,
    pub default: Value,
}

/// zombie.network.ServerOptions
#[derive(Clone, Debug, PartialEq)]
pub struct ServerOptions {
    pub options: Vec<ServerOption>,
    by_name: HashMap<&'static str, usize>,
}

impl ServerOptions {
    /// zombie.network.ServerOptions.<init>: the field initialisers in order, each option registering itself
    /// (`addOption`), the random defaults drawn from `rand` (Rand, i.e. RandStandard) as they come. The constructor
    /// body only fills `publicOptions`, which no script reads at load.
    pub fn new<R: JavaRandom>(rand: &mut RandAbstract<R>) -> Self {
        let mut options = Vec::with_capacity(OPTIONS.len());
        let mut by_name = HashMap::new();
        for def in OPTIONS.iter() {
            let default = match def.kind {
                Kind::Boolean(b) => Value::Boolean(b),
                Kind::Double { default, .. } => Value::Double(default),
                Kind::Integer { default, .. }
                | Kind::Str { default, .. }
                | Kind::Text { default, .. } => match default {
                    Default::Int(i) => Value::Integer(i),
                    Default::Rand(bound) => Value::Integer(rand.next(bound)),
                    Default::Str(s) => Value::Str(s.to_string()),
                    Default::RandString(bound) => Value::Str(rand.next(bound).to_string()),
                },
                Kind::Enum { default, .. } => Value::Integer(default),
            };
            by_name.insert(def.name, options.len());
            options.push(ServerOption {
                def,
                value: default.clone(),
                default,
            });
        }
        ServerOptions { options, by_name }
    }

    /// ServerOptions.getNumOptions
    pub fn get_num_options(&self) -> usize {
        self.options.len()
    }

    /// ServerOptions.getOptionByName
    pub fn get_option_by_name(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }
}

impl ServerOption {
    /// ConfigOption.getType of the option's class.
    pub fn get_type(&self) -> &'static str {
        match self.def.kind {
            Kind::Boolean(_) => "boolean",
            Kind::Double { .. } => "double",
            Kind::Integer { .. } => "integer",
            Kind::Enum { .. } => "enum",
            Kind::Str { .. } => "string",
            Kind::Text { .. } => "text",
        }
    }

    /// *ConfigOption.getValueAsString: `String.valueOf(value)` (the string itself for a string option).
    pub fn get_value_as_string(&self) -> String {
        match &self.value {
            Value::Boolean(b) => b.to_string(),
            Value::Double(d) => java::double_to_string(*d),
            Value::Integer(i) => i.to_string(),
            Value::Str(s) => s.clone(),
        }
    }

    /// EnumConfigOption.getNumValues: its max.
    pub fn get_num_values(&self) -> Option<i32> {
        match self.def.kind {
            Kind::Enum { num_values, .. } => Some(num_values),
            _ => None,
        }
    }

    /// ServerOptions$*ServerOption.getTooltip: `UI_ServerOption_<name>_tooltip` (or null), and for a number
    /// option `Sandbox_MinMaxDefault` with its range after it (`\n` written as backslash and n). `Err` is a
    /// translator failure the port has not written.
    pub fn get_tooltip(&self, translator: &mut Translator) -> Result<Option<String>, String> {
        let key = format!("UI_ServerOption_{}_tooltip", self.def.name);
        let s1 = translator.get_text_or_null(&key, &[])?;
        let args = match self.def.kind {
            Kind::Double { min, max, default } => {
                let f = |d: f64| {
                    java_formatter::format("%.02f", &[FormatArg::Double(d)])
                        .map_err(|_| "not ported: String.format(\"%.02f\")".to_string())
                        .map(FormatArg::Str)
                };
                vec![f(min)?, f(max)?, f(default)?]
            }
            Kind::Integer { min, max, .. } => {
                let default = match self.default {
                    Value::Integer(i) => i,
                    _ => 0,
                };
                [min, max, default]
                    .iter()
                    .map(|i| integer_arg(*i))
                    .collect()
            }
            _ => return Ok(s1),
        };
        let s2 = translator.get_text("Sandbox_MinMaxDefault", &args)?;
        Ok(Some(match s1 {
            None => s2,
            Some(s1) => format!("{s1}\\n{s2}"),
        }))
    }

    /// EnumServerOption.getValueTranslationByIndex: `UI_ServerOption_AntiCheat_option<index>` (or null);
    /// `Err` is the ArrayIndexOutOfBoundsException outside 1..=getNumValues.
    pub fn get_value_translation_by_index(
        &self,
        translator: &mut Translator,
        index: i32,
    ) -> Result<Option<String>, String> {
        let n = self.get_num_values().unwrap_or(0);
        if index < 1 || index > n {
            return Err("java.lang.ArrayIndexOutOfBoundsException".into());
        }
        translator.get_text_or_null(&format!("UI_ServerOption_AntiCheat_option{index}"), &[])
    }
}

/// A boxed java.lang.Integer as a format argument.
pub fn integer_arg(i: i32) -> FormatArg {
    FormatArg::Object {
        class: "java.lang.Integer".into(),
        string: i.to_string(),
        hash_code: i,
    }
}
