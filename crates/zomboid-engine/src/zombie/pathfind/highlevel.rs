//! `zombie.pathfind.highLevel`: `HLChunkLevel` (regions of walkable squares per chunk level, staircases and sloped
//! surfaces), `HLChunkRegion`, `FloodFill`, `HLStaircase`, `HLSlopedSurface`, `HLSearchNode` and `HLAStar`, the search
//! over those regions that decides which chunk levels and which staircases the low-level search may use. The parts
//! that need a visibility graph (vehicles) are not ported: a map without them has none.
use std::collections::HashMap;

use super::a_star::{self, AStarNodes};
use super::path::{distance_to, distance_to_xyz, ChunkLevelKey, PmMover, DIRECTIONS};
use super::path_map::{Chunk, PathMap, Square};
use super::vga_star::can_move_between;
use crate::zombie::core::math::pz_math;
use crate::zombie::iso::iso_directions::IsoDirections;

/// zombie.pathfind.highLevel.HLStaircase / HLSlopedSurface (an `HLLevelTransition`): the bottom floor square and the
/// direction up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition {
    pub is_stair: bool,
    pub dir: IsoDirections,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Transition {
    pub fn bottom_floor_x(&self) -> i32 {
        self.x
    }
    pub fn bottom_floor_y(&self) -> i32 {
        self.y
    }
    pub fn bottom_floor_z(&self) -> i32 {
        self.z
    }
    pub fn top_floor_x(&self) -> i32 {
        self.x + self.dir.dx()
    }
    pub fn top_floor_y(&self) -> i32 {
        self.y + self.dir.dy()
    }
    pub fn top_floor_z(&self) -> i32 {
        self.z + 1
    }
    /// HLLevelTransition.getSearchNodeX
    pub fn search_node_x(&self, bottom: bool) -> f32 {
        (if bottom { self.x } else { self.top_floor_x() }) as f32 + 0.5
    }
    pub fn search_node_y(&self, bottom: bool) -> f32 {
        (if bottom { self.y } else { self.top_floor_y() }) as f32 + 0.5
    }
    fn is_bottom_floor_at(&self, x: i32, y: i32) -> bool {
        x == self.x && y == self.y
    }
    /// HLLevelTransition.getBottomFloorSquare
    pub fn bottom_floor_square<'a>(&self, m: &'a PathMap) -> Option<&'a Square> {
        m.get_square(
            self.bottom_floor_x(),
            self.bottom_floor_y(),
            self.bottom_floor_z(),
        )
    }
    /// HLLevelTransition.getTopFloorSquare
    pub fn top_floor_square<'a>(&self, m: &'a PathMap) -> Option<&'a Square> {
        m.get_square(self.top_floor_x(), self.top_floor_y(), self.top_floor_z())
    }
}

/// zombie.pathfind.highLevel.HLChunkRegion
#[derive(Clone, Debug)]
pub struct Region {
    pub level: ChunkLevelKey,
    pub mask: [bool; 64],
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
    pub edge_n: [i32; 8],
    pub edge_s: [i32; 8],
    pub edge_w: [i32; 8],
    pub edge_e: [i32; 8],
}

impl Region {
    /// zombie.pathfind.highLevel.HLChunkRegion.containsSquare (world coordinates)
    pub fn contains_square(&self, wx: i32, wy: i32) -> bool {
        self.contains_local(wx - self.level.0 * 8, wy - self.level.1 * 8)
    }

    /// zombie.pathfind.highLevel.HLChunkRegion.containsSquareLocal (`BooleanGrid.getValue`: false off the grid)
    pub fn contains_local(&self, x: i32, y: i32) -> bool {
        (0..8).contains(&x) && (0..8).contains(&y) && self.mask[(x + y * 8) as usize]
    }

    /// zombie.pathfind.highLevel.HLChunkRegion.initEdges
    fn init_edges(&mut self) {
        for y in 0..8 {
            self.edge_n[y as usize] = 0;
            self.edge_s[y as usize] = 0;
            for x in 0..8 {
                if self.contains_local(x, y) {
                    if !self.contains_local(x, y - 1) {
                        self.edge_n[y as usize] |= 1 << x;
                    }
                    if !self.contains_local(x, y + 1) {
                        self.edge_s[y as usize] |= 1 << x;
                    }
                }
            }
        }
        for x in 0..8 {
            self.edge_w[x as usize] = 0;
            self.edge_e[x as usize] = 0;
            for y in 0..8 {
                if self.contains_local(x, y) {
                    if !self.contains_local(x - 1, y) {
                        self.edge_w[x as usize] |= 1 << y;
                    }
                    if !self.contains_local(x + 1, y) {
                        self.edge_e[x as usize] |= 1 << y;
                    }
                }
            }
        }
    }

    /// zombie.pathfind.highLevel.HLChunkRegion.isOnEdgeOfLoadedArea
    fn is_on_edge_of_loaded_area(&self, m: &PathMap) -> bool {
        let (wx, wy) = (self.level.0, self.level.1);
        let mut edge = false;
        if self.edge_n[0] != 0 {
            edge |= m.get_chunk_from_chunk_pos(wx, wy - 1).is_none();
        }
        if self.edge_s[7] != 0 {
            edge |= m.get_chunk_from_chunk_pos(wx, wy + 1).is_none();
        }
        if self.edge_w[0] != 0 {
            edge |= m.get_chunk_from_chunk_pos(wx - 1, wy).is_none();
        }
        if self.edge_e[7] != 0 {
            edge |= m.get_chunk_from_chunk_pos(wx + 1, wy).is_none();
        }
        edge
    }
}

/// zombie.pathfind.highLevel.HLChunkLevel
#[derive(Clone, Debug, Default)]
pub struct HlLevel {
    pub regions: Vec<usize>,
    pub stairs: Vec<usize>,
    pub sloped: Vec<usize>,
}

/// The `HLChunkLevel`s of every chunk level of the cache, with their regions and level transitions.
#[derive(Debug, Default)]
pub struct HlStore {
    pub regions: Vec<Region>,
    pub transitions: Vec<Transition>,
    pub levels: HashMap<ChunkLevelKey, HlLevel>,
}

/// zombie.pathfind.highLevel.HLChunkLevel.canWalkOnSquare
fn can_walk_on_square(m: &PathMap, square: Option<&Square>) -> bool {
    match square {
        None => false,
        Some(s) => s.treat_as_solid_floor() && !s.is_really_solid(m),
    }
}

