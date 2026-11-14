//! zombie.characters.PlayerCraftHistory: how often and when a player crafted each recipe, as a save stores it.
use std::collections::BTreeMap;

use crate::java_nio::ByteBuffer;

/// zombie.characters.PlayerCraftHistory.CraftHistoryEntry
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CraftHistoryEntry {
    pub craft_count: i32,
    pub last_craft_time: f64,
}

/// zombie.characters.PlayerCraftHistory, the saved map (a HashMap in the original; ordered here).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerCraftHistory {
    pub craft_history: BTreeMap<String, CraftHistoryEntry>,
}

impl PlayerCraftHistory {
    /// zombie.characters.PlayerCraftHistory.load: each key as a char count and Java chars (UTF-16 units).
    pub fn load(b: &mut ByteBuffer) -> Result<Self, String> {
        let mut h = PlayerCraftHistory::default();
        let entry_count = b.get_int()?;
        for _ in 0..entry_count {
            let key_char_count = b.get_int()?;
            let mut units = Vec::new();
            for _ in 0..key_char_count {
                units.push(b.get_char()?);
            }
            let key = String::from_utf16_lossy(&units);
            let craft_count = b.get_int()?;
            let last_craft_time = b.get_double()?;
            h.craft_history.insert(
                key,
                CraftHistoryEntry {
                    craft_count,
                    last_craft_time,
                },
            );
        }
        Ok(h)
    }
}
