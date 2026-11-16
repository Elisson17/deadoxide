//! zombie.radio.ZomboidRadio: the radio of a world. Ported: the channel name tables (`addChannelName`,
//! `removeChannelName`, `getRandomFrequency`), `Init` (the vanilla channels, `OnLoadRadioScripts`, the start times of a new
//! game, `checkGameModeSpecificStart`, `Load`), `Save` (the text of `radio/data/RADIO_SAVE.txt`), `UpdateScripts`,
//! `update`, `scrambleString`, `computerize`, `getRandomBzztFzzt`, `ObfuscateChannelCheck` and the transmission part
//! that a world without devices shows (the weather interference).
//!
//! Not ported: the client side (`GameMode.Client`: the server data packet, `PlayerListensChannel` over the network),
//! devices (`WaveSignalDevice` registration and `DistributeTransmission`, which need `DeviceData` with its parent object),
//! the story sounds, `RecordedMedia.init`/`save` called from `Init`/`Save`, the player parts of `update` (the chat line
//! of a speaking player, `LouisvilleObfuscationCheck`) and the network packets (`GameServer.sendRadioPostSilence`,
//! `sendIsoWaveSignal`).
use std::collections::HashMap;
use std::path::Path;

use super::channel_category::GameMode;
use super::parse_int_java;
use super::radio_data::{fetch_all_radio_data, RadioText};
use super::scripting::radio_channel::{RadioChannel, Transmission};
use super::scripting::radio_script_manager::RadioScriptManager;
use crate::java;
use crate::java::JavaRandom;
use crate::java_chars;
use crate::zombie::core::random::rand_abstract::RandAbstract;

/// `ZomboidRadio.staticSounds`
const STATIC_SOUNDS: [&str; 4] = ["<bzzt>", "<fzzt>", "<wzzt>", "<szzt>"];

/// `ZomboidRadio.obfuscateChannels`
const OBFUSCATE_CHANNELS: [i32; 6] = [200, 201, 204, 93200, 98000, 101200];

/// `ZomboidRadio.isStaticSound`
pub fn is_static_sound(s: Option<&str>) -> bool {
    s.is_some_and(|s| STATIC_SOUNDS.contains(&s))
}

/// zombie.iso.weather.ClimateManager.getWeatherInterference from the state of the weather period: 0.35 of its current
/// strength for rain, 0.7 for a thunderstorm, tropical storm or blizzard, 0 when no weather is running.
pub fn weather_interference(
    running: bool,
    strength: f32,
    thunder: bool,
    tropical: bool,
    blizzard: bool,
) -> f32 {
    if running {
        if !thunder && !tropical && !blizzard {
            0.35 * strength
        } else {
            0.7 * strength
        }
    } else {
        0.0
    }
}

/// The name tables of `ZomboidRadio`: `channelNames`, `categorizedChannels` and `knownFrequencies`.
#[derive(Clone, Debug, Default)]
pub struct ChannelNames {
    pub channel_names: HashMap<i32, String>,
    pub categorized_channels: HashMap<String, HashMap<i32, String>>,
    pub known_frequencies: Vec<i32>,
}

impl ChannelNames {
    /// `addChannelName(name, frequency, category, overwrite)`: with `overwrite` false a known frequency keeps its name.
    pub fn add_channel_name(
        &mut self,
        name: &str,
        frequency: i32,
        category: &str,
        overwrite: bool,
    ) {
        if overwrite || !self.channel_names.contains_key(&frequency) {
            self.categorized_channels
                .entry(category.to_string())
                .or_default()
                .insert(frequency, name.to_string());
            self.channel_names.insert(frequency, name.to_string());
            self.known_frequencies.push(frequency);
        }
    }

    /// `removeChannelName(frequency)`
    pub fn remove_channel_name(&mut self, frequency: i32) {
        if self.channel_names.contains_key(&frequency) {
            self.channel_names.remove(&frequency);
            for m in self.categorized_channels.values_mut() {
                m.remove(&frequency);
            }
        }
    }

    /// `GetChannelList(category)`
    pub fn get_channel_list(&self, category: &str) -> Option<&HashMap<i32, String>> {
        self.categorized_channels.get(category)
    }