/// zombie.pathfind.highLevel.HLChunkLevel.isCanPathTransition
fn is_can_path_transition(s1: Option<&Square>, s2: Option<&Square>) -> bool {
    let (Some(s1), Some(s2)) = (s1, s2) else {
        return false;
    };
    if s1.z != s2.z || (s1.x == s2.x && s1.y == s2.y) {
        return false;
    }
    let dx = s2.x - s1.x;
    let dy = s2.y - s1.y;
    if dx < 0 {
        if s1.has(8192) {
            return true;
        }
    } else if dx > 0 && s2.has(8192) {
        return true;
    }
    if dy < 0 {
        if s1.has(16384) {
            return true;
        }
    } else if dy > 0 && s2.has(16384) {
        return true;
    }
    false
}

/// zombie.pathfind.highLevel.FloodFill (`shouldVisit`, `push`, `pop`, `calculate`) of one region
struct FloodFill<'a> {
    map: &'a PathMap,
    chunk: &'a Chunk,
    level: i32,
    mover: PmMover,
    thumpable: bool,
    visited: [bool; 64],
    choices: Vec<(i32, i32)>,
    stack: Vec<(i32, i32)>,
    min_x: i32,
    min_y: i32,
}

impl FloodFill<'_> {
    fn square_at(&self, gx: i32, gy: i32) -> Option<&Square> {
        self.chunk
            .get_level_data(self.level)
            .and_then(|l| l.get_local(gx, gy))
    }

    fn visited(&self, gx: i32, gy: i32) -> bool {
        (0..8).contains(&gx) && (0..8).contains(&gy) && self.visited[(gx + gy * 8) as usize]
    }

    /// zombie.pathfind.highLevel.FloodFill.shouldVisit
    fn should_visit(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) -> bool {
        let (gx2, gy2) = (x2 - self.min_x, y2 - self.min_y);
        if !(0..8).contains(&gx2) || !(0..8).contains(&gy2) {
            return false;
        }
        if self.visited(gx2, gy2) {
            return false;
        }
        let square2 = self.square_at(gx2, gy2);
        if self.thumpable {
            let Some(s2) = square2 else { return false };
            if !s2.has(131072) {
                return false;
            }
            if !s2.treat_as_solid_floor() {
                return false;
            }
        } else {
            if !can_walk_on_square(self.map, square2) {
                return false;
            }
            let square1 = self.square_at(x1 - self.min_x, y1 - self.min_y);
            if is_can_path_transition(square1, square2) {
                return false;
            }
        }
        self.mover.kind = Some(super::path::MoverType::Player);
        self.mover.min_level = self.level;
        self.mover.max_level = self.level;
        // PolygonalMap2.canMoveBetween: both squares must exist
        let a = self.map.get_square(x1, y1, self.level);
        let b = self.map.get_square(x2, y2, self.level);
        match (a, b) {
            (Some(a), Some(b)) => can_move_between(self.map, &self.mover, a, b, false),
            _ => false,
        }
    }

    /// zombie.pathfind.highLevel.FloodFill.calculate
    fn calculate(&mut self, region: &mut Region, start: &Square) {
        self.thumpable = start.has(131072);
        self.min_x = self.chunk.get_min_x();
        self.min_y = self.chunk.get_min_y();
        self.stack.push((start.x, start.y));
        while let Some((x, mut y1)) = self.stack.pop() {
            while self.should_visit(x, y1, x, y1 - 1) {
                y1 -= 1;
            }
            let mut span_left = false;
            let mut span_right = false;
            loop {
                let (gx, gy) = (x - self.min_x, y1 - self.min_y);
                if (0..8).contains(&gx) && (0..8).contains(&gy) {
                    self.visited[(gx + gy * 8) as usize] = true;
                }
                if self.square_at(gx, gy).is_some() {
                    self.choices.push((x, y1));
                }
                if !span_left && self.should_visit(x, y1, x - 1, y1) {
                    self.stack.push((x - 1, y1));
                    span_left = true;
                } else if span_left && !self.should_visit(x, y1, x - 1, y1) {
                    span_left = false;
                } else if span_left && !self.should_visit(x - 1, y1, x - 1, y1 - 1) {
                    self.stack.push((x - 1, y1));
                }
                if !span_right && self.should_visit(x, y1, x + 1, y1) {
                    self.stack.push((x + 1, y1));
                    span_right = true;
                } else if span_right && !self.should_visit(x, y1, x + 1, y1) {
                    span_right = false;
                } else if span_right && !self.should_visit(x + 1, y1, x + 1, y1 - 1) {
                    self.stack.push((x + 1, y1));
                }
                y1 += 1;
                if self.should_visit(x, y1 - 1, x, y1) {
                    continue;
                }
                break;
            }
        }
        region.min_x = i32::MAX;
        region.min_y = i32::MAX;
        region.max_x = i32::MIN;
        region.max_y = i32::MIN;
        for &(x, y) in &self.choices {
            region.mask[((x - self.min_x) + (y - self.min_y) * 8) as usize] = true;
            region.min_x = region.min_x.min(x);
            region.min_y = region.min_y.min(y);
            region.max_x = region.max_x.max(x);
            region.max_y = region.max_y.max(y);
        }
    }
}

impl HlStore {
    /// zombie.pathfind.highLevel.HLChunkLevel.initRegions + initStairs for every chunk level of the cache. The
    /// original does this lazily, on the first `getHighLevelData()` after `HLAStar.modificationCount` changed; the
    /// result depends only on the squares.
    pub fn rebuild(&mut self, m: &PathMap) {
        self.regions.clear();
        self.transitions.clear();
        self.levels.clear();
        let mut keys: Vec<(i32, i32)> = m.chunks.keys().copied().collect();
        keys.sort_unstable();
        for (wx, wy) in keys {
            let Some(chunk) = m.chunks.get(&(wx, wy)) else {
                continue;
            };
            for z in chunk.min_level..=chunk.max_level {
                let key = (wx, wy, z);
                let mut level = HlLevel::default();
                self.init_regions(m, chunk, key, &mut level);
                self.init_stairs(chunk, key, &mut level);
                self.levels.insert(key, level);
            }
        }
    }

