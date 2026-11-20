//! The square cache of `zombie.pathfind.PolygonalMap2`: `Square`, `Chunk`, `ChunkLevel`, `Cell` (the cells are a bounds
//! check over a chunk table), `ChunkUpdateTask`, `SquareUpdateTask` (`getBits`, `getCost`). Vehicles, vehicle clusters
//! and their visibility graphs are NOT ported: a map without vehicles has no graphs, and every branch of the search
//! that needs one is dead there. The player-built `IsoThumpable` and `IsoBarricade` do not exist in this port's
//! objects, so their bits stay clear.
use std::collections::HashMap;

use crate::zombie::core::math::pz_math;
use crate::zombie::core::properties::iso_property_type as pt;
use crate::zombie::core::tile_property_alias_map::TilePropertyAliasMap;
use crate::zombie::iso::iso_chunk::IsoChunk;
use crate::zombie::iso::iso_directions::IsoDirections;
use crate::zombie::iso::iso_grid_square::IsoGridSquare;
use crate::zombie::iso::iso_object::{IsoObject, ObjectKind};
use crate::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;
use crate::zombie::iso::sprite_details::iso_flag_type::IsoFlagType as F;
use crate::zombie::iso::sprite_details::iso_object_type::IsoObjectType as T;

pub const BIT_SOLID: i32 = 1;
pub const BIT_COLLIDE_W: i32 = 2;
pub const BIT_COLLIDE_N: i32 = 4;
pub const BIT_STAIR_TW: i32 = 8;
pub const BIT_STAIR_MW: i32 = 16;
pub const BIT_STAIR_BW: i32 = 32;
pub const BIT_STAIR_TN: i32 = 64;
pub const BIT_STAIR_MN: i32 = 128;
pub const BIT_STAIR_BN: i32 = 256;
pub const BIT_SOLID_FLOOR: i32 = 512;
pub const BIT_SOLID_TRANS: i32 = 1024;
pub const BIT_WINDOW_W: i32 = 2048;
pub const BIT_WINDOW_N: i32 = 4096;
pub const BIT_CAN_PATH_W: i32 = 8192;
pub const BIT_CAN_PATH_N: i32 = 16384;
pub const BIT_THUMP_W: i32 = 32768;
pub const BIT_THUMP_N: i32 = 65536;
pub const BIT_THUMPABLE: i32 = 131072;
pub const BIT_DOOR_E: i32 = 262144;
pub const BIT_DOOR_S: i32 = 524288;
pub const BIT_WINDOW_W_UNBLOCKED: i32 = 1048576;
pub const BIT_WINDOW_N_UNBLOCKED: i32 = 2097152;
pub const BIT_DOOR_W_UNBLOCKED: i32 = 4194304;
pub const BIT_DOOR_N_UNBLOCKED: i32 = 8388608;
/// `PolygonalMap2.ALL_STAIR_BITS`
pub const ALL_STAIR_BITS: i32 = 504;

/// zombie.pathfind.Square: `bits` as `SquareUpdateTask.getBits` made them, the cost, the sloped surface.
#[derive(Clone, Debug, PartialEq)]
pub struct Square {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub bits: i32,
    pub cost: i16,
    pub sloped_dir: Option<IsoDirections>,
    pub sloped_min: f32,
    pub sloped_max: f32,
}

impl Square {
    /// zombie.pathfind.Square.has
    pub fn has(&self, bit: i32) -> bool {
        self.bits & bit != 0
    }

    /// zombie.pathfind.Square.TreatAsSolidFloor
    pub fn treat_as_solid_floor(&self) -> bool {
        self.has(512) || self.has(504)
    }

    /// zombie.pathfind.Square.isReallySolid
    pub fn is_really_solid(&self, m: &PathMap) -> bool {
        self.has(1)
            || (self.has(1024)
                && !self.is_adjacent_to_window(m)
                && !self.is_adjacent_to_hoppable(m))
    }

