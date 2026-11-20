//! `zombie.pathfind.VGAStar` and `zombie.pathfind.SearchNode`: the low-level search over the squares of the cache. There
//! are no visibility graphs (no vehicles are ported), so a search node is a square, or the "unloaded area" goal.
use std::collections::HashMap;

use super::a_star::{self, AStarNodes};
use super::highlevel::Transition;
use super::path::{Mover, PathFindRequest, PmMover};
use super::path_map::{PathMap, Square};
use crate::zombie::core::math::pz_math;
use crate::zombie::iso::iso_directions::IsoDirections;

/// zombie.pathfind.VGAStar.canNotMoveBetween (a player or a zombie; `isKnownBlocked` has no edges to know)
pub fn can_not_move_between(
    m: &PathMap,
    mover: &PmMover,
    square1: &Square,
    square2: &Square,
    is_between: bool,
) -> bool {
    let test_w = square2.x < square1.x;
    let test_e = square2.x > square1.x;
    let test_n = square2.y < square1.y;
    let test_s = square2.y > square1.y;
    if !(!square2.is_non_thumpable_solid(m) && (mover.can_thump || !square2.is_really_solid(m))) {
        return true;
    }
    if square2.y < square1.y && square1.has(64) {
        return true;
    }
    if square2.x < square1.x && square1.has(8) {
        return true;
    }
    if square2.y > square1.y && square2.x == square1.x && square2.has(64) {
        return true;
    }
    if square2.x > square1.x && square2.y == square1.y && square2.has(8) {
        return true;
    }
    if square2.x != square1.x && square2.has(448) {
        return true;
    }
    if square2.y != square1.y && square2.has(56) {
        return true;
    }
    if square2.x != square1.x && square1.has(448) {
        return true;
    }
    if square2.y != square1.y && square1.has(56) {
        return true;
    }
    if square2.z == square1.z {
        if square2.x == square1.x
            && square2.y == square1.y - 1
            && (square1.is_sloped_surface_edge_blocked(m, IsoDirections::N)
                || square2.is_sloped_surface_edge_blocked(m, IsoDirections::S))
        {
            return true;
        }
        if square2.x == square1.x
            && square2.y == square1.y + 1
            && (square1.is_sloped_surface_edge_blocked(m, IsoDirections::S)
                || square2.is_sloped_surface_edge_blocked(m, IsoDirections::N))
        {
            return true;
        }
        if square2.x == square1.x - 1
            && square2.y == square1.y
            && (square1.is_sloped_surface_edge_blocked(m, IsoDirections::W)
                || square2.is_sloped_surface_edge_blocked(m, IsoDirections::E))
        {
            return true;
        }
        if square2.x == square1.x + 1
            && square2.y == square1.y
            && (square1.is_sloped_surface_edge_blocked(m, IsoDirections::E)
                || square2.is_sloped_surface_edge_blocked(m, IsoDirections::W))
        {
            return true;
        }
    }
    if !square2.has(512) && !square2.has(504) {
        return true;
    }
    // isKnownBlocked: no known blocked edges
    let mut can_path_n = square1.is_can_path_n(m) && (mover.can_thump || !square1.is_thump_n(m));
    let mut can_path_w = square1.is_can_path_w(m) && (mover.can_thump || !square1.is_thump_w(m));
    let col_n =
        test_n && square1.is_collide_n(m) && (square1.x != square2.x || is_between || !can_path_n);
    let col_w =
        test_w && square1.is_collide_w(m) && (square1.y != square2.y || is_between || !can_path_w);
    can_path_n = square2.is_can_path_n(m) && (mover.can_thump || !square2.is_thump_n(m));
    can_path_w = square2.is_can_path_w(m) && (mover.can_thump || !square2.is_thump_w(m));
    let col_s =
        test_s && square2.has(131076) && (square1.x != square2.x || is_between || !can_path_n);
    let col_e =
        test_e && square2.has(131074) && (square1.y != square2.y || is_between || !can_path_w);
    if col_n || col_w || col_s || col_e {
        return true;
    }
    let diag = square2.x != square1.x && square2.y != square1.y;
    if diag {
        let between_a = m.get_square(square1.x, square2.y, square1.z);
        let between_b = m.get_square(square2.x, square1.y, square1.z);
        if square2.x == square1.x + 1 && square2.y == square1.y + 1 {
            if let (Some(a), Some(b)) = (between_a, between_b) {
                if a.has(4096) && b.has(2048) {
                    return true;
                }
                if a.is_thump_n(m) && b.is_thump_w(m) {
                    return true;
                }
            }
        }
        if square2.x == square1.x - 1 && square2.y == square1.y - 1 {
            if let (Some(a), Some(b)) = (between_a, between_b) {
                if a.has(2048) && b.has(4096) {
                    return true;
                }
                if a.is_thump_w(m) && b.is_thump_n(m) {
                    return true;
                }
            }
        }
        let Some(a) = between_a else { return true };
        if can_not_move_between(m, mover, square1, a, true) {
            return true;
        }
        let Some(b) = between_b else { return true };
        if can_not_move_between(m, mover, square1, b, true) {
            return true;
        }
        if can_not_move_between(m, mover, square2, a, true) {
            return true;
        }
        if can_not_move_between(m, mover, square2, b, true) {
            return true;
        }
    }
    false
}

