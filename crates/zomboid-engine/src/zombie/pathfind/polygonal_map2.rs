//! zombie.pathfind.PolygonalMap2, the Java (non-native) path search over the squares of the chunks that were added to
//! it: `addRequest`, `updateThread` (queues, rebuild, two requests per update), `findPathHighLevelThenLowLevel`,
//! `findPath`, `cleanPath`, `smoothPath`, `fixPathZ`. Without vehicles there are no visibility graphs (no obstacle
//! polygons, no clusters, no graph nodes), so a search node is a square or the "unloaded area" goal. NOT PORTED:
//! vehicles, known blocked edges, animals, the render hooks, the native (libPZPathFind) search.
use std::collections::VecDeque;

use super::highlevel::{HlAStar, HlResult, HlStore};
use super::line_clear_collide::is_not_clear;
use super::path::{Mover, Path, PathFindRequest, PathNode, PmMover};
use super::path_map::PathMap;
use super::vga_star::{is_blocked_in_all_directions, VgaStar};
use crate::zombie::core::math::pz_math;

/// zombie.pathfind.RequestQueue
#[derive(Default)]
struct RequestQueue {
    player_q: VecDeque<PathFindRequest>,
    aggro_zombie_q: VecDeque<PathFindRequest>,
    other_q: VecDeque<PathFindRequest>,
}

impl RequestQueue {
    fn is_empty(&self) -> bool {
        self.player_q.is_empty() && self.aggro_zombie_q.is_empty() && self.other_q.is_empty()
    }

    /// zombie.pathfind.RequestQueue.removeFirst
    fn remove_first(&mut self) -> Option<PathFindRequest> {
        if !self.player_q.is_empty() {
            self.player_q.pop_front()
        } else if !self.aggro_zombie_q.is_empty() {
            self.aggro_zombie_q.pop_front()
        } else {
            self.other_q.pop_front()
        }
    }
}

/// `IsoUtils.DistanceToSquared(fromX, fromY, toX, toY)`: `(float)(Math.pow(dx, 2.0) + Math.pow(dy, 2.0))`
fn distance_to_squared(from_x: f32, from_y: f32, to_x: f32, to_y: f32) -> f32 {
    let dx = f64::from(to_x - from_x);
    let dy = f64::from(to_y - from_y);
    (dx * dx + dy * dy) as f32
}

/// zombie.pathfind.PolygonalMap2 without vehicles.
pub struct PolygonalMap2 {
    pub map: PathMap,
    store: HlStore,
    /// `PolygonalMap2.rebuild`: a chunk task ran, the level data (`HLAStar.modificationCount`) is stale.
    rebuild: bool,
    store_stale: bool,
    /// `astar.mover`
    mover: PmMover,
    requests: RequestQueue,
    pub request_to_main: Vec<PathFindRequest>,
    request_task_queue: Vec<PathFindRequest>,
    next_id: u64,
    /// `DebugOptions.instance.pathfindSmoothPlayerPath` (default true)
    pub smooth_player_path: bool,
    hl_path: Path,
    shortest_path: Path,
    /// the search of `libPZPathFind` (`PathfindNative.useNativeCode`, the game's default) instead of the Java one
    pub native: bool,
}

impl PolygonalMap2 {
    /// zombie.pathfind.PolygonalMap2.init: the cells of the meta grid (`min_x`, `min_y`, `width`, `height` in cells).
    pub fn new(min_x: i32, min_y: i32, width: i32, height: i32) -> Self {
        PolygonalMap2 {
            map: PathMap::new(min_x, min_y, width, height),
            store: HlStore::default(),
            rebuild: true,
            store_stale: true,
            mover: PmMover::default(),
            requests: RequestQueue::default(),
            request_to_main: Vec::new(),
            request_task_queue: Vec::new(),
            next_id: 1,
            smooth_player_path: true,
            hl_path: Path::default(),
            shortest_path: Path::default(),
            native: false,
        }
    }

    /// The chunk task queue of `updateThread` ran: what follows is a rebuild (`HLAStar.modificationCount++`).
    pub fn mark_rebuild(&mut self) {
        self.rebuild = true;
    }

    /// zombie.pathfind.PolygonalMap2.addRequest: the request waits in the request task queue until the next
    /// `updateThread`. Returns the id of the request.
    #[allow(clippy::too_many_arguments)]
    pub fn add_request(
        &mut self,
        mover: Mover,
        start_x: f32,
        start_y: f32,
        start_z: f32,
        target_x: f32,
        target_y: f32,
        target_z: f32,
        target_xyz: &[f32],
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let mut request = PathFindRequest::init(
            id, mover, start_x, start_y, start_z, target_x, target_y, target_z,
        );
        request.target_xyz = target_xyz.to_vec();
        self.request_task_queue.push(request);
        id
    }

