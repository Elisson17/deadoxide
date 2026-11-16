//! zombie.entity.components.crafting.ItemApplyMode

/// zombie.entity.components.crafting.ItemApplyMode
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ItemApplyMode {
    #[default]
    Normal,
    Keep,
    Destroy,
}

impl ItemApplyMode {
    /// `name()` / `toString()`.
    pub fn name(self) -> &'static str {
        match self {
            ItemApplyMode::Normal => "Normal",
            ItemApplyMode::Keep => "Keep",
            ItemApplyMode::Destroy => "Destroy",
        }
    }
}