/// zombie.pathfind.VGAStar.canMoveBetween
pub fn can_move_between(
    m: &PathMap,
    mover: &PmMover,
    s1: &Square,
    s2: &Square,
    is_between: bool,
) -> bool {
    !can_not_move_between(m, mover, s1, s2, is_between)
}

/// zombie.pathfind.PolygonalMap2.isBlockedInAllDirections (with the mover of `astar`)
pub fn is_blocked_in_all_directions(m: &PathMap, mover: &PmMover, x: i32, y: i32, z: i32) -> bool {
    let Some(square) = m.get_square(x, y, z) else {
        return false;
    };
    let blocked =
        |sq: Option<&Square>| sq.is_some_and(|s| can_not_move_between(m, mover, square, s, false));
    let blocked_n = blocked(m.get_square(x, y - 1, z));
    let blocked_s = blocked(m.get_square(x, y + 1, z));
    let blocked_w = blocked(m.get_square(x - 1, y, z));
    let blocked_e = blocked(m.get_square(x + 1, y, z));
    blocked_n && blocked_s && blocked_w && blocked_e
}

const SQRT2: f64 = std::f64::consts::SQRT_2;

/// zombie.pathfind.SearchNode
#[derive(Clone, Debug)]
pub struct SearchNode {
    pub square: Option<Square>,
    pub unloaded_x: i32,
    pub unloaded_y: i32,
    pub in_unloaded_area: bool,
    pub parent: Option<usize>,
    g: f64,
    depth: i32,
}

impl SearchNode {
    /// zombie.pathfind.SearchNode.getX
    pub fn get_x(&self) -> f32 {
        match &self.square {
            Some(s) => s.x as f32 + 0.5,
            None => self.unloaded_x as f32,
        }
    }

    /// zombie.pathfind.SearchNode.getY
    pub fn get_y(&self) -> f32 {
        match &self.square {
            Some(s) => s.y as f32 + 0.5,
            None => self.unloaded_y as f32,
        }
    }

    /// zombie.pathfind.SearchNode.getZ
    pub fn get_z(&self) -> f32 {
        match &self.square {
            Some(s) => s.z as f32,
            None => 32.0,
        }
    }

