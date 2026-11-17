//! zombie.core.skinnedmodel.advancedanimation.AnimCondition
use super::animation_variable_reference::AnimationVariableReference;
use super::i_animation_variable_source::IAnimationVariableSource;
use crate::java;
use crate::zombie::util::pz_xml_util::{jaxb_parse_boolean, jaxb_parse_float, XmlElement};
use crate::zombie::util::string_utils;

/// zombie.core.skinnedmodel.advancedanimation.AnimCondition.Type (`@XmlEnumValue` = the name).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    String,
    Strneq,
    Bool,
    Equ,
    Neq,
    Less,
    Gtr,
    Absless,
    Absgtr,
    Or,
}

impl Type {
    const ALL: [(Type, &'static str); 10] = [
        (Type::String, "STRING"),
        (Type::Strneq, "STRNEQ"),
        (Type::Bool, "BOOL"),
        (Type::Equ, "EQU"),
        (Type::Neq, "NEQ"),
        (Type::Less, "LESS"),
        (Type::Gtr, "GTR"),
        (Type::Absless, "ABSLESS"),
        (Type::Absgtr, "ABSGTR"),
        (Type::Or, "OR"),
    ];

    /// JAXB's `@XmlEnumValue` lookup: the constant whose value is exactly `s`, else null (`None`; the game's old
    /// numeric types such as "3" become null this way).
    pub fn from_xml(s: &str) -> Option<Type> {
        Type::ALL.iter().find(|(_, n)| *n == s).map(|(t, _)| *t)
    }

    /// java.lang.Enum.name
    pub fn name(self) -> &'static str {
        Type::ALL
            .iter()
            .find(|(t, _)| *t == self)
            .map_or("", |(_, n)| n)
    }
}

/// zombie.core.skinnedmodel.advancedanimation.AnimCondition: a test of an animation variable.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimCondition {
    pub name: String,
    /// `None` is null (an unknown `m_Type`).
    pub type_: Option<Type>,
    pub value: String,
    pub float_value: f32,
    pub bool_value: bool,
    pub string_value: String,
    /// `@XmlTransient variableReference`: set by `parse`.
    pub variable_reference: Option<AnimationVariableReference>,
}

impl Default for AnimCondition {
    /// zombie.core.skinnedmodel.advancedanimation.AnimCondition(): the field initialisers.
    fn default() -> Self {
        AnimCondition {
            name: String::new(),
            type_: Some(Type::String),
            value: String::new(),
            float_value: 0.0,
            bool_value: false,
            string_value: String::new(),
            variable_reference: None,
        }
    }
}

/// java.lang.String.replaceAll(regex, replacement) for a regex that matches `target` literally: every occurrence,
/// left to right. A replacement with '$' or '\' (group references and escapes of `Matcher.appendReplacement`) is an
/// `Err` here (not ported; no node name in the game has one).
fn replace_all_literal(s: &str, target: &str, replacement: &str) -> Result<String, String> {
    if replacement.contains(['$', '\\']) {
        return Err(format!(
            "String.replaceAll with replacement {replacement:?} is not ported"
        ));
    }
    Ok(s.replace(target, replacement))
}

