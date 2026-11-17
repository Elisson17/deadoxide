//! zombie.ai.WalkingOnTheSpot
/// zombie.ai.WalkingOnTheSpot: how long the character has been within 0.1 of one place while walking.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WalkingOnTheSpot {
    pub x: f32,
    pub y: f32,
    pub time: f32,
    pub seconds: f32,
}

impl WalkingOnTheSpot {
    /// zombie.ai.WalkingOnTheSpot.reset(x, y)
    pub fn reset(&mut self, x1: f32, y1: f32) {
        self.x = x1;
        self.y = y1;
        self.time = 0.0;
        self.seconds = 0.0;
    }

    /// zombie.ai.WalkingOnTheSpot.check(x, y): `multiplier` is `GameTime.getMultiplier()` and `thirty_fps` is
    /// `getThirtyFPSMultiplier()`.
    pub fn check(&mut self, x1: f32, y1: f32, multiplier: f32, thirty_fps: f32) -> bool {
        // IsoUtils.DistanceToSquared(fromX, fromY, toX, toY)
        let dx = f64::from(x1 - self.x);
        let dy = f64::from(y1 - self.y);
        let dist_sq = (dx * dx + dy * dy) as f32;
        if dist_sq < 0.010000001f32 {
            self.time += multiplier;
            self.seconds += thirty_fps / 30.0f32;
        } else {
            self.x = x1;
            self.y = y1;
            self.time = 0.0;
            self.seconds = 0.0;
        }
        self.time > 400.0f32
    }
}