    /// zombie.pathfind.Square.isNonThumpableSolid
    pub fn is_non_thumpable_solid(&self, m: &PathMap) -> bool {
        self.is_really_solid(m) && !self.has(131072)
    }

    /// zombie.pathfind.Square.isCanPathW
    pub fn is_can_path_w(&self, m: &PathMap) -> bool {
        if self.has(8192) {
            return true;
        }
        m.get_square(self.x - 1, self.y, self.z)
            .is_some_and(|w| w.has(131072) || w.has(262144))
    }

    /// zombie.pathfind.Square.isCanPathN
    pub fn is_can_path_n(&self, m: &PathMap) -> bool {
        if self.has(16384) {
            return true;
        }
        m.get_square(self.x, self.y - 1, self.z)
            .is_some_and(|n| n.has(131072) || n.has(524288))
    }

    /// zombie.pathfind.Square.isCollideW
    pub fn is_collide_w(&self, m: &PathMap) -> bool {
        if self.has(2) {
            return true;
        }
        m.get_square(self.x - 1, self.y, self.z)
            .is_some_and(|w| w.has(262144) || w.has(448) || w.is_really_solid(m))
    }

    /// zombie.pathfind.Square.isCollideN
    pub fn is_collide_n(&self, m: &PathMap) -> bool {
        if self.has(4) {
            return true;
        }
        m.get_square(self.x, self.y - 1, self.z)
            .is_some_and(|n| n.has(524288) || n.has(56) || n.is_really_solid(m))
    }

    /// zombie.pathfind.Square.isThumpW
    pub fn is_thump_w(&self, m: &PathMap) -> bool {
        if self.has(32768) {
            return true;
        }
        m.get_square(self.x - 1, self.y, self.z)
            .is_some_and(|w| w.has(131072))
    }

    /// zombie.pathfind.Square.isThumpN
    pub fn is_thump_n(&self, m: &PathMap) -> bool {
        if self.has(65536) {
            return true;
        }
        m.get_square(self.x, self.y - 1, self.z)
            .is_some_and(|n| n.has(131072))
    }

    /// zombie.pathfind.Square.isAdjacentToWindow
    pub fn is_adjacent_to_window(&self, m: &PathMap) -> bool {
        if self.has(2048) || self.has(4096) {
            return true;
        }
        if m.get_square(self.x, self.y + 1, self.z)
            .is_some_and(|s| s.has(4096))
        {
            return true;
        }
        m.get_square(self.x + 1, self.y, self.z)
            .is_some_and(|e| e.has(2048))
    }

    /// zombie.pathfind.Square.isAdjacentToHoppable: the hoppable bits (16777216, 33554432) are never set by
    /// `SquareUpdateTask.getBits` of this build, so the answer is false for every square the cache holds.
    pub fn is_adjacent_to_hoppable(&self, m: &PathMap) -> bool {
        if self.has(16777216) || self.has(33554432) {
            return true;
        }
        if m.get_square(self.x, self.y + 1, self.z)
            .is_some_and(|s| s.has(16777216))
        {
            return true;
        }
        m.get_square(self.x + 1, self.y, self.z)
            .is_some_and(|e| e.has(33554432))
    }

    /// zombie.pathfind.Square.isUnblockedWindowN
    pub fn is_unblocked_window_n(&self, m: &PathMap) -> bool {
        if !self.has(2097152) || self.is_really_solid(m) {
            return false;
        }
        m.get_square(self.x, self.y - 1, self.z)
            .is_some_and(|n| !n.is_really_solid(m))
    }

    /// zombie.pathfind.Square.isUnblockedWindowW
    pub fn is_unblocked_window_w(&self, m: &PathMap) -> bool {
        if !self.has(1048576) || self.is_really_solid(m) {
            return false;
        }
        m.get_square(self.x - 1, self.y, self.z)
            .is_some_and(|w| !w.is_really_solid(m))
    }

