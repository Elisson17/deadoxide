//! zombie.util.FrameDelay: lets an object act on every (`delay` + 1)-th frame.

/// zombie.util.FrameDelay
#[derive(Clone, Copy, Debug)]
pub struct FrameDelay {
    pub delay: i32,
    count: i32,
    delta: f32,
    multiplier: f32,
}

impl FrameDelay {
    /// `new FrameDelay(delay)`
    pub fn new(delay: i32) -> FrameDelay {
        FrameDelay {
            delay,
            count: 0,
            delta: 0.0,
            multiplier: 0.0,
        }
    }

    /// FrameDelay.update: `time_delta` is `GameTime.instance.getTimeDelta()`, `multiplier`
    /// `GameTime.instance.getMultiplier()` and `per_object_multiplier` `GameTime.instance.perObjectMultiplier`. True
    /// when the counter has passed the delay (it restarts then).
    pub fn update(&mut self, time_delta: f32, multiplier: f32, per_object_multiplier: f32) -> bool {
        if self.count == 0 {
            self.delta = 0.0;
            self.multiplier = 0.0;
        }
        self.delta += time_delta;
        self.multiplier += multiplier;
        // Math.round(float): floor(x + 0.5)
        self.count = self
            .count
            .wrapping_add((per_object_multiplier + 0.5).floor() as i32);
        if self.count > self.delay {
            self.count = 0;
            true
        } else {
            false
        }
    }
}
