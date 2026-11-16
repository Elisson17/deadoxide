//! zombie.core.skinnedmodel.advancedanimation.AnimationVariableSource, with the handle pool and the Bool, String and
//! Float slots it creates (what ActionState's `<variables>` set).
use std::collections::HashMap;

use crate::java;
use crate::zombie::util::string_utils;

/// zombie.core.skinnedmodel.advancedanimation.AnimationVariableHandlePool: the game-wide variable handles, by name
/// ignoring case (the first spelling stays the handle's name), numbered from 1 in allocation order. A static of the
/// original; one value here.
#[derive(Default)]
pub struct AnimationVariableHandlePool {
    by_key: HashMap<String, usize>,
    names: Vec<String>,
}

impl AnimationVariableHandlePool {
    /// zombie.core.skinnedmodel.advancedanimation.AnimationVariableHandlePool.getOrCreate: the handle (index, name) of
    /// the trimmed name; `None` for a blank name.
    pub fn get_or_create(&mut self, name: &str) -> Option<(usize, String)> {
        if string_utils::is_null_or_whitespace(Some(name)) {
            return None;
        }
        let key = java::trim(name);
        let ci = java::case_insensitive_key(key);
        if let Some(&i) = self.by_key.get(&ci) {
            return Some((i, self.names[i - 1].clone()));
        }
        self.names.push(key.to_string());
        let i = self.names.len();
        self.by_key.insert(ci, i);
        Some((i, key.to_string()))
    }
}

/// The value of a slot, by slot class.
#[derive(Clone, Debug, PartialEq)]
pub enum SlotValue {
    /// zombie.core.skinnedmodel.advancedanimation.AnimationVariableSlotBool
    Bool(bool),
    /// zombie.core.skinnedmodel.advancedanimation.AnimationVariableSlotString
    String(Option<String>),
    /// zombie.core.skinnedmodel.advancedanimation.AnimationVariableSlotFloat
    Float(f32),
}

/// zombie.core.skinnedmodel.advancedanimation.IAnimationVariableSlot as the Bool, String and Float slots implement it.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationVariableSlot {
    pub key: String,
    pub value: SlotValue,
    pub read_only: bool,
}

impl AnimationVariableSlot {
    /// IAnimationVariableSlot.getValueString: "true"/"false", the string, or `String.valueOf(float)`.
    pub fn get_value_string(&self) -> Option<String> {
        match &self.value {
            SlotValue::Bool(b) => Some(if *b { "true" } else { "false" }.to_string()),
            SlotValue::String(s) => s.clone(),
            SlotValue::Float(f) => Some(java::float_to_string(*f)),
        }
    }

    /// IAnimationVariableSlot.getType: `AnimationVariableType` name.
    pub fn type_name(&self) -> &'static str {
        match self.value {
            SlotValue::Bool(_) => "Boolean",
            SlotValue::String(_) => "String",
            SlotValue::Float(_) => "Float",
        }
    }

    /// java.lang.Class.getSimpleName of the slot.
    pub fn class_name(&self) -> &'static str {
        match self.value {
            SlotValue::Bool(_) => "AnimationVariableSlotBool",
            SlotValue::String(_) => "AnimationVariableSlotString",
            SlotValue::Float(_) => "AnimationVariableSlotFloat",
        }
    }

    /// setValue(String) of each slot class (ignored when read-only): `StringUtils.tryParseBoolean`, the string,
    /// `PZMath.tryParseFloat(val, 0)`.
    fn set_string(&mut self, val: &str) {
        if self.read_only {
            return;
        }
        self.value = match self.value {
            SlotValue::Bool(_) => SlotValue::Bool(string_utils::try_parse_boolean(Some(val))),
            SlotValue::String(_) => SlotValue::String(Some(val.to_string())),
            SlotValue::Float(_) => {
                SlotValue::Float(if string_utils::is_null_or_whitespace(Some(val)) {
                    0.0
                } else {
                    java::parse_float(java::trim(val)).unwrap_or(0.0)
                })
            }
        };
    }

    /// setValue(boolean) of each slot class (ignored when read-only).
    fn set_bool(&mut self, val: bool) {
        if self.read_only {
            return;
        }
        self.value = match self.value {
            SlotValue::Bool(_) => SlotValue::Bool(val),
            SlotValue::String(_) => {
                SlotValue::String(Some(if val { "true" } else { "false" }.to_string()))
            }
            SlotValue::Float(_) => SlotValue::Float(if val { 1.0 } else { 0.0 }),
        };
    }

    /// setValue(float) of each slot class (ignored when read-only).
    fn set_float(&mut self, val: f32) {
        if self.read_only {
            return;
        }
        self.value = match self.value {
            SlotValue::Bool(_) => SlotValue::Bool(f64::from(val) != 0.0),
            SlotValue::String(_) => SlotValue::String(Some(java::float_to_string(val))),
            SlotValue::Float(_) => SlotValue::Float(val),
        };
    }
}

/// zombie.core.skinnedmodel.advancedanimation.AnimationVariableSource: slots by handle index (`cachedGameVariableSlots`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimationVariableSource {
    slots: Vec<Option<AnimationVariableSlot>>,
}

impl AnimationVariableSource {
    /// AnimationVariableSource.getOrCreateVariable(String, creator): the slot of the key's handle, else a new slot of
    /// the creator's class named after the handle.
    fn get_or_create(
        &mut self,
        pool: &mut AnimationVariableHandlePool,
        key: &str,
        value: SlotValue,
    ) -> Option<&mut AnimationVariableSlot> {
        let (index, name) = pool.get_or_create(key)?;
        if index >= self.slots.len() {
            self.slots.resize(index + 1, None);
        }
        let slot = &mut self.slots[index];
        if slot.is_none() {
            *slot = Some(AnimationVariableSlot {
                key: name,
                value,
                read_only: false,
            });
        }
        slot.as_mut()
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimationVariableSource.setVariable(String, String)
    pub fn set_variable_string(
        &mut self,
        pool: &mut AnimationVariableHandlePool,
        key: &str,
        value: &str,
    ) {
        if let Some(s) = self.get_or_create(pool, key, SlotValue::String(None)) {
            s.set_string(value);
        }
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimationVariableSource.setVariable(String, boolean)
    pub fn set_variable_bool(
        &mut self,
        pool: &mut AnimationVariableHandlePool,
        key: &str,
        value: bool,
    ) {
        if let Some(s) = self.get_or_create(pool, key, SlotValue::Bool(false)) {
            s.set_bool(value);
        }
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimationVariableSource.setVariable(String, float)
    pub fn set_variable_float(
        &mut self,
        pool: &mut AnimationVariableHandlePool,
        key: &str,
        value: f32,
    ) {
        if let Some(s) = self.get_or_create(pool, key, SlotValue::Float(0.0)) {
            s.set_float(value);
        }
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimationVariableSource.getGameVariables: the slots in handle order.
    pub fn get_game_variables(&self) -> impl Iterator<Item = &AnimationVariableSlot> {
        self.slots.iter().flatten()
    }

    /// The slots in handle order, to change (`slot.setReadOnly`).
    pub fn get_game_variables_mut(&mut self) -> impl Iterator<Item = &mut AnimationVariableSlot> {
        self.slots.iter_mut().flatten()
    }
}