    /// zombie.pathfind.PolygonalMap2.cancelRequest: a request that has not been answered yet is dropped when its turn
    /// comes (`request.cancel`); returns whether there was one.
    pub fn cancel_request(&mut self, id: u64) -> bool {
        let mut found = false;
        let mut cancel = |r: &mut PathFindRequest| {
            if r.id == id {
                r.cancel = true;
                found = true;
            }
        };
        self.request_task_queue.iter_mut().for_each(&mut cancel);
        self.requests.player_q.iter_mut().for_each(&mut cancel);
        self.requests
            .aggro_zombie_q
            .iter_mut()
            .for_each(&mut cancel);
        self.requests.other_q.iter_mut().for_each(&mut cancel);
        found
    }

    /// zombie.pathfind.PolygonalMap2.updateMain: the answered requests, in the order they were answered.
    pub fn update_main(&mut self) -> Vec<PathFindRequest> {
        std::mem::take(&mut self.request_to_main)
    }

    /// zombie.pathfind.PolygonalMap2.updateThread (after the chunk and square tasks, which the caller applied to
    /// `map`): the request tasks sort the requests into the queues, the level data is rebuilt after a chunk task, and
    /// two requests are answered.
    pub fn update_thread(&mut self) {
        let tasks = std::mem::take(&mut self.request_task_queue);
        for request in tasks {
            // zombie.pathfind.PathRequestTask.execute
            match request.mover {
                Mover::Player { .. } => self.requests.player_q.push_back(request),
                Mover::Zombie {
                    has_target: true, ..
                } => self.requests.aggro_zombie_q.push_back(request),
                Mover::Zombie { .. } => self.requests.other_q.push_back(request),
            }
        }
        if self.rebuild {
            self.rebuild = false;
            self.store_stale = true;
        }
        let mut requests_per_update = 2;
        while !self.requests.is_empty() {
            let Some(mut request) = self.requests.remove_first() else {
                break;
            };
            if request.cancel {
                self.request_to_main.push(request);
                continue;
            }
            // the original logs an exception and goes on with whatever the request holds
            let _ = self.find_path_high_level_then_low_level(&mut request);
            if !request.target_xyz.is_empty() {
                self.shortest_path.copy_from(&request.path);
                let mut target_x = request.target_x;
                let mut target_y = request.target_y;
                let mut target_z = request.target_z;
                let mut min_length = if self.shortest_path.is_empty() {
                    f32::MAX
                } else {
                    self.shortest_path.length()
                };
                let mut i = 0;
                while i < request.target_xyz.len() {
                    request.target_x = request.target_xyz[i];
                    request.target_y = request.target_xyz[i + 1];
                    request.target_z = request.target_xyz[i + 2];
                    request.path.clear();
                    let _ = self.find_path_high_level_then_low_level(&mut request);
                    if !request.path.is_empty() {
                        let length = request.path.length();
                        if length < min_length {
                            min_length = length;
                            self.shortest_path.copy_from(&request.path);
                            target_x = request.target_x;
                            target_y = request.target_y;
                            target_z = request.target_z;
                        }
                    }
                    i += 3;
                }
                request.path.copy_from(&self.shortest_path);
                request.target_x = target_x;
                request.target_y = target_y;
                request.target_z = target_z;
            }
            self.request_to_main.push(request);
            requests_per_update -= 1;
            if requests_per_update == 0 {
                break;
            }
        }
    }

    /// Whether `update_thread` still has work (`PMThread.shouldWait` is false).
    pub fn has_work(&self) -> bool {
        !self.request_task_queue.is_empty() || !self.requests.is_empty()
    }

    /// The high-level data of every chunk level (what `HLChunkLevel.getHighLevelData` / `initStairsIfNeeded` give),
    /// rebuilt first when a chunk task made it stale.
    pub fn high_level_store(&mut self) -> &HlStore {
        self.refresh_store();
        &self.store
    }

    /// The high-level data as of the last `high_level_store` / search.
    pub fn store(&self) -> &HlStore {
        &self.store
    }

    fn refresh_store(&mut self) {
        if self.store_stale {
            self.store.rebuild(&self.map);
            self.store_stale = false;
        }
    }