    /// zombie.pathfind.Square.isUnblockedDoorN
    pub fn is_unblocked_door_n(&self, m: &PathMap) -> bool {
        if !self.has(8388608) || self.has(1025) {
            return false;
        }
        m.get_square(self.x, self.y - 1, self.z)
            .is_some_and(|n| !n.has(1025))
    }

    /// zombie.pathfind.Square.isUnblockedDoorW
    pub fn is_unblocked_door_w(&self, m: &PathMap) -> bool {
        if !self.has(4194304) || self.has(1025) {
            return false;
        }
        m.get_square(self.x - 1, self.y, self.z)
            .is_some_and(|w| !w.has(1025))
    }

    /// zombie.pathfind.Square.getAdjacentSquare
    pub fn get_adjacent_square<'a>(
        &self,
        m: &'a PathMap,
        dir: IsoDirections,
    ) -> Option<&'a Square> {
        m.get_square(self.x + dir.dx(), self.y + dir.dy(), self.z)
    }

    /// zombie.pathfind.Square.isInside
    pub fn is_inside(&self, x1: i32, y1: i32, x2: i32, y2: i32) -> bool {
        self.x >= x1 && self.x < x2 && self.y >= y1 && self.y < y2
    }

    /// zombie.pathfind.Square.hasTransitionToLevelAbove
    pub fn has_transition_to_level_above(&self, edge: IsoDirections) -> bool {
        if edge == IsoDirections::N && self.has(64) {
            return true;
        }
        if edge == IsoDirections::W && self.has(8) {
            return true;
        }
        self.has_sloped_surface_to_level_above(edge)
    }

    /// zombie.pathfind.Square.hasSlopedSurface
    pub fn has_sloped_surface(&self) -> bool {
        self.sloped_dir.is_some()
    }

    /// zombie.pathfind.Square.hasIdenticalSlopedSurface
    pub fn has_identical_sloped_surface(&self, other: &Square) -> bool {
        self.sloped_dir == other.sloped_dir
            && self.sloped_min == other.sloped_min
            && self.sloped_max == other.sloped_max
    }

    /// zombie.pathfind.Square.getSlopedSurfaceHeight (dx, dy)
    pub fn get_sloped_surface_height_at(&self, dx: f32, dy: f32) -> f32 {
        let Some(dir) = self.sloped_dir else {
            return 0.0;
        };
        let dx = pz_math::clamp(dx, 0.0, 1.0);
        let dy = pz_math::clamp(dy, 0.0, 1.0);
        let (min, max) = (self.sloped_min, self.sloped_max);
        let z = match dir {
            IsoDirections::N => pz_math::lerp(min, max, 1.0 - dy),
            IsoDirections::S => pz_math::lerp(min, max, dy),
            IsoDirections::W => pz_math::lerp(min, max, 1.0 - dx),
            IsoDirections::E => pz_math::lerp(min, max, dx),
            _ => -1.0,
        };
        if z < 0.0 {
            0.0
        } else {
            z
        }
    }

    /// zombie.pathfind.Square.getSlopedSurfaceHeight (edge)
    pub fn get_sloped_surface_height(&self, edge: IsoDirections) -> f32 {
        let Some(dir) = self.sloped_dir else {
            return 0.0;
        };
        if dir == edge {
            self.sloped_max
        } else if dir.rot180() == edge {
            self.sloped_min
        } else {
            -1.0
        }
    }

    /// zombie.pathfind.Square.isSlopedSurfaceEdgeBlocked
    pub fn is_sloped_surface_edge_blocked(&self, m: &PathMap, edge: IsoDirections) -> bool {
        if self.sloped_dir.is_none() {
            return false;
        }
        match self.get_adjacent_square(m, edge) {
            None => true,
            Some(s2) => {
                self.get_sloped_surface_height(edge) != s2.get_sloped_surface_height(edge.rot180())
            }
        }
    }

    /// zombie.pathfind.Square.hasSlopedSurfaceToLevelAbove
    pub fn has_sloped_surface_to_level_above(&self, dir: IsoDirections) -> bool {
        self.sloped_dir.is_some() && self.get_sloped_surface_height(dir) == 1.0
    }
}

