//! `zombie.pathfind.Path`, `PathNode`, `PMMover`, `MoverType` and `PathFindRequest`.
use crate::zombie::core::math::pz_math;
use crate::zombie::iso::iso_directions::IsoDirections;

/// zombie.pathfind.PathNode
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathNode {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub flags: i32,
}

impl PathNode {
    /// zombie.pathfind.PathNode.hasFlag
    pub fn has_flag(&self, flag: i32) -> bool {
        self.flags & flag != 0
    }

    /// zombie.pathfind.PathNode.isApproximatelyEqual (x, y, z)
    pub fn is_approximately_equal_xyz(&self, x: f32, y: f32, z: f32) -> bool {
        (self.x - x).abs() < 0.01 && (self.y - y).abs() < 0.01 && (self.z - z).abs() < 0.01
    }

    /// zombie.pathfind.PathNode.isApproximatelyEqual (PathNode)
    pub fn is_approximately_equal(&self, other: &PathNode) -> bool {
        self.is_approximately_equal_xyz(other.x, other.y, other.z)
    }
}

/// zombie.pathfind.Path
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    pub nodes: Vec<PathNode>,
}

impl Path {
    /// zombie.pathfind.Path.clear
    pub fn clear(&mut self) {
        self.nodes.clear();
    }

    /// zombie.pathfind.Path.isEmpty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// zombie.pathfind.Path.addNode (x, y, z, flags)
    pub fn add_node(&mut self, x: f32, y: f32, z: f32, flags: i32) {
        self.nodes.push(PathNode { x, y, z, flags });
    }

    /// zombie.pathfind.Path.getLastNode
    pub fn last_node(&self) -> &PathNode {
        &self.nodes[self.nodes.len() - 1]
    }

    /// zombie.pathfind.Path.copyFrom
    pub fn copy_from(&mut self, other: &Path) {
        self.nodes.clear();
        self.nodes.extend_from_slice(&other.nodes);
    }

    /// zombie.pathfind.Path.length: the sum of `IsoUtils.DistanceTo` (x, y, z) of the legs, in float.
    pub fn length(&self) -> f32 {
        let mut dist = 0.0f32;
        for i in 0..self.nodes.len().saturating_sub(1) {
            let (a, b) = (&self.nodes[i], &self.nodes[i + 1]);
            dist += distance_to_xyz(a.x, a.y, a.z, b.x, b.y, b.z);
        }
        dist
    }
}

/// zombie.iso.IsoUtils.DistanceTo (x, y): `(float) Math.sqrt(Math.pow(dx, 2.0) + Math.pow(dy, 2.0))`
pub fn distance_to(from_x: f32, from_y: f32, to_x: f32, to_y: f32) -> f32 {
    let dx = f64::from(to_x - from_x);
    let dy = f64::from(to_y - from_y);
    (dx * dx + dy * dy).sqrt() as f32
}

/// zombie.iso.IsoUtils.DistanceTo (x, y, z)
pub fn distance_to_xyz(
    from_x: f32,
    from_y: f32,
    from_z: f32,
    to_x: f32,
    to_y: f32,
    to_z: f32,
) -> f32 {
    let dx = f64::from(to_x - from_x);
    let dy = f64::from(to_y - from_y);
    let dz = f64::from(to_z - from_z);
    (dx * dx + dy * dy + dz * dz).sqrt() as f32
}

/// zombie.pathfind.MoverType
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoverType {
    Player,
    Zombie,
}

/// The requester of a path (`PathFindRequest.mover`): what the search reads off it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mover {
    /// `IsoPlayer`; `npc` is `isNpc()`.
    Player { npc: bool },
    /// `IsoZombie`: `isCrawling`, `isCanCrawlUnderVehicle`, `isCanWalk`; `has_target` is `target != null` (the
    /// request goes to the aggro queue).
    Zombie {
        crawling: bool,
        can_crawl_under_vehicle: bool,
        can_walk: bool,
        has_target: bool,
    },
}

/// zombie.pathfind.highLevel.HLChunkLevel as the key of the chunk level (`wx`, `wy`, level + 32).
pub type ChunkLevelKey = (i32, i32, i32);

/// zombie.pathfind.PMMover
#[derive(Clone, Debug, Default)]
pub struct PmMover {
    pub kind: Option<MoverType>,
    pub can_crawl: bool,
    pub crawling: bool,
    pub ignore_crawl_cost: bool,
    pub can_thump: bool,
    pub can_climb_fences: bool,
    pub can_climb_tall_fences: bool,
    pub min_level: i32,
    pub max_level: i32,
    pub allowed_chunk_levels: Vec<ChunkLevelKey>,
    /// Indices into the high-level store's level transitions.
    pub allowed_level_transitions: Vec<usize>,
}