    /// zombie.pathfind.highLevel.HLChunkLevel.initRegions
    fn init_regions(
        &mut self,
        m: &PathMap,
        chunk: &Chunk,
        key: ChunkLevelKey,
        level: &mut HlLevel,
    ) {
        let Some(data) = chunk.get_level_data(key.2) else {
            return;
        };
        for y in 0..8 {
            for x in 0..8 {
                let square = data.get_local(x, y);
                let Some(sq) = square else { continue };
                if can_walk_on_square(m, square)
                    && !level
                        .regions
                        .iter()
                        .any(|&r| self.regions[r].contains_square(sq.x, sq.y))
                {
                    let mut region = Region {
                        level: key,
                        mask: [false; 64],
                        min_x: 0,
                        min_y: 0,
                        max_x: 0,
                        max_y: 0,
                        edge_n: [0; 8],
                        edge_s: [0; 8],
                        edge_w: [0; 8],
                        edge_e: [0; 8],
                    };
                    let mut ff = FloodFill {
                        map: m,
                        chunk,
                        level: key.2,
                        mover: PmMover::default(),
                        thumpable: false,
                        visited: [false; 64],
                        choices: Vec::new(),
                        stack: Vec::new(),
                        min_x: 0,
                        min_y: 0,
                    };
                    ff.calculate(&mut region, sq);
                    region.init_edges();
                    level.regions.push(self.regions.len());
                    self.regions.push(region);
                }
            }
        }
    }

    /// zombie.pathfind.highLevel.HLChunkLevel.initStairsInternal + initSlopedSurfaces
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn init_stairs(&mut self, chunk: &Chunk, key: ChunkLevelKey, level: &mut HlLevel) {
        let Some(data) = chunk.get_level_data(key.2) else {
            return;
        };
        for y in 0..8 {
            for x in 0..8 {
                if let Some(sq) = data.get_local(x, y) {
                    if sq.has(64) {
                        level.stairs.push(self.transitions.len());
                        self.transitions.push(Transition {
                            is_stair: true,
                            dir: IsoDirections::N,
                            x: sq.x,
                            y: sq.y,
                            z: sq.z,
                        });
                    }
                    if sq.has(8) {
                        level.stairs.push(self.transitions.len());
                        self.transitions.push(Transition {
                            is_stair: true,
                            dir: IsoDirections::W,
                            x: sq.x,
                            y: sq.y,
                            z: sq.z,
                        });
                    }
                }
            }
        }
        for y in 0..8 {
            for x in 0..8 {
                if let Some(sq) = data.get_local(x, y) {
                    if let Some(dir) = sq.sloped_dir {
                        if !(sq.sloped_max < 1.0) {
                            level.sloped.push(self.transitions.len());
                            self.transitions.push(Transition {
                                is_stair: false,
                                dir,
                                x: sq.x,
                                y: sq.y,
                                z: sq.z,
                            });
                        }
                    }
                }
            }
        }
    }

    /// zombie.pathfind.highLevel.HLChunkLevel.findRegionContainingSquare
    pub fn find_region_containing_square(
        &self,
        key: ChunkLevelKey,
        x: i32,
        y: i32,
    ) -> Option<usize> {
        self.levels
            .get(&key)?
            .regions
            .iter()
            .copied()
            .find(|&r| self.regions[r].contains_square(x, y))
    }

    /// zombie.pathfind.highLevel.HLChunkLevel.getLevelTransitionAt: a staircase first, then a sloped surface.
    pub fn get_level_transition_at(&self, key: ChunkLevelKey, x: i32, y: i32) -> Option<usize> {
        let level = self.levels.get(&key)?;
        level
            .stairs
            .iter()
            .chain(level.sloped.iter())
            .copied()
            .find(|&t| self.transitions[t].is_bottom_floor_at(x, y))
    }
}

/// zombie.pathfind.highLevel.HLSearchNode
#[derive(Clone, Debug)]
struct HlNode {
    region: Option<usize>,
    transition: Option<usize>,
    bottom_of_staircase: bool,
    unloaded_x: i32,
    unloaded_y: i32,
    in_unloaded_area: bool,
    on_edge_of_loaded_area: bool,
    /// HLSearchNode.successors: (node, lowest cost)
    successors: Vec<(usize, f64)>,
    parent: Option<usize>,
    g: f64,
    depth: i32,
}

/// What `HLAStar.findPath` fills in.
#[derive(Debug, Default)]
pub struct HlResult {
    pub level_transitions: Vec<usize>,
    pub chunk_levels: Vec<ChunkLevelKey>,
    pub bottom_of_level_transition: Vec<bool>,
}

/// zombie.pathfind.highLevel.HLAStar
pub struct HlAStar<'a> {
    map: &'a PathMap,
    store: &'a HlStore,
    mover: &'a PmMover,
    nodes: Vec<HlNode>,
    node_map_region: HashMap<usize, usize>,
    node_map_transition: HashMap<usize, usize>,
    goal_in_unloaded_area: bool,
    unloaded_node: Option<usize>,
    goal: usize,
    /// the search of `libPZPathFind` (`a_star::search_native`) instead of `astar.AStar`
    pub native: bool,
}

impl<'a> HlAStar<'a> {
    pub fn new(map: &'a PathMap, store: &'a HlStore, mover: &'a PmMover) -> Self {
        HlAStar {
            map,
            store,
            mover,
            nodes: Vec::new(),
            node_map_region: HashMap::new(),
            node_map_transition: HashMap::new(),
            goal_in_unloaded_area: false,
            unloaded_node: None,
            goal: 0,
            native: false,
        }
    }

    fn new_node(
        &mut self,
        region: Option<usize>,
        transition: Option<usize>,
        bottom: bool,
    ) -> usize {
        let n = self.nodes.len();
        self.nodes.push(HlNode {
            region,
            transition,
            bottom_of_staircase: bottom,
            unloaded_x: -1,
            unloaded_y: -1,
            in_unloaded_area: false,
            on_edge_of_loaded_area: false,
            successors: Vec::new(),
            parent: None,
            g: 0.0,
            depth: 0,
        });
        let edge = self.goal_in_unloaded_area && self.calculate_on_edge_of_loaded_area(n);
        self.nodes[n].on_edge_of_loaded_area = edge;
        n
    }

    /// zombie.pathfind.highLevel.HLAStar.getSearchNode (HLChunkRegion)
    fn get_search_node_region(&mut self, region: usize) -> usize {
        if let Some(&n) = self.node_map_region.get(&region) {
            return n;
        }
        let n = self.new_node(Some(region), None, false);
        self.node_map_region.insert(region, n);
        n
    }

    /// zombie.pathfind.highLevel.HLAStar.getSearchNode (HLLevelTransition, bottom): an existing node keeps the flag it
    /// was made with.
    fn get_search_node_transition(&mut self, t: usize, bottom: bool) -> usize {
        if let Some(&n) = self.node_map_transition.get(&t) {
            return n;
        }
        let n = self.new_node(None, Some(t), bottom);
        self.node_map_transition.insert(t, n);
        n
    }

