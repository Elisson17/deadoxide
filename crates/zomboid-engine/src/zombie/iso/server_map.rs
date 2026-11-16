//! zombie.network.ServerMap: so far the chunks a server has loaded and the square lookup `IsoCell.getGridSquare` does
//! through them, with the square edits (`IsoGridSquare.AddTileObject`, `RemoveTileObject`) and `IsoCell.checkHaveRoof`
//! that go through that lookup. The server's cells of 64 by 64 squares are flattened into one chunk table: a lookup
//! only ever needs the chunk, and every cell the original would load is a set of chunks.
use std::collections::BTreeMap;

use super::iso_chunk::IsoChunk;
use super::iso_grid_square::{GetSquare, IsoGridSquare};
use super::iso_object::IsoObject;
use super::sprite::iso_sprite_manager::IsoSpriteManager;
use crate::zombie::core::properties::iso_property_type as pt;
use crate::zombie::core::tile_property_alias_map::TilePropertyAliasMap;

/// zombie.network.ServerMap.ServerCell: the cell of 8 by 8 chunks, as far as the lookups need it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerCell {
    pub wx: i32,
    pub wy: i32,
    /// zombie.network.ServerMap.ServerCell.isLoaded: the lookups only see the chunks of a loaded cell.
    pub is_loaded: bool,
}

/// The chunks of a server map, by chunk coordinates, and the cells they belong to in the order they were loaded
/// (`ServerMap.loadedCells`).
#[derive(Clone, Debug, Default)]
pub struct ServerMap {
    pub chunks: BTreeMap<(i32, i32), IsoChunk>,
    pub loaded_cells: Vec<ServerCell>,
}

impl ServerMap {
    pub fn new() -> Self {
        ServerMap::default()
    }

    /// Puts a chunk into its cell (`cell.chunks[x][y] = chunk`), making the cell (not yet `isLoaded`) when it is the
    /// first of its cell.
    pub fn add_chunk(&mut self, chunk: IsoChunk) {
        let (cx, cy) = (chunk.wx.div_euclid(8), chunk.wy.div_euclid(8));
        if !self.loaded_cells.iter().any(|c| (c.wx, c.wy) == (cx, cy)) {
            self.loaded_cells.push(ServerCell {
                wx: cx,
                wy: cy,
                is_loaded: false,
            });
        }
        self.chunks.insert((chunk.wx, chunk.wy), chunk);
    }

    /// `cell.isLoaded = true` on every cell.
    pub fn set_all_cells_loaded(&mut self) {
        for c in &mut self.loaded_cells {
            c.is_loaded = true;
        }
    }

    fn chunk_visible(&self, wx: i32, wy: i32) -> bool {
        let (cx, cy) = (wx.div_euclid(8), wy.div_euclid(8));
        self.loaded_cells
            .iter()
            .any(|c| (c.wx, c.wy) == (cx, cy) && c.is_loaded)
    }

    /// zombie.network.ServerMap.getChunk
    pub fn get_chunk(&self, wx: i32, wy: i32) -> Option<&IsoChunk> {
        self.chunks.get(&(wx, wy))
    }

    /// zombie.network.ServerMap.getGridSquare, without the meta grid's validity test (only squares of loaded chunks
    /// exist here): only the chunks of a loaded cell are seen.
    pub fn get_grid_square(&self, x: i32, y: i32, z: i32) -> Option<&IsoGridSquare> {
        let key = (x.div_euclid(8), y.div_euclid(8));
        if !self.chunk_visible(key.0, key.1) {
            return None;
        }
        let c = self.chunks.get(&key)?;
        c.get_grid_square(x.rem_euclid(8), y.rem_euclid(8), z)
    }

    pub fn get_grid_square_mut(&mut self, x: i32, y: i32, z: i32) -> Option<&mut IsoGridSquare> {
        let key = (x.div_euclid(8), y.div_euclid(8));
        if !self.chunk_visible(key.0, key.1) {
            return None;
        }
        let c = self.chunks.get_mut(&key)?;
        c.get_grid_square_mut(x.rem_euclid(8), y.rem_euclid(8), z)
    }

    /// zombie.iso.IsoGridSquare.AddTileObject (IsoObject): inserts the object where `placeWallAndDoorCheck` says and
    /// recalculates the square's properties (`RecalcAllWithNeighbours`; the neighbours' collide, path and vision
    /// matrices are not modelled). Returns the index the object went to.
    pub fn add_tile_object(
        &mut self,
        pos: (i32, i32, i32),
        obj: IsoObject,
        sm: &IsoSpriteManager,
    ) -> Option<usize> {
        let sq = self.get_grid_square_mut(pos.0, pos.1, pos.2)?;
        let index = sq.place_wall_and_door_check(sm, &obj, -1);
        let at = if index >= 0 && index as usize <= sq.objects.len() {
            index as usize
        } else {
            sq.objects.len()
        };
        sq.objects.insert(at, obj);
        self.recalc_all_with_neighbours(pos, sm);
        Some(at)
    }