impl PmMover {
    /// zombie.pathfind.PMMover.set (PathFindRequest)
    pub fn set_request(&mut self, r: &PathFindRequest) {
        self.crawling = false;
        self.can_climb_fences = false;
        self.can_climb_tall_fences = false;
        match r.mover {
            Mover::Player { .. } => {
                self.kind = Some(MoverType::Player);
                self.can_climb_tall_fences = true;
            }
            Mover::Zombie { crawling, .. } => {
                self.kind = Some(MoverType::Zombie);
                self.crawling = crawling;
            }
        }
        self.can_crawl = r.can_crawl;
        self.ignore_crawl_cost = r.ignore_crawl_cost;
        self.can_thump = r.can_thump;
        self.min_level = r.min_level;
        self.max_level = r.max_level;
        self.allowed_chunk_levels = r.allowed_chunk_levels.clone();
        self.allowed_level_transitions = r.allowed_level_transitions.clone();
    }

    /// zombie.pathfind.PMMover.set (PMMover): `canClimbFences` is not copied.
    pub fn set_mover(&mut self, o: &PmMover) {
        self.kind = o.kind;
        self.can_crawl = o.can_crawl;
        self.crawling = o.crawling;
        self.ignore_crawl_cost = o.ignore_crawl_cost;
        self.can_thump = o.can_thump;
        self.can_climb_tall_fences = o.can_climb_tall_fences;
        self.min_level = o.min_level;
        self.max_level = o.max_level;
        self.allowed_chunk_levels = o.allowed_chunk_levels.clone();
        self.allowed_level_transitions = o.allowed_level_transitions.clone();
    }

    /// zombie.pathfind.PMMover.isPlayer
    pub fn is_player(&self) -> bool {
        self.kind == Some(MoverType::Player)
    }

    /// zombie.pathfind.PMMover.isZombie
    pub fn is_zombie(&self) -> bool {
        self.kind == Some(MoverType::Zombie)
    }
}

/// zombie.pathfind.PathFindRequest
#[derive(Clone, Debug)]
pub struct PathFindRequest {
    pub id: u64,
    pub mover: Mover,
    pub can_crawl: bool,
    pub ignore_crawl_cost: bool,
    pub can_thump: bool,
    pub min_level: i32,
    pub max_level: i32,
    pub allowed_chunk_levels: Vec<ChunkLevelKey>,
    pub allowed_level_transitions: Vec<usize>,
    pub start_x: f32,
    pub start_y: f32,
    pub start_z: f32,
    pub target_x: f32,
    pub target_y: f32,
    pub target_z: f32,
    pub target_xyz: Vec<f32>,
    pub path: Path,
    pub cancel: bool,
}

impl PathFindRequest {
    /// zombie.pathfind.PathFindRequest.init: the known blocked edges of the mover's map knowledge are empty.
    #[allow(clippy::too_many_arguments)]
    pub fn init(
        id: u64,
        mover: Mover,
        start_x: f32,
        start_y: f32,
        start_z: f32,
        target_x: f32,
        target_y: f32,
        target_z: f32,
    ) -> Self {
        let (mut can_crawl, mut ignore_crawl_cost, mut can_thump) = (false, false, false);
        if let Mover::Zombie {
            crawling,
            can_crawl_under_vehicle,
            can_walk,
            ..
        } = mover
        {
            can_crawl = crawling || can_crawl_under_vehicle;
            ignore_crawl_cost = crawling && !can_walk;
            can_thump = true;
        }
        PathFindRequest {
            id,
            mover,
            can_crawl,
            ignore_crawl_cost,
            can_thump,
            min_level: 0,
            max_level: 63,
            allowed_chunk_levels: Vec::new(),
            allowed_level_transitions: Vec::new(),
            start_x,
            start_y,
            start_z,
            target_x,
            target_y,
            target_z,
            target_xyz: Vec::new(),
            path: Path::default(),
            cancel: false,
        }
    }
}

/// `PZMath.fastfloor(PZMath.min(a, b))` and friends are in `pz_math`; this is only the grid-edge direction table the
/// search walks (`IsoDirections.values()`).
pub const DIRECTIONS: [IsoDirections; 8] = [
    IsoDirections::N,
    IsoDirections::NW,
    IsoDirections::W,
    IsoDirections::SW,
    IsoDirections::S,
    IsoDirections::SE,
    IsoDirections::E,
    IsoDirections::NE,
];

/// `(int) Math.ceil(float)`
pub fn ceil_i32(v: f32) -> i32 {
    pz_math::fastfloor(v.ceil())
}