    /// `getChannelName(frequency)`
    pub fn get_channel_name(&self, frequency: i32) -> Option<&String> {
        self.channel_names.get(&frequency)
    }

    /// `getRandomFrequency(rangemin, rangemax)`: draws `Rand.Next(min, max) / 200 * 200` until the frequency is unknown.
    pub fn get_random_frequency<R: JavaRandom>(
        &self,
        rangemin: i32,
        rangemax: i32,
        rand: &mut RandAbstract<R>,
    ) -> i32 {
        loop {
            let freq = rand.next_range(rangemin, rangemax) / 200 * 200;
            if !self.known_frequencies.contains(&freq) {
                return freq;
            }
        }
    }
}

/// What `Init` needs from the rest of the game (the `GameTime`, `SandboxOptions`, `Core` and save folder it reads).
#[derive(Clone, Debug)]
pub struct InitInputs<'a> {
    /// the `savedWorldVersion` argument: -1 for a new game
    pub saved_world_version: i32,
    /// `SandboxOptions.instance.timeSinceApo.getValue()`
    pub time_since_apo: i32,
    /// `GameTime.instance.getNightsSurvived()`
    pub nights_survived: i32,
    /// `GameTime.instance.getHour()`
    pub hour: i32,
    /// `Core.gameMode`
    pub game_mode: &'a str,
    /// the bytes of `radio/data/RADIO_SAVE.txt`, `None` when the file does not exist
    pub radio_save: Option<&'a [u8]>,
}

/// zombie.radio.ZomboidRadio (the world's radio, and its static flags)
#[derive(Debug)]
pub struct ZomboidRadio {
    pub script_manager: Option<RadioScriptManager>,
    pub days_since_start: i32,
    pub last_recorded_hour: i32,
    pub names: ChannelNames,
    pub game_mode: GameMode,
    /// static `postRadioSilence`
    pub post_radio_silence: bool,
    /// static `disableBroadcasting`
    pub disable_broadcasting: bool,
    /// static `louisvilleObfuscation`
    pub louisville_obfuscation: bool,
    last_save_file: Option<String>,
    last_save_content: Option<String>,
    has_applied_interference: bool,
    /// `ClimateManager.getInstance().getWeatherInterference()`, set by the caller (see `weather_interference`)
    pub weather_interference: f32,
    /// whether `radio/data` exists in the current save (`Init` makes it)
    pub save_dir_exists: bool,
}

/// `System.lineSeparator()` of the platform the oracle runs on.
pub const LINE_SEPARATOR: &str = if cfg!(windows) { "\r\n" } else { "\n" };

const NPE_CURRENT_SCRIPT: &str = "java.lang.NullPointerException: Cannot invoke \"zombie.radio.scripting.RadioScript.GetName()\" because the return value of \"zombie.radio.scripting.RadioChannel.getCurrentScript()\" is null";
const NPE_SET_START: &str = "java.lang.NullPointerException: Cannot invoke \"zombie.radio.scripting.RadioScript.setStartDayStamp(int)\" because the return value of \"zombie.radio.scripting.RadioChannel.getCurrentScript()\" is null";

impl ZomboidRadio {
    /// `new ZomboidRadio()` (what `getInstance()` makes after `Reset()`): `lastRecordedHour` is the game's current hour.
    /// The three static flags are not touched by `Reset`, so the caller passes the old instance's.
    pub fn new(game_mode: GameMode, hour: i32) -> Self {
        ZomboidRadio {
            script_manager: None,
            days_since_start: 0,
            last_recorded_hour: hour,
            names: ChannelNames::default(),
            game_mode,
            post_radio_silence: false,
            disable_broadcasting: false,
            louisville_obfuscation: false,
            last_save_file: None,
            last_save_content: None,
            has_applied_interference: false,
            weather_interference: 0.0,
            save_dir_exists: false,
        }
    }

    /// `getGameMode`
    pub fn get_game_mode(&self) -> GameMode {
        self.game_mode
    }

    /// `AddChannel` of the script manager (which adds the name to this radio's tables)
    pub fn add_channel(&mut self, channel: RadioChannel, overwrite: bool) -> bool {
        match self.script_manager.as_mut() {
            Some(m) => m.add_channel(channel, overwrite, &mut self.names),
            None => false,
        }
    }

