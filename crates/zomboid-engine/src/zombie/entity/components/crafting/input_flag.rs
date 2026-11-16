//! zombie.entity.components.crafting.InputFlag, and the `EnumSet<InputFlag>` of an input line.

/// zombie.entity.components.crafting.InputFlag, by ordinal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputFlag(pub usize);

/// The constants of zombie.entity.components.crafting.InputFlag in declaration (ordinal) order.
pub const NAMES: [&str; 69] = [
    "HandcraftOnly",
    "AutomationOnly",
    "IsFull",
    "NotFull",
    "ItemIsUses",
    "ItemIsFluid",
    "ItemIsEnergy",
    "IsEmpty",
    "NotEmpty",
    "Prop1",
    "Prop2",
    "ToolLeft",
    "ToolRight",
    "IsDamaged",
    "IsUndamaged",
    "IsWholeFoodItem",
    "IsEmptyContainer",
    "IsUncookedFoodItem",
    "IsCookedFoodItem",
    "IsNotDull",
    "IsHeadPart",
    "IsSharpenable",
    "DontPutBack",
    "InheritColor",
    "InheritCondition",
    "InheritEquipped",
    "InheritSharpness",
    "InheritHeadCondition",
    "MayDegrade",
    "MayDegradeLight",
    "MayDegradeVeryLight",
    "MayDegradeHeavy",
    "SharpnessCheck",
    "InheritUses",
    "InheritUsesAndEmpty",
    "InheritFood",
    "InheritFoodAge",
    "InheritCooked",
    "InheritModelVariation",
    "InheritWeight",
    "InheritName",
    "InheritFreezingTime",
    "DontInheritCondition",
    "AllowFrozenItem",
    "AllowRottenItem",
    "NoBrokenItems",
    "AllowDestroyedItem",
    "IsWorn",
    "IsNotWorn",
    "InheritAmmunition",
    "CopyClothing",
    "AllowFavorite",
    "InheritFavorite",
    "FakeOutput",
    "DontReplace",
    "CanBeDoneFromFloor",
    "ItemCount",
    "IsExclusive",
    "RecordInput",
    "DontRecordInput",
    "ResearchInput",
    "IsBlunt",
    "HasOneUse",
    "HasNoUses",
    "IsSealed",
    "IsNotSealed",
    "Unseal",
    "EquipSecondary",
    "SetActivated",
];

impl InputFlag {
    pub const PROP1: InputFlag = InputFlag(9);
    pub const PROP2: InputFlag = InputFlag(10);
    pub const TOOL_LEFT: InputFlag = InputFlag(11);
    pub const TOOL_RIGHT: InputFlag = InputFlag(12);

    /// zombie.entity.components.crafting.InputFlag.valueOf: the constant of exactly that name, else the
    /// IllegalArgumentException.
    pub fn value_of(name: &str) -> Result<Self, String> {
        NAMES.iter().position(|n| *n == name).map(InputFlag).ok_or_else(|| {
            format!("java.lang.IllegalArgumentException: No enum constant zombie.entity.components.crafting.InputFlag.{name}")
        })
    }

    /// `name()` / `toString()`.
    pub fn name(self) -> &'static str {
        NAMES[self.0]
    }
}

/// java.util.EnumSet<InputFlag>: membership by ordinal, iterated in ordinal order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputFlags(pub u128);

impl InputFlags {
    /// `EnumSet.add`
    pub fn add(&mut self, f: InputFlag) {
        self.0 |= 1u128 << f.0;
    }
    /// `EnumSet.contains`
    pub fn contains(self, f: InputFlag) -> bool {
        self.0 & (1u128 << f.0) != 0
    }
    /// The members in ordinal order (the EnumSet's iteration order).
    pub fn iter(self) -> impl Iterator<Item = InputFlag> {
        (0..NAMES.len())
            .filter(move |i| self.0 & (1u128 << i) != 0)
            .map(InputFlag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_constants_have_their_ordinals() {
        assert_eq!(InputFlag::PROP1.name(), "Prop1");
        assert_eq!(InputFlag::PROP2.name(), "Prop2");
        assert_eq!(InputFlag::TOOL_LEFT.name(), "ToolLeft");
        assert_eq!(InputFlag::TOOL_RIGHT.name(), "ToolRight");
        assert!(InputFlag::value_of("prop1").is_err());
    }
}
