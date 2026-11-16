//! zombie.entity.components.crafting.OutputFlag, and the `EnumSet<OutputFlag>` of an output line.

/// zombie.entity.components.crafting.OutputFlag, by ordinal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputFlag(pub usize);

/// The constants of zombie.entity.components.crafting.OutputFlag in declaration (ordinal) order.
pub const NAMES: [&str; 12] = [
    "HandcraftOnly",
    "AutomationOnly",
    "IsEmpty",
    "ForceEmpty",
    "AlwaysFill",
    "RespectCapacity",
    "IsBlunt",
    "HasOneUse",
    "HasNoUses",
    "DontInheritCondition",
    "EquipSecondary",
    "SetActivated",
];

impl OutputFlag {
    /// zombie.entity.components.crafting.OutputFlag.valueOf: the constant of exactly that name, else the
    /// IllegalArgumentException.
    pub fn value_of(name: &str) -> Result<Self, String> {
        NAMES.iter().position(|n| *n == name).map(OutputFlag).ok_or_else(|| {
            format!("java.lang.IllegalArgumentException: No enum constant zombie.entity.components.crafting.OutputFlag.{name}")
        })
    }

    /// `name()` / `toString()`.
    pub fn name(self) -> &'static str {
        NAMES[self.0]
    }
}

/// java.util.EnumSet<OutputFlag>: membership by ordinal, iterated in ordinal order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OutputFlags(pub u32);

impl OutputFlags {
    /// `EnumSet.add`
    pub fn add(&mut self, f: OutputFlag) {
        self.0 |= 1u32 << f.0;
    }
    /// The members in ordinal order (the EnumSet's iteration order).
    pub fn iter(self) -> impl Iterator<Item = OutputFlag> {
        (0..NAMES.len())
            .filter(move |i| self.0 & (1u32 << i) != 0)
            .map(OutputFlag)
    }
}
