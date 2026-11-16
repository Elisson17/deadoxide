//! zombie.characters.action.conditions.CharacterVariableCondition
use crate::java;
use crate::zombie::core::skinnedmodel::advancedanimation::animation_variable_reference::AnimationVariableReference;
use crate::zombie::util::pz_xml_util::XmlElement;

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
}