/// zombie.pathfind.ChunkLevel: the 8x8 squares of one level of a chunk (`squares[x][y]` as `x + y * 8`).
#[derive(Clone, Debug)]
pub struct ChunkLevel {
    pub z: i32,
    pub squares: Vec<Option<Square>>,
}

impl ChunkLevel {
    fn new(z: i32) -> Self {
        ChunkLevel {
            z,
            squares: vec![None; 64],
        }
    }

    /// zombie.pathfind.ChunkLevel.getSquare with chunk-local x, y (the original takes world coordinates and subtracts
    /// the chunk's minimum)
    pub fn get_local(&self, x: i32, y: i32) -> Option<&Square> {
        if (0..8).contains(&x) && (0..8).contains(&y) {
            self.squares[(x + y * 8) as usize].as_ref()
        } else {
            None
        }
    }
}

/// zombie.pathfind.Chunk
#[derive(Clone, Debug)]
pub struct Chunk {
    pub wx: i32,
    pub wy: i32,
    pub min_level: i32,
    pub max_level: i32,
    pub levels: Vec<ChunkLevel>,
}

impl Chunk {
    /// zombie.pathfind.Chunk.<init>: one level, 32.
    fn new(wx: i32, wy: i32) -> Self {
        Chunk {
            wx,
            wy,
            min_level: 32,
            max_level: 32,
            levels: vec![ChunkLevel::new(32)],
        }
    }

    pub fn get_min_x(&self) -> i32 {
        self.wx * 8
    }
    pub fn get_min_y(&self) -> i32 {
        self.wy * 8
    }
    pub fn get_max_x(&self) -> i32 {
        (self.wx + 1) * 8 - 1
    }
    pub fn get_max_y(&self) -> i32 {
        (self.wy + 1) * 8 - 1
    }

    /// zombie.pathfind.Chunk.isValidLevel
    pub fn is_valid_level(&self, level: i32) -> bool {
        level >= self.min_level && level <= self.max_level
    }

    /// zombie.pathfind.Chunk.contains
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.get_min_x()
            && y >= self.get_min_y()
            && x <= self.get_max_x()
            && y <= self.get_max_y()
    }

    /// zombie.pathfind.Chunk.getLevelData
    pub fn get_level_data(&self, level: i32) -> Option<&ChunkLevel> {
        if self.is_valid_level(level) {
            Some(&self.levels[(level - self.min_level) as usize])
        } else {
            None
        }
    }

    /// zombie.pathfind.Chunk.getSquare (world coordinates)
    pub fn get_square(&self, x: i32, y: i32, z: i32) -> Option<&Square> {
        self.get_level_data(z)?
            .get_local(x - self.get_min_x(), y - self.get_min_y())
    }

    /// zombie.pathfind.Chunk.setMinMaxLevel
    fn set_min_max_level(&mut self, min_level: i32, max_level: i32) {
        if min_level == self.min_level && max_level == self.max_level {
            return;
        }
        let mut new_levels = Vec::with_capacity((max_level - min_level + 1).max(0) as usize);
        for z in min_level..=max_level {
            if self.is_valid_level(z) {
                new_levels.push(self.levels[(z - self.min_level) as usize].clone());
            } else {
                new_levels.push(ChunkLevel::new(z));
            }
        }
        self.min_level = min_level;
        self.max_level = max_level;
        self.levels = new_levels;
    }
}

/// zombie.pathfind.ChunkUpdateTaskLevel
#[derive(Clone, Debug)]
pub struct UpdateLevel {
    pub data: Vec<i32>,
    pub cost: Vec<i16>,
    /// (x, y, direction, heightMin, heightMax)
    pub sloped: Vec<(i32, i32, IsoDirections, f32, f32)>,
}