    /// zombie.iso.IsoGridSquare.RemoveTileObject (IsoObject, boolean) with `safelyRemove` false: takes the object
    /// out, recalculates the square and, through `IsoCell.checkHaveRoof`, the roof state of the column. The Lua event
    /// OnObjectAboutToBeRemoved is not triggered; no handler is registered on a dedicated server (measured, see
    /// COMPATIBILITY.md).
    pub fn remove_tile_object(
        &mut self,
        pos: (i32, i32, i32),
        index: usize,
        sm: &IsoSpriteManager,
        alias: &TilePropertyAliasMap,
    ) {
        let Some(sq) = self.get_grid_square_mut(pos.0, pos.1, pos.2) else {
            return;
        };
        if index >= sq.objects.len() {
            return;
        }
        if sq.objects[index].is_table_surface(sm, alias) {
            let offset = sq.objects[index].get_surface_offset(sm, alias);
            for i in index + 1..sq.objects.len() {
                let o = &mut sq.objects[i];
                let table_top = o
                    .sprite
                    .is_some_and(|s| sm.sprite(s).properties.is_table_top(alias));
                if table_top
                    || o.is_table_surface(sm, alias)
                        && !o.has_property_str(sm, alias, pt::COUNTERTOP)
                {
                    o.render_y_offset -= offset;
                }
            }
        }
        sq.objects.remove(index);
        self.recalc_all_with_neighbours(pos, sm);
        self.check_have_roof(pos.0, pos.1, sm);
    }

    /// zombie.iso.IsoGridSquare.RemoveTileObjectErosionNoRecalc: takes the object out and builds the square's
    /// properties again (the square is dirty while its chunk is loaded).
    pub fn remove_tile_object_erosion_no_recalc(
        &mut self,
        pos: (i32, i32, i32),
        index: usize,
        sm: &IsoSpriteManager,
    ) {
        if let Some(sq) = self.get_grid_square_mut(pos.0, pos.1, pos.2) {
            if index < sq.objects.len() {
                sq.objects.remove(index);
            }
            sq.recalc_properties_if_needed(sm);
        }
    }

    /// zombie.iso.IsoGridSquare.RecalcAllWithNeighbours (true): the square's own properties if dirty, then each
    /// existing square of the 3x3x3 block around it (`RecalcPropertiesIfNeeded` of both through `ReCalculateAll`).
    /// The collide, path and vision matrices and the grid navigation it also rebuilds are not modelled.
    pub fn recalc_all_with_neighbours(&mut self, pos: (i32, i32, i32), sm: &IsoSpriteManager) {
        if let Some(sq) = self.get_grid_square_mut(pos.0, pos.1, pos.2) {
            sq.recalc_properties_if_needed(sm);
        }
        for x in pos.0 - 1..=pos.0 + 1 {
            for y in pos.1 - 1..=pos.1 + 1 {
                for z in pos.2 - 1..=pos.2 + 1 {
                    if (x, y, z) == pos || !(-32..=31).contains(&z) {
                        continue;
                    }
                    if self.get_grid_square(x, y, z).is_some() {
                        if let Some(sq) = self.get_grid_square_mut(pos.0, pos.1, pos.2) {
                            sq.recalc_properties_if_needed(sm);
                        }
                        if let Some(sq) = self.get_grid_square_mut(x, y, z) {
                            sq.recalc_properties_if_needed(sm);
                        }
                    }
                }
            }
        }
    }

    /// zombie.iso.IsoCell.checkHaveRoof: walking down the column, a square gets `haveRoof` when a rain blocking tile
    /// is above it.
    pub fn check_have_roof(&mut self, x: i32, y: i32, sm: &IsoSpriteManager) {
        let mut have_roof = false;
        for z in (0..=31).rev() {
            let Some(sq) = self.get_grid_square_mut(x, y, z) else {
                continue;
            };
            if have_roof != sq.have_roof {
                sq.have_roof = have_roof;
                self.recalc_all_with_neighbours((x, y, z), sm);
            }
            if self
                .get_grid_square(x, y, z)
                .is_some_and(|s| s.has_rain_blocking_tile())
            {
                have_roof = true;
            }
        }
    }
}

impl GetSquare for ServerMap {
    fn get_grid_square(&self, x: i32, y: i32, z: i32) -> Option<&IsoGridSquare> {
        ServerMap::get_grid_square(self, x, y, z)
    }
}
