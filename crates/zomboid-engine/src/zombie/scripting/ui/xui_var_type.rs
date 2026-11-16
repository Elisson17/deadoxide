//! zombie.scripting.ui.XuiVarType

/// zombie.scripting.ui.XuiVarType
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum XuiVarType {
    Function,
    String,
    StringList,
    TranslateString,
    Integer,
    Double,
    Float,
    Boolean,
    FontType,
    VectorPosAlign,
    TextAlign,
    Color,
    Unit,
    Vector,
    Texture,
}

const NAMES: [&str; 15] = [
    "Function",
    "String",
    "StringList",
    "TranslateString",
    "Integer",
    "Double",
    "Float",
    "Boolean",
    "FontType",
    "VectorPosAlign",
    "TextAlign",
    "Color",
    "Unit",
    "Vector",
    "Texture",
];

const VALUES: [XuiVarType; 15] = [
    XuiVarType::Function,
    XuiVarType::String,
    XuiVarType::StringList,
    XuiVarType::TranslateString,
    XuiVarType::Integer,
    XuiVarType::Double,
    XuiVarType::Float,
    XuiVarType::Boolean,
    XuiVarType::FontType,
    XuiVarType::VectorPosAlign,
    XuiVarType::TextAlign,
    XuiVarType::Color,
    XuiVarType::Unit,
    XuiVarType::Vector,
    XuiVarType::Texture,
];

impl XuiVarType {
    /// java.lang.Enum.name
    pub fn name(self) -> &'static str {
        NAMES[self as usize]
    }

    /// XuiVarType.valueOf: the constant of exactly that name; `Err` is the IllegalArgumentException.
    pub fn value_of(name: &str) -> Result<XuiVarType, String> {
        NAMES
            .iter()
            .position(|n| *n == name)
            .map(|i| VALUES[i])
            .ok_or_else(|| {
                format!(
                    "java.lang.IllegalArgumentException: No enum constant zombie.scripting.ui.XuiVarType.{name}"
                )
            })
    }
}
