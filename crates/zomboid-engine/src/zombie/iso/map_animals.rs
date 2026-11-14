//! zombie.iso.IsoMetaGrid.loadAnimalZones(ByteBuffer, int) (`map_animals.bin`), registerAnimalZone, and
//! IsoMetaCell.load for `metagrid/metacell_X_Y.bin`: the next steps of `IsoWorld.init` after `map_zone.bin`.
use super::iso_meta_grid::{AnimalZoneRef, IsoMetaGrid};
use crate::java_nio::{read_string, ByteBuffer};
use crate::zombie::characters::animals::animal_zone::AnimalZone;
use crate::zombie::characters::animals::animal_zone_junction::AnimalZoneJunction;

/// What `IsoMetaGrid.loadAnimalZones` reads besides the zones it registers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapAnimals {
    pub world_version: i32,
    /// The zone count the file states (`DebugLog.log("loading N zones from map_animals.bin")`).
    pub zones_in_file: i32,
    pub junctions_in_file: i32,
    /// What the original logs (`DebugLog.log`) while reading.
    pub log: Vec<String>,
}

impl IsoMetaGrid {
    /// zombie.iso.IsoMetaGrid.registerAnimalZone(AnimalZone, boolean): into the handler (by id; a later zone with
    /// the same id replaces the earlier there) and into every cell of the grid the zone's box touches (cells created
    /// as needed). `bHotSave` (`addCellToSave`) only matters for saving and is not modelled.
    pub fn register_animal_zone(&mut self, zone: AnimalZone) -> AnimalZoneRef {
        let r = AnimalZoneRef(self.animal_zones.len() as u32);
        let (x, y, w, h) = (zone.zone.x, zone.zone.y, zone.zone.w, zone.zone.h);
        if let Some(id) = zone.zone.id {
            self.animal_zone_handler.insert(id, r);
        }
        self.animal_zones.push(zone);
        // (int) Math.ceil((x + w) / 256.0F)
        let cell_max_x = f64::from(x.wrapping_add(w) as f32 / 256.0).ceil() as i32;
        let cell_max_y = f64::from(y.wrapping_add(h) as f32 / 256.0).ceil() as i32;
        for yy in y / 256..cell_max_y {
            for xx in x / 256..cell_max_x {
                if xx >= self.min_x && xx <= self.max_x && yy >= self.min_y && yy <= self.max_y {
                    let (gx, gy) = (xx - self.min_x, yy - self.min_y);
                    if let Some(cell) = self.get_cell_or_create(gx, gy) {
                        cell.animal_zones.push(r);
                    }
                }
            }
        }
        r
    }

    /// zombie.iso.IsoMetaGrid.loadAnimalZones: the "ZONE" magic and world version, then every animal zone the grid
    /// had is dropped (from the handler and the cells), then the string map, the zones (`AnimalZone.load`,
    /// `registerAnimalZone` without hot save) and the junctions (`AnimalZoneJunction.load`, added to their own zone).
    /// `Err` where the original throws: a junction whose own zone id is unknown (`NullPointerException` in its
    /// constructor) or whose point index is past the zone's points; what was read before stays, as in the original,
    /// whose caller logs the exception.
    pub fn load_animal_zones(
        &mut self,
        b: &mut ByteBuffer,
        mut world_version: i32,
    ) -> Result<MapAnimals, String> {
        let mut m = MapAnimals::default();
        if world_version == -1 {
            let magic = b.get_bytes(4)?;
            if magic != b"ZONE" {
                m.log
                    .push("ERROR: expected 'ZONE' at start of map_animals.bin".to_string());
                return Ok(m);
            }
            world_version = b.get_int()?;
        }
        m.world_version = world_version;
        for zone in &mut self.animal_zones {
            zone.dispose();
        }
        self.animal_zones.clear();
        self.animal_zone_handler.clear();
        for y in 0..self.grid_y() {
            for x in 0..self.grid_x() {
                let (wx, wy) = (self.min_x + x, self.min_y + y);
                if let Some(cell) = self.get_cell_data_mut(wx, wy) {
                    cell.animal_zones.clear();
                }
            }
        }
        // IsoMetaGrid.loadStringMap
        let num_strings = b.get_int()?;
        let mut string_map = Vec::new();
        for _ in 0..num_strings {
            string_map.push(read_string(b)?);
        }
        m.zones_in_file = b.get_int()?;
        m.log.push(format!(
            "loading {} zones from map_animals.bin",
            m.zones_in_file
        ));
        for _ in 0..m.zones_in_file {
            let zone = AnimalZone::load(b, world_version, &string_map)?;
            self.register_animal_zone(zone);
        }
        m.junctions_in_file = b.get_int()?;
        for _ in 0..m.junctions_in_file {
            // AnimalZoneJunction.load
            let point_index_self = b.get_int()?;
            let point_index_other = b.get_int()?;
            let self_id = (b.get_long()?, b.get_long()?);
            let other_id = (b.get_long()?, b.get_long()?);
            let zone_self = self.animal_zone_handler.get(&self_id).copied();
            let zone_other = self.animal_zone_handler.get(&other_id).copied();
            let Some(zone_self) = zone_self else {
                return Err(
                    "java.lang.NullPointerException: AnimalZoneJunction of an unknown animal zone"
                        .to_string(),
                );
            };
            let z = &mut self.animal_zones[zone_self.0 as usize];
            let distance_from_start = z.get_distance_of_point_from_start(point_index_self)?;
            z.add_junction(AnimalZoneJunction {
                zone_self,
                point_index_self,
                zone_other,
                point_index_other,
                distance_from_start,
            });
        }
        Ok(m)
    }

