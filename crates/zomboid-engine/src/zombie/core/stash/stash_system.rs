//! zombie.core.stash.StashSystem: which annotated-map stashes can still spawn, as `map_meta.bin` stores them.
use crate::java_nio::{read_string, ByteBuffer};

/// zombie.core.stash.StashBuilding
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StashBuilding {
    pub stash_name: String,
    pub building_x: i32,
    pub building_y: i32,
}

impl StashBuilding {
    fn load(b: &mut ByteBuffer) -> Result<Self, String> {
        Ok(StashBuilding {
            stash_name: read_string(b)?,
            building_x: b.get_int()?,
            building_y: b.get_int()?,
        })
    }
}

/// zombie.core.stash.StashSystem, the saved lists.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StashSystem {
    pub possible_stashes: Vec<StashBuilding>,
    pub buildings_to_do: Vec<StashBuilding>,
    pub already_read_map: Vec<String>,
}

impl StashSystem {
    /// zombie.core.stash.StashSystem.load: the three lists replace what `init()` built. `init()` itself (the
    /// `StashDescriptions` Lua table) is not part of this.
    pub fn load(b: &mut ByteBuffer, _world_version: i32) -> Result<Self, String> {
        let mut s = StashSystem::default();
        for _ in 0..b.get_int()? {
            s.possible_stashes.push(StashBuilding::load(b)?);
        }
        for _ in 0..b.get_int()? {
            s.buildings_to_do.push(StashBuilding::load(b)?);
        }
        for _ in 0..b.get_int()? {
            s.already_read_map.push(read_string(b)?);
        }
        Ok(s)
    }
}