/// zombie.pathfind.ChunkUpdateTask: the bits of every square of a chunk, taken on the main thread.
#[derive(Clone, Debug)]
pub struct ChunkUpdateTask {
    pub wx: i32,
    pub wy: i32,
    pub min_level: i32,
    pub max_level: i32,
    pub levels: Vec<UpdateLevel>,
}

/// zombie.pathfind.SquareUpdateTask: the bits of one square.
#[derive(Clone, Debug)]
pub struct SquareUpdateTask {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub bits: i32,
    pub cost: i16,
    pub sloped: Option<(IsoDirections, f32, f32)>,
}

/// What the game's sprite and tile property tables say, passed to `get_bits`.
pub struct BitsContext<'a> {
    pub sm: &'a IsoSpriteManager,
    pub alias: &'a TilePropertyAliasMap,
}

/// zombie.iso.objects.IsoDoor.getSpriteEdge
fn door_sprite_edge(
    cx: &BitsContext,
    obj: &IsoObject,
    open: bool,
    north: bool,
    ignore_open: bool,
) -> IsoDirections {
    if open && !ignore_open {
        if obj.has_property_str(cx.sm, cx.alias, pt::GARAGE_DOOR) {
            if north {
                IsoDirections::N
            } else {
                IsoDirections::W
            }
        } else if obj.has_property(cx.sm, F::attachedE) {
            IsoDirections::E
        } else if obj.has_property(cx.sm, F::attachedS) {
            IsoDirections::S
        } else if north {
            IsoDirections::W
        } else {
            IsoDirections::N
        }
    } else if north {
        IsoDirections::N
    } else {
        IsoDirections::W
    }
}

/// zombie.iso.objects.IsoWindow.isInvincible
fn window_is_invincible(cx: &BitsContext, sq: &IsoGridSquare, index: usize, north: bool) -> bool {
    if !sq.has(F::makeWindowInvincible) {
        return false;
    }
    for (i, obj) in sq.objects.iter().enumerate() {
        if i != index
            && obj.has_property(cx.sm, if north { F::cutN } else { F::cutW })
            && obj.has_property(cx.sm, F::makeWindowInvincible)
        {
            return true;
        }
    }
    sq.objects[index].has_property(cx.sm, F::makeWindowInvincible)
}

/// zombie.pathfind.SquareUpdateTask.isWindowUnblocked. Barricades are not modelled (a loaded map has none).
fn is_window_unblocked(cx: &BitsContext, sq: &IsoGridSquare, north: bool) -> bool {
    for (i, obj) in sq.objects.iter().enumerate() {
        if !obj.special {
            continue;
        }
        if let ObjectKind::IsoWindow(w) = &obj.kind {
            if north == w.north {
                if window_is_invincible(cx, sq, i, north) {
                    return false;
                }
                if w.open {
                    return true;
                }
                return w.smashed && w.glass_removed;
            }
        }
    }
    // getWindowFrame(...) != null && frame.canClimbThrough(null)
    sq.objects
        .iter()
        .any(|o| matches!(&o.kind, ObjectKind::IsoWindowFrame { north: n } if *n == north))
}

/// zombie.pathfind.SquareUpdateTask.hasSquareThumpable: `IsoObject.isMovedThumpable` (an `IsoThumpable` is not modelled).
fn has_square_thumpable(sq: &IsoGridSquare) -> bool {
    if sq.has_stairs() {
        return false;
    }
    sq.objects
        .iter()
        .any(|o| o.save.as_ref().is_some_and(|s| s.moved_thumpable))
}