    /// zombie.pathfind.highLevel.HLAStar.getSearchNode (x, y)
    fn get_unloaded_search_node(&mut self, x: i32, y: i32) -> usize {
        if let Some(n) = self.unloaded_node {
            return n;
        }
        let n = self.nodes.len();
        self.nodes.push(HlNode {
            region: None,
            transition: None,
            bottom_of_staircase: false,
            unloaded_x: x,
            unloaded_y: y,
            in_unloaded_area: true,
            on_edge_of_loaded_area: false,
            successors: Vec::new(),
            parent: None,
            g: 0.0,
            depth: 0,
        });
        self.unloaded_node = Some(n);
        n
    }

    /// zombie.pathfind.highLevel.HLSearchNode.calculateOnEdgeOfLoadedArea
    fn calculate_on_edge_of_loaded_area(&self, n: usize) -> bool {
        let node = &self.nodes[n];
        if let Some(r) = node.region {
            self.store.regions[r].is_on_edge_of_loaded_area(self.map)
        } else {
            // HLLevelTransition.isOnEdgeOfLoadedArea is false
            false
        }
    }

    /// zombie.pathfind.highLevel.HLSearchNode.getX
    fn node_x(&self, n: usize) -> f32 {
        let node = &self.nodes[n];
        if let Some(r) = node.region {
            let r = &self.store.regions[r];
            (r.min_x + r.max_x + 1) as f32 / 2.0
        } else if let Some(t) = node.transition {
            self.store.transitions[t].search_node_x(node.bottom_of_staircase)
        } else {
            node.unloaded_x as f32
        }
    }

    fn node_y(&self, n: usize) -> f32 {
        let node = &self.nodes[n];
        if let Some(r) = node.region {
            let r = &self.store.regions[r];
            (r.min_y + r.max_y + 1) as f32 / 2.0
        } else if let Some(t) = node.transition {
            self.store.transitions[t].search_node_y(node.bottom_of_staircase)
        } else {
            node.unloaded_y as f32
        }
    }

    fn node_z(&self, n: usize) -> f32 {
        let node = &self.nodes[n];
        if let Some(r) = node.region {
            self.store.regions[r].level.2 as f32
        } else if let Some(t) = node.transition {
            let t = &self.store.transitions[t];
            if node.bottom_of_staircase {
                t.bottom_floor_z() as f32
            } else {
                t.top_floor_z() as f32
            }
        } else {
            32.0
        }
    }

    /// zombie.pathfind.highLevel.HLAStar.getRegionAt
    fn get_region_at(&self, x: f32, y: f32, z: i32) -> Option<usize> {
        let chunk = self
            .map
            .get_chunk_from_chunk_pos(pz_math::fastfloor(x / 8.0), pz_math::fastfloor(y / 8.0))?;
        if !chunk.is_valid_level(z) {
            return None;
        }
        self.store.find_region_containing_square(
            (chunk.wx, chunk.wy, z),
            pz_math::fastfloor(x),
            pz_math::fastfloor(y),
        )
    }

    /// zombie.pathfind.highLevel.HLAStar.getLevelTransitionAt
    pub fn get_level_transition_at(
        map: &PathMap,
        store: &HlStore,
        x: i32,
        y: i32,
        z: i32,
    ) -> Option<usize> {
        let chunk = map.get_chunk_from_square_pos(x, y)?;
        if !chunk.is_valid_level(z) {
            return None;
        }
        store.get_level_transition_at((chunk.wx, chunk.wy, z), x, y)
    }

    /// zombie.pathfind.highLevel.HLAStar.initStartNode (no visibility graph node exists for a square)
    fn init_start_node(&mut self, x1: f32, y1: f32, z1: i32) -> Option<usize> {
        if let Some(r) = self.get_region_at(x1, y1, z1) {
            return Some(self.get_search_node_region(r));
        }
        let t = Self::get_level_transition_at(
            self.map,
            self.store,
            pz_math::fastfloor(x1),
            pz_math::fastfloor(y1),
            z1,
        )?;
        Some(self.get_search_node_transition(t, true))
    }

    /// zombie.pathfind.highLevel.HLAStar.initGoalNode
    fn init_goal_node(&mut self, x2: f32, y2: f32, z2: i32) -> Option<usize> {
        let (fx, fy) = (pz_math::fastfloor(x2), pz_math::fastfloor(y2));
        if self.map.get_chunk_from_square_pos(fx, fy).is_none() {
            return Some(self.get_unloaded_search_node(fx, fy));
        }
        if let Some(r) = self.get_region_at(x2, y2, z2) {
            return Some(self.get_search_node_region(r));
        }
        let t = Self::get_level_transition_at(self.map, self.store, fx, fy, z2)?;
        Some(self.get_search_node_transition(t, true))
    }

    /// zombie.pathfind.highLevel.HLAStar.findPath
    #[allow(clippy::too_many_arguments)]
    pub fn find_path(
        &mut self,
        x1: f32,
        y1: f32,
        z1: i32,
        x2: f32,
        y2: f32,
        z2: i32,
        result: &mut HlResult,
    ) -> Result<(), String> {
        result.level_transitions.clear();
        result.chunk_levels.clear();
        let chunk = self
            .map
            .get_chunk_from_square_pos(pz_math::fastfloor(x2), pz_math::fastfloor(y2));
        self.goal_in_unloaded_area = chunk.is_none();
        self.unloaded_node = None;
        let Some(initial) = self.init_start_node(x1, y1, z1) else {
            return Ok(());
        };
        let Some(goal1) = self.init_goal_node(x2, y2, z2) else {
            return Ok(());
        };
        let (ir, gr) = (self.nodes[initial].region, self.nodes[goal1].region);
        if ir == gr && ir.is_some() {
            let level = self.store.regions[ir.unwrap_or(0)].level;
            self.add_chunk_level_and_adjacent_to_list_level(level, &mut result.chunk_levels);
            return Ok(());
        }
        let (it, gt) = (self.nodes[initial].transition, self.nodes[goal1].transition);
        if it.is_none() || it != gt {
            self.goal = goal1;
            let found = if self.native {
                a_star::search_native(self, initial, -1)?
            } else {
                a_star::search(self, initial, -1)?
            };
            let Some(end) = found else {
                return Ok(());
            };
            let path = a_star::path(self, end)?;
            result.bottom_of_level_transition.clear();
            for pi in 0..path.len() {
                let sn = path[pi];
                if let Some(region) = self.nodes[sn].region {
                    self.add_chunk_level_and_adjacent_to_list_region(
                        region,
                        &mut result.chunk_levels,
                    )?;
                }
                if let Some(t) = self.nodes[sn].transition {
                    result.level_transitions.push(t);
                    self.add_level_transition_chunk_level_data(t, &mut result.chunk_levels);
                    let to_bottom = if pi == path.len() - 1 {
                        true
                    } else {
                        let next = path[pi + 1];
                        pz_math::fastfloor(self.node_z(next))
                            <= self.store.transitions[t].bottom_floor_z()
                    };
                    result.bottom_of_level_transition.push(to_bottom);
                }
            }
            return Ok(());
        }
        let t = it.unwrap_or(0);
        result.level_transitions.push(t);
        self.add_level_transition_chunk_level_data(t, &mut result.chunk_levels);
        Ok(())
    }

