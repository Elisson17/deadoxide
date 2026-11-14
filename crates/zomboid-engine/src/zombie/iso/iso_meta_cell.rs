use std::collections::HashMap;

use super::iso_meta_chunk::IsoMetaChunk;
use super::iso_meta_grid::{BuildingRef, RoomRef, ZoneRef};
use super::room_def::RoomDef;
use super::zones::zone::Zone;
use crate::zombie::core::math::pz_math;

/// zombie.iso.IsoMetaCell: per-cell meta data: rooms and buildings from the lotheader, the 32x32 meta chunks and the
/// zones registered in them. Triggers, vehicle/animal/mannequin zones and runtime rooms are not ported yet.
#[derive(Debug)]
pub struct IsoMetaCell {
    wx: i32,
    wy: i32,
    /// zombie.iso.IsoMetaCell.info, as the priority index of the map folder whose header created the cell
    /// (`info.mapFiles.priority`); `None` for a cell without a header (made by `getCellOrCreate` for a zone).
    pub info_priority: Option<usize>,
    chunk_map: Vec<Option<IsoMetaChunk>>,
    /// zombie.iso.IsoMetaCell.roomList: a room's position here is the index part of its id.
    pub room_list: Vec<RoomRef>,
    pub rooms: HashMap<i64, RoomRef>,
    /// zombie.iso.IsoMetaCell.roomByMetaId (a duplicate meta id keeps the later room).
    pub room_by_meta_id: HashMap<i64, RoomRef>,
    pub buildings: Vec<BuildingRef>,
    pub building_by_meta_id: HashMap<i64, BuildingRef>,
    /// zombie.iso.IsoMetaCell.animalZones
    pub animal_zones: Vec<super::iso_meta_grid::AnimalZoneRef>,
    /// zombie.iso.IsoMetaCell.animalZonesGenerated (`metagrid/metacell_X_Y.bin`, `IsoMetaCell.load`).
    pub animal_zones_generated: bool,
}

impl IsoMetaCell {
    /// zombie.iso.IsoMetaCell(int, int), with `info` set as the lot loader does.
    pub fn new(wx: i32, wy: i32, info_priority: Option<usize>) -> Self {
        IsoMetaCell {
            wx,
            wy,
            info_priority,
            chunk_map: vec![None; 1024],
            room_list: Vec::new(),
            rooms: HashMap::new(),
            room_by_meta_id: HashMap::new(),
            buildings: Vec::new(),
            building_by_meta_id: HashMap::new(),
            animal_zones: Vec::new(),
            animal_zones_generated: false,
        }
    }
    /// zombie.iso.IsoMetaCell.getX
    pub fn get_x(&self) -> i32 {
        self.wx
    }
    /// zombie.iso.IsoMetaCell.getY
    pub fn get_y(&self) -> i32 {
        self.wy
    }
    /// zombie.iso.IsoMetaCell.hasChunk
    pub fn has_chunk(&self, x: i32, y: i32) -> bool {
        (0..32).contains(&x)
            && (0..32).contains(&y)
            && self.chunk_map[(y * 32 + x) as usize].is_some()
    }
    /// The chunk if it exists; `getChunk` of the original also creates it (see `IsoMetaGrid::get_chunk_data`).
    pub fn chunk(&self, i: usize) -> Option<&IsoMetaChunk> {
        self.chunk_map[i].as_ref()
    }
    /// zombie.iso.IsoMetaCell.getChunk (int): created on first use with `intensity`, the value of
    /// `LotHeader.getZombieIntensityForChunk` (negative means none, stored as 0).
    pub fn get_chunk(&mut self, i: usize, intensity: impl FnOnce() -> i32) -> &mut IsoMetaChunk {
        self.chunk_map[i].get_or_insert_with(|| {
            let mut c = IsoMetaChunk::default();
            let v = intensity();
            c.set_zombie_intensity(if v >= 0 { v as u8 } else { 0 });
            c
        })
    }
    /// zombie.iso.IsoMetaCell.clearChunk: drops the chunk when it holds neither rooms nor zones.
    pub fn clear_chunk(&mut self, i: usize) {
        if self.chunk_map[i]
            .as_ref()
            .is_some_and(|c| c.get_rooms_size() == 0 && c.get_zones_size() == 0)
        {
            self.chunk_map[i] = None;
        }
    }
    /// The chunk if it exists, for changes that must not create it.
    pub fn chunk_mut(&mut self, i: usize) -> Option<&mut IsoMetaChunk> {
        self.chunk_map[i].as_mut()
    }
    /// zombie.iso.IsoMetaCell.addZone: registers the zone in every chunk of THIS cell (origin `cell_x`, `cell_y` in
    /// squares) that its shape intersects. The chunk range is clamped to the cell; the lower bounds to
    /// `(cell + 256) / 8`, one past the cell, as in the original (the loop is then empty).
    pub fn add_zone(
        &mut self,
        r: ZoneRef,
        zone: &mut Zone,
        cell_x: i32,
        cell_y: i32,
        mut intensity: impl FnMut(i32, i32) -> i32,
    ) {
        let mut xmin = zone.x / 8;
        let mut ymin = zone.y / 8;
        let mut xmax = (zone.x + zone.w) / 8;
        if (zone.x + zone.w) % 8 == 0 {
            xmax -= 1;
        }
        let mut ymax = (zone.y + zone.h) / 8;
        if (zone.y + zone.h) % 8 == 0 {
            ymax -= 1;
        }
        xmin = pz_math::clamp_i32(xmin, cell_x / 8, (cell_x + 256) / 8);
        ymin = pz_math::clamp_i32(ymin, cell_y / 8, (cell_y + 256) / 8);
        xmax = pz_math::clamp_i32(xmax, cell_x / 8, (cell_x + 256) / 8 - 1);
        ymax = pz_math::clamp_i32(ymax, cell_y / 8, (cell_y + 256) / 8 - 1);
        for yy in ymin..=ymax {
            for xx in xmin..=xmax {
                if zone.intersects(xx * 8, yy * 8, zone.z, 8, 8) {
                    let i = (xx - cell_x / 8 + (yy - cell_y / 8) * 32) as usize;
                    self.get_chunk(i, || intensity((i % 32) as i32, (i / 32) as i32))
                        .add_zone(r);
                }
            }
        }
    }
    /// zombie.iso.IsoMetaCell.addRoom: registers the room in every chunk of THIS cell its bounding box touches.
    /// `cell_x`/`cell_y` are the cell origin in squares. Java's truncating division is kept.
    pub fn add_room(
        &mut self,
        r: RoomRef,
        room: &RoomDef,
        cell_x: i32,
        cell_y: i32,
        mut intensity: impl FnMut(i32, i32) -> i32,
    ) {
        let mut xmax = room.x2 / 8;
        if room.x2 % 8 == 0 {
            xmax -= 1;
        }
        let mut ymax = room.y2 / 8;
        if room.y2 % 8 == 0 {
            ymax -= 1;
        }
        for yy in room.y / 8..=ymax {
            for xx in room.x / 8..=xmax {
                if xx >= cell_x / 8
                    && xx < (cell_x + 256) / 8
                    && yy >= cell_y / 8
                    && yy < (cell_y + 256) / 8
                {
                    let i = (xx - cell_x / 8 + (yy - cell_y / 8) * 32) as usize;
                    self.get_chunk(i, || intensity((i % 32) as i32, (i / 32) as i32))
                        .add_room(r);
                }
            }
        }
    }
}