/// zombie.pathfind.SquareUpdateTask.getBits
pub fn get_bits(cx: &BitsContext, sq: &IsoGridSquare) -> i32 {
    let mut bits = 0;
    if sq.has(F::solidfloor) {
        bits |= 512;
    }
    if sq.has(F::solid) {
        bits |= 1;
    }
    if sq.has(F::solidtrans) {
        bits |= 1024;
    }
    if sq.has(F::collideW) {
        bits |= 2;
    }
    if sq.has(F::collideN) {
        bits |= 4;
    }
    if sq.has_type(T::stairsTW) {
        bits |= 8;
    }
    if sq.has_type(T::stairsMW) {
        bits |= 16;
    }
    if sq.has_type(T::stairsBW) {
        bits |= 32;
    }
    if sq.has_type(T::stairsTN) {
        bits |= 64;
    }
    if sq.has_type(T::stairsMN) {
        bits |= 128;
    }
    if sq.has_type(T::stairsBN) {
        bits |= 256;
    }
    if sq.has(F::windowW) || sq.has(F::WindowW) {
        bits |= 2050;
        if is_window_unblocked(cx, sq, false) {
            bits |= 1048576;
        }
    }
    if sq.has(F::windowN) || sq.has(F::WindowN) {
        bits |= 4100;
        if is_window_unblocked(cx, sq, true) {
            bits |= 2097152;
        }
    }
    if sq.has(F::canPathW) {
        bits |= 8192;
    }
    if sq.has(F::canPathN) {
        bits |= 16384;
    }
    let (mut door_w, mut door_n) = (false, false);
    for obj in sq.objects.iter().filter(|o| o.special) {
        let mut dir: Option<IsoDirections> = None;
        if let ObjectKind::IsoDoor(d) = &obj.kind {
            dir = Some(door_sprite_edge(cx, obj, d.open, d.north, false));
            if d.open {
                let e = door_sprite_edge(cx, obj, d.open, d.north, true);
                if e == IsoDirections::N {
                    bits |= 8388608;
                } else if e == IsoDirections::W {
                    bits |= 4194304;
                }
                dir = None;
            }
        }
        match dir {
            Some(IsoDirections::W) => {
                bits |= 8192;
                bits |= 2;
                door_w = true;
            }
            Some(IsoDirections::N) => {
                bits |= 16384;
                bits |= 4;
                door_n = true;
            }
            Some(IsoDirections::S) => bits |= 524288,
            Some(IsoDirections::E) => bits |= 262144,
            _ => {}
        }
    }
    if sq.has(F::DoorWallW) {
        bits |= 8192;
        bits |= 2;
        if !door_w {
            bits |= 4194304;
        }
    }
    if sq.has(F::DoorWallN) {
        bits |= 16384;
        bits |= 4;
        if !door_n {
            bits |= 8388608;
        }
    }
    if has_square_thumpable(sq) {
        bits |= 8192;
        bits |= 16384;
        bits |= 131072;
    }
    bits
}

/// zombie.pathfind.SquareUpdateTask.getCost: 5 for a tree or a bush.
pub fn get_cost(cx: &BitsContext, sq: &IsoGridSquare) -> i16 {
    let bush = sq.objects.iter().any(|o| {
        o.sprite.is_some_and(|s| {
            let sp = cx.sm.sprite(s);
            sp.tileset_name.as_deref() == Some("f_bushes_1")
                || sp.properties.has_str(cx.alias, "Bush")
        })
    });
    if sq.has_tree || bush {
        5
    } else {
        0
    }
}

impl ChunkUpdateTask {
    /// zombie.pathfind.ChunkUpdateTask.init: the bits of every square of `chunk` (levels `minLevel + 32` ..
    /// `maxLevel + 32`).
    pub fn init(cx: &BitsContext, chunk: &IsoChunk) -> Self {
        let min_level = chunk.get_min_level() + 32;
        let max_level = chunk.get_max_level() + 32;
        let mut levels = Vec::new();
        for z in min_level..=max_level {
            let mut lv = UpdateLevel {
                data: vec![0; 64],
                cost: vec![0; 64],
                sloped: Vec::new(),
            };
            for y in 0..8 {
                for x in 0..8 {
                    if let Some(sq) = chunk.get_grid_square(x, y, z - 32) {
                        lv.data[(x + y * 8) as usize] = get_bits(cx, sq);
                        lv.cost[(x + y * 8) as usize] = get_cost(cx, sq);
                        if let Some(dir) = sq.get_sloped_surface_direction(cx.alias) {
                            lv.sloped.push((
                                x,
                                y,
                                dir,
                                sq.get_sloped_surface_height_min(cx.alias),
                                sq.get_sloped_surface_height_max(cx.alias),
                            ));
                        }
                    }
                }
            }
            levels.push(lv);
        }
        ChunkUpdateTask {
            wx: chunk.wx,
            wy: chunk.wy,
            min_level,
            max_level,
            levels,
        }
    }
}

