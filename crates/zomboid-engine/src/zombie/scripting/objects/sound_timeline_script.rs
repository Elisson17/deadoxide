//! zombie.scripting.objects.SoundTimelineScript: a `soundTimeline` script (`soundTimeline Event/Name { Marker = 3500, }`),
//! the position of each named marker of an FMOD event's timeline.
use std::collections::HashMap;

use super::base_script_object::{first_child, BaseScriptObject, ScriptObject};
use crate::java;
use crate::zombie::core::math::pz_math;
use crate::zombie::core::translator::Translator;
use crate::zombie::scripting::script_parser;

/// zombie.scripting.objects.SoundTimelineScript
#[derive(Clone, Debug, PartialEq)]
pub struct SoundTimelineScript {
    pub base: BaseScriptObject,
    /// SoundTimelineScript.eventName
    pub event_name: Option<String>,
    /// SoundTimelineScript.positionByName (a HashMap: its order is not observable through the class)
    pub position_by_name: HashMap<String, i32>,
}

impl SoundTimelineScript {
    /// zombie.scripting.objects.SoundTimelineScript()
    pub fn new(module: Option<&str>) -> SoundTimelineScript {
        SoundTimelineScript {
            base: BaseScriptObject::new(module),
            event_name: None,
            position_by_name: HashMap::new(),
        }
    }

    /// zombie.scripting.objects.SoundTimelineScript.getPosition: the marker's position, -1 for an unknown one.
    pub fn get_position(&self, id: &str) -> i32 {
        self.position_by_name.get(id).copied().unwrap_or(-1)
    }
}

impl ScriptObject for SoundTimelineScript {
    fn base(&self) -> &BaseScriptObject {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseScriptObject {
        &mut self.base
    }

    /// zombie.scripting.objects.SoundTimelineScript.reset: `positionByName.clear()`.
    fn reset(&mut self) {
        self.position_by_name.clear();
    }

    /// zombie.scripting.objects.SoundTimelineScript.Load: the event name, the common block, then every value of the
    /// block (`enabled` and `debugOnly` too) is a marker: `positionByName.put(key, PZMath.tryParseInt(value, 0))`.
    fn load(&mut self, name: &str, body: &str, _translator: &Translator) -> Result<(), String> {
        self.event_name = Some(name.to_string());
        let parsed = script_parser::parse(body);
        let block = first_child(&parsed)?;
        self.base.load_common_block(block);
        for value in &block.values {
            let k = java::trim(value.get_key());
            let v = java::trim(value.get_value());
            self.position_by_name
                .insert(k.to_string(), pz_math::try_parse_int(Some(v), 0));
        }
        Ok(())
    }
}
