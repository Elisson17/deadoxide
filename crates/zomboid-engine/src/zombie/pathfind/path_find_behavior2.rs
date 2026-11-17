//! zombie.pathfind.PathFindBehavior2: the part a wandering zombie's `pathToLocation` writes, and `update` while the path
//! search is not finished. The path search itself (the polygonal map's thread), the walk along the path and the
//! request to the polygonal map are not ported: a request that was queued never completes headless (measured: a zombie
//! stays in PathFindState with `PathFindProgress.notyetfound` for thousands of ticks, oracle `zombieai`).

/// zombie.pathfind.PathFindBehavior2.Goal
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Goal {
    #[default]
    None,
    Character,
    Location,
    Sound,
}

impl Goal {
    /// `Enum.name()`
    pub fn name(self) -> &'static str {
        match self {
            Goal::None => "None",
            Goal::Character => "Character",
            Goal::Location => "Location",
            Goal::Sound => "Sound",
        }
    }
}

/// zombie.ai.astar.AStarPathFinder.PathFindProgress (`chr.getFinder().progress`, kept here with the behaviour).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PathFindProgress {
    #[default]
    NotRunning,
    NotYetFound,
    Failed,
    Found,
}

/// zombie.pathfind.PathFindBehavior2.BehaviorResult
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BehaviorResult {
    Working,
    Failed,
    Succeeded,
}

/// zombie.pathfind.PathFindBehavior2, the fields `pathToLocation`/`setData` write (field initialisers: `isCancel`
/// true, goal `None`).
#[derive(Clone, Debug, PartialEq)]
pub struct PathFindBehavior2 {
    pub is_cancel: bool,
    pub started_moving: bool,
    pub goal: Goal,
    pub start_x: f32,
    pub start_y: f32,
    pub start_z: f32,
    pub target_x: f32,
    pub target_y: f32,
    pub target_z: f32,
    pub path_index: i32,
    pub stopping: bool,
    /// `chr.getFinder().progress`
    pub finder_progress: PathFindProgress,
    /// `pathNextIsSet` (false until a found path sets its next point)
    pub path_next_is_set: bool,
    /// `walkingOnTheSpot`
    pub walking_on_the_spot: crate::zombie::ai::walking_on_the_spot::WalkingOnTheSpot,
}

impl Default for PathFindBehavior2 {
    fn default() -> Self {
        PathFindBehavior2 {
            is_cancel: true,
            started_moving: false,
            goal: Goal::None,
            start_x: 0.0,
            start_y: 0.0,
            start_z: 0.0,
            target_x: 0.0,
            target_y: 0.0,
            target_z: 0.0,
            path_index: 0,
            stopping: false,
            finder_progress: PathFindProgress::NotRunning,
            path_next_is_set: false,
            walking_on_the_spot: Default::default(),
        }
    }
}

impl PathFindBehavior2 {
    /// zombie.pathfind.PathFindBehavior2.pathToLocation(int, int, int) for a character at (`chr_x`, `chr_y`,
    /// `chr_z`). `setData` also cancels the request in the polygonal map and sets the character's finder progress
    /// to `notrunning`: neither is modelled.
    pub fn path_to_location(&mut self, chr: (f32, f32, f32), x: i32, y: i32, z: i32) {
        self.is_cancel = false;
        self.started_moving = false;
        self.goal = Goal::Location;
        self.set_data(chr, x as f32 + 0.5, y as f32 + 0.5, z as f32);
    }

    /// zombie.pathfind.PathFindBehavior2.update(): the part before a path is found. A not-yet-running request is queued
    /// at the polygonal map (`addRequest`, not modelled: it never completes headless) and the progress becomes
    /// `notyetfound`; a pending one keeps the behaviour `Working`. `Err` for the branches that need a found path.
    pub fn update(&mut self) -> Result<BehaviorResult, String> {
        match self.finder_progress {
            PathFindProgress::NotRunning => {
                self.finder_progress = PathFindProgress::NotYetFound;
                // walkingOnTheSpot.reset(x, y): only read once a path is found
                self.update_while_running_pathfind()?;
                Ok(BehaviorResult::Working)
            }
            PathFindProgress::NotYetFound => {
                self.update_while_running_pathfind()?;
                Ok(BehaviorResult::Working)
            }
            PathFindProgress::Failed => Ok(BehaviorResult::Failed),
            PathFindProgress::Found => Err(
                "PathFindBehavior2.update: walking a found path is not ported (the request never completes headless)"
                    .into(),
            ),
        }
    }

    /// zombie.pathfind.PathFindBehavior2.updateWhileRunningPathfind: `moveToPoint` only once a next point is set.
    fn update_while_running_pathfind(&self) -> Result<(), String> {
        if self.path_next_is_set {
            return Err("PathFindBehavior2.moveToPoint is not ported".into());
        }
        Ok(())
    }

    /// zombie.pathfind.PathFindBehavior2.setData
    fn set_data(&mut self, chr: (f32, f32, f32), target_x: f32, target_y: f32, target_z: f32) {
        self.start_x = chr.0;
        self.start_y = chr.1;
        self.start_z = chr.2;
        self.target_x = target_x;
        self.target_y = target_y;
        self.target_z = target_z;
        self.path_index = 0;
        // PolygonalMap2.cancelRequest(chr); chr.getFinder().progress = notrunning
        self.finder_progress = PathFindProgress::NotRunning;
        self.stopping = false;
    }

    /// PathFindBehavior2.cancel's effect on the request (PathFindState.exit): `cancelRequest`, progress `notrunning`.
    pub fn cancel_request(&mut self) {
        self.finder_progress = PathFindProgress::NotRunning;
    }
}