    /// zombie.pathfind.highLevel.HLAStar.addLevelTransitionChunkLevelData
    fn add_level_transition_chunk_level_data(&self, t: usize, list: &mut Vec<ChunkLevelKey>) {
        let tr = self.store.transitions[t];
        for (x, y, z) in [
            (
                tr.bottom_floor_x(),
                tr.bottom_floor_y(),
                tr.bottom_floor_z(),
            ),
            (tr.top_floor_x(), tr.top_floor_y(), tr.top_floor_z()),
        ] {
            if let Some(chunk) = self.map.get_chunk_from_square_pos(x, y) {
                self.add_chunk_level_and_adjacent_to_list_level((chunk.wx, chunk.wy, z), list);
            }
        }
    }

    /// zombie.pathfind.highLevel.HLAStar.addChunkLevelAndAdjacentToList (HLChunkLevel)
    fn add_chunk_level_and_adjacent_to_list_level(
        &self,
        level: ChunkLevelKey,
        list: &mut Vec<ChunkLevelKey>,
    ) {
        if !list.contains(&level) {
            list.push(level);
        }
        for dir in DIRECTIONS {
            if let Some(chunk2) = self
                .map
                .get_chunk_from_chunk_pos(level.0 + dir.dx(), level.1 + dir.dy())
            {
                if chunk2.is_valid_level(level.2) {
                    let l2 = (chunk2.wx, chunk2.wy, level.2);
                    if !list.contains(&l2) {
                        list.push(l2);
                    }
                }
            }
        }
    }

    /// zombie.pathfind.highLevel.HLAStar.addChunkLevelAndAdjacentToList (HLChunkRegion): an adjacent chunk level is
    /// only added when the region's node has a successor region on it.
    fn add_chunk_level_and_adjacent_to_list_region(
        &mut self,
        region: usize,
        list: &mut Vec<ChunkLevelKey>,
    ) -> Result<(), String> {
        let level = self.store.regions[region].level;
        if !list.contains(&level) {
            list.push(level);
        }
        let search_node = self.node_map_region.get(&region).copied();
        for dir in DIRECTIONS {
            let Some(chunk2) = self
                .map
                .get_chunk_from_chunk_pos(level.0 + dir.dx(), level.1 + dir.dy())
            else {
                continue;
            };
            if !chunk2.is_valid_level(level.2) {
                continue;
            }
            let l2 = (chunk2.wx, chunk2.wy, level.2);
            if let Some(sn) = search_node {
                let on_level2 = |s: &HlAStar, n: usize| {
                    s.nodes[n]
                        .region
                        .is_some_and(|r| s.store.regions[r].level == l2)
                };
                if self.nodes[sn].successors.is_empty() && self.goal == sn {
                    let mut successors = Vec::new();
                    self.get_successors(sn, &mut successors)?;
                    if !successors.iter().any(|&n| on_level2(self, n)) {
                        continue;
                    }
                } else if !self.nodes[sn]
                    .successors
                    .iter()
                    .any(|&(n, _)| on_level2(self, n))
                {
                    continue;
                }
            }
            if !list.contains(&l2) {
                list.push(l2);
            }
        }
        Ok(())
    }

    /// zombie.pathfind.highLevel.HLAStar.calculateCost (node, square1, region2, square2)
    fn calculate_cost_region(
        &self,
        node: usize,
        square1: &Square,
        region2: usize,
        square2: &Square,
    ) -> f64 {
        let r = &self.store.regions[region2];
        self.calculate_cost(
            node,
            square1,
            (r.min_x + r.max_x + 1) as f32 / 2.0,
            (r.min_y + r.max_y + 1) as f32 / 2.0,
            square2,
        )
    }

    /// zombie.pathfind.highLevel.HLAStar.calculateCost (node, square1, node2X, node2Y, square2)
    fn calculate_cost(
        &self,
        node: usize,
        square1: &Square,
        node2_x: f32,
        node2_y: f32,
        square2: &Square,
    ) -> f64 {
        let x1 = square1.x as f32 + 0.5;
        let y1 = square1.y as f32 + 0.5;
        let z1 = square1.z as f32;
        let x2 = square1.x as f32 + 0.5;
        let y2 = square1.y as f32 + 0.5;
        let z2 = square1.z as f32;
        let mut cost = f64::from(distance_to(self.node_x(node), self.node_y(node), x1, y1));
        let dx = f64::from(x1 - x2);
        let dy = f64::from(y1 - y2);
        let dz = f64::from((z1 - z2) * 2.5f32);
        cost += (dx * dx + dy * dy + dz * dz).sqrt();
        cost += f64::from(distance_to(x2, y2, node2_x, node2_y));
        let crawling_zombie = self.mover.is_zombie() && self.mover.crawling;
        let avoid_windows = !self.mover.is_zombie() || self.mover.crawling;
        if avoid_windows {
            let m = self.map;
            let adjacent = |dir: IsoDirections| {
                square1.x + dir.dx() == square2.x
                    && square1.y + dir.dy() == square2.y
                    && square1.z == square2.z
            };
            if adjacent(IsoDirections::N) {
                if !crawling_zombie && square1.is_unblocked_window_n(m) {
                    cost += 20.0;
                } else if square1.has(4096) {
                    cost += 200.0;
                }
            } else if adjacent(IsoDirections::S) {
                if !crawling_zombie && square2.is_unblocked_window_n(m) {
                    cost += 20.0;
                } else if square2.has(4096) {
                    cost += 200.0;
                }
            } else if adjacent(IsoDirections::W) {
                if !crawling_zombie && square1.is_unblocked_window_w(m) {
                    cost += 20.0;
                } else if square1.has(2048) {
                    cost += 200.0;
                }
            } else if adjacent(IsoDirections::E) {
                if !crawling_zombie && square2.is_unblocked_window_w(m) {
                    cost += 20.0;
                } else if square2.has(2048) {
                    cost += 200.0;
                }
            }
        }
        cost
    }

