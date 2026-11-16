//! zombie.ui.UITransition: a fade's progress. Its clock is the static `currentTimeMS`/`elapsedTimeMS` pair that
//! `UpdateAll` (once a frame, from the wall clock) advances; no frame runs headless, so both stay 0 here.
#[derive(Clone, Debug, PartialEq)]
pub struct UITransition {
    pub duration: f32,
    pub elapsed: f32,
    pub frac: f32,
    pub fade_out: bool,
    pub ignore_update_time: bool,
    pub update_time_ms: i64,
}

/// UITransition.currentTimeMS and elapsedTimeMS before the first UpdateAll.
const CURRENT_TIME_MS: i64 = 0;
const ELAPSED_TIME_MS: i64 = 0;

impl Default for UITransition {
    fn default() -> Self {
        UITransition {
            duration: 100.0,
            elapsed: 0.0,
            frac: 0.0,
            fade_out: false,
            ignore_update_time: false,
            update_time_ms: 0,
        }
    }
}

impl UITransition {
    /// zombie.ui.UITransition.init
    pub fn init(&mut self, duration: f32, fade_out: bool) {
        self.duration = duration.max(1.0);
        if self.frac >= 1.0 {
            self.elapsed = 0.0;
        } else if self.fade_out != fade_out {
            self.elapsed = (1.0 - self.frac) * self.duration;
        } else {
            self.elapsed = self.frac * self.duration;
        }
        self.fade_out = fade_out;
    }

    /// zombie.ui.UITransition.update (with the clock statics before UpdateAll).
    pub fn update(&mut self) {
        if !self.ignore_update_time && self.update_time_ms != 0 {
            let ms_duration = self.duration as i64;
            if self.update_time_ms + ms_duration < CURRENT_TIME_MS {
                self.elapsed = self.duration;
            }
        }
        self.update_time_ms = CURRENT_TIME_MS;
        self.frac = self.elapsed / self.duration;
        self.elapsed = (self.elapsed + ELAPSED_TIME_MS as f32).min(self.duration);
    }

    /// zombie.ui.UITransition.fraction
    pub fn fraction(&self) -> f32 {
        if self.fade_out {
            1.0 - self.frac
        } else {
            self.frac
        }
    }

    /// zombie.ui.UITransition.setFadeIn
    pub fn set_fade_in(&mut self, fade_in: bool) {
        if fade_in {
            if self.fade_out {
                self.init(100.0, false);
            }
        } else if !self.fade_out {
            self.init(200.0, true);
        }
    }
}
