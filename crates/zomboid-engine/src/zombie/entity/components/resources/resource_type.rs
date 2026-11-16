//! zombie.entity.components.resources.ResourceType

/// zombie.entity.components.resources.ResourceType
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceType {
    Item,
    Fluid,
    Energy,
    Any,
}

impl ResourceType {
    /// `name()` / `toString()`.
    pub fn name(self) -> &'static str {
        match self {
            ResourceType::Item => "Item",
            ResourceType::Fluid => "Fluid",
            ResourceType::Energy => "Energy",
            ResourceType::Any => "Any",
        }
    }
}