    /// zombie.pathfind.highLevel.HLAStar.calculateCost (node, square2)
    fn calculate_cost_square(&self, node: usize, square2: &Square) -> f64 {
        f64::from(distance_to(
            self.node_x(node),
            self.node_y(node),
            square2.x as f32 + 0.5,
            square2.y as f32 + 0.5,
        ))
    }

    /// zombie.pathfind.highLevel.HLAStar.setLowestCostSuccessor
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn set_lowest_cost_successor(&mut self, node: usize, successor: usize, cost: f64) {
        let list = &mut self.nodes[node].successors;
        match list.iter().position(|&(n, _)| n == successor) {
            None => list.push((successor, cost)),
            Some(i) => {
                if !(list[i].1 <= cost) {
                    list[i].1 = cost;
                }
            }
        }
    }

    /// zombie.pathfind.highLevel.HLAStar.addSuccessor (cost, node, HLChunkRegion, successors)
    fn add_successor_region(
        &mut self,
        cost: f64,
        node: usize,
        region: usize,
        out: &mut Vec<usize>,
    ) {
        let s = self.get_search_node_region(region);
        if !out.contains(&s) {
            out.push(s);
        }
        self.set_lowest_cost_successor(node, s, cost);
    }

    /// zombie.pathfind.highLevel.HLAStar.addSuccessor (cost, node, HLLevelTransition, bottom, successors)
    fn add_successor_transition(
        &mut self,
        cost: f64,
        node: usize,
        t: usize,
        bottom: bool,
        out: &mut Vec<usize>,
    ) {
        let s = self.get_search_node_transition(t, bottom);
        if !out.contains(&s) {
            out.push(s);
        }
        self.set_lowest_cost_successor(node, s, cost);
    }

    /// PolygonalMap2.canMoveBetween with the `HLGlobals.mover`
    fn can_move(&self, x1: i32, y1: i32, z1: i32, x2: i32, y2: i32, z2: i32) -> bool {
        match (
            self.map.get_square(x1, y1, z1),
            self.map.get_square(x2, y2, z2),
        ) {
            (Some(a), Some(b)) => can_move_between(self.map, self.mover, a, b, false),
            _ => false,
        }
    }

    /// zombie.pathfind.highLevel.HLAStar.getSuccessorChunkRegionsFromLevelTransition
    fn successors_from_level_transition(&mut self, node: usize, out: &mut Vec<usize>) {
        let Some(t) = self.nodes[node].transition else {
            return;
        };
        let tr = self.store.transitions[t];
        let (Some(square1), Some(square2)) = (
            tr.top_floor_square(self.map),
            tr.bottom_floor_square(self.map),
        ) else {
            return;
        };
        let (square1, square2) = (square1.clone(), square2.clone());
        if let Some(chunk2) = self
            .map
            .get_chunk_from_square_pos(tr.bottom_floor_x(), tr.bottom_floor_y())
        {
            let key = (chunk2.wx, chunk2.wy, tr.bottom_floor_z());
            if let Some(r2) = self.store.find_region_containing_square(
                key,
                tr.bottom_floor_x(),
                tr.bottom_floor_y(),
            ) {
                let cost = self.calculate_cost_region(node, &square1, r2, &square2);
                self.add_successor_region(cost, node, r2, out);
            }
        }
        if let Some(chunk2) = self
            .map
            .get_chunk_from_square_pos(tr.top_floor_x(), tr.top_floor_y())
        {
            let key = (chunk2.wx, chunk2.wy, tr.top_floor_z());
            if let Some(r2) =
                self.store
                    .find_region_containing_square(key, tr.top_floor_x(), tr.top_floor_y())
            {
                let cost = self.calculate_cost_region(node, &square2, r2, &square1);
                self.add_successor_region(cost, node, r2, out);
            }
        }
    }

    /// zombie.pathfind.highLevel.HLAStar.addSuccessorsOnEdge
    #[allow(clippy::too_many_arguments)]
    fn add_successors_on_edge(
        &mut self,
        node: usize,
        region2: usize,
        xy: i32,
        bits: i32,
        chunk: &Chunk,
        dir: IsoDirections,
        level: i32,
        out: &mut Vec<usize>,
    ) -> Result<(), String> {
        let data = chunk
            .get_level_data(level)
            .ok_or("addSuccessorsOnEdge: the chunk has no such level")?;
        for n in 0..8 {
            if bits & (1 << n) != 0 {
                let sq = if dir.dx() == 0 {
                    data.get_local(n, xy)
                } else {
                    data.get_local(xy, n)
                };
                let sq1 = sq.ok_or(
                    "addSuccessorsOnEdge: no square on the region edge (a NullPointerException)",
                )?;
                if self.can_move(
                    sq1.x,
                    sq1.y,
                    sq1.z,
                    sq1.x + dir.dx(),
                    sq1.y + dir.dy(),
                    sq1.z,
                ) {
                    let adj = self
                        .map
                        .get_square(sq1.x + dir.dx(), sq1.y + dir.dy(), sq1.z)
                        .ok_or("addSuccessorsOnEdge: no adjacent square")?
                        .clone();
                    let sq1 = sq1.clone();
                    let cost = self.calculate_cost_region(node, &sq1, region2, &adj);
                    self.add_successor_region(cost, node, region2, out);
                }
            }
        }
        Ok(())
    }