    /// zombie.pathfind.PolygonalMap2.findPathHighLevelThenLowLevel: the high-level search picks the chunk levels and
    /// staircases, then `findPath` runs between them. `Ok(false)`: no path (`request.path` empty).
    pub fn find_path_high_level_then_low_level(
        &mut self,
        request: &mut PathFindRequest,
    ) -> Result<bool, String> {
        self.refresh_store();
        self.mover.set_request(request);
        let mut hl = HlResult::default();
        {
            let mut astar = HlAStar::new(&self.map, &self.store, &self.mover);
            astar.native = self.native;
            astar.find_path(
                request.start_x,
                request.start_y,
                pz_math::fastfloor(request.start_z + 32.0),
                request.target_x,
                request.target_y,
                pz_math::fastfloor(request.target_z + 32.0),
                &mut hl,
            )?;
        }
        if hl.chunk_levels.is_empty() {
            return Ok(false);
        }
        request.allowed_chunk_levels = hl.chunk_levels.clone();
        request.allowed_level_transitions = hl.level_transitions.clone();
        self.mover.allowed_chunk_levels = hl.chunk_levels.clone();
        self.mover.allowed_level_transitions = hl.level_transitions.clone();
        if hl.level_transitions.is_empty() {
            request.min_level = pz_math::fastfloor(request.start_z + 32.0);
            request.max_level = pz_math::fastfloor(request.target_z + 32.0);
            return self.find_path(request);
        }
        self.hl_path.clear();
        let mut start_x = request.start_x;
        let mut start_y = request.start_y;
        let mut start_z = request.start_z;
        let target_x = request.target_x;
        let target_y = request.target_y;
        let target_z = request.target_z;
        let b_target_on_stair = HlAStar::get_level_transition_at(
            &self.map,
            &self.store,
            pz_math::fastfloor(target_x),
            pz_math::fastfloor(target_y),
            pz_math::fastfloor(target_z + 32.0),
        )
        .is_some();
        let count = hl.level_transitions.len();
        for i in 0..count {
            let transition = self.store.transitions[hl.level_transitions[i]];
            let b_last_transition = i == count - 1;
            if b_target_on_stair && b_last_transition {
                break;
            }
            let b_going_down = hl
                .bottom_of_level_transition
                .get(i)
                .copied()
                .ok_or("bottomOfLevelTransition")?;
            request.start_x = start_x;
            request.start_y = start_y;
            request.start_z = start_z;
            request.target_x = (if b_going_down {
                transition.bottom_floor_x()
            } else {
                transition.top_floor_x()
            }) as f32
                + 0.5;
            request.target_y = (if b_going_down {
                transition.bottom_floor_y()
            } else {
                transition.top_floor_y()
            }) as f32
                + 0.5;
            request.target_z = (if b_going_down {
                transition.bottom_floor_z()
            } else {
                transition.top_floor_z()
            } - 32) as f32;
            request.min_level = transition.bottom_floor_z();
            request.max_level = transition.top_floor_z();
            if !self.find_path(request)? {
                return Ok(false);
            }
            self.add_path_segment_nodes(&request.path, true);
            request.path.nodes.clear();
            start_x = request.target_x;
            start_y = request.target_y;
            start_z = request.target_z;
        }
        request.start_x = start_x;
        request.start_y = start_y;
        request.start_z = start_z;
        request.target_x = target_x;
        request.target_y = target_y;
        request.target_z = target_z;
        request.min_level = pz_math::fastfloor(start_z.min(target_z)) + 32;
        request.max_level = pz_math::fastfloor(start_z.max(target_z)) + 32;
        if self.find_path(request)? {
            self.add_path_segment_nodes(&request.path, true);
            request.path.nodes.clear();
            request
                .path
                .nodes
                .extend(self.hl_path.nodes.iter().copied());
            self.hl_path.nodes.clear();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// zombie.pathfind.PolygonalMap2.addPathSegmentNodes: `src` onto `HLGlobals.path`, skipping a node equal to the
    /// last one once there are two.
    fn add_path_segment_nodes(&mut self, src: &Path, _to_hl_path: bool) {
        for node in &src.nodes {
            let dest = &mut self.hl_path;
            if dest.nodes.len() < 2 || !node.is_approximately_equal(dest.last_node()) {
                dest.nodes.push(*node);
            }
        }
    }

    /// zombie.pathfind.PolygonalMap2.findPath: one low-level search between the squares of `request` (the cache's
    /// z is the game's + 32, `request.start_z`/`target_z` are the game's).
    pub fn find_path(&mut self, request: &mut PathFindRequest) -> Result<bool, String> {
        let request_start_z = request.start_z + 32.0;
        let request_target_z = request.target_z + 32.0;
        let mut flags = 16;
        if !matches!(request.mover, Mover::Zombie { .. }) {
            flags |= 4;
        }
        if pz_math::fastfloor(request_start_z) == pz_math::fastfloor(request_target_z)
            && !is_not_clear(
                &self.map,
                &self.mover,
                request.start_x,
                request.start_y,
                request.target_x,
                request.target_y,
                pz_math::fastfloor(request_start_z),
                flags,
            )
        {
            request
                .path
                .add_node(request.start_x, request.start_y, request.start_z, 0);
            request
                .path
                .add_node(request.target_x, request.target_y, request.target_z, 0);
            return Ok(true);
        }
        let map = &self.map;
        let transitions = &self.store.transitions;
        let mut astar = VgaStar::new(map, self.mover.clone(), transitions);
        astar.native = self.native;
        let start_square = map.get_square(
            pz_math::fastfloor(request.start_x),
            pz_math::fastfloor(request.start_y),
            pz_math::fastfloor(request_start_z),
        );
        let Some(start_square) = start_square.filter(|s| !s.is_really_solid(map)) else {
            return Ok(false);
        };
        let start_node = astar.get_search_node(start_square);
        let goal_node = if map
            .get_chunk_from_square_pos(
                pz_math::fastfloor(request.target_x),
                pz_math::fastfloor(request.target_y),
            )
            .is_none()
        {
            astar.get_unloaded_search_node(
                pz_math::fastfloor(request.target_x),
                pz_math::fastfloor(request.target_y),
            )
        } else {
            let target_square = map.get_square(
                pz_math::fastfloor(request.target_x),
                pz_math::fastfloor(request.target_y),
                pz_math::fastfloor(request_target_z),
            );
            let Some(target_square) = target_square.filter(|s| !s.is_really_solid(map)) else {
                return Ok(false);
            };
            if (pz_math::fastfloor(request.start_x) != pz_math::fastfloor(request.target_x)
                || pz_math::fastfloor(request.start_y) != pz_math::fastfloor(request.target_y)
                || pz_math::fastfloor(request.start_z) != pz_math::fastfloor(request.target_z))
                && is_blocked_in_all_directions(
                    map,
                    &self.mover,
                    pz_math::fastfloor(request.target_x),
                    pz_math::fastfloor(request.target_y),
                    pz_math::fastfloor(request_target_z),
                )
            {
                return Ok(false);
            }
            astar.get_search_node(target_square)
        };
        let Some(path) = astar.shortest_path(request, start_node, goal_node)? else {
            return Ok(false);
        };
        if path.len() == 1 {
            let n = &astar.nodes[start_node];
            request.path.add_node(n.get_x(), n.get_y(), n.get_z(), 0);
            let g = &astar.nodes[goal_node];
            if g.square.as_ref().is_some_and(|s| {
                s.x as f32 + 0.5 != request.target_x && s.y as f32 + 0.5 != request.target_y
            }) {
                request
                    .path
                    .add_node(request.target_x, request.target_y, request_target_z, 0);
            } else {
                request.path.add_node(g.get_x(), g.get_y(), g.get_z(), 0);
            }
            fix_path_z(&mut request.path);
            return Ok(true);
        }
        clean_path(&astar, &path, request, goal_node);
        if self.smooth_player_path && matches!(request.mover, Mover::Player { npc: false }) {
            smooth_path(&self.map, &self.mover, &mut request.path);
        }
        fix_path_z(&mut request.path);
        Ok(true)
    }
}

/// zombie.pathfind.PolygonalMap2.cleanPath: the search nodes onto `request.path`, a straight run of squares in one
/// direction becomes a single leg (unless the player is an NPC), then the goal point for a player.
fn clean_path(astar: &VgaStar, path: &[usize], request: &mut PathFindRequest, goal_node: usize) {
    let b_npc = matches!(request.mover, Mover::Player { npc: true });
    let mut square_prev: Option<&super::path_map::Square> = None;
    let mut dx_old = -123;
    let mut dy_old = -123;
    for i in 0..path.len() {
        let node = &astar.nodes[path[i]];
        let x = node.get_x();
        let y = node.get_y();
        let z = node.get_z();
        let mut flags = 0;
        let square_cur = node.square.as_ref();
        let mut extend_prev = false;
        if let (Some(cur), Some(prev)) = (square_cur, square_prev) {
            if cur.z == prev.z {
                let dx = cur.x - prev.x;
                let dy = cur.y - prev.y;
                if dx == dx_old && dy == dy_old {
                    if request.path.nodes.len() > 1 {
                        extend_prev = !request.path.last_node().has_flag(65536);
                    }
                    if (dx == 0 && dy == -1 && prev.has(16384))
                        || (dx == 0 && dy == 1 && cur.has(16384))
                        || (dx == -1 && dy == 0 && prev.has(8192))
                        || (dx == 1 && dy == 0 && cur.has(8192))
                    {
                        extend_prev = false;
                    }
                } else {
                    dx_old = dx;
                    dy_old = dy;
                }
            } else {
                dy_old = -123;
                dx_old = -123;
            }
        } else {
            dy_old = -123;
            dx_old = -123;
        }
        square_prev = square_cur;
        if b_npc {
            extend_prev = false;
        }
        if extend_prev {
            if let (Some(last), Some(cur)) = (request.path.nodes.last_mut(), square_cur) {
                last.x = cur.x as f32 + 0.5;
                last.y = cur.y as f32 + 0.5;
            }
        } else {
            if request.path.nodes.len() > 1 {
                if let Some(last) = request.path.nodes.last_mut() {
                    if last.is_approximately_equal_xyz(x, y, z) {
                        last.x = x;
                        last.y = y;
                        last.z = z;
                        continue;
                    }
                }
            }
            if i > 0 {
                if let Some(cur) = node.square.as_ref() {
                    let node_prev = &astar.nodes[path[i - 1]];
                    if let Some(prev) = node_prev.square.as_ref() {
                        let dx = cur.x - prev.x;
                        let dy = cur.y - prev.y;
                        if (dx == 0 && dy == -1 && prev.has(16384))
                            || (dx == 0 && dy == 1 && cur.has(16384))
                            || (dx == -1 && dy == 0 && prev.has(8192))
                            || (dx == 1 && dy == 0 && cur.has(8192))
                        {
                            flags |= 65536;
                        }
                    }
                }
            }
            request.path.add_node(x, y, z, flags);
        }
    }
    if matches!(request.mover, Mover::Player { .. }) && !b_npc {
        let goal = &astar.nodes[goal_node];
        if goal.square.as_ref().is_some_and(|s| {
            distance_to_squared(
                s.x as f32 + 0.5,
                s.y as f32 + 0.5,
                request.target_x,
                request.target_y,
            ) > 0.010000001f32
        }) {
            request.path.add_node(
                request.target_x,
                request.target_y,
                request.target_z + 32.0,
                0,
            );
        }
    }
    let count = request.path.nodes.len();
    let mut prev_flags: Option<i32> = None;
    for i in 0..count {
        let next_flags = if i < count - 1 {
            Some(request.path.nodes[i + 1].flags)
        } else {
            None
        };
        let node = &mut request.path.nodes[i];
        if node.has_flag(1) {
            let crawl =
                prev_flags.is_some_and(|f| f & 2 != 0) || next_flags.is_some_and(|f| f & 2 != 0);
            if !crawl {
                node.flags &= -4;
            }
        }
        prev_flags = Some(node.flags);
    }
}

/// zombie.pathfind.PolygonalMap2.smoothPath: drop a node when the segment from the one before it to the one after it
/// is clear on its level.
fn smooth_path(map: &PathMap, mover: &PmMover, path: &mut Path) {
    let mut index = 0;
    while index + 2 < path.nodes.len() {
        let node1: PathNode = path.nodes[index];
        let node2: PathNode = path.nodes[index + 1];
        let node3: PathNode = path.nodes[index + 2];
        if pz_math::fastfloor(node1.z) == pz_math::fastfloor(node2.z)
            && pz_math::fastfloor(node1.z) == pz_math::fastfloor(node3.z)
        {
            if !is_not_clear(
                map,
                mover,
                node1.x,
                node1.y,
                node3.x,
                node3.y,
                pz_math::fastfloor(node1.z),
                20,
            ) {
                path.nodes.remove(index + 1);
            } else {
                index += 1;
            }
        } else {
            index += 1;
        }
    }
}

/// zombie.pathfind.PolygonalMap2.fixPathZ: the cache's z (+32) back to the game's.
fn fix_path_z(path: &mut Path) {
    for node in &mut path.nodes {
        node.z -= 32.0;
    }
}
