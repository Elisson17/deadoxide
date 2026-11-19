use std::collections::BTreeMap;

use super::chunk_save::ChunkSave;
use super::iso_grid_square::{GetSquare, IsoGridSquare};
use super::iso_meta_grid::IsoMetaGrid;
use super::sprite::iso_sprite_manager::IsoSpriteManager;
use super::sprite_details::iso_flag_type::IsoFlagType;
use crate::zombie::core::tile_property_alias_map::TilePropertyAliasMap;
use crate::zombie::erosion::erosion_data;

/// zombie.iso.IsoChunk: an 8x8 column of squares over all levels. So far the square storage the map loader fills.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct IsoChunk {
    pub wx: i32,
    pub wy: i32,
    min_level: i32,
    max_level: i32,
    /// level -> 64 squares (`x + y * 8`)
    squares: BTreeMap<i32, Vec<Option<Box<IsoGridSquare>>>>,
    /// What a chunk save carried beyond the squares (`None` for chunks made from the map).
    pub save: Option<Box<ChunkSave>>,
    /// zombie.iso.IsoChunk.erosion (`getErosionData`): the erosion state of the chunk.
    pub erosion: erosion_data::Chunk,
    /// zombie.iso.IsoChunk.loaded: set by `ServerMap.ServerCell.RecalcAll2` once the cell's squares are in place.
    pub loaded: bool,
}

impl IsoChunk {
    pub fn new(wx: i32, wy: i32) -> Self {
        IsoChunk {
            wx,
            wy,
            erosion: erosion_data::Chunk::new(),
            ..Default::default()
        }
    }
    /// zombie.iso.IsoChunk.getMinLevel
    pub fn get_min_level(&self) -> i32 {
        self.min_level
    }
    /// zombie.iso.IsoChunk.getMaxLevel
    pub fn get_max_level(&self) -> i32 {
        self.max_level
    }
    /// zombie.iso.IsoChunk.getGridSquare: chunk-local x and y.
    pub fn get_grid_square(&self, x: i32, y: i32, z: i32) -> Option<&IsoGridSquare> {
        if !(0..8).contains(&x) || !(0..8).contains(&y) {
            return None;
        }
        self.squares.get(&z)?[(x + y * 8) as usize].as_deref()
    }
    pub fn get_grid_square_mut(&mut self, x: i32, y: i32, z: i32) -> Option<&mut IsoGridSquare> {
        if !(0..8).contains(&x) || !(0..8).contains(&y) {
            return None;
        }
        self.squares.get_mut(&z)?[(x + y * 8) as usize].as_deref_mut()
    }
    /// zombie.iso.IsoChunk.setMinMaxLevel: levels outside the new range are dropped, new levels start empty.
    pub fn set_min_max_level(&mut self, min_level: i32, max_level: i32) {
        if min_level == self.min_level && max_level == self.max_level {
            return;
        }
        self.squares
            .retain(|z, _| (min_level..=max_level).contains(z));
        for z in min_level..=max_level {
            self.squares.entry(z).or_insert_with(|| vec![None; 64]);
        }
        self.min_level = min_level;
        self.max_level = max_level;
    }
    /// zombie.iso.IsoChunk.setSquare: also widens the chunk's level range to include `z`.
    pub fn set_square(&mut self, x: i32, y: i32, z: i32, square: Option<IsoGridSquare>) {
        self.min_level = self.min_level.min(z);
        self.max_level = self.max_level.max(z);
        for level in self.min_level..=self.max_level {
            self.squares.entry(level).or_insert_with(|| vec![None; 64]);
        }
        let loaded = self.loaded;
        if let Some(l) = self.squares.get_mut(&z) {
            l[(x + y * 8) as usize] = square.map(|mut sq| {
                sq.chunk_loaded = loaded;
                Box::new(sq)
            });
        }
    }

    /// The part of zombie.network.ServerMap.ServerCell.RecalcAll2 that marks a chunk loaded: `chunk.loaded = true`
    /// and `propertiesDirty = true` on every square.
    pub fn set_loaded(&mut self) {
        self.loaded = true;
        for level in self.squares.values_mut() {
            for sq in level.iter_mut().flatten() {
                sq.chunk_loaded = true;
                sq.properties_dirty = true;
            }
        }
    }
}

impl IsoChunk {
    /// zombie.iso.IsoChunk.assignRoom: a square without a room takes the id of the meta grid's room at its position.
    fn assign_room(sq: &mut IsoGridSquare, meta_grid: &IsoMetaGrid) {
        if sq.room.is_none() {
            sq.room_id = meta_grid
                .get_room_at(sq.x, sq.y, sq.z)
                .map_or(-1, |r| meta_grid.room(r).id);
        }
    }

    /// zombie.iso.IsoChunk.ensureNotNull
    fn ensure_not_null(
        &mut self,
        lx: i32,
        ly: i32,
        z: i32,
        dx: i32,
        dy: i32,
        meta_grid: &IsoMetaGrid,
    ) {
        if lx + dx >= 0 && lx + dx < 8 && ly + dy >= 0 && ly + dy < 8 {
            let (x, y) = (lx + dx, ly + dy);
            if self.get_grid_square(x, y, z).is_none() {
                let mut sq = IsoGridSquare::new(self.wx * 8 + x, self.wy * 8 + y, z);
                Self::assign_room(&mut sq, meta_grid);
                self.set_square(x, y, z, Some(sq));
            }
        }
    }