    /// `ObfuscateChannelCheck(channel)`
    pub fn obfuscate_channel_check(channel: &mut RadioChannel) {
        if channel.is_vanilla() {
            let freq = channel.frequency;
            for f in OBFUSCATE_CHANNELS {
                if freq == f {
                    channel.louisville_obfuscate = true;
                }
            }
        }
    }

    /// `Init(savedWorldVersion)` on a server or in single player. `on_load_radio_scripts(radio, isNewGame, rand)` stands
    /// for `LuaEventManager.triggerEvent("OnLoadRadioScripts", scriptManager, savedWorldVersion == -1)`: the callers
    /// supply what the registered Lua handlers do. An exception that the original's `Init` catches and logs is returned,
    /// and the state is what it was when it was thrown. `RecordedMedia.init()` and the debug output are not ported.
    pub fn init<R: JavaRandom, T: RadioText>(
        &mut self,
        root: &Path,
        inp: &InitInputs,
        tr: &mut T,
        rand: &mut RandAbstract<R>,
        on_load_radio_scripts: &mut dyn FnMut(&mut ZomboidRadio, bool, &mut RandAbstract<R>),
    ) -> Result<(), String> {
        self.post_radio_silence = false;
        self.last_recorded_hour = inp.hour;
        if self.get_game_mode() == GameMode::Client {
            self.script_manager = None;
            return Ok(());
        }
        if self.script_manager.is_none() {
            self.script_manager = Some(RadioScriptManager::default());
        }
        if self
            .script_manager
            .as_ref()
            .is_some_and(|m| !m.channels.is_empty())
        {
            return Err(
                "ZomboidRadio.Init on a script manager that already has channels is not ported"
                    .to_string(),
            );
        }
        self.save_dir_exists = true;
        let (datas, broadcasts) = fetch_all_radio_data(root, tr, rand)?;
        if let Some(m) = self.script_manager.as_mut() {
            m.broadcasts = broadcasts;
        }
        for rd in &datas {
            for channel in &rd.radio_channels {
                let mut channel = channel.clone();
                Self::obfuscate_channel_check(&mut channel);
                let found = self
                    .script_manager
                    .as_ref()
                    .and_then(|m| m.channel(channel.frequency));
                let taken = match found {
                    Some(f) => {
                        let fv = f
                            .radio_data
                            .as_ref()
                            .ok_or("java.lang.NullPointerException")?
                            .is_vanilla;
                        !fv || channel.radio_data.as_ref().is_some_and(|d| d.is_vanilla)
                    }
                    None => false,
                };
                if !taken {
                    self.add_channel(channel, true);
                }
            }
        }
        on_load_radio_scripts(self, inp.saved_world_version == -1, rand);
        if inp.saved_world_version == -1 {
            let mut months = inp.time_since_apo - 1;
            if months < 0 {
                months = 0;
            }
            if months > 0 {
                self.days_since_start = (months as f32 * 30.5f32) as i32;
                let d = self.days_since_start;
                if let Some(m) = self.script_manager.as_mut() {
                    m.simulate_scripts_until(d, true, rand);
                }
            }
            self.check_game_mode_specific_start(inp.game_mode, rand)?;
        } else {
            let is_loaded = self.load(inp.radio_save, rand);
            if !is_loaded {
                let mut months = inp.time_since_apo - 1;
                if months < 0 {
                    months = 0;
                }
                self.days_since_start = (months as f32 * 30.5f32) as i32;
                self.days_since_start = self.days_since_start.wrapping_add(inp.nights_survived);
            }
            if self.days_since_start > 0 {
                let d = self.days_since_start;
                if let Some(m) = self.script_manager.as_mut() {
                    m.simulate_scripts_until(d, false, rand);
                }
            }
        }
        Ok(())
    }

