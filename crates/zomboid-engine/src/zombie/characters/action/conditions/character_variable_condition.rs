//! zombie.characters.action.conditions.CharacterVariableCondition
use crate::java;
use crate::zombie::core::skinnedmodel::advancedanimation::animation_variable_reference::AnimationVariableReference;
use crate::zombie::core::skinnedmodel::advancedanimation::anim_node::float_compare;
use crate::zombie::core::skinnedmodel::advancedanimation::i_animation_variable_source::IAnimationVariableSource;
use crate::zombie::util::pz_xml_util::XmlElement;
use std::cmp::Ordering;

/// zombie.characters.action.conditions.CharacterVariableCondition.Operator
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator {
    Equal,
    NotEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
}

impl Operator {
    /// java.lang.Enum.name
    pub fn name(self) -> &'static str {
        match self {
            Operator::Equal => "Equal",
            Operator::NotEqual => "NotEqual",
            Operator::Less => "Less",
            Operator::Greater => "Greater",
            Operator::LessEqual => "LessEqual",
            Operator::GreaterEqual => "GreaterEqual",
        }
    }
}

/// A side of the comparison: what `parseValue` returns (String, Integer, Float, Boolean) or a
/// `CharacterVariableLookup` of a variable.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    String(String),
    Integer(i32),
    Float(f32),
    Boolean(bool),
    Lookup(AnimationVariableReference),
}

/// zombie.characters.action.conditions.CharacterVariableCondition: `lhs op rhs` over character variables.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterVariableCondition {
    pub op: Option<Operator>,
    pub lhs_value: Option<Value>,
    pub rhs_value: Option<Value>,
}

/// zombie.characters.action.conditions.CharacterVariableCondition.parseValue, as written: a value starting with a sign
/// or digit is read as digits (',' skipped) into an int, or past a '.' into a float (the sign applies only to the
/// float, digits accumulate with int wrap-around), and any other character returns the text; "true"/"yes" and
/// "false"/"no" (any case) are booleans; else, for a variable lookup, a quoted text is unquoted (a backslash is kept,
/// not the character after it) and anything else is a variable name. `Err` is the NegativeArraySizeException of a
/// lone quote.
pub fn parse_value(value: &str, for_lookup: bool) -> Result<Value, String> {
    let u: Vec<u16> = value.encode_utf16().collect();
    if u.is_empty() {
        return Ok(Value::String(value.to_string()));
    }
    let first = u[0];
    let digit = |c: u16| (u16::from(b'0')..=u16::from(b'9')).contains(&c);
    if first == u16::from(b'-') || first == u16::from(b'+') || digit(first) {
        let mut int_val: i32 = 0;
        if digit(first) {
            int_val = i32::from(first) - 48;
        }
        let mut read_pos = 1usize;
        while read_pos < u.len() {
            let chr = u[read_pos];
            if digit(chr) {
                int_val = int_val
                    .wrapping_mul(10)
                    .wrapping_add(i32::from(chr))
                    .wrapping_sub(48);
            } else if chr != u16::from(b',') {
                if chr != u16::from(b'.') {
                    return Ok(Value::String(value.to_string()));
                }
                read_pos += 1;
                break;
            }
            read_pos += 1;
        }
        if read_pos == u.len() {
            return Ok(Value::Integer(int_val));
        }
        let mut float_val = int_val as f32;
        let mut divisor = 10.0f32;
        while read_pos < u.len() {
            let chr = u[read_pos];
            if digit(chr) {
                float_val += (i32::from(chr) - 48) as f32 / divisor;
                divisor *= 10.0;
            } else if chr != u16::from(b',') {
                return Ok(Value::String(value.to_string()));
            }
            read_pos += 1;
        }
        if first == u16::from(b'-') {
            float_val *= -1.0;
        }
        return Ok(Value::Float(float_val));
    }
    if java::equals_ignore_case(value, "true") || java::equals_ignore_case(value, "yes") {
        return Ok(Value::Boolean(true));
    }
    if java::equals_ignore_case(value, "false") || java::equals_ignore_case(value, "no") {
        return Ok(Value::Boolean(false));
    }
    if !for_lookup {
        return Ok(Value::String(value.to_string()));
    }
    if first != u16::from(b'\'') && first != u16::from(b'"') {
        return Ok(Value::Lookup(
            AnimationVariableReference::from_raw_variable_name(value),
        ));
    }
    if u.len() < 2 {
        return Err("java.lang.NegativeArraySizeException: -1".into());
    }
    let mut sb: Vec<u16> = Vec::new();
    for &c in &u[1..] {
        if (c == u16::from(b'"') || c == u16::from(b'\'')) && c == first {
            return Ok(Value::String(String::from_utf16_lossy(&sb)));
        }
        sb.push(c);
    }
    Ok(Value::String(String::from_utf16_lossy(&sb)))
}