    /// zombie.iso.IsoChunk.ensureNotNull3x3: the square and its eight neighbours inside the chunk exist (DIRECTIONS
    /// is the eight directions in `IsoDirections` order).
    fn ensure_not_null_3x3(&mut self, lx: i32, ly: i32, z: i32, meta_grid: &IsoMetaGrid) {
        self.ensure_not_null(lx, ly, z, 0, 0, meta_grid);
        for d in super::iso_directions::VALUES {
            self.ensure_not_null(lx, ly, z, d.dx(), d.dy(), meta_grid);
        }
    }

    /// zombie.iso.IsoChunk.loadInWorldStreamerThread: the squares around objects exist, every square's properties
    /// are built (with the chunk not loaded, so no square is left dirty by them), the squares under a rain blocking
    /// tile get `haveRoof` (and lose `exterior`), `RecalcAllWithNeighbours(true, chunkGetter)` builds the properties
    /// of the squares that are still dirty (those `ensureNotNull` made after their level had been passed) and every
    /// square is marked dirty. The same pass leaves each square's collide, path and vision matrices (the chunk getter
    /// sees only this chunk's squares, so the bits towards a neighbouring chunk stay as `getNew` made them; the
    /// border recalculation of `IsoChunk`'s main-thread pass is not part of this method).
    pub fn load_in_world_streamer_thread(
        &mut self,
        sm: &IsoSpriteManager,
        meta_grid: &IsoMetaGrid,
        alias: &TilePropertyAliasMap,
    ) {
        for z in self.min_level..=self.max_level {
            for y in 0..8 {
                for x in 0..8 {
                    if self.get_grid_square(x, y, z).is_none() && z == 0 {
                        let sq = IsoGridSquare::new(self.wx * 8 + x, self.wy * 8 + y, z);
                        self.set_square(x, y, z, Some(sq));
                    }
                    let Some(sq) = self.get_grid_square(x, y, z) else {
                        continue;
                    };
                    if !sq.objects.is_empty() {
                        self.ensure_not_null_3x3(x, y, z, meta_grid);
                        let mut zz = z - 1;
                        while zz > self.min_level {
                            self.ensure_not_null_3x3(x, y, zz, meta_grid);
                            zz -= 1;
                        }
                    }
                    if let Some(sq) = self.get_grid_square_mut(x, y, z) {
                        sq.recalc_properties(sm);
                    }
                }
            }
        }
        for y in 0..8 {
            for x in 0..8 {
                let mut z = self.max_level;
                while z > 0 {
                    if self
                        .get_grid_square(x, y, z)
                        .is_some_and(|s| s.has_rain_blocking_tile())
                    {
                        z -= 1;
                        while z >= self.min_level {
                            if let Some(sq) = self.get_grid_square_mut(x, y, z) {
                                if !sq.have_roof {
                                    sq.have_roof = true;
                                    sq.properties.unset(IsoFlagType::exterior);
                                }
                            }
                            z -= 1;
                        }
                        break;
                    }
                    z -= 1;
                }
            }
        }
        for z in self.min_level..=self.max_level {
            for y in 0..8 {
                for x in 0..8 {
                    if let Some(sq) = self.get_grid_square_mut(x, y, z) {
                        sq.recalc_properties_if_needed(sm);
                    }
                }
            }
        }
        // RecalcAllWithNeighbours(true, chunkGetter): every square's matrices from the squares of this chunk
        let mut matrices = Vec::new();
        {
            let getter = ChunkGetter(self);
            for z in self.min_level..=self.max_level {
                for y in 0..8 {
                    for x in 0..8 {
                        if let Some(sq) = self.get_grid_square(x, y, z) {
                            matrices.push((x, y, z, sq.calculate_matrices(alias, &getter)));
                        }
                    }
                }
            }
        }
        for (x, y, z, (c, p, v)) in matrices {
            if let Some(sq) = self.get_grid_square_mut(x, y, z) {
                sq.collide_matrix = c;
                sq.path_matrix = p;
                sq.vision_matrix = v;
            }
        }
        for level in self.squares.values_mut() {
            for sq in level.iter_mut().flatten() {
                sq.properties_dirty = true;
            }
        }
    }
}

/// zombie.iso.IsoChunk.ChunkGetter: the squares of one chunk by world coordinates, nothing outside it.
struct ChunkGetter<'a>(&'a IsoChunk);

impl GetSquare for ChunkGetter<'_> {
    fn get_grid_square(&self, x: i32, y: i32, z: i32) -> Option<&IsoGridSquare> {
        let (lx, ly) = (x - self.0.wx * 8, y - self.0.wy * 8);
        if (0..8).contains(&lx) && (0..8).contains(&ly) && (-32..=31).contains(&z) {
            self.0.get_grid_square(lx, ly, z)
        } else {
            None
        }
    }
}