    /// `checkGameModeSpecificStart` (private)
    fn check_game_mode_specific_start<R: JavaRandom>(
        &mut self,
        game_mode: &str,
        rand: &mut RandAbstract<R>,
    ) -> Result<(), String> {
        let days = self.days_since_start;
        let Some(m) = self.script_manager.as_mut() else {
            return Ok(());
        };
        if game_mode == "Initial Infection" {
            for (_, c) in &mut m.channels {
                if c.scripts.contains_key("init_infection") {
                    let cur = c.current_script().ok_or(NPE_CURRENT_SCRIPT)?.name.clone();
                    if let Some(s) = c.scripts.get_mut("init_infection") {
                        s.clear_exit_options();
                        s.add_exit_option(&cur, 100, 0);
                    }
                    c.set_active_script(Some("init_infection"), days, rand);
                } else {
                    let name = c.current_script.clone().ok_or(NPE_SET_START)?;
                    c.scripts
                        .get_mut(&name)
                        .ok_or(NPE_SET_START)?
                        .set_start_day_stamp(days.wrapping_add(1));
                }
            }
        } else if game_mode == "Six Months Later" {
            for (_, c) in &mut m.channels {
                if c.name == "Classified M1A1" {
                    c.set_active_script(Some("numbers"), days, rand);
                } else if c.name == "NNR Radio" {
                    c.set_active_script(Some("pastor"), days, rand);
                }
            }
        }
        Ok(())
    }

    /// `Load()` on a server or in single player: every channel's script is cleared, then `radio_save` (`None` when there
    /// is no file) is read: `DaysSinceStart` and `LvObfuscation` set their fields, every other line is a channel line for
    /// `RadioScriptManager.Load`. An exception ends the read (logged) and returns false. The result is true only when
    /// `RadioScriptManager.Load` returned normally (the bytecode stores `result = true` after that call; the decompiled
    /// text, which shows `finally { result = true; }` after `return result;`, misleads: measured with the oracle, the
    /// days of a save file are used).
    pub fn load<R: JavaRandom>(
        &mut self,
        radio_save: Option<&[u8]>,
        rand: &mut RandAbstract<R>,
    ) -> bool {
        let mode = self.get_game_mode();
        if mode != GameMode::Server && mode != GameMode::SinglePlayer {
            return false;
        }
        let Some(m) = self.script_manager.as_mut() else {
            return false;
        };
        for (_, c) in &mut m.channels {
            c.set_active_script_null();
        }
        let Some(data) = radio_save else {
            return false;
        };
        let text = String::from_utf8_lossy(data).into_owned();
        let mut channel_lines: Vec<String> = Vec::new();
        for line in java::buffered_reader_lines(&text) {
            let line = java::trim(line);
            if !line.starts_with("DaysSinceStart") && !line.starts_with("LvObfuscation") {
                channel_lines.push(line.to_string());
            } else {
                let res = (|| -> Result<(), String> {
                    if line.starts_with("DaysSinceStart") {
                        let s = java::split_char(line, '=');
                        let v = s.get(1).ok_or_else(|| {
                            format!(
                                "java.lang.ArrayIndexOutOfBoundsException: Index 1 out of bounds for length {}",
                                s.len()
                            )
                        })?;
                        self.days_since_start = parse_int_java(java::trim(v))?;
                    }
                    if line.starts_with("LvObfuscation") {
                        let s = java::split_char(line, '=');
                        let v = s.get(1).ok_or_else(|| {
                            format!(
                                "java.lang.ArrayIndexOutOfBoundsException: Index 1 out of bounds for length {}",
                                s.len()
                            )
                        })?;
                        self.louisville_obfuscation = java::trim(v).eq_ignore_ascii_case("true");
                    }
                    Ok(())
                })();
                if res.is_err() {
                    return false;
                }
            }
        }
        match self.script_manager.as_mut() {
            Some(m) => m.load(&channel_lines, rand).is_ok(),
            None => false,
        }
    }

    /// `Save()`: the text to write to `radio/data/RADIO_SAVE.txt`, or `None` when the original writes nothing (no radio
    /// folder, a client, or the same text as the last save of the same file). The `RecordedMedia.save()` after it is
    /// not ported.
    pub fn save(&mut self, file_name: &str, line_separator: &str) -> Option<String> {
        let mode = self.get_game_mode();
        if (mode == GameMode::Server || mode == GameMode::SinglePlayer) && self.save_dir_exists {
            let m = self.script_manager.as_ref()?;
            let mut content = format!("DaysSinceStart = {}{line_separator}", self.days_since_start);
            content.push_str(&format!(
                "LvObfuscation = {}{line_separator}",
                self.louisville_obfuscation
            ));
            content.push_str(&m.save(line_separator));
            if self.last_save_file.as_deref() == Some(file_name)
                && self.last_save_content.as_deref() == Some(content.as_str())
            {
                return None;
            }
            self.last_save_file = Some(file_name.to_string());
            self.last_save_content = Some(content.clone());
            return Some(content);
        }
        None
    }

