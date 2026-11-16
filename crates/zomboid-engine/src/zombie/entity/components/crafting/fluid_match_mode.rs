//! zombie.entity.components.crafting.FluidMatchMode

/// zombie.entity.components.crafting.FluidMatchMode
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FluidMatchMode {
    #[default]
    Exact,
    Primary,
    Mixture,
    Anything,
}

impl FluidMatchMode {
    /// `name()` / `toString()`.
    pub fn name(self) -> &'static str {
        match self {
            FluidMatchMode::Exact => "Exact",
            FluidMatchMode::Primary => "Primary",
            FluidMatchMode::Mixture => "Mixture",
            FluidMatchMode::Anything => "Anything",
        }
    }
}
