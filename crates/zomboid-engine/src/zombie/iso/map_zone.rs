//! zombie.iso.IsoMetaGrid.loadZone(ByteBuffer, int): `map_zone.bin`, the zones of a save (the map's zones as
//! `objects.lua` made them, plus what the game added: zone stories, spawn points, construction state), and the
//! registration of zones in the meta cells and chunks.
use std::collections::BTreeMap;

use super::iso_meta_cell::IsoMetaCell;
use super::iso_meta_grid::{IsoMetaGrid, ZoneRef};
use super::zones::zone::Zone;
use crate::java;
use crate::java_nio::{read_string, ByteBuffer};

/// What `IsoMetaGrid.loadZone` reads besides the zones it registers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapZone {
    pub world_version: i32,
    /// The zone count the file states (`DebugLog.log("loading N zones from map_zone.bin")`).
    pub zones_in_file: i32,
    /// zombie.iso.IsoWorld.spawnedZombieZone: zone name -> zone ids (`None` where the original draws a random UUID,
    /// before world version 215). A name that occurs twice keeps the later list, as `HashMap.put` does.
    pub spawned_zombie_zone: BTreeMap<String, Vec<Option<(i64, i64)>>>,
    /// What the original logs (`DebugLog.log`) while reading.
    pub log: Vec<String>,
}

fn java_str(s: &Option<String>) -> &str {
    s.as_deref().unwrap_or("null")
}

impl IsoMetaGrid {
    /// zombie.iso.IsoMetaGrid.getCellOrCreate: grid-relative coordinates; a new cell has no header (`info` null).
    pub fn get_cell_or_create(&mut self, x: i32, y: i32) -> Option<&mut IsoMetaCell> {
        let (wx, wy) = (self.min_x + x, self.min_y + y);
        let idx = self.cell_index(wx, wy)?;
        if self.get_cell_data(wx, wy).is_none() {
            self.set_cell(idx, IsoMetaCell::new(wx, wy, None));
        }
        self.get_cell_data_mut(wx, wy)
    }

    /// zombie.iso.IsoMetaGrid.registerZone(Zone): adds the zone unless it looks broken: outside the grid by more than
    /// 100 squares, a level outside -32..31, or wider or taller than 1202. The right and bottom checks add the GRID's
    /// width and height (in cells) to the zone's position, not the zone's own size, as the original does.
    pub fn register_zone(&mut self, zone: Zone, log: &mut Vec<String>) -> Option<ZoneRef> {
        if zone.x >= self.min_x * 256 - 100
            && zone.y >= self.min_y * 256 - 100
            && zone.x + self.grid_x() <= (self.max_x + 1) * 256 + 100
            && zone.y + self.grid_y() <= (self.max_y + 1) * 256 + 100
            && zone.z >= -32
            && zone.z <= 31
            && zone.w <= 1202
            && zone.h <= 1202
        {
            Some(self.add_zone(zone))
        } else {
            log.push(format!(
                "111ERROR: not adding suspicious zone \"{}\" \"{}\" {},{},{} {}x{}",
                java_str(&zone.name),
                java_str(&zone.type_),
                zone.x,
                zone.y,
                zone.z,
                zone.w,
                zone.h
            ));
            None
        }
    }

    /// zombie.iso.IsoMetaGrid.addZone: appends the zone and registers it in every grid cell its box touches (the
    /// cell range is inclusive of `(x + w) / 256`), creating cells without a header where needed.
    pub fn add_zone(&mut self, mut zone: Zone) -> ZoneRef {
        let r = ZoneRef(self.zones.len() as u32);
        for yy in zone.y / 256..=(zone.y + zone.h) / 256 {
            for xx in zone.x / 256..=(zone.x + zone.w) / 256 {
                if xx >= self.min_x && xx <= self.max_x && yy >= self.min_y && yy <= self.max_y {
                    self.add_zone_to_cell(xx, yy, r, &mut zone);
                }
            }
        }
        self.zones.push(zone);
        r
    }

    /// `getCellOrCreate(xx - minX, yy - minY).addZone(zone, xx * 256, yy * 256)`; new meta chunks take their zombie
    /// intensity from the cell's header (`LotHeader.getZombieIntensityForChunk`, -1 -> 0 without one).
    fn add_zone_to_cell(&mut self, cell_x: i32, cell_y: i32, r: ZoneRef, zone: &mut Zone) {
        let Some(priority) = self
            .get_cell_or_create(cell_x - self.min_x, cell_y - self.min_y)
            .map(|c| c.info_priority)
        else {
            return;
        };
        let Some(mut cell) = self.take_cell(cell_x, cell_y) else {
            return;
        };
        cell.add_zone(r, zone, cell_x * 256, cell_y * 256, |cx, cy| {
            priority.map_or(-1, |p| {
                self.get_zombie_intensity_for_chunk(p, cell_x, cell_y, cx, cy)
            })
        });
        self.put_cell(cell);
    }

    /// zombie.iso.IsoMetaGrid.loadZone: the "ZONE" magic and world version (when `world_version` is -1), then every
    /// zone the grid had is dropped (each meta chunk loses its zones and goes if it has no rooms either), then the
    /// string map, the zones (those of type "WorldGen" are not registered) and `IsoWorld.spawnedZombieZone`.
    pub fn load_zone(
        &mut self,
        b: &mut ByteBuffer,
        mut world_version: i32,
    ) -> Result<MapZone, String> {
        let mut m = MapZone::default();
        if world_version == -1 {
            let magic = b.get_bytes(4)?;
            if magic != b"ZONE" {
                m.log
                    .push("ERROR: expected 'ZONE' at start of map_zone.bin".to_string());
                return Ok(m);
            }
            world_version = b.get_int()?;
        }
        m.world_version = world_version;
        for zone in &mut self.zones {
            zone.dispose();
        }
        self.zones.clear();
        for y in 0..self.grid_y() {
            for x in 0..self.grid_x() {
                let (wx, wy) = (self.min_x + x, self.min_y + y);
                if let Some(cell) = self.get_cell_data_mut(wx, wy) {
                    for i in 0..1024 {
                        if let Some(c) = cell.chunk_mut(i) {
                            c.clear_zones();
                            cell.clear_chunk(i);
                        }
                    }
                }
            }
        }
        // IsoMetaGrid.loadStringMap: index -> string
        let num_strings = b.get_int()?;
        let mut string_map = Vec::new();
        for _ in 0..num_strings {
            string_map.push(read_string(b)?);
        }
        m.zones_in_file = b.get_int()?;
        m.log.push(format!(
            "loading {} zones from map_zone.bin",
            m.zones_in_file
        ));
        for _ in 0..m.zones_in_file {
            let zone = Zone::load(b, world_version, &string_map)?;
            if !zone
                .type_
                .as_deref()
                .is_some_and(|t| java::equals_ignore_case("WorldGen", t))
            {
                self.register_zone(zone, &mut m.log);
            }
        }
        let spawned = b.get_int()?;
        for _ in 0..spawned {
            let zone_name = read_string(b)?;
            let n = b.get_int()?;
            let mut ids = Vec::new();
            for _ in 0..n {
                ids.push(if world_version >= 215 {
                    Some((b.get_long()?, b.get_long()?))
                } else {
                    None
                });
            }
            m.spawned_zombie_zone.insert(zone_name, ids);
        }
        Ok(m)
    }
}