    /// `UpdateScripts(hour, mins)` on a server or in single player: the day counter turns over at midnight, the scripts
    /// are updated, then the save is written (the returned text, see `save`).
    pub fn update_scripts<R: JavaRandom>(
        &mut self,
        hour: i32,
        mins: i32,
        save_file_name: &str,
        rand: &mut RandAbstract<R>,
    ) -> Option<String> {
        let mode = self.get_game_mode();
        if mode == GameMode::Server || mode == GameMode::SinglePlayer {
            if hour == 0 && self.last_recorded_hour != 0 {
                self.days_since_start += 1;
            }
            self.last_recorded_hour = hour;
            let d = self.days_since_start;
            if let Some(m) = self.script_manager.as_mut() {
                m.update_scripts(d, hour, mins, rand);
            }
            return self.save(save_file_name, LINE_SEPARATOR);
        }
        None
    }

    /// `PlayerListensChannel(channel, listenmode, isTV)` on a server or in single player
    pub fn player_listens_channel(&mut self, channel: i32, listen_mode: bool, is_tv: bool) {
        let mode = self.get_game_mode();
        if mode == GameMode::SinglePlayer || mode == GameMode::Server {
            if let Some(m) = self.script_manager.as_mut() {
                m.player_listens_channel(channel, listen_mode, is_tv);
            }
        }
    }

    /// `update()` on a server or in single player (once per tick): the post-silence flag after 14 days, then every channel
    /// airs the line that is due. What each channel transmits goes through `send_transmission`, in channel order (a
    /// Louisville scramble draws in between); the transmissions with the message that would be distributed are returned.
    /// `multiplier` is `GameTime.getMultiplier()`.
    pub fn update<R: JavaRandom>(
        &mut self,
        multiplier: f32,
        rand: &mut RandAbstract<R>,
    ) -> Vec<(Transmission, String)> {
        let mode = self.get_game_mode();
        let mut sent = Vec::new();
        if mode != GameMode::Server && mode != GameMode::SinglePlayer {
            return sent;
        }
        if self.days_since_start > 14 && !self.post_radio_silence {
            self.post_radio_silence = true;
        }
        let (disable, louis) = (self.disable_broadcasting, self.louisville_obfuscation);
        let len = self.script_manager.as_ref().map_or(0, |m| m.channels.len());
        for i in 0..len {
            let t = match self.script_manager.as_mut() {
                Some(m) => m.channels[i]
                    .1
                    .update(multiplier, disable, louis, &mut m.broadcasts),
                None => break,
            };
            if let Some(mut t) = t {
                if t.scramble {
                    let tmp = t.text.clone();
                    t.text = self.scramble_string(&tmp, 85, true, None, rand);
                }
                let out = self.send_transmission(&t, rand);
                sent.push((t, out));
            }
        }
        sent
    }

    /// `SendTransmission(0, 0, channel, msg, null, effects, r, g, b, -1, isTV)` on a server or in single player: the weather
    /// interference may scramble a radio (not a television) message; with no devices registered nothing else happens.
    /// Returns the message that would be distributed.
    pub fn send_transmission<R: JavaRandom>(
        &mut self,
        t: &Transmission,
        rand: &mut RandAbstract<R>,
    ) -> String {
        let mode = self.get_game_mode();
        let mut msg = t.text.clone();
        if !t.is_tv && (mode == GameMode::Server || mode == GameMode::SinglePlayer) {
            self.has_applied_interference = false;
            msg = self.apply_weather_interference(&msg, -1, rand);
        }
        msg
    }

    /// `applyWeatherInterference(msg, signalStrength)` (private)
    fn apply_weather_interference<R: JavaRandom>(
        &mut self,
        msg: &str,
        signal_strength: i32,
        rand: &mut RandAbstract<R>,
    ) -> String {
        if self.weather_interference <= 0.0 {
            msg.to_string()
        } else {
            let intensity = (self.weather_interference * 100.0f32) as i32;
            self.scramble_string(msg, intensity, signal_strength == -1, None, rand)
        }
    }

