//! zombie.entity.ComponentType: the entity component kinds, by name and id (the component and script classes and
//! the flags are not ported).

/// zombie.entity.ComponentType
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ComponentType {
    Attributes,
    FluidContainer,
    SpriteConfig,
    Lua,
    Parts,
    Signals,
    Script,
    UiConfig,
    CraftLogic,
    FurnaceLogic,
    TestComponent,
    MashingLogic,
    DryingLogic,
    MetaTag,
    Resources,
    CraftBench,
    CraftRecipe,
    Durability,
    DryingCraftLogic,
    ContextMenuConfig,
    SpriteOverlayConfig,
    CraftBenchSounds,
    WallCoveringConfig,
    Undefined,
}

use ComponentType::*;

/// ComponentType.values(), with the ids the constructors give.
const VALUES: [(ComponentType, &str, i16); 24] = [
    (Attributes, "Attributes", 1),
    (FluidContainer, "FluidContainer", 2),
    (SpriteConfig, "SpriteConfig", 3),
    (Lua, "Lua", 6),
    (Parts, "Parts", 7),
    (Signals, "Signals", 8),
    (Script, "Script", 9),
    (UiConfig, "UiConfig", 11),
    (CraftLogic, "CraftLogic", 12),
    (FurnaceLogic, "FurnaceLogic", 13),
    (TestComponent, "TestComponent", 14),
    (MashingLogic, "MashingLogic", 15),
    (DryingLogic, "DryingLogic", 16),
    (MetaTag, "MetaTag", 17),
    (Resources, "Resources", 18),
    (CraftBench, "CraftBench", 19),
    (CraftRecipe, "CraftRecipe", 20),
    (Durability, "Durability", 21),
    (DryingCraftLogic, "DryingCraftLogic", 22),
    (ContextMenuConfig, "ContextMenuConfig", 23),
    (SpriteOverlayConfig, "SpriteOverlayConfig", 24),
    (CraftBenchSounds, "CraftBenchSounds", 25),
    (WallCoveringConfig, "WallCoveringConfig", 26),
    (Undefined, "Undefined", 0),
];

impl ComponentType {
    /// java.lang.Enum.name
    pub fn name(self) -> &'static str {
        VALUES[self as usize].1
    }

    /// zombie.entity.ComponentType.GetID
    pub fn get_id(self) -> i16 {
        VALUES[self as usize].2
    }

    /// ComponentType.valueOf: the constant of exactly that name; `Err` is the IllegalArgumentException.
    pub fn value_of(name: &str) -> Result<ComponentType, String> {
        VALUES
            .iter()
            .find(|v| v.1 == name)
            .map(|v| v.0)
            .ok_or_else(|| {
                format!("java.lang.IllegalArgumentException: No enum constant zombie.entity.ComponentType.{name}")
            })
    }
}