impl SquareUpdateTask {
    /// zombie.pathfind.SquareUpdateTask.init
    pub fn init(cx: &BitsContext, sq: &IsoGridSquare) -> Self {
        SquareUpdateTask {
            x: sq.x,
            y: sq.y,
            z: sq.z + 32,
            bits: get_bits(cx, sq),
            cost: get_cost(cx, sq),
            sloped: sq.get_sloped_surface_direction(cx.alias).map(|d| {
                (
                    d,
                    sq.get_sloped_surface_height_min(cx.alias),
                    sq.get_sloped_surface_height_max(cx.alias),
                )
            }),
        }
    }
}

/// The cells and chunks of `PolygonalMap2` (`Cell.chunks`), looked up the way `getCellFromSquarePos` and
/// `Cell.getChunkFromChunkPos` do.
#[derive(Debug, Default)]
pub struct PathMap {
    /// `metaGrid.getMinX()` / `getMinY()`, in cells.
    pub min_x: i32,
    pub min_y: i32,
    pub width: i32,
    pub height: i32,
    pub chunks: HashMap<(i32, i32), Chunk>,
}

impl PathMap {
    pub fn new(min_x: i32, min_y: i32, width: i32, height: i32) -> Self {
        PathMap {
            min_x,
            min_y,
            width,
            height,
            chunks: HashMap::new(),
        }
    }

    /// zombie.pathfind.PolygonalMap2.getCellFromSquarePos: whether the square lies in a cell of the map.
    fn in_cells(&self, x: i32, y: i32) -> bool {
        let x = x - self.min_x * 256;
        let y = y - self.min_y * 256;
        x >= 0 && y >= 0 && x / 256 < self.width && y / 256 < self.height
    }

    /// zombie.pathfind.PolygonalMap2.getChunkFromChunkPos
    pub fn get_chunk_from_chunk_pos(&self, wx: i32, wy: i32) -> Option<&Chunk> {
        if !self.in_cells(wx * 8, wy * 8) {
            return None;
        }
        self.chunks.get(&(wx, wy))
    }

    /// zombie.pathfind.PolygonalMap2.getChunkFromSquarePos
    pub fn get_chunk_from_square_pos(&self, x: i32, y: i32) -> Option<&Chunk> {
        if !self.in_cells(x, y) {
            return None;
        }
        self.chunks
            .get(&(pz_math::coorddivision(x, 8), pz_math::coorddivision(y, 8)))
    }

    /// zombie.pathfind.PolygonalMap2.getSquare (also `getSquareRawZ`)
    pub fn get_square(&self, x: i32, y: i32, z: i32) -> Option<&Square> {
        self.get_chunk_from_square_pos(x, y)?.get_square(x, y, z)
    }

    /// zombie.pathfind.PolygonalMap2.allocChunkIfNeeded: `None` outside the cells.
    fn alloc_chunk_if_needed(&mut self, wx: i32, wy: i32) -> Option<&mut Chunk> {
        if !self.in_cells(wx * 8, wy * 8) {
            return None;
        }
        let ch = self
            .chunks
            .entry((wx, wy))
            .or_insert_with(|| Chunk::new(wx, wy));
        ch.wx = wx;
        ch.wy = wy;
        Some(ch)
    }