    /// zombie.pathfind.SearchNode.dist
    fn dist(&self, other: &SearchNode, native: bool) -> f64 {
        match (&self.square, &other.square) {
            (Some(a), Some(b)) if (a.x - b.x).abs() <= 1 && (a.y - b.y).abs() <= 1 => {
                if a.x != b.x && a.y != b.y {
                    SQRT2
                } else {
                    1.0
                }
            }
            _ => {
                let (x1, y1, z1) = (self.get_x(), self.get_y(), self.get_z());
                let (x2, y2, z2) = (other.get_x(), other.get_y(), other.get_z());
                if native {
                    // libPZPathFind SearchNode::dist: the whole sum in float, `z * 2.5`, z squared first
                    let (fx, fy) = (x1 - x2, y1 - y2);
                    let fz = (z1 - z2) * 2.5f32;
                    return f64::from((fz * fz + fx * fx + fy * fy).sqrt());
                }
                let dx = f64::from(x1 - x2);
                let dy = f64::from(y1 - y2);
                let dz = f64::from((z1 - z2) * 2.5f32);
                (dx * dx + dy * dy + dz * dz).sqrt()
            }
        }
    }
}

/// zombie.pathfind.VGAStar over the squares of a [`PathMap`]
pub struct VgaStar<'a> {
    pub map: &'a PathMap,
    pub mover: PmMover,
    pub transitions: &'a [Transition],
    pub nodes: Vec<SearchNode>,
    square_to_node: HashMap<(i32, i32, i32), usize>,
    pub goal: usize,
    /// the search of `libPZPathFind` (`a_star::search_native`) instead of `astar.AStar`
    pub native: bool,
}

impl<'a> VgaStar<'a> {
    /// zombie.pathfind.VGAStar.init (no visibility graphs, so no search nodes for graph nodes)
    pub fn new(map: &'a PathMap, mover: PmMover, transitions: &'a [Transition]) -> Self {
        VgaStar {
            map,
            mover,
            transitions,
            nodes: Vec::new(),
            square_to_node: HashMap::new(),
            goal: 0,
            native: false,
        }
    }

    /// zombie.pathfind.VGAStar.getSearchNode (Square)
    pub fn get_search_node(&mut self, square: &Square) -> usize {
        let key = (square.x, square.y, square.z);
        if let Some(&n) = self.square_to_node.get(&key) {
            return n;
        }
        let n = self.nodes.len();
        self.nodes.push(SearchNode {
            square: Some(square.clone()),
            unloaded_x: -1,
            unloaded_y: -1,
            in_unloaded_area: false,
            parent: None,
            g: 0.0,
            depth: 0,
        });
        self.square_to_node.insert(key, n);
        n
    }

    /// zombie.pathfind.VGAStar.getSearchNode (x, y): a node for the unloaded area
    pub fn get_unloaded_search_node(&mut self, x: i32, y: i32) -> usize {
        let n = self.nodes.len();
        self.nodes.push(SearchNode {
            square: None,
            unloaded_x: x,
            unloaded_y: y,
            in_unloaded_area: true,
            parent: None,
            g: 0.0,
            depth: 0,
        });
        n
    }

    /// zombie.pathfind.VGAStar.shortestPath: the nodes of the path, or `None`.
    pub fn shortest_path(
        &mut self,
        request: &PathFindRequest,
        start: usize,
        goal: usize,
    ) -> Result<Option<Vec<usize>>, String> {
        self.mover.set_request(request);
        self.goal = goal;
        let found = if self.native {
            a_star::search_native(self, start, -1)?
        } else {
            a_star::search(self, start, 5000)?
        };
        match found {
            None => Ok(None),
            Some(end) => Ok(Some(a_star::path(self, end)?)),
        }
    }

    /// zombie.pathfind.PMMover.isAllowedChunkLevel
    fn is_allowed_chunk_level(&self, square: &Square) -> bool {
        if self.mover.allowed_chunk_levels.is_empty() {
            return true;
        }
        let Some(chunk) = self.map.get_chunk_from_square_pos(square.x, square.y) else {
            return false;
        };
        self.mover
            .allowed_chunk_levels
            .contains(&(chunk.wx, chunk.wy, square.z))
    }

