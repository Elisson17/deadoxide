//! zombie.characters.Moodles.Moodles: the moodles of a character, updated each tick.
use super::moodle::{Moodle, MoodleCtx};
use crate::zombie::scripting::objects::moodle_type::{MoodleType, ALL_MOODLE_TYPES};

/// zombie.characters.Moodles.Moodles. The original keeps its moodles in a `HashMap` keyed by identity-hashed
/// `MoodleType` objects, so its update order is arbitrary; no moodle reads another's level during `Update`, so the
/// registry order is used.
#[derive(Clone, Debug, PartialEq)]
pub struct Moodles {
    pub moodles_state_changed: bool,
    pub moodles: Vec<Moodle>,
}

impl Default for Moodles {
    fn default() -> Self {
        Self::new()
    }
}

impl Moodles {
    /// `new Moodles(parent)`: a moodle per registered type.
    pub fn new() -> Self {
        Moodles {
            moodles_state_changed: false,
            moodles: ALL_MOODLE_TYPES.iter().map(|t| Moodle::new(*t)).collect(),
        }
    }

    /// `Moodles.getMoodleLevel`
    pub fn get_moodle_level(&self, t: MoodleType) -> i32 {
        self.moodles[t as usize].level
    }

    /// zombie.characters.Moodles.Moodles.Update
    pub fn update(&mut self, c: &mut MoodleCtx) -> Result<(), String> {
        for m in &mut self.moodles {
            if m.update(c)? {
                self.moodles_state_changed = true;
            }
        }
        Ok(())
    }
}
