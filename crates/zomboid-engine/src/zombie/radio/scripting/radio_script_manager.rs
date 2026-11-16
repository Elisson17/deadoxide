//! zombie.radio.scripting.RadioScriptManager: the channels of the radio by frequency, in the order they were added
//! (a `LinkedHashMap`), the clock the scripts run on, and the `RADIO_SAVE.txt` lines of the channels.
use super::radio_broad_cast::Broadcasts;
use super::radio_channel::{RadioChannel, Transmission};
use crate::java;
use crate::java::JavaRandom;
use crate::zombie::core::random::rand_abstract::RandAbstract;
use crate::zombie::radio::parse_int_java;
use crate::zombie::radio::zomboid_radio::ChannelNames;

/// zombie.radio.scripting.RadioScriptManager. `channels` keeps insertion order; a second `put` of a frequency replaces
/// the value in place, as `LinkedHashMap.put` does. Every broadcast of every loaded script lives in `broadcasts`.
#[derive(Clone, Debug, Default)]
pub struct RadioScriptManager {
    pub channels: Vec<(i32, RadioChannel)>,
    pub broadcasts: Broadcasts,
    pub current_time_stamp: i32,
}

impl RadioScriptManager {
    /// `channels.get(frequency)`
    pub fn channel(&self, frequency: i32) -> Option<&RadioChannel> {
        self.channels
            .iter()
            .find(|(f, _)| *f == frequency)
            .map(|(_, c)| c)
    }

    /// `channels.get(frequency)`, mutable
    pub fn channel_mut(&mut self, frequency: i32) -> Option<&mut RadioChannel> {
        self.channels
            .iter_mut()
            .find(|(f, _)| *f == frequency)
            .map(|(_, c)| c)
    }

    /// `getRadioChannel(uuid)`: the first channel (in order) with that guid.
    pub fn get_radio_channel(&self, uuid: &str) -> Option<&RadioChannel> {
        self.channels
            .iter()
            .map(|(_, c)| c)
            .find(|c| c.guid == uuid)
    }

    /// `simulateScriptsUntil(days, force)`
    pub fn simulate_scripts_until<R: JavaRandom>(
        &mut self,
        days: i32,
        force: bool,
        rand: &mut RandAbstract<R>,
    ) {
        let freqs: Vec<i32> = self.channels.iter().map(|(_, c)| c.frequency).collect();
        for f in freqs {
            self.simulate_channel_until(f, days, force, rand);
        }
    }

    /// `simulateChannelUntil(frequency, days, force)`: runs the channel's scripts over `days` days of its own time
    /// (note the original passes the clock as the time stamp and the day number in minutes as the day).
    pub fn simulate_channel_until<R: JavaRandom>(
        &mut self,
        frequency: i32,
        days: i32,
        force: bool,
        rand: &mut RandAbstract<R>,
    ) {
        let current = self.current_time_stamp;
        let Some(idx) = self.channels.iter().position(|(f, _)| *f == frequency) else {
            return;
        };
        if self.channels[idx].1.is_time_synced && !force {
            return;
        }
        for i in 0..days {
            let time_stamp = i.wrapping_mul(24).wrapping_mul(60);
            let chan = &mut self.channels[idx].1;
            chan.update_scripts(current, time_stamp, &self.broadcasts, rand);
        }
        self.channels[idx].1.is_time_synced = true;
    }

    /// `PlayerListensChannel(chanfrequency, mode, sourceIsTV)`
    pub fn player_listens_channel(&mut self, frequency: i32, mode: bool, source_is_tv: bool) {
        let Some(idx) = self.channels.iter().position(|(f, _)| *f == frequency) else {
            return;
        };
        if self.channels[idx].1.is_tv == source_is_tv {
            self.channels[idx]
                .1
                .set_player_is_listening(mode, &mut self.broadcasts);
        }
    }

    /// `AddChannel(channel, overwrite)`: false (the original logs) for an existing frequency without `overwrite`.
    /// The channel's name is added to the radio's name tables under its category.
    pub fn add_channel(
        &mut self,
        channel: RadioChannel,
        overwrite: bool,
        names: &mut ChannelNames,
    ) -> bool {
        let freq = channel.frequency;
        if !overwrite && self.channels.iter().any(|(f, _)| *f == freq) {
            return false;
        }
        let name = channel.name.clone();
        let category = channel.category.name();
        if let Some(slot) = self.channels.iter_mut().find(|(f, _)| *f == freq) {
            slot.1 = channel;
        } else {
            self.channels.push((freq, channel));
        }
        names.add_channel_name(&name, freq, category, overwrite);
        true
    }

