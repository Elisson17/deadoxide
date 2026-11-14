//! zombie.characters.PlayerCheats with zombie.characters.CheatType: the cheat flags a save stores for a character.
use std::collections::BTreeSet;

use crate::java_nio::ByteBuffer;

/// zombie.characters.CheatType names in ordinal order.
pub const CHEAT_TYPES: [&str; 25] = [
    "GodMod",
    "Invisible",
    "UnlimitedEndurance",
    "UnlimitedAmmo",
    "KnowAllRecipes",
    "UnlimitedCarry",
    "BuildCheat",
    "FarmingCheat",
    "FishingCheat",
    "HealthCheat",
    "MechanicsCheat",
    "FastMove",
    "MoveableCheat",
    "TimedActionInstant",
    "BrushTool",
    "NoClip",
    "CanSeeAll",
    "CanHearAll",
    "ZombiesDontAttack",
    "LootZed",
    "LootLog",
    "DebugContextMenu",
    "AnimalCheat",
    "AnimalExtraValues",
    "AlwaysDay",
];

/// zombie.characters.PlayerCheats: the set cheats as `CheatType` ordinals (an EnumSet, so ordered).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayerCheats {
    pub cheats: BTreeSet<usize>,
}

impl PlayerCheats {
    /// zombie.characters.PlayerCheats.load: a count, then one ordinal byte per cheat, each added unconditionally
    /// (the private `set(CheatType)`). `CheatType.fromId` of an unknown id throws.
    pub fn load(&mut self, b: &mut ByteBuffer) -> Result<(), String> {
        let size = b.get_int()?;
        for _ in 0..size {
            let id = b.get()?;
            let i = usize::try_from(id)
                .ok()
                .filter(|&i| i < CHEAT_TYPES.len())
                .ok_or_else(|| {
                    format!("java.lang.ArrayIndexOutOfBoundsException: Index {id} out of bounds")
                })?;
            self.cheats.insert(i);
        }
        Ok(())
    }
}
