//! zombie.pathfind.PathFindBehavior2: the part a wandering zombie's `pathToLocation` writes. The path search, the
//! walk along the path and the request to the polygonal map are not ported.

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

    /// zombie.pathfind.PathFindBehavior2.setData
    fn set_data(&mut self, chr: (f32, f32, f32), target_x: f32, target_y: f32, target_z: f32) {
        self.start_x = chr.0;
        self.start_y = chr.1;
        self.start_z = chr.2;
        self.target_x = target_x;
        self.target_y = target_y;
        self.target_z = target_z;
        self.path_index = 0;
        self.stopping = false;
    }
}