    /// zombie.pathfind.PMMover.isAllowedLevelTransition (a top floor square)
    fn is_allowed_level_transition(&self, square: &Square) -> bool {
        if self.mover.allowed_level_transitions.is_empty() {
            return true;
        }
        self.mover.allowed_level_transitions.iter().any(|&i| {
            let t = &self.transitions[i];
            t.top_floor_x() == square.x
                && t.top_floor_y() == square.y
                && t.top_floor_z() == square.z
        })
    }

    /// zombie.pathfind.SearchNode.isOnEdgeOfLoadedArea
    fn is_on_edge_of_loaded_area(&self, n: usize) -> bool {
        let node = &self.nodes[n];
        let x = pz_math::fastfloor(node.get_x());
        let y = pz_math::fastfloor(node.get_y());
        let m = self.map;
        let mut edge = false;
        if pz_math::coordmodulo(x, 8) == 0 && m.get_chunk_from_square_pos(x - 1, y).is_none() {
            edge = true;
        }
        if pz_math::coordmodulo(x, 8) == 7 && m.get_chunk_from_square_pos(x + 1, y).is_none() {
            edge = true;
        }
        if pz_math::coordmodulo(y, 8) == 0 && m.get_chunk_from_square_pos(x, y - 1).is_none() {
            edge = true;
        }
        if pz_math::coordmodulo(y, 8) == 7 && m.get_chunk_from_square_pos(x, y + 1).is_none() {
            edge = true;
        }
        edge
    }

    /// `if (!successors.contains(searchNode)) successors.add(searchNode)` of `SearchNode.getSuccessors`
    fn add_unique(&mut self, square: &Square, out: &mut Vec<usize>) {
        let node = self.get_search_node(square);
        if !out.contains(&node) {
            out.push(node);
        }
    }
}

