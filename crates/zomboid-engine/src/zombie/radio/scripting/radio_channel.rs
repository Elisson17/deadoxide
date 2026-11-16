//! zombie.radio.scripting.RadioChannel (and DynamicRadioChannel): a frequency with its scripts, the script it is on and
//! the broadcast it is airing.
use std::collections::HashMap;

use super::radio_broad_cast::{BroadcastId, Broadcasts};
use super::radio_line::RadioLine;
use super::radio_script::RadioScript;
use crate::java::JavaRandom;
use crate::zombie::core::random::rand_abstract::RandAbstract;
use crate::zombie::radio::channel_category::ChannelCategory;

/// What a channel keeps of the `RadioData` it came from (only `isVanilla` is read).
#[derive(Clone, Debug, PartialEq)]
pub struct RadioDataInfo {
    pub is_vanilla: bool,
    pub guid: Option<String>,
}

/// zombie.radio.scripting.RadioChannel. `DynamicRadioChannel` is the same with `LoadAiringBroadcast` doing nothing
/// (`dynamic`).
#[derive(Clone, Debug)]
pub struct RadioChannel {
    pub guid: String,
    pub radio_data: Option<RadioDataInfo>,
    pub is_time_synced: bool,
    /// The scripts by name (a Java `HashMap`: only looked up).
    pub scripts: HashMap<String, RadioScript>,
    pub frequency: i32,
    pub name: String,
    pub is_tv: bool,
    pub category: ChannelCategory,
    pub player_is_listening: bool,
    /// The name of the current script (a key of `scripts`).
    pub current_script: Option<String>,
    pub current_script_loop: i32,
    pub current_script_max_loops: i32,
    pub airing_broadcast: Option<BroadcastId>,
    pub air_counter: f32,
    pub last_aired_line: String,
    pub last_broadcast_id: Option<String>,
    air_counter_multiplier: f32,
    pub louisville_obfuscate: bool,
    pub minmod: f32,
    pub maxmod: f32,
    pub dynamic: bool,
}

/// What `RadioChannel.update` hands to `ZomboidRadio.SendTransmission(0, 0, frequency, text, null, effects, r, g, b, -1,
/// isTV)`. `scramble` is the Louisville obfuscation (`scrambleString(text, 85, true, null)`, whose draws the
/// caller makes).
#[derive(Clone, Debug, PartialEq)]
pub struct Transmission {
    pub frequency: i32,
    pub text: String,
    pub effects: String,
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub is_tv: bool,
    pub scramble: bool,
}

impl RadioChannel {
    /// `RadioChannel(n, freq, c, guid)`
    pub fn new(name: &str, frequency: i32, category: ChannelCategory, guid: &str) -> Self {
        RadioChannel {
            guid: guid.to_string(),
            radio_data: None,
            is_time_synced: false,
            scripts: HashMap::new(),
            frequency,
            name: name.to_string(),
            is_tv: category == ChannelCategory::Television,
            category,
            player_is_listening: false,
            current_script: None,
            current_script_loop: 1,
            current_script_max_loops: 1,
            airing_broadcast: None,
            air_counter: 0.0,
            last_aired_line: String::new(),
            last_broadcast_id: None,
            air_counter_multiplier: 1.0,
            louisville_obfuscate: false,
            minmod: 1.5,
            maxmod: 5.0,
            dynamic: false,
        }
    }

    /// `DynamicRadioChannel(n, freq, c, guid)`
    pub fn new_dynamic(name: &str, frequency: i32, category: ChannelCategory, guid: &str) -> Self {
        let mut c = Self::new(name, frequency, category, guid);
        c.dynamic = true;
        c
    }

    /// `isVanilla`: no data counts as vanilla.
    pub fn is_vanilla(&self) -> bool {
        self.radio_data.as_ref().is_none_or(|d| d.is_vanilla)
    }

    /// `getCurrentScript`
    pub fn current_script(&self) -> Option<&RadioScript> {
        self.current_script.as_ref().and_then(|n| self.scripts.get(n))
    }

    /// `getRadioScript(name)`
    pub fn get_radio_script(&self, name: &str) -> Option<&RadioScript> {
        self.scripts.get(name)
    }

    /// `AddRadioScript`: false (logged) when a script of that name exists.
    pub fn add_radio_script(&mut self, script: RadioScript) -> bool {
        if self.scripts.contains_key(&script.name) {
            false
        } else {
            self.scripts.insert(script.name.clone(), script);
            true
        }
    }

    /// `getAirCounterMultiplier`
    pub fn air_counter_multiplier(&self) -> f32 {
        self.air_counter_multiplier
    }

    /// `setAirCounterMultiplier`: clamped to 0.1..10 (`PZMath.clamp` on floats).
    pub fn set_air_counter_multiplier(&mut self, m: f32) {
        self.air_counter_multiplier = if m < 0.1 {
            0.1
        } else if m > 10.0 {
            10.0
        } else {
            m
        };
    }

    /// `setActiveScriptNull`
    pub fn set_active_script_null(&mut self) {
        self.current_script = None;
        self.airing_broadcast = None;
    }

    /// `LoadAiringBroadcast(guid, line)`: the broadcast of the current script with that id becomes the airing one at
    /// `line` (a negative line only remembers the id); nothing for a dynamic channel or without a current script.
    pub fn load_airing_broadcast(&mut self, guid: &str, line: i32, b: &mut Broadcasts) {
        if self.dynamic {
            return;
        }
        let Some(name) = self.current_script.clone() else {
            return;
        };
        let Some(script) = self.scripts.get_mut(&name) else {
            return;
        };
        self.airing_broadcast = script.get_broadcast_with_id(guid, b);
        if line < 0 {
            self.last_broadcast_id = Some(guid.to_string());
            self.airing_broadcast = None;
        }
        if let Some(a) = self.airing_broadcast {
            if line >= 0 {
                b.reset_line_counter(a, true);
                b.get_mut(a).set_current_line_number(line);
                self.air_counter = 120.0;
                self.player_is_listening = true;
            }
        }
    }