impl AnimCondition {
    /// JAXB's unmarshalling of `m_Name`, `m_Type`, `m_Value`, `m_FloatValue`, `m_BoolValue`, `m_StringValue` (a
    /// repeated element keeps the last value, a value that does not convert keeps the field, an unknown type is null,
    /// other elements such as the old `m_IntValue` are ignored).
    pub fn from_xml(e: &XmlElement) -> Self {
        let mut c = AnimCondition::default();
        for x in e.elements() {
            match x.name.as_str() {
                "m_Name" => c.name = x.text(),
                "m_Type" => c.type_ = Type::from_xml(&x.text()),
                "m_Value" => c.value = x.text(),
                "m_FloatValue" => {
                    if let Some(v) = jaxb_parse_float(&x.text()) {
                        c.float_value = v;
                    }
                }
                "m_BoolValue" => {
                    if let Some(v) = jaxb_parse_boolean(&x.text()) {
                        c.bool_value = v;
                    }
                }
                "m_StringValue" => c.string_value = x.text(),
                _ => {}
            }
        }
        c
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimCondition.parse: `parseValue`, the variable reference, then in a
    /// string condition `$this` and `$source` become the from-node's name and `$target` the to-node's (without a
    /// to-node the original logs an error and leaves it). `Err` is the original's exception (a null type with a value).
    pub fn parse(
        &mut self,
        from_node_name: &str,
        to_node_name: Option<&str>,
    ) -> Result<(), String> {
        self.parse_value()?;
        self.variable_reference = Some(AnimationVariableReference::from_raw_variable_name(
            &self.name,
        ));
        if self.is_type_string() {
            if self.string_value.contains("$this") {
                self.string_value =
                    replace_all_literal(&self.string_value, "$this", from_node_name)?;
            }
            if self.string_value.contains("$source") {
                self.string_value =
                    replace_all_literal(&self.string_value, "$source", from_node_name)?;
            }
            if self.string_value.contains("$target") {
                if let Some(to) = to_node_name {
                    self.string_value = replace_all_literal(&self.string_value, "$target", to)?;
                }
            }
        }
        Ok(())
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimCondition.parseValue: a non-empty `m_Value` sets the string,
    /// bool (`StringUtils.tryParseBoolean`) or float (`StringUtils.tryParseFloat`) value of the type. A null type
    /// with a value is the `switch`'s NullPointerException (`Err`).
    pub fn parse_value(&mut self) -> Result<(), String> {
        if self.value.is_empty() {
            return Ok(());
        }
        match self.type_ {
            None => return Err("java.lang.NullPointerException: AnimCondition.parseValue".into()),
            Some(Type::String | Type::Strneq) => self.string_value = self.value.clone(),
            Some(Type::Bool) => {
                self.bool_value = string_utils::try_parse_boolean(Some(&self.value));
            }
            Some(Type::Equ | Type::Neq | Type::Less | Type::Gtr | Type::Absless | Type::Absgtr) => {
                self.float_value = string_utils::try_parse_float(Some(&self.value));
            }
            Some(Type::Or) => {}
        }
        Ok(())
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimCondition.isTypeString
    pub fn is_type_string(&self) -> bool {
        matches!(self.type_, Some(Type::String | Type::Strneq))
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimCondition.check: an OR never passes by itself; a variable the
    /// owner does not have counts as "" (string tests), false (BOOL) or fails (number tests, which warn once); else the
    /// slot's value is compared (`==` and `!=` on floats literally, string tests ignoring case). `Err` is the
    /// original's exception (a null type, or a condition whose `parse` never ran).
    pub fn check(&self, var_source: &dyn IAnimationVariableSource) -> Result<bool, String> {
        let npe = || "java.lang.NullPointerException: AnimCondition.check".to_string();
        let var_type = self.type_.ok_or_else(npe)?;
        if var_type == Type::Or {
            return Ok(false);
        }
        let reference = self.variable_reference.as_ref().ok_or_else(npe)?;
        let Some(slot) = reference.get_variable(var_source) else {
            return Ok(match var_type {
                Type::String => java::equals_ignore_case(&self.string_value, ""),
                Type::Strneq => !java::equals_ignore_case(&self.string_value, ""),
                Type::Bool => !self.bool_value,
                _ => false,
            });
        };
        Ok(match var_type {
            Type::String => slot
                .get_value_string()
                .is_some_and(|v| java::equals_ignore_case(&self.string_value, &v)),
            Type::Strneq => !slot
                .get_value_string()
                .is_some_and(|v| java::equals_ignore_case(&self.string_value, &v)),
            Type::Bool => slot.get_value_bool() == self.bool_value,
            Type::Equ => self.float_value == slot.get_value_float(),
            Type::Neq => self.float_value != slot.get_value_float(),
            Type::Less => slot.get_value_float() < self.float_value,
            Type::Gtr => slot.get_value_float() > self.float_value,
            Type::Absless => slot.get_value_float().abs() < self.float_value,
            Type::Absgtr => slot.get_value_float().abs() > self.float_value,
            Type::Or => false,
        })
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimCondition.pass: the conditions are AND-ed, an OR condition
    /// ends the run when everything before it passed and otherwise starts a new run (every condition is checked until
    /// then, as `valid && check` short-circuits).
    pub fn pass(
        var_source: &dyn IAnimationVariableSource,
        conditions: &[AnimCondition],
    ) -> Result<bool, String> {
        let mut valid = true;
        for condition in conditions {
            if condition.type_ == Some(Type::Or) {
                if valid {
                    break;
                }
                valid = true;
            } else {
                valid = valid && condition.check(var_source)?;
            }
        }
        Ok(valid)
    }
}
