//! zombie.core.skinnedmodel.advancedanimation.IAnimationVariableSource
use super::animation_variable_source::{AnimationVariableSlot, AnimationVariableSource};
use crate::java;

/// zombie.core.skinnedmodel.advancedanimation.IAnimationVariableSource: where conditions read variables from. The slot
/// is returned by value (a snapshot; the original's callback slots evaluate when read).
pub trait IAnimationVariableSource {
    /// getVariable(AnimationVariableHandle) of the handle named `key` (ignoring case, trimmed).
    fn get_variable(&self, key: &str) -> Option<AnimationVariableSlot>;

    /// getSubVariableSource: null unless the owner has sub sources.
    fn get_sub_variable_source(&self, _name: &str) -> Option<&dyn IAnimationVariableSource> {
        None
    }

    /// getVariableString: the slot's string (null stays `None`), "" without a slot.
    fn get_variable_string(&self, key: &str) -> Option<String> {
        match self.get_variable(key) {
            Some(s) => s.get_value_string(),
            None => Some(String::new()),
        }
    }

    /// getVariableFloat
    fn get_variable_float(&self, key: &str, default_val: f32) -> f32 {
        self.get_variable(key)
            .map_or(default_val, |s| s.get_value_float())
    }

    /// getVariableBoolean(String)
    fn get_variable_boolean(&self, key: &str) -> bool {
        self.get_variable(key).is_some_and(|s| s.get_value_bool())
    }

    /// isVariable: the variable's string equals `val` ignoring case (null equals nothing).
    fn is_variable(&self, name: &str, val: &str) -> bool {
        self.get_variable_string(name)
            .is_some_and(|v| java::equals_ignore_case(&v, val))
    }
}

impl IAnimationVariableSource for AnimationVariableSource {
    fn get_variable(&self, key: &str) -> Option<AnimationVariableSlot> {
        AnimationVariableSource::get_variable(self, key).cloned()
    }
}
