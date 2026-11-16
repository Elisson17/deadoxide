//! zombie.scripting.ui.XuiAutoApply

/// zombie.scripting.ui.XuiAutoApply
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XuiAutoApply {
    No,
    Always,
    IfSet,
    IfSetAndKeyExists,
    Forbidden,
}

impl XuiAutoApply {
    /// java.lang.Enum.name
    pub fn name(self) -> &'static str {
        match self {
            XuiAutoApply::No => "No",
            XuiAutoApply::Always => "Always",
            XuiAutoApply::IfSet => "IfSet",
            XuiAutoApply::IfSetAndKeyExists => "IfSetAndKeyExists",
            XuiAutoApply::Forbidden => "Forbidden",
        }
    }
}