    /// zombie.pathfind.ChunkUpdateTask.execute with zombie.pathfind.Chunk.setData(ChunkUpdateTask)
    pub fn execute_chunk_update(&mut self, task: &ChunkUpdateTask) {
        let Some(chunk) = self.alloc_chunk_if_needed(task.wx, task.wy) else {
            return;
        };
        chunk.set_min_max_level(task.min_level, task.max_level);
        for z in chunk.min_level..=chunk.max_level {
            let tl = &task.levels[(z - chunk.min_level) as usize];
            let level = &mut chunk.levels[(z - chunk.min_level) as usize];
            let (cx, cy) = (task.wx * 8, task.wy * 8);
            for y in 0..8 {
                for x in 0..8 {
                    let i = (x + y * 8) as usize;
                    let bits = tl.data[i];
                    if bits == 0 {
                        level.squares[i] = None;
                    } else {
                        let sq = level.squares[i].get_or_insert(Square {
                            x: 0,
                            y: 0,
                            z: 0,
                            bits: 0,
                            cost: 0,
                            sloped_dir: None,
                            sloped_min: 0.0,
                            sloped_max: 0.0,
                        });
                        sq.x = cx + x;
                        sq.y = cy + y;
                        sq.z = z;
                        sq.bits = bits;
                        sq.cost = tl.cost[i];
                    }
                    if let Some(sq) = level.squares[i].as_mut() {
                        sq.sloped_dir = None;
                        sq.sloped_min = 0.0;
                        sq.sloped_max = 0.0;
                    }
                }
            }
            for &(x, y, dir, min, max) in &tl.sloped {
                if let Some(sq) = level.squares[(x + y * 8) as usize].as_mut() {
                    sq.sloped_dir = Some(dir);
                    sq.sloped_min = min;
                    sq.sloped_max = max;
                }
            }
        }
    }

    /// zombie.pathfind.SquareUpdateTask.execute with zombie.pathfind.Chunk.setData(SquareUpdateTask): whether the
    /// cache changed (the original then sets `rebuild`).
    pub fn execute_square_update(&mut self, task: &SquareUpdateTask) -> Result<bool, String> {
        let (wx, wy) = (
            pz_math::coorddivision(task.x, 8),
            pz_math::coorddivision(task.y, 8),
        );
        if !self.in_cells(task.x, task.y) {
            return Ok(false);
        }
        let Some(chunk) = self.chunks.get_mut(&(wx, wy)) else {
            return Ok(false);
        };
        let x = task.x - chunk.wx * 8;
        let y = task.y - chunk.wy * 8;
        if !(0..8).contains(&x) || !(0..8).contains(&y) {
            return Ok(false);
        }
        if !chunk.is_valid_level(task.z) {
            return Err(format!(
                "Chunk.setData: level {} outside the chunk's {}..{} (a NullPointerException in the original)",
                task.z, chunk.min_level, chunk.max_level
            ));
        }
        let min = chunk.min_level;
        let slot = &mut chunk.levels[(task.z - min) as usize].squares[(x + y * 8) as usize];
        if task.bits == 0 {
            return Ok(slot.take().is_some());
        }
        let sq = slot.get_or_insert(Square {
            x: task.x,
            y: task.y,
            z: task.z,
            bits: 0,
            cost: 0,
            sloped_dir: None,
            sloped_min: 0.0,
            sloped_max: 0.0,
        });
        let mut changed = sq.bits != task.bits || sq.cost != task.cost;
        match task.sloped {
            None => {
                changed |= sq.sloped_dir.is_some() || sq.sloped_min != 0.0 || sq.sloped_max != 0.0;
                sq.sloped_dir = None;
                sq.sloped_min = 0.0;
                sq.sloped_max = 0.0;
            }
            Some((d, min_h, max_h)) => {
                changed |=
                    sq.sloped_dir != Some(d) || sq.sloped_min != min_h || sq.sloped_max != max_h;
                sq.sloped_dir = Some(d);
                sq.sloped_min = min_h;
                sq.sloped_max = max_h;
            }
        }
        if changed {
            sq.bits = task.bits;
            sq.cost = task.cost;
        }
        Ok(changed)
    }
}