    /// `SetPlayerIsListening`
    pub fn set_player_is_listening(&mut self, listening: bool, b: &mut Broadcasts) {
        self.player_is_listening = listening;
        if self.player_is_listening && self.airing_broadcast.is_none() {
            if let Some(script) = self.current_script.as_ref().and_then(|n| self.scripts.get_mut(n)) {
                self.airing_broadcast = script.get_valid_air_broadcast(b);
                if let Some(a) = self.airing_broadcast {
                    b.reset_line_counter(a, true);
                }
                self.air_counter = 0.0;
            }
        }
    }

    /// `setActiveScript(scriptName, day)`
    pub fn set_active_script<R: JavaRandom>(
        &mut self,
        script_name: Option<&str>,
        day: i32,
        rand: &mut RandAbstract<R>,
    ) {
        self.set_active_script_loops(script_name, day, 1, -1, rand);
    }

    /// `setActiveScript(scriptName, day, loop, maxloops)`: `maxloops` -1 draws it from the script's loop range.
    pub fn set_active_script_loops<R: JavaRandom>(
        &mut self,
        script_name: Option<&str>,
        day: i32,
        loop_: i32,
        mut maxloops: i32,
        rand: &mut RandAbstract<R>,
    ) {
        let Some(name) = script_name else {
            return;
        };
        let Some(script) = self.scripts.get_mut(name) else {
            return;
        };
        self.current_script = Some(name.to_string());
        script.reset();
        script.set_start_day_stamp(day);
        self.current_script_loop = loop_;
        if maxloops == -1 {
            let (min, max) = (script.loop_min, script.loop_max);
            maxloops = if min != max && min <= max {
                rand.next_range(min, max)
            } else {
                min
            };
        }
        self.current_script_max_loops = maxloops;
    }

    /// `getNextScript(day)` (private): another loop of the script, or one of its exit options.
    fn get_next_script<R: JavaRandom>(&mut self, day: i32, rand: &mut RandAbstract<R>) {
        let Some(name) = self.current_script.clone() else {
            return;
        };
        if self.current_script_loop < self.current_script_max_loops {
            self.current_script_loop += 1;
            if let Some(s) = self.scripts.get_mut(&name) {
                s.reset();
                s.set_start_day_stamp(day);
            }
        } else {
            let exit = self.scripts.get(&name).and_then(|s| s.get_next_script(rand));
            self.current_script = None;
            if let Some(e) = exit {
                self.set_active_script(
                    Some(&e.scriptname),
                    day.wrapping_add(e.start_delay),
                    rand,
                );
            }
        }
    }

    /// `UpdateScripts(timestamp, day)`
    pub fn update_scripts<R: JavaRandom>(
        &mut self,
        timestamp: i32,
        day: i32,
        b: &Broadcasts,
        rand: &mut RandAbstract<R>,
    ) {
        self.player_is_listening = false;
        if let Some(name) = self.current_script.clone() {
            let alive = self
                .scripts
                .get_mut(&name)
                .is_some_and(|s| s.update_script(timestamp, b));
            if !alive {
                self.get_next_script(day.wrapping_add(1), rand);
            }
        }
    }

    /// `update()`: counts the air counter down by `1.25 * multiplier` and airs the next line when it is below 0,
    /// returning what the original hands to `SendTransmission` (`None` when broadcasting is disabled or nothing aired).
    pub fn update(
        &mut self,
        multiplier: f32,
        disable_broadcasting: bool,
        louisville_obfuscation: bool,
        b: &mut Broadcasts,
    ) -> Option<Transmission> {
        let a = self.airing_broadcast?;
        self.air_counter -= 1.25 * multiplier;
        if self.air_counter >= 0.0 {
            return None;
        }
        let line: Option<RadioLine> = b.get_next_line(a, true);
        let Some(line) = line else {
            self.last_broadcast_id = Some(b.get(a).id.clone());
            self.airing_broadcast = None;
            self.player_is_listening = false;
            return None;
        };
        self.last_aired_line = line.text.clone();
        let mut sent = None;
        if !disable_broadcasting {
            sent = Some(if self.louisville_obfuscate && louisville_obfuscation {
                Transmission {
                    frequency: self.frequency,
                    text: line.text.clone(),
                    effects: String::new(),
                    r: 0.7,
                    g: 0.5,
                    b: 0.5,
                    is_tv: self.is_tv,
                    scramble: true,
                }
            } else {
                Transmission {
                    frequency: self.frequency,
                    text: line.text.clone(),
                    effects: line.effects.clone(),
                    r: line.r,
                    g: line.g,
                    b: line.b,
                    is_tv: self.is_tv,
                    scramble: false,
                }
            });
        }
        if line.is_custom_air_time() {
            self.air_counter = line.air_time() * 60.0;
        } else {
            self.air_counter = line.text.encode_utf16().count() as f32 / 10.0 * 60.0;
            if self.air_counter < 60.0 * self.minmod {
                self.air_counter = 60.0 * self.minmod;
            } else if self.air_counter > 60.0 * self.maxmod {
                self.air_counter = 60.0 * self.maxmod;
            }
            self.air_counter *= self.air_counter_multiplier;
        }
        sent
    }
}
