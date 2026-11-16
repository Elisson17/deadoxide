//! zombie.radio.scripting.RadioScript: an ordered list of broadcasts with a day count and the scripts that may follow it.
use super::radio_broad_cast::{BroadcastId, Broadcasts};
use crate::java::JavaRandom;
use crate::zombie::core::random::rand_abstract::RandAbstract;

/// zombie.radio.scripting.RadioScript.ExitOption
#[derive(Clone, Debug, PartialEq)]
pub struct ExitOption {
    pub scriptname: String,
    pub chance: i32,
    pub start_delay: i32,
}

/// zombie.radio.scripting.RadioScript
#[derive(Clone, Debug, PartialEq)]
pub struct RadioScript {
    pub broadcasts: Vec<BroadcastId>,
    pub exit_options: Vec<ExitOption>,
    pub guid: String,
    pub name: String,
    pub start_day: i32,
    pub start_day_stamp: i32,
    pub loop_min: i32,
    pub loop_max: i32,
    internal_stamp: i32,
    pub current_broadcast: Option<BroadcastId>,
    current_has_aired: bool,
}

impl RadioScript {
    /// `RadioScript(n, loopmin, loopmax, guid)`
    pub fn new(name: &str, loop_min: i32, loop_max: i32, guid: &str) -> Self {
        RadioScript {
            broadcasts: Vec::new(),
            exit_options: Vec::new(),
            guid: guid.to_string(),
            name: name.to_string(),
            start_day: 0,
            start_day_stamp: 0,
            loop_min,
            loop_max,
            internal_stamp: 0,
            current_broadcast: None,
            current_has_aired: false,
        }
    }

    /// `clearExitOptions`
    pub fn clear_exit_options(&mut self) {
        self.exit_options.clear();
    }

    /// `setStartDayStamp(day)`
    pub fn set_start_day_stamp(&mut self, day: i32) {
        self.start_day = day;
        self.start_day_stamp = day.wrapping_mul(24).wrapping_mul(60);
    }

    /// `getValidAirBroadcast`: the current broadcast, once, while the script's own time is inside it.
    pub fn get_valid_air_broadcast(&mut self, b: &Broadcasts) -> Option<BroadcastId> {
        if !self.current_has_aired {
            if let Some(c) = self.current_broadcast {
                let bc = b.get(c);
                if self.internal_stamp >= bc.start_stamp && self.internal_stamp < bc.end_stamp {
                    self.current_has_aired = true;
                    return Some(c);
                }
            }
        }
        None
    }

    /// `Reset`
    pub fn reset(&mut self) {
        self.current_broadcast = None;
        self.current_has_aired = false;
    }

    /// `getNextBroadcast` (private): the current broadcast while it has not ended, else the first that has not.
    fn get_next_broadcast(&mut self, b: &Broadcasts) -> Option<BroadcastId> {
        if let Some(c) = self.current_broadcast {
            if b.get(c).end_stamp > self.internal_stamp {
                return Some(c);
            }
        }
        for &id in &self.broadcasts {
            if b.get(id).end_stamp > self.internal_stamp {
                self.current_has_aired = false;
                return Some(id);
            }
        }
        None
    }

    /// `getBroadcastWithID`: makes the broadcast with this id current and already aired.
    pub fn get_broadcast_with_id(&mut self, guid: &str, b: &Broadcasts) -> Option<BroadcastId> {
        for &id in &self.broadcasts {
            if b.get(id).id == guid {
                self.current_broadcast = Some(id);
                self.current_has_aired = true;
                return Some(id);
            }
        }
        None
    }

    /// `UpdateScript(timeStamp)`: true while there is a broadcast left.
    pub fn update_script(&mut self, time_stamp: i32, b: &Broadcasts) -> bool {
        self.internal_stamp = time_stamp.wrapping_sub(self.start_day_stamp);
        self.current_broadcast = self.get_next_broadcast(b);
        self.current_broadcast.is_some()
    }

    /// `getNextScript`: one draw `Rand.Next(100)` picks the exit option whose chance range holds it.
    pub fn get_next_script<R: JavaRandom>(&self, rand: &mut RandAbstract<R>) -> Option<ExitOption> {
        let mut chance_start = 0;
        let randval = rand.next(100);
        for o in &self.exit_options {
            if randval >= chance_start && randval < chance_start + o.chance {
                return Some(o.clone());
            }
            chance_start += o.chance;
        }
        None
    }

    /// `AddBroadcast(broadcast, ignoreTimestamps)`: false (the original logs) for a broadcast with no id (not
    /// modelled: ids are always set) or, without `ignore_timestamps`, a bad or overlapping time range.
    pub fn add_broadcast(&mut self, id: BroadcastId, ignore_timestamps: bool, b: &Broadcasts) -> bool {
        let bc = b.get(id);
        if ignore_timestamps {
            self.broadcasts.push(id);
            true
        } else if bc.start_stamp >= 0 && bc.end_stamp > bc.start_stamp {
            if self.broadcasts.is_empty()
                || b.get(self.broadcasts[self.broadcasts.len() - 1]).end_stamp <= bc.start_stamp
            {
                self.broadcasts.push(id);
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// `AddExitOption`: refused (logged) when the chances would add up past 100.
    pub fn add_exit_option(&mut self, scriptname: &str, chance: i32, start_delay: i32) -> bool {
        let mut totchance = chance;
        for o in &self.exit_options {
            totchance += o.chance;
        }
        if totchance <= 100 {
            self.exit_options.push(ExitOption {
                scriptname: scriptname.to_string(),
                chance,
                start_delay,
            });
            true
        } else {
            false
        }
    }

    /// `internalStamp` (private in the original; the oracle prints it).
    pub fn internal_stamp(&self) -> i32 {
        self.internal_stamp
    }

    /// `currentHasAired` (private in the original; the oracle prints it).
    pub fn current_has_aired(&self) -> bool {
        self.current_has_aired
    }
}