impl AStarNodes for VgaStar<'_> {
    fn g(&self, n: usize) -> f64 {
        self.nodes[n].g
    }

    fn set_g(&mut self, n: usize, g: f64) {
        self.nodes[n].g = g;
    }

    /// zombie.pathfind.SearchNode.h
    fn h(&self, n: usize) -> f64 {
        self.nodes[n].dist(&self.nodes[self.goal], self.native)
    }

    /// zombie.pathfind.SearchNode.c
    fn c(&self, from: usize, to: usize) -> f64 {
        let this = &self.nodes[from];
        let other = &self.nodes[to];
        if other.in_unloaded_area {
            return this.dist(other, self.native);
        }
        let m = self.map;
        let mut add = 0.0f64;
        let crawling_zombie = self.mover.is_zombie() && self.mover.crawling;
        let avoid_windows = !self.mover.is_zombie() || self.mover.crawling;
        if avoid_windows {
            if let (Some(ts), Some(os)) = (&this.square, &other.square) {
                if ts.x == os.x - 1 && ts.y == os.y {
                    if os.has(2048) {
                        add = if !crawling_zombie && os.has(1048576) {
                            20.0
                        } else {
                            200.0
                        };
                    }
                } else if ts.x == os.x + 1 && ts.y == os.y {
                    if ts.has(2048) {
                        add = if !crawling_zombie && ts.has(1048576) {
                            20.0
                        } else {
                            200.0
                        };
                    }
                } else if ts.y == os.y - 1 && ts.x == os.x {
                    if os.has(4096) {
                        add = if !crawling_zombie && os.has(2097152) {
                            20.0
                        } else {
                            200.0
                        };
                    }
                } else if ts.y == os.y + 1 && ts.x == os.x && ts.has(4096) {
                    add = if !crawling_zombie && ts.has(2097152) {
                        20.0
                    } else {
                        200.0
                    };
                }
            }
        }
        if other.square.as_ref().is_some_and(|s| s.has(131072)) {
            add = add.max(20.0);
        }
        // square1/square2: the node's square, or the square under its graph node (there are none)
        if let (Some(s1), Some(s2)) = (&this.square, &other.square) {
            if s1.x == s2.x - 1 && s1.y == s2.y {
                if s2.has(32768) {
                    add = add.max(20.0);
                }
            } else if s1.x == s2.x + 1 && s1.y == s2.y {
                if s1.has(32768) {
                    add = add.max(20.0);
                }
            } else if s1.y == s2.y - 1 && s1.x == s2.x {
                if s2.has(65536) {
                    add = add.max(20.0);
                }
            } else if s1.y == s2.y + 1 && s1.x == s2.x && s1.has(65536) {
                add = add.max(20.0);
            }
            if crawling_zombie {
                if s1.x == s2.x - 1 && s1.y == s2.y {
                    if s2.has(2) && s2.has(8192) {
                        add = add.max(20.0);
                    }
                } else if s1.x == s2.x + 1 && s1.y == s2.y {
                    if s1.has(2) && s1.has(8192) {
                        add = add.max(20.0);
                    }
                } else if s1.y == s2.y - 1 && s1.x == s2.x {
                    if s2.has(4) && s2.has(16384) {
                        add = add.max(20.0);
                    }
                } else if s1.y == s2.y + 1 && s1.x == s2.x && s1.has(4) && s1.has(16384) {
                    add = add.max(20.0);
                }
            }
        }
        let _ = m;
        if let Some(s) = &other.square {
            add += f64::from(s.cost);
        }
        this.dist(other, self.native) + add
    }

    /// zombie.pathfind.SearchNode.getSuccessors
    fn get_successors(&mut self, n: usize, out: &mut Vec<usize>) -> Result<(), String> {
        if self.nodes[self.goal].in_unloaded_area && self.is_on_edge_of_loaded_area(n) {
            out.push(self.goal);
        }
        let Some(sq) = self.nodes[n].square.clone() else {
            return Ok(());
        };
        let m = self.map;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx != 0 || dy != 0 {
                    if let Some(s1) = m.get_square(sq.x + dx, sq.y + dy, sq.z) {
                        if self.is_allowed_chunk_level(s1)
                            && can_move_between(m, &self.mover, &sq, s1, false)
                        {
                            self.add_unique(s1, out);
                        }
                    }
                }
            }
        }
        // `this.square.has(288)`: the vgNode of the stair bottom has the stair flag only inside a visibility graph
        if sq.z > self.mover.min_level {
            let t = |dx: i32, dy: i32, edge: IsoDirections| -> Option<&Square> {
                m.get_square(sq.x + dx, sq.y + dy, sq.z - 1)
                    .filter(|s| s.has_transition_to_level_above(edge))
            };
            for (dx, dy, edge) in [
                (0, 1, IsoDirections::N),
                (0, -1, IsoDirections::S),
                (1, 0, IsoDirections::W),
                (-1, 0, IsoDirections::E),
            ] {
                if let Some(s1) = t(dx, dy, edge) {
                    if self.is_allowed_level_transition(&sq) {
                        self.add_unique(s1, out);
                    }
                }
            }
        }
        if sq.z < self.mover.max_level {
            for (dx, dy, edge) in [
                (0, -1, IsoDirections::N),
                (0, 1, IsoDirections::S),
                (-1, 0, IsoDirections::W),
                (1, 0, IsoDirections::E),
            ] {
                if sq.has_transition_to_level_above(edge) {
                    if let Some(s1) = m.get_square(sq.x + dx, sq.y + dy, sq.z + 1) {
                        if self.is_allowed_level_transition(s1) {
                            self.add_unique(s1, out);
                        }
                    }
                }
            }
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

    /// zombie.pathfind.GoalNode.inGoal
    fn in_goal(&self, n: usize) -> bool {
        n == self.goal
    }
}

/// The mover of `PolygonalMap2.findPath` as the request names it.
pub fn mover_is_zombie(m: &Mover) -> bool {
    matches!(m, Mover::Zombie { .. })
}