    /// zombie.pathfind.highLevel.HLAStar.getSuccessorsOnSameChunk
    fn successors_on_same_chunk(
        &mut self,
        node: usize,
        out: &mut Vec<usize>,
    ) -> Result<(), String> {
        let Some(r1) = self.nodes[node].region else {
            return Ok(());
        };
        let map = self.map;
        let level_key = self.store.regions[r1].level;
        let chunk1 = map
            .get_chunk_from_chunk_pos(level_key.0, level_key.1)
            .ok_or("no chunk for the region")?;
        let level = level_key.2;
        let region_list: Vec<usize> = self
            .store
            .levels
            .get(&level_key)
            .map(|l| l.regions.clone())
            .unwrap_or_default();
        for y in 0..8usize {
            let (e1n, e1s) = (
                self.store.regions[r1].edge_n[y],
                self.store.regions[r1].edge_s[y],
            );
            if y > 0 && e1n != 0 {
                for &r2 in &region_list {
                    let both = self.store.regions[r2].edge_s[y - 1] & e1n;
                    if r1 != r2 && both != 0 {
                        self.add_successors_on_edge(
                            node,
                            r2,
                            y as i32,
                            both,
                            chunk1,
                            IsoDirections::N,
                            level,
                            out,
                        )?;
                    }
                }
            }
            if y < 7 && e1s != 0 {
                for &r2 in &region_list {
                    let both = self.store.regions[r2].edge_n[y + 1] & e1s;
                    if r1 != r2 && both != 0 {
                        self.add_successors_on_edge(
                            node,
                            r2,
                            y as i32,
                            both,
                            chunk1,
                            IsoDirections::S,
                            level,
                            out,
                        )?;
                    }
                }
            }
        }
        for x in 0..8usize {
            let (e1w, e1e) = (
                self.store.regions[r1].edge_w[x],
                self.store.regions[r1].edge_e[x],
            );
            if x > 0 && e1w != 0 {
                for &r2 in &region_list {
                    let both = self.store.regions[r2].edge_e[x - 1] & e1w;
                    if r1 != r2 && both != 0 {
                        self.add_successors_on_edge(
                            node,
                            r2,
                            x as i32,
                            both,
                            chunk1,
                            IsoDirections::W,
                            level,
                            out,
                        )?;
                    }
                }
            }
            if x < 7 && e1e != 0 {
                for &r2 in &region_list {
                    let both = self.store.regions[r2].edge_w[x + 1] & e1e;
                    if r1 != r2 && both != 0 {
                        self.add_successors_on_edge(
                            node,
                            r2,
                            x as i32,
                            both,
                            chunk1,
                            IsoDirections::E,
                            level,
                            out,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    /// zombie.pathfind.highLevel.HLAStar.getSuccessorsOnAdjacentChunks
    fn successors_on_adjacent_chunks(
        &mut self,
        node: usize,
        out: &mut Vec<usize>,
    ) -> Result<(), String> {
        let Some(r1) = self.nodes[node].region else {
            return Ok(());
        };
        let map = self.map;
        let level_key = self.store.regions[r1].level;
        let chunk1 = map
            .get_chunk_from_chunk_pos(level_key.0, level_key.1)
            .ok_or("no chunk for the region")?;
        let level = level_key.2;
        let region1 = self.store.regions[r1].clone();
        let regions_of = |s: &Self, wx: i32, wy: i32| -> Option<Vec<usize>> {
            let c2 = map.get_chunk_from_chunk_pos(wx, wy)?;
            if !c2.is_valid_level(level) {
                return None;
            }
            Some(
                s.store
                    .levels
                    .get(&(wx, wy, level))
                    .map(|l| l.regions.clone())
                    .unwrap_or_default(),
            )
        };
        if region1.edge_n[0] != 0 {
            if let Some(list) = regions_of(self, chunk1.wx, chunk1.wy - 1) {
                for r2 in list {
                    let both = self.store.regions[r2].edge_s[7] & region1.edge_n[0];
                    if both != 0 {
                        self.add_successors_on_edge(
                            node,
                            r2,
                            0,
                            both,
                            chunk1,
                            IsoDirections::N,
                            level,
                            out,
                        )?;
                    }
                }
            }
        }
        if region1.edge_s[7] != 0 {
            if let Some(list) = regions_of(self, chunk1.wx, chunk1.wy + 1) {
                for r2 in list {
                    let both = self.store.regions[r2].edge_n[0] & region1.edge_s[7];
                    if both != 0 {
                        self.add_successors_on_edge(
                            node,
                            r2,
                            7,
                            both,
                            chunk1,
                            IsoDirections::S,
                            level,
                            out,
                        )?;
                    }
                }
            }
        }
        if region1.edge_w[0] != 0 {
            if let Some(list) = regions_of(self, chunk1.wx - 1, chunk1.wy) {
                for r2 in list {
                    let both = self.store.regions[r2].edge_e[7] & region1.edge_w[0];
                    if both != 0 {
                        for _ in 0..8 {
                            self.add_successors_on_edge(
                                node,
                                r2,
                                0,
                                both,
                                chunk1,
                                IsoDirections::W,
                                level,
                                out,
                            )?;
                        }
                    }
                }
            }
        }
        if region1.edge_e[7] != 0 {
            if let Some(list) = regions_of(self, chunk1.wx + 1, chunk1.wy) {
                for r2 in list {
                    let both = self.store.regions[r2].edge_w[0] & region1.edge_e[7];
                    if both != 0 {
                        for _ in 0..8 {
                            self.add_successors_on_edge(
                                node,
                                r2,
                                7,
                                both,
                                chunk1,
                                IsoDirections::E,
                                level,
                                out,
                            )?;
                        }
                    }
                }
            }
        }
        // the four diagonal neighbours of the chunk's corner squares
        let corners = [
            (chunk1.get_min_x(), chunk1.get_min_y(), -1, -1, 0, 0, 7, 7),
            (chunk1.get_max_x(), chunk1.get_min_y(), 1, -1, 7, 0, 0, 7),
            (chunk1.get_max_x(), chunk1.get_max_y(), 1, 1, 7, 7, 0, 0),
            (chunk1.get_min_x(), chunk1.get_max_y(), -1, 1, 0, 7, 7, 0),
        ];
        for (x1, y1, dx, dy, lx1, ly1, lx2, ly2) in corners {
            if !region1.contains_square(x1, y1) {
                continue;
            }
            let (wx2, wy2) = (chunk1.wx + dx, chunk1.wy + dy);
            let Some(chunk2) = map.get_chunk_from_chunk_pos(wx2, wy2) else {
                continue;
            };
            if !chunk2.is_valid_level(level) {
                continue;
            }
            let Some(r2) =
                self.store
                    .find_region_containing_square((wx2, wy2, level), x1 + dx, y1 + dy)
            else {
                continue;
            };
            if !self.can_move(x1, y1, level, x1 + dx, y1 + dy, level) {
                continue;
            }
            let sq1 = chunk1
                .get_level_data(level)
                .and_then(|l| l.get_local(lx1, ly1))
                .ok_or("successors_on_adjacent_chunks: no corner square")?
                .clone();
            let sq2 = chunk2
                .get_level_data(level)
                .and_then(|l| l.get_local(lx2, ly2))
                .ok_or("successors_on_adjacent_chunks: no corner square")?
                .clone();
            let cost = self.calculate_cost_region(node, &sq1, r2, &sq2);
            self.add_successor_region(cost, node, r2, out);
        }
        Ok(())
    }

    /// zombie.pathfind.highLevel.HLAStar.getSuccessorsGoingUp
    fn successors_going_up(&mut self, node: usize, out: &mut Vec<usize>) -> Result<(), String> {
        let Some(r1) = self.nodes[node].region else {
            return Ok(());
        };
        let key = self.store.regions[r1].level;
        let Some(level) = self.store.levels.get(&key) else {
            return Ok(());
        };
        let (stairs, sloped) = (level.stairs.clone(), level.sloped.clone());
        let map = self.map;
        let chunk1 = map
            .get_chunk_from_chunk_pos(key.0, key.1)
            .ok_or("no chunk for the region")?;
        for (list, _) in [(stairs, true), (sloped, false)] {
            for t in list {
                let tr = self.store.transitions[t];
                if self.store.regions[r1].contains_square(tr.bottom_floor_x(), tr.bottom_floor_y())
                {
                    let sq2 = chunk1
                        .get_level_data(key.2)
                        .and_then(|l| {
                            l.get_local(
                                pz_math::coordmodulo(tr.bottom_floor_x(), 8),
                                pz_math::coordmodulo(tr.bottom_floor_y(), 8),
                            )
                        })
                        .ok_or("successors_going_up: no bottom square")?;
                    let cost = self.calculate_cost_square(node, sq2);
                    self.add_successor_transition(cost, node, t, true, out);
                }
            }
        }
        Ok(())
    }

    /// zombie.pathfind.highLevel.HLAStar.getSuccessorsGoingDown_Staircases / _SlopedSurfaces
    fn successors_going_down_at(
        &mut self,
        node: usize,
        wx: i32,
        wy: i32,
        stairs: bool,
        out: &mut Vec<usize>,
    ) -> Result<(), String> {
        let Some(r1) = self.nodes[node].region else {
            return Ok(());
        };
        let key = self.store.regions[r1].level;
        let map = self.map;
        let chunk1 = map
            .get_chunk_from_chunk_pos(key.0, key.1)
            .ok_or("no chunk for the region")?;
        let Some(chunk2) = map.get_chunk_from_chunk_pos(wx, wy) else {
            return Ok(());
        };
        if !chunk2.is_valid_level(key.2 - 1) {
            return Ok(());
        }
        let Some(below) = self.store.levels.get(&(wx, wy, key.2 - 1)) else {
            return Ok(());
        };
        let list = if stairs {
            below.stairs.clone()
        } else {
            below.sloped.clone()
        };
        for t in list {
            let tr = self.store.transitions[t];
            if self.store.regions[r1].contains_square(tr.top_floor_x(), tr.top_floor_y()) {
                let sq2 = chunk1
                    .get_level_data(key.2)
                    .and_then(|l| {
                        l.get_local(
                            pz_math::coordmodulo(tr.top_floor_x(), 8),
                            pz_math::coordmodulo(tr.top_floor_y(), 8),
                        )
                    })
                    .ok_or("successors_going_down: no top square")?;
                let cost = self.calculate_cost_square(node, sq2);
                self.add_successor_transition(cost, node, t, false, out);
            }
        }
        Ok(())
    }

    /// zombie.pathfind.highLevel.HLAStar.getSuccessorsGoingDown
    fn successors_going_down(&mut self, node: usize, out: &mut Vec<usize>) -> Result<(), String> {
        let Some(r1) = self.nodes[node].region else {
            return Ok(());
        };
        let key = self.store.regions[r1].level;
        let (wx, wy) = (key.0, key.1);
        self.successors_going_down_at(node, wx, wy, true, out)?;
        self.successors_going_down_at(node, wx, wy + 1, true, out)?;
        self.successors_going_down_at(node, wx + 1, wy, true, out)?;
        self.successors_going_down_at(node, wx, wy, false, out)?;
        self.successors_going_down_at(node, wx, wy + 1, false, out)?;
        self.successors_going_down_at(node, wx, wy - 1, false, out)?;
        self.successors_going_down_at(node, wx + 1, wy, false, out)?;
        self.successors_going_down_at(node, wx - 1, wy, false, out)?;
        Ok(())
    }
}

impl AStarNodes for HlAStar<'_> {
    fn g(&self, n: usize) -> f64 {
        self.nodes[n].g
    }

    fn set_g(&mut self, n: usize, g: f64) {
        self.nodes[n].g = g;
    }

    /// zombie.pathfind.highLevel.HLSearchNode.h
    fn h(&self, n: usize) -> f64 {
        let (x1, y1, z1) = (self.node_x(n), self.node_y(n), self.node_z(n));
        let (x2, y2, z2) = (
            self.node_x(self.goal),
            self.node_y(self.goal),
            self.node_z(self.goal),
        );
        let dx = f64::from(x1 - x2);
        let dy = f64::from(y1 - y2);
        let dz = f64::from((z1 - z2) * 2.5f32);
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// zombie.pathfind.highLevel.HLSearchNode.c
    fn c(&self, from: usize, to: usize) -> f64 {
        if self.nodes[to].in_unloaded_area {
            return f64::from(distance_to_xyz(
                self.node_x(from),
                self.node_y(from),
                self.node_z(from),
                self.node_x(to),
                self.node_y(to),
                self.node_z(to),
            ));
        }
        self.nodes[from]
            .successors
            .iter()
            .find(|&&(n, _)| n == to)
            .map_or(f64::NAN, |&(_, c)| c)
    }

    /// zombie.pathfind.highLevel.HLAStar.getSuccessors
    fn get_successors(&mut self, n: usize, out: &mut Vec<usize>) -> Result<(), String> {
        self.nodes[n].successors.clear();
        if self.nodes[n].in_unloaded_area {
            return Ok(());
        }
        if self.nodes[n].on_edge_of_loaded_area {
            if let Some(u) = self.unloaded_node {
                out.push(u);
            }
        }
        if self.nodes[n].transition.is_some() {
            self.successors_from_level_transition(n, out);
        } else {
            self.successors_on_same_chunk(n, out)?;
            self.successors_on_adjacent_chunks(n, out)?;
            self.successors_going_up(n, out)?;
            self.successors_going_down(n, out)?;
        }
        Ok(())
    }

    fn parent(&self, n: usize) -> Option<usize> {
        self.nodes[n].parent
    }

    fn set_parent(&mut self, n: usize, p: Option<usize>) {
        self.nodes[n].parent = p;
    }

    fn depth(&self, n: usize) -> i32 {
        self.nodes[n].depth
    }

    fn set_depth(&mut self, n: usize, d: i32) {
        self.nodes[n].depth = d;
    }

    /// zombie.pathfind.highLevel.HLGoalNode.inGoal
    fn in_goal(&self, n: usize) -> bool {
        n == self.goal
    }
}
