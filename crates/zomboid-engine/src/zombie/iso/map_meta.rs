//! zombie.iso.IsoMetaGrid.load(ByteBuffer): `map_meta.bin`, the saved state of the meta grid's rooms and buildings and
//! the world lists stored after them (safehouses, non-PvP zones, factions, designation zones, stashes, unique
//! randomized-building stories).
use super::iso_meta_grid::IsoMetaGrid;
use crate::java_nio::{read_string, ByteBuffer};
use crate::zombie::characters::faction::Faction;
use crate::zombie::core::stash::stash_system::StashSystem;
use crate::zombie::iso::areas::designation_zone::DesignationZone;
use crate::zombie::iso::areas::non_pvp_zone::NonPvpZone;
use crate::zombie::iso::areas::safe_house::SafeHouse;

/// Which kind of game reads the file: `GameServer.server`, `GameClient.client`, or neither (single player). The
/// stash part differs: a server's file has the stash block's end position in front of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetaSide {
    SinglePlayer,
    Server,
    Client,
}

/// What `IsoMetaGrid.load` reads besides the room and building state it writes into the grid.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapMeta {
    pub world_version: i32,
    pub total_buildings: i32,
    pub total_alarmed: i32,
    /// zombie.iso.areas.SafeHouse.safehouseList
    pub safehouses: Vec<SafeHouse>,
    /// zombie.iso.areas.NonPvpZone.nonPvpZoneList
    pub non_pvp_zones: Vec<NonPvpZone>,
    /// zombie.characters.Faction.factions
    pub factions: Vec<Faction>,
    pub designation_zones: Vec<DesignationZone>,
    /// `None` on a client, which skips the stash block.
    pub stash_system: Option<StashSystem>,
    /// zombie.randomizedWorld.randomizedBuilding.RBBasic.uniqueRDSSpawned
    pub unique_rds_spawned: Vec<String>,
    /// What the original logs (size mismatch, meta ids the grid does not know) and reads past.
    pub log: Vec<String>,
}

impl IsoMetaGrid {
    /// zombie.iso.IsoMetaGrid.load(ByteBuffer): magic, world version and the saved cell range, then for every cell of
    /// the range (columns first) its rooms (meta id, flags: explored, lights active, done spawn, roof fixed) and
    /// buildings (meta id, alarmed, key id, seen, visited, loot respawn hour, alarm decay from world version 201),
    /// applied to the room or building with that meta id in a loaded cell; then the safehouses, non-PvP zones,
    /// factions, designation zones, the stash system and `RBBasic.uniqueRDSSpawned`.
    pub fn load(&mut self, b: &mut ByteBuffer, side: MetaSide) -> Result<MapMeta, String> {
        let _magic = b.get_bytes(4)?;
        let world_version = b.get_int()?;
        let (x1, y1, x2, y2) = (b.get_int()?, b.get_int()?, b.get_int()?, b.get_int()?);
        let mut m = MapMeta {
            world_version,
            ..MapMeta::default()
        };
        let (w, h) = (
            x2.wrapping_sub(x1).wrapping_add(1),
            y2.wrapping_sub(y1).wrapping_add(1),
        );
        if w != self.grid_x() || h != self.grid_y() {
            m.log.push(format!(
                "map_meta.bin world size ({w}x{h}) does not match the current map size ({}x{})",
                self.grid_x(),
                self.grid_y()
            ));
        }
        for x in x1..=x2 {
            for y in y1..=y2 {
                // cell != null && cell.info != null
                let loaded = self
                    .get_cell_data(x, y)
                    .is_some_and(|c| c.info_priority.is_some());
                for _ in 0..b.get_int()? {
                    let meta_id = b.get_long()?;
                    let flags = b.get_short()?;
                    if !loaded {
                        continue;
                    }
                    let r = self
                        .get_cell_data(x, y)
                        .and_then(|c| c.room_by_meta_id.get(&meta_id).copied());
                    match r {
                        Some(r) => {
                            let room = self.room_mut(r);
                            room.explored = flags & 1 != 0;
                            room.lights_active = flags & 2 != 0;
                            room.done_spawn = flags & 4 != 0;
                            room.roof_fixed = flags & 8 != 0;
                        }
                        None => m.log.push(format!(
                            "invalid room metaID #{meta_id} in cell {x},{y} while reading map_meta.bin"
                        )),
                    }
                }
                let num_buildings = b.get_int()?;
                m.total_buildings = m.total_buildings.wrapping_add(num_buildings);
                for _ in 0..num_buildings {
                    let meta_id = b.get_long()?;
                    let alarmed = b.get()? != 0;
                    let key_id = b.get_int()?;
                    let seen = b.get()? != 0;
                    let has_been_visited = b.get()? != 0;
                    let loot_respawn_hour = b.get_int()?;
                    let alarm_decay = if world_version >= 201 {
                        b.get_int()?
                    } else {
                        0
                    };
                    if !loaded {
                        continue;
                    }
                    let d = self
                        .get_cell_data(x, y)
                        .and_then(|c| c.building_by_meta_id.get(&meta_id).copied());
                    match d {
                        Some(d) => {
                            if alarmed {
                                m.total_alarmed += 1;
                            }
                            let def = self.building_mut(d);
                            def.alarmed = alarmed;
                            def.key_id = key_id;
                            def.seen = seen;
                            def.has_been_visited = has_been_visited;
                            def.loot_respawn_hour = loot_respawn_hour;
                            def.alarm_decay = alarm_decay;
                        }
                        None => m.log.push(format!(
                            "invalid building metaID #{meta_id} in cell {x},{y} while reading map_meta.bin"
                        )),
                    }
                }
            }
        }
        for _ in 0..b.get_int()? {
            m.safehouses.push(SafeHouse::load(b, world_version)?);
        }
        for _ in 0..b.get_int()? {
            m.non_pvp_zones.push(NonPvpZone::load(b, world_version)?);
        }
        for _ in 0..b.get_int()? {
            m.factions.push(Faction::load(b, world_version)?);
        }
        for _ in 0..b.get_int()? {
            if let Some(z) = DesignationZone::load(b, world_version)? {
                m.designation_zones.push(z);
            }
        }
        match side {
            MetaSide::Server => {
                let _position = b.get_int()?;
                m.stash_system = Some(StashSystem::load(b, world_version)?);
            }
            MetaSide::Client => {
                let position = b.get_int()?;
                b.set_position(usize::try_from(position).map_err(|e| e.to_string())?)?;
            }
            MetaSide::SinglePlayer => m.stash_system = Some(StashSystem::load(b, world_version)?),
        }
        for _ in 0..b.get_int()? {
            m.unique_rds_spawned.push(read_string(b)?);
        }
        Ok(m)
    }
}
