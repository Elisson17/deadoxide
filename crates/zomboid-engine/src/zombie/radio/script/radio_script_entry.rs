//! zombie.radio.script.RadioScriptEntry: a script name with a delay and a chance range, the numbers clamped.

/// zombie.radio.script.RadioScriptEntry
#[derive(Clone, Debug, PartialEq)]
pub struct RadioScriptEntry {
    chance_min: i32,
    chance_max: i32,
    pub script_name: String,
    delay: i32,
}

impl RadioScriptEntry {
    /// `RadioScriptEntry(name, delay)`: chance 0..100.
    pub fn new(name: &str, delay: i32) -> Self {
        Self::with_chance(name, delay, 0, 100)
    }

    /// `RadioScriptEntry(name, delay, min, max)`
    pub fn with_chance(name: &str, delay: i32, min: i32, max: i32) -> Self {
        let mut e = RadioScriptEntry {
            chance_min: 0,
            chance_max: 100,
            script_name: name.to_string(),
            delay: 0,
        };
        e.set_chance_min(min);
        e.set_chance_max(max);
        e.set_delay(delay);
        e
    }

    pub fn set_chance_min(&mut self, min: i32) {
        self.chance_min = min.clamp(0, 100);
    }

    pub fn chance_min(&self) -> i32 {
        self.chance_min
    }

    pub fn set_chance_max(&mut self, max: i32) {
        self.chance_max = max.clamp(0, 100);
    }

    pub fn chance_max(&self) -> i32 {
        self.chance_max
    }

    /// `setDelay`: negative delays become 0.
    pub fn set_delay(&mut self, delay: i32) {
        self.delay = if delay >= 0 { delay } else { 0 };
    }

    pub fn delay(&self) -> i32 {
        self.delay
    }
}