impl CharacterVariableCondition {
    /// zombie.characters.action.conditions.CharacterVariableCondition.Factory.create with `load`: by element name,
    /// `isTrue`/`isFalse` (the trimmed text is the variable, compared Equal with true/false), `compare` (the trimmed `op`
    /// attribute: = ==, != <>, <, >, <=, >=; another op gives no condition), `gtr`, `less`, `equals`, `notEquals`,
    /// `lessEqual`, `gtrEqual` (attributes `a`, a lookup, and `b`, a value, both trimmed). `Ok(None)` is null.
    pub fn create(node: &XmlElement) -> Result<Option<CharacterVariableCondition>, String> {
        let mut c = CharacterVariableCondition {
            op: None,
            lhs_value: None,
            rhs_value: None,
        };
        let op = match node.name.as_str() {
            "isTrue" | "isFalse" => {
                c.op = Some(Operator::Equal);
                c.lhs_value = Some(Value::Lookup(
                    AnimationVariableReference::from_raw_variable_name(java::trim(
                        &node.text_content(),
                    )),
                ));
                c.rhs_value = Some(Value::Boolean(node.name == "isTrue"));
                return Ok(Some(c));
            }
            "compare" => match java::trim(node.attribute("op")) {
                "=" | "==" => Operator::Equal,
                "!=" | "<>" => Operator::NotEqual,
                "<" => Operator::Less,
                ">" => Operator::Greater,
                "<=" => Operator::LessEqual,
                ">=" => Operator::GreaterEqual,
                _ => return Ok(None),
            },
            "gtr" => Operator::Greater,
            "less" => Operator::Less,
            "equals" => Operator::Equal,
            "notEquals" => Operator::NotEqual,
            "lessEqual" => Operator::LessEqual,
            "gtrEqual" => Operator::GreaterEqual,
            _ => return Ok(None),
        };
        c.op = Some(op);
        // loadCompareValues
        let lhs = java::trim(node.attribute("a"));
        let rhs = java::trim(node.attribute("b"));
        c.lhs_value = Some(parse_value(lhs, true)?);
        c.rhs_value = Some(parse_value(rhs, false)?);
        Ok(Some(c))
    }

    /// CharacterVariableCondition.resolveValue: a lookup is the variable's string (null without a slot or with a null
    /// string) read again by `parseValue(.., false)`; another value is itself.
    fn resolve_value(value: Option<&Value>, owner: &dyn IAnimationVariableSource) -> Option<Value> {
        match value? {
            Value::Lookup(reference) => {
                let slot = reference.get_variable(owner)?;
                let variable_value = slot.get_value_string()?;
                parse_value(&variable_value, false).ok()
            }
            other => Some(other.clone()),
        }
    }

    /// CharacterVariableCondition.resolveCompareTo
    fn resolve_compare_to(&self, result: Ordering) -> bool {
        match self.op {
            Some(Operator::Equal) => result == Ordering::Equal,
            Some(Operator::NotEqual) => result != Ordering::Equal,
            Some(Operator::Less) => result == Ordering::Less,
            Some(Operator::Greater) => result == Ordering::Greater,
            Some(Operator::LessEqual) => result != Ordering::Greater,
            Some(Operator::GreaterEqual) => result != Ordering::Less,
            None => false,
        }
    }

    /// CharacterVariableCondition.passes: both sides resolved against the owner; a null left side against an empty
    /// string is `Equal` true / `NotEqual` false; two values of one class compare as that class
    /// (`String.compareTo`, `Integer`, `Float.compareTo`, `Boolean`); an int and a float compare as floats when both
    /// sides are lookups or neither is, else the side that is not a lookup is converted to the type of the lookup
    /// (a float to an int by truncation when the lookup is an int); anything else, or a null side, is false.
    pub fn passes(&self, owner: &dyn IAnimationVariableSource) -> bool {
        let lhs = Self::resolve_value(self.lhs_value.as_ref(), owner);
        let rhs = Self::resolve_value(self.rhs_value.as_ref(), owner);
        if lhs.is_none() {
            if let Some(Value::String(s)) = &rhs {
                if s.is_empty() {
                    match self.op {
                        Some(Operator::Equal) => return true,
                        Some(Operator::NotEqual) => return false,
                        _ => {}
                    }
                }
            }
        }
        let (Some(lhs), Some(rhs)) = (lhs, rhs) else {
            return false;
        };
        match (&lhs, &rhs) {
            (Value::String(a), Value::String(b)) => {
                return self.resolve_compare_to(java::string_compare_to(a, b))
            }
            (Value::Integer(a), Value::Integer(b)) => return self.resolve_compare_to(a.cmp(b)),
            (Value::Float(a), Value::Float(b)) => {
                return self.resolve_compare_to(float_compare(*a, *b))
            }
            (Value::Boolean(a), Value::Boolean(b)) => return self.resolve_compare_to(a.cmp(b)),
            _ => {}
        }
        let numeric = |v: &Value| matches!(v, Value::Integer(_) | Value::Float(_));
        if !(numeric(&lhs) && numeric(&rhs)) {
            return false;
        }
        let as_float = |v: &Value| match v {
            Value::Float(f) => *f,
            Value::Integer(i) => *i as f32,
            _ => 0.0,
        };
        let as_int = |v: &Value| match v {
            Value::Float(f) => *f as i32,
            Value::Integer(i) => *i,
            _ => 0,
        };
        let lhs_was_lookup = matches!(self.lhs_value, Some(Value::Lookup(_)));
        let rhs_was_lookup = matches!(self.rhs_value, Some(Value::Lookup(_)));
        let rhs_is_float = matches!(rhs, Value::Float(_));
        let lhs_is_float = matches!(lhs, Value::Float(_));
        if lhs_was_lookup == rhs_was_lookup {
            self.resolve_compare_to(float_compare(as_float(&lhs), as_float(&rhs)))
        } else if lhs_was_lookup {
            if rhs_is_float {
                self.resolve_compare_to(float_compare(as_float(&lhs), as_float(&rhs)))
            } else {
                self.resolve_compare_to(as_int(&lhs).cmp(&as_int(&rhs)))
            }
        } else if lhs_is_float {
            self.resolve_compare_to(float_compare(as_float(&lhs), as_float(&rhs)))
        } else {
            self.resolve_compare_to(as_int(&lhs).cmp(&as_int(&rhs)))
        }
    }
}