    /// zombie.iso.IsoMetaGrid.loadCells("metagrid", "metacell_(-?[0-9]+)_(-?[0-9]+)\\.bin", IsoMetaCell::load):
    /// every matching file of the save's `metagrid` folder, its cell made by `getCellOrCreate`, then
    /// `IsoMetaCell.load`: one byte, whether the cell's animal zones were generated. The original takes the files in
    /// the order `File.list` gives (unspecified); here they are sorted by name. Errors are logged and skipped, as the
    /// original's `ExceptionLogger.logException` does.
    pub fn load_metacells(&mut self, save: &std::path::Path) -> Vec<String> {
        let mut log = Vec::new();
        let dir = save.join("metagrid");
        let Ok(rd) = std::fs::read_dir(&dir) else {
            return log;
        };
        let mut names: Vec<String> = rd
            .filter_map(|e| e.ok().and_then(|e| e.file_name().into_string().ok()))
            .collect();
        names.sort();
        for name in names {
            let Some((cx, cy)) = metacell_coords(&name) else {
                continue;
            };
            let data = match std::fs::read(dir.join(&name)) {
                Ok(d) => d,
                Err(e) => {
                    log.push(format!("{name}: {e}"));
                    continue;
                }
            };
            let (gx, gy) = (cx - self.min_x, cy - self.min_y);
            let Some(cell) = self.get_cell_or_create(gx, gy) else {
                log.push(format!(
                    "{name}: java.lang.ArrayIndexOutOfBoundsException (cell outside the grid)"
                ));
                continue;
            };
            match data.first() {
                Some(&v) => cell.animal_zones_generated = v != 0,
                None => log.push(format!("{name}: java.nio.BufferUnderflowException")),
            }
        }
        log
    }
}

/// The two numbers of `metacell_(-?[0-9]+)_(-?[0-9]+)\.bin` (`Integer.parseInt` of each group).
fn metacell_coords(name: &str) -> Option<(i32, i32)> {
    let rest = name.strip_prefix("metacell_")?.strip_suffix(".bin")?;
    let (a, b) = split_signed_pair(rest)?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

/// Splits `-?[0-9]+_-?[0-9]+` at the underscore between the two numbers.
fn split_signed_pair(s: &str) -> Option<(&str, &str)> {
    let (a, b) = s.split_once('_')?;
    let num = |t: &str| {
        let d = t.strip_prefix('-').unwrap_or(t);
        !d.is_empty() && d.bytes().all(|c| c.is_ascii_digit())
    };
    (num(a) && num(b)).then_some((a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metacell_names_follow_the_original_pattern() {
        assert_eq!(metacell_coords("metacell_32_45.bin"), Some((32, 45)));
        assert_eq!(metacell_coords("metacell_-3_-1.bin"), Some((-3, -1)));
        assert_eq!(metacell_coords("metacell_32_45.bin.bak"), None);
        assert_eq!(metacell_coords("metacell_3a_45.bin"), None);
        assert_eq!(metacell_coords("metacell__45.bin"), None);
    }
}
