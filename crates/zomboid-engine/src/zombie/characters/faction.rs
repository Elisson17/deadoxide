//! zombie.characters.Faction: a player faction as `map_meta.bin` stores it.
use crate::java_nio::{read_string, ByteBuffer};
use crate::zombie::inventory::inventory_item::not_ported;

/// zombie.characters.Faction, the saved fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Faction {
    pub name: String,
    pub owner: String,
    pub tag: Option<String>,
    /// `tagColor` r, g, b (alpha is 1).
    pub tag_color: [f32; 3],
    pub players: Vec<String>,
}

impl Faction {
    /// zombie.characters.Faction.load. A faction saved without a tag gets a random colour (three `Rand` draws): that
    /// branch is `not ported:`. The faction chat of a running chat server is not modelled.
    pub fn load(b: &mut ByteBuffer, _world_version: i32) -> Result<Self, String> {
        let name = read_string(b)?;
        let owner = read_string(b)?;
        let player_size = b.get_int()?;
        if b.get()? == 0 {
            return Err(not_ported("Faction.load without a tag: Rand tag colour"));
        }
        let tag = Some(read_string(b)?);
        let tag_color = [b.get_float()?, b.get_float()?, b.get_float()?];
        let mut players = Vec::new();
        for _ in 0..player_size {
            players.push(read_string(b)?);
        }
        Ok(Faction {
            name,
            owner,
            tag,
            tag_color,
            players,
        })
    }
}
