//! zombie.scripting.objects.TimedActionScript: a `timedAction` script, which craft recipes name.
use crate::java;
use crate::zombie::characters::iso_game_character::PERK_IDS;
use crate::zombie::scripting::script_parser;

/// zombie.characters.BodyDamage.Metabolics: the constant names (`valueOf`).
const METABOLICS: [&str; 24] = [
    "Sleeping",
    "SeatedResting",
    "StandingAtRest",
    "SedentaryActivity",
    "Default",
    "DrivingCar",
    "LightDomestic",
    "HeavyDomestic",
    "DefaultExercise",
    "UsingTools",
    "LightWork",
    "MediumWork",
    "DiggingSpade",
    "HeavyWork",
    "ForestryAxe",
    "Walking2kmh",
    "Walking5kmh",
    "Running10kmh",
    "Running15kmh",
    "JumpFence",
    "ClimbRope",
    "Fitness",
    "FitnessHeavy",
    "MAX",
];

/// zombie.scripting.objects.ActionSoundTime: the ids (`fromValue`).
const ACTION_SOUND_TIMES: [&str; 3] = ["action_start", "animation_event", "animation_start"];

/// zombie.scripting.objects.TimedActionScript
#[derive(Clone, Debug, PartialEq)]
pub struct TimedActionScript {
    pub module: String,
    pub name: String,
    pub enabled: bool,
    pub debug_only: bool,
    pub metabolics: &'static str,
    pub time: i32,
    pub face_object: bool,
    pub prop1: Option<String>,
    pub prop2: Option<String>,
    pub action_anim: Option<String>,
    pub anim_var_key: Option<String>,
    pub anim_var_val: Option<String>,
    pub sound: Option<String>,
    pub sound_time: &'static str,
    pub completion_sound: Option<String>,
    pub muscle_strain_factor: f32,
    /// The trimmed names; `BodyPartType.FromString` (MAX for an unknown name) is not resolved here.
    pub muscle_strain_parts: Vec<String>,
    pub muscle_strain_skill: Option<&'static str>,
    pub cant_sit: bool,
}

impl TimedActionScript {
    /// zombie.scripting.objects.TimedActionScript(): the field initialisers.
    pub fn new(module: &str, name: &str) -> Self {
        TimedActionScript {
            module: module.to_string(),
            name: name.to_string(),
            enabled: true,
            debug_only: false,
            metabolics: "Default",
            time: -1,
            face_object: false,
            prop1: None,
            prop2: None,
            action_anim: None,
            anim_var_key: None,
            anim_var_val: None,
            sound: None,
            sound_time: "action_start",
            completion_sound: None,
            muscle_strain_factor: 0.0,
            muscle_strain_parts: Vec::new(),
            muscle_strain_skill: None,
            cant_sit: false,
        }
    }

    /// zombie.scripting.objects.TimedActionScript.Load: `Err` is the exception that makes the bucket drop the script
    /// (`Metabolics.valueOf`, `Integer.parseInt`, `ActionSoundTime.fromValue`, `Float.parseFloat`).
    pub fn load(&mut self, body: &str) -> Result<(), String> {
        let parsed = script_parser::parse(body);
        let block = parsed
            .children
            .first()
            .ok_or("java.lang.IndexOutOfBoundsException: Index 0 out of bounds for length 0")?;
        // BaseScriptObject.LoadCommonBlock
        for value in &block.values {
            let (key, val) = (java::trim(value.get_key()), java::trim(value.get_value()));
            if !key.is_empty() && !val.is_empty() {
                if java::equals_ignore_case(key, "enabled") {
                    self.enabled = java::equals_ignore_case(val, "true");
                } else if java::equals_ignore_case(key, "debugOnly") {
                    self.debug_only = java::equals_ignore_case(val, "true");
                }
            }
        }
        let eq = java::equals_ignore_case;
        for value in &block.values {
            let (key, val) = (java::trim(value.get_key()), java::trim(value.get_value()));
            if key.is_empty() || val.is_empty() {
                continue;
            }
            if eq(key, "metabolics") {
                self.metabolics = METABOLICS.iter().copied().find(|m| *m == val).ok_or_else(|| {
                    format!("java.lang.IllegalArgumentException: No enum constant zombie.characters.BodyDamage.Metabolics.{val}")
                })?;
            } else if eq(key, "time") {
                self.time = java::parse_int(val).ok_or_else(|| {
                    format!(
                        "java.lang.NumberFormatException: {}",
                        java::number_format_message(val)
                    )
                })?;
            } else if eq(key, "faceObject") {
                self.face_object = eq(val, "true");
            } else if eq(key, "cantSit") {
                self.cant_sit = eq(val, "true");
            } else if eq(key, "prop1") {
                self.prop1 = Some(val.to_string());
            } else if eq(key, "prop2") {
                self.prop2 = Some(val.to_string());
            } else if eq(key, "actionAnim") {
                self.action_anim = Some(val.to_string());
            } else if eq(key, "animVarKey") {
                self.anim_var_key = Some(val.to_string());
            } else if eq(key, "animVarVal") {
                self.anim_var_val = Some(val.to_string());
            } else if eq(key, "sound") {
                self.sound = Some(val.to_string());
            } else if eq(key, "soundTime") {
                self.sound_time = ACTION_SOUND_TIMES
                    .iter()
                    .copied()
                    .find(|s| *s == val)
                    .ok_or_else(|| {
                        format!(
                            "java.lang.IllegalArgumentException: No enum constant for value: {val}"
                        )
                    })?;
            } else if eq(key, "completionSound") {
                self.completion_sound = Some(val.to_string());
            } else if eq(key, "muscleStrainFactor") {
                self.muscle_strain_factor = java::parse_float(val).ok_or_else(|| {
                    format!(
                        "java.lang.NumberFormatException: {}",
                        java::number_format_message(val)
                    )
                })?;
            } else if eq(key, "muscleStrainSkill") {
                // PerkFactory.Perks.FromString: an unknown id is MAX, which is only warned about.
                if let Some(p) = PERK_IDS.iter().copied().find(|p| *p == val) {
                    self.muscle_strain_skill = Some(p);
                }
            } else if eq(key, "muscleStrainParts") {
                self.muscle_strain_parts = java::split_char(val, ';')
                    .into_iter()
                    .map(|p| java::trim(p).to_string())
                    .collect();
            }
        }
        Ok(())
    }
}
