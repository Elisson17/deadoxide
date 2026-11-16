//! zombie.scripting.objects.GameSoundScript: a `sound` script (`sound Name { category = ..., clip { ... } }`) and its
//! `GameSound`.
use super::base_script_object::{first_child, BaseScriptObject, ScriptObject};
use crate::java;
use crate::zombie::audio::game_sound::{
    GameSound, GameSoundClip, MasterVolume, INIT_FLAG_DISTANCE_MAX, INIT_FLAG_DISTANCE_MIN,
    INIT_FLAG_STOP_IMMEDIATE,
};
use crate::zombie::core::math::pz_math;
use crate::zombie::core::translator::Translator;
use crate::zombie::scripting::script_parser::{self, Block};

/// zombie.scripting.objects.GameSoundScript
#[derive(Clone, Debug, PartialEq)]
pub struct GameSoundScript {
    pub base: BaseScriptObject,
    /// GameSoundScript.gameSound
    pub game_sound: GameSound,
}

/// `String.split("=")` of a value, `ss[0].trim()` and `ss[1].trim()`: `Err` is the ArrayIndexOutOfBoundsException of
/// a value without text before or after its '=' (the empty pieces at the end are dropped).
fn key_and_value(value: &str) -> Result<(String, String), String> {
    let ss = java::split_char(value, '=');
    let out_of_bounds = |index: usize| {
        format!(
            "java.lang.ArrayIndexOutOfBoundsException: Index {index} out of bounds for length {}",
            ss.len()
        )
    };
    let k = ss.first().ok_or_else(|| out_of_bounds(0))?;
    let k = java::trim(k).to_string();
    let v = ss.get(1).ok_or_else(|| out_of_bounds(1))?;
    Ok((k, java::trim(v).to_string()))
}

/// Integer.parseInt; `Err` is the NumberFormatException.
fn parse_int(s: &str) -> Result<i32, String> {
    java::parse_int(s).ok_or_else(|| {
        format!(
            "java.lang.NumberFormatException: {}",
            java::number_format_message(s)
        )
    })
}

/// Float.parseFloat; `Err` is the NumberFormatException ("empty String" for a blank string).
fn parse_float(s: &str) -> Result<f32, String> {
    java::parse_float(s).ok_or_else(|| {
        format!(
            "java.lang.NumberFormatException: {}",
            if java::trim(s).is_empty() {
                "empty String".to_string()
            } else {
                java::number_format_message(s)
            }
        )
    })
}

impl GameSoundScript {
    /// zombie.scripting.objects.GameSoundScript()
    pub fn new(module: Option<&str>) -> GameSoundScript {
        GameSoundScript {
            base: BaseScriptObject::new(module),
            game_sound: GameSound::default(),
        }
    }

    /// zombie.scripting.objects.GameSoundScript.LoadClip
    fn load_clip(&self, block: &Block) -> Result<GameSoundClip, String> {
        let mut clip = GameSoundClip::new(&self.game_sound);
        for value in &block.values {
            let (k, v) = key_and_value(&value.string)?;
            match k.as_str() {
                "distanceMax" => {
                    clip.distance_max = parse_int(&v)? as f32;
                    clip.init_flags |= INIT_FLAG_DISTANCE_MAX;
                }
                "distanceMin" => {
                    clip.distance_min = parse_int(&v)? as f32;
                    clip.init_flags |= INIT_FLAG_DISTANCE_MIN;
                }
                "event" => clip.event = Some(v),
                "file" => clip.file = Some(v),
                "pitch" => clip.pitch = parse_float(&v)?,
                "volume" => clip.volume = parse_float(&v)?,
                "reverbFactor" => clip.reverb_factor = parse_float(&v)?,
                "reverbMaxRange" => clip.reverb_max_range = parse_float(&v)?,
                "stopImmediate" => clip.init_flags |= INIT_FLAG_STOP_IMMEDIATE,
                _ => {}
            }
        }
        Ok(clip)
    }
}

impl ScriptObject for GameSoundScript {
    fn base(&self) -> &BaseScriptObject {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseScriptObject {
        &mut self.base
    }

    /// zombie.scripting.objects.GameSoundScript.reset: `gameSound.reset()`.
    fn reset(&mut self) {
        self.game_sound.reset();
    }

    /// zombie.scripting.objects.GameSoundScript.Load: the name first, then the common block, the sound's own values
    /// (every value is `split("=")` into key and value, so a value without both throws) and the `clip` children, in
    /// that order; a throw leaves what was read so far in `game_sound`. `Err` is the exception's `toString()`.
    fn load(&mut self, name: &str, body: &str, _translator: &Translator) -> Result<(), String> {
        self.game_sound.name = Some(name.to_string());
        let parsed = script_parser::parse(body);
        let block = first_child(&parsed)?;
        self.base.load_common_block(block);
        for value in &block.values {
            let (k, v) = key_and_value(&value.string)?;
            match k.as_str() {
                "category" => self.game_sound.category = v,
                "is3D" => self.game_sound.is_3d = java::equals_ignore_case("true", &v),
                "loop" => self.game_sound.r#loop = java::equals_ignore_case("true", &v),
                "master" => self.game_sound.master = MasterVolume::value_of(&v)?,
                "maxInstancesPerEmitter" => {
                    self.game_sound.max_instances_per_emitter = pz_math::try_parse_int(Some(&v), -1)
                }
                _ => {}
            }
        }
        for child in &block.children {
            if child.r#type == "clip" {
                let clip = self.load_clip(child)?;
                self.game_sound.clips.push(clip);
            }
        }
        Ok(())
    }
}