    /// `RemoveChannel(frequency)`
    pub fn remove_channel(&mut self, frequency: i32, names: &mut ChannelNames) {
        if let Some(i) = self.channels.iter().position(|(f, _)| *f == frequency) {
            self.channels.remove(i);
            names.remove_channel_name(frequency);
        }
    }

    /// `UpdateScripts(day, hour, mins)`
    pub fn update_scripts<R: JavaRandom>(
        &mut self,
        day: i32,
        hour: i32,
        mins: i32,
        rand: &mut RandAbstract<R>,
    ) {
        self.current_time_stamp = day
            .wrapping_mul(24)
            .wrapping_mul(60)
            .wrapping_add(hour.wrapping_mul(60))
            .wrapping_add(mins);
        let ts = self.current_time_stamp;
        for (_, c) in &mut self.channels {
            c.update_scripts(ts, day, &self.broadcasts, rand);
        }
    }

    /// `update()`: every channel's `update()` in order; what they transmit is returned in that order.
    pub fn update(
        &mut self,
        multiplier: f32,
        disable_broadcasting: bool,
        louisville_obfuscation: bool,
    ) -> Vec<Transmission> {
        let mut out = Vec::new();
        for (_, c) in &mut self.channels {
            if let Some(t) = c.update(
                multiplier,
                disable_broadcasting,
                louisville_obfuscation,
                &mut self.broadcasts,
            ) {
                out.push(t);
            }
        }
        out
    }

    /// `Save(Writer)`: one line per channel, `freq,loop,maxloops,script,startDay,broadcastId,lineNumber`, each ended
    /// with `line_separator` (`System.lineSeparator()`).
    pub fn save(&self, line_separator: &str) -> String {
        let mut w = String::new();
        for (key, c) in &self.channels {
            let cs = c.current_script();
            let airing = c.airing_broadcast.map(|a| self.broadcasts.get(a));
            let mut bcid = match airing {
                Some(b) => b.id.clone(),
                None => "none".to_string(),
            };
            if airing.is_none() {
                if let Some(l) = &c.last_broadcast_id {
                    bcid = l.clone();
                }
            }
            let parts = [
                key.to_string(),
                c.current_script_loop.to_string(),
                c.current_script_max_loops.to_string(),
                cs.map_or("none".to_string(), |s| s.name.clone()),
                cs.map_or("-1".to_string(), |s| s.start_day.to_string()),
                bcid,
                airing.map_or("-1".to_string(), |b| b.current_line_number().to_string()),
            ];
            w.push_str(&parts.join(","));
            w.push_str(line_separator);
        }
        w
    }

    /// `Load(channelLines)`: applies each line to its channel. An exception (a number that does not parse, a line with
    /// no first field) ends the load and is returned; what earlier lines did stays.
    pub fn load<R: JavaRandom>(
        &mut self,
        channel_lines: &[String],
        rand: &mut RandAbstract<R>,
    ) -> Result<(), String> {
        for line in channel_lines {
            if line.chars().all(java::is_whitespace) {
                continue;
            }
            let line = java::trim(line);
            let s = java::split_char(line, ',');
            let first = s.first().ok_or_else(|| {
                "java.lang.ArrayIndexOutOfBoundsException: Index 0 out of bounds for length 0"
                    .to_string()
            })?;
            let freq = parse_int_java(first)?;
            let Some(idx) = self.channels.iter().position(|(f, _)| *f == freq) else {
                continue;
            };
            self.channels[idx].1.is_time_synced = true;
            if s.len() == 7 {
                if s[3] != "none" {
                    let day = parse_int_java(s[4])?;
                    let lp = parse_int_java(s[1])?;
                    let max = parse_int_java(s[2])?;
                    self.channels[idx]
                        .1
                        .set_active_script_loops(Some(s[3]), day, lp, max, rand);
                }
                if s[5] != "none" {
                    let n = parse_int_java(s[6])?;
                    let ch = &mut self.channels[idx].1;
                    ch.load_airing_broadcast(s[5], n, &mut self.broadcasts);
                }
            }
        }
        Ok(())
    }
}
