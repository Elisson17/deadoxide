//! zombie.scripting.ui.XuiScriptType

/// zombie.scripting.ui.XuiScriptType
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XuiScriptType {
    Layout,
    Style,
    DefaultStyle,
    Reference,
}

impl XuiScriptType {
    /// java.lang.Enum.name
    pub fn name(self) -> &'static str {
        match self {
            XuiScriptType::Layout => "Layout",
            XuiScriptType::Style => "Style",
            XuiScriptType::DefaultStyle => "DefaultStyle",
            XuiScriptType::Reference => "Reference",
        }
    }
}