    /// `getRandomBzztFzzt`
    pub fn get_random_bzzt_fzzt<R: JavaRandom>(rand: &mut RandAbstract<R>) -> &'static str {
        STATIC_SOUNDS[rand.next(STATIC_SOUNDS.len() as i32) as usize]
    }

    /// `scrambleString(msg, intensity, ignoreBBcode, customScramble)`: words (or, with `ignore_bb_code`, the pieces between
    /// spaces and `[...]` codes) are replaced by one static sound per run, each with a `Rand.Next(100)` against `intensity`.
    pub fn scramble_string<R: JavaRandom>(
        &mut self,
        msg: &str,
        intensity: i32,
        ignore_bb_code: bool,
        custom_scramble: Option<&str>,
        rand: &mut RandAbstract<R>,
    ) -> String {
        self.has_applied_interference = false;
        if intensity <= 0 {
            return msg.to_string();
        }
        if intensity >= 100 {
            return match custom_scramble {
                Some(c) => c.to_string(),
                None => Self::get_random_bzzt_fzzt(rand).to_string(),
            };
        }
        self.has_applied_interference = true;
        let mut new_msg: Vec<u16> = Vec::new();
        let push = |v: &mut Vec<u16>, s: &str| v.extend(s.encode_utf16());
        let scramble_text = |rand: &mut RandAbstract<R>| -> String {
            match custom_scramble {
                Some(c) => c.to_string(),
                None => Self::get_random_bzzt_fzzt(rand).to_string(),
            }
        };
        let is_ws = |u: u16| char::from_u32(u32::from(u)).is_some_and(java::is_whitespace);
        if ignore_bb_code {
            let chars: Vec<u16> = msg.encode_utf16().collect();
            let mut scrmbl = false;
            let mut has_opened = false;
            let mut word: Vec<u16> = Vec::new();
            for i in 0..chars.len() {
                let c = chars[i];
                if has_opened {
                    word.push(c);
                    if c == u16::from(b']') {
                        new_msg.extend(word.iter());
                        word.clear();
                        has_opened = false;
                    }
                } else if c == u16::from(b'[') || (is_ws(c) && i > 0 && !is_ws(chars[i - 1])) {
                    let r = rand.next(100);
                    if r > intensity {
                        new_msg.extend(word.iter());
                        push(&mut new_msg, " ");
                        scrmbl = false;
                    } else if !scrmbl {
                        let t = scramble_text(rand);
                        push(&mut new_msg, &t);
                        push(&mut new_msg, " ");
                        scrmbl = true;
                    }
                    if c == u16::from(b'[') {
                        word = vec![u16::from(b'[')];
                        has_opened = true;
                    } else {
                        word.clear();
                    }
                } else {
                    word.push(c);
                }
            }
            if !word.is_empty() {
                new_msg.extend(word.iter());
            }
        } else {
            let mut scrmbl = false;
            for word in java::split_whitespace_regex(msg) {
                let rx = rand.next(100);
                if rx > intensity {
                    push(&mut new_msg, word);
                    push(&mut new_msg, " ");
                    scrmbl = false;
                } else if !scrmbl {
                    let t = scramble_text(rand);
                    push(&mut new_msg, &t);
                    push(&mut new_msg, " ");
                    scrmbl = true;
                }
            }
        }
        String::from_utf16_lossy(&new_msg)
    }

    /// `computerize(str)`: each `Character.isLetter` unit in random case (`Rand.NextBool(2)`: lower when 0).
    pub fn computerize<R: JavaRandom>(s: &str, rand: &mut RandAbstract<R>) -> String {
        let mut out: Vec<u16> = Vec::new();
        for u in s.encode_utf16() {
            match char::from_u32(u32::from(u)) {
                Some(c) if java_chars::is_letter(u) => {
                    let lower = rand.next_bool(2);
                    let r = if lower {
                        java_chars::to_lower_char(c)
                    } else {
                        java_chars::to_upper_char(c)
                    };
                    let mut buf = [0u16; 2];
                    out.extend_from_slice(r.encode_utf16(&mut buf));
                }
                _ => out.push(u),
            }
        }
        String::from_utf16_lossy(&out)
    }
}
