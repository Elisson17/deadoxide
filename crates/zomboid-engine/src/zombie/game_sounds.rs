//! zombie.GameSounds: the registry of the game's sounds (`sounds` in load order, `soundByName`), filled from the
//! `sound` scripts by `ScriptsLoaded`, with the lookups the game uses (`getSound`, `getOrCreateSound`,
//! `isKnownSound`, `getCategories`, `getSoundsInCategory`) and `loadINI`.
//!
//! The original keeps one object in both containers; here the objects live in `arena` and both containers hold
//! indices into it, so that one object stays one object (`get_sound` hands out the index; `reset` empties the arena
//! with the containers).
//!
//! What runs without FMOD is what this ports: the original's FMOD branches test `GameServer.server` (initClipEvents
//! does nothing on a dedicated server) and `BaseSoundBank.instance instanceof FMODSoundBank` (the standalone server
//! and a `-nosound` client hold no FMOD sound bank, as E46 found `getFMODSoundBank()` nil there); they are not
//! ported: the event lookups of `addSound` and `getOrCreateSound`, the debug-only unused-event report of
//! `ScriptsLoaded`, `OnReloadSound`, `saveINI`, the sound preview and `fix3DListenerPosition`.
use std::collections::{HashMap, HashSet};

use crate::java;
use crate::zombie::audio::game_sound::{GameSound, GameSoundClip};
use crate::zombie::config::config_file::ConfigFile;
use crate::zombie::core::math::pz_math;
use crate::zombie::scripting::objects::game_sound_script::GameSoundScript;

/// zombie.GameSounds.VERSION
pub const VERSION: i32 = 1;

/// zombie.GameSounds' static state.
#[derive(Clone, Debug, Default)]
pub struct GameSounds {
    /// The sound objects both containers point into.
    pub arena: Vec<GameSound>,
    /// zombie.GameSounds.sounds: indices into `arena`, in list order.
    pub sounds: Vec<usize>,
    /// zombie.GameSounds.soundByName
    pub sound_by_name: HashMap<String, usize>,
    /// zombie.GameSounds.soundIsPaused
    pub sound_is_paused: bool,
}

impl GameSounds {
    /// The sound at an arena index.
    pub fn sound(&self, index: usize) -> &GameSound {
        &self.arena[index]
    }

    /// The sound at an arena index, to change.
    pub fn sound_mut(&mut self, index: usize) -> &mut GameSound {
        &mut self.arena[index]
    }

    /// The position of an arena index in `sounds` (the first, as `List.indexOf` would find it).
    pub fn list_position(&self, index: usize) -> Option<usize> {
        self.sounds.iter().position(|&i| i == index)
    }

    /// zombie.GameSounds.addSound (the server's path: `initClipEvents` does nothing when `GameServer.server`). A
    /// sound whose name is already in `soundByName` replaces the first one of that name in `sounds`, at its place;
    /// when `soundByName` holds the name but no sound in the list has it (`getOrCreateSound` files a sound named
    /// "x.wav" under "x"), `sounds.remove(index)` throws an IndexOutOfBoundsException (`Err`, nothing changed). A
    /// sound without a name is UNKNOWN (the original's HashMap takes a null key): `Err`, nothing changed; scripts
    /// always have one.
    pub fn add_sound(&mut self, sound: GameSound) -> Result<usize, String> {
        let Some(name) = sound.name.clone() else {
            return Err("java.lang.NullPointerException".to_string());
        };
        let mut index = self.sounds.len();
        if self.sound_by_name.contains_key(&name) {
            index = 0;
            while index < self.sounds.len()
                && self.arena[self.sounds[index]].name.as_deref() != Some(name.as_str())
            {
                index += 1;
            }
            if index >= self.sounds.len() {
                return Err(format!(
                    "java.lang.IndexOutOfBoundsException: Index {index} out of bounds for length {}",
                    self.sounds.len()
                ));
            }
            self.sounds.remove(index);
        }
        self.arena.push(sound);
        let arena_index = self.arena.len() - 1;
        self.sounds.insert(index, arena_index);
        self.sound_by_name.insert(name, arena_index);
        Ok(arena_index)
    }

    /// zombie.GameSounds.isKnownSound
    pub fn is_known_sound(&self, name: Option<&str>) -> bool {
        name.is_some_and(|n| self.sound_by_name.contains_key(n))
    }

    /// zombie.GameSounds.getSound
    pub fn get_sound(&mut self, name: Option<&str>) -> Option<usize> {
        self.get_or_create_sound(name)
    }

    /// zombie.GameSounds.getOrCreateSound: `None` for a null or empty name, the sound filed under `name`, else a new
    /// "AUTO" sound called `name` with one empty clip, appended to `sounds` and filed under `name` without ".wav" and
    /// ".ogg" (so a name ending in either is never found again and every call makes a new sound). The FMOD event,
    /// `.ogg` and `.wav` lookups that follow only run with an FMOD sound bank and are not ported.
    pub fn get_or_create_sound(&mut self, name: Option<&str>) -> Option<usize> {
        let name = name.filter(|n| !n.is_empty())?;
        if let Some(&index) = self.sound_by_name.get(name) {
            return Some(index);
        }
        let mut game_sound = GameSound::default();
        game_sound.name = Some(name.to_string());
        game_sound.category = "AUTO".to_string();
        let clip = GameSoundClip::new(&game_sound);
        game_sound.clips.push(clip);
        self.arena.push(game_sound);
        let index = self.arena.len() - 1;
        self.sounds.push(index);
        self.sound_by_name
            .insert(name.replace(".wav", "").replace(".ogg", ""), index);
        Some(index)
    }

    /// zombie.GameSounds.ScriptsLoaded: every script sound with clips is added (in the order of
    /// `ScriptManager.getAllGameSounds`), `loadNonBankSounds` (FMOD sound bank only) is nothing, then `loadINI` runs
    /// when the caller has a cache directory (`ZomboidFileSystem.getCacheDir()`). `Err` is an exception of
    /// `addSound`, which ends the loop.
    pub fn scripts_loaded(
        &mut self,
        script_sounds: &[GameSoundScript],
        cache_dir: Option<&str>,
    ) -> Result<(), String> {
        for script_sound in script_sounds {
            if !script_sound.game_sound.clips.is_empty() {
                self.add_sound(script_sound.game_sound.clone())?;
            }
        }
        if let Some(cache_dir) = cache_dir {
            self.load_ini(cache_dir);
        }
        Ok(())
    }

    /// zombie.GameSounds.getCategories: the distinct categories, sorted (`String.compareTo`).
    pub fn get_categories(&self) -> Vec<String> {
        let mut categories: HashSet<&str> = HashSet::new();
        for &index in &self.sounds {
            categories.insert(&self.arena[index].category);
        }
        let mut sorted: Vec<String> = categories.into_iter().map(str::to_string).collect();
        sorted.sort_by(|a, b| java::string_compare_to(a, b));
        sorted
    }

    /// zombie.GameSounds.getSoundsInCategory: the sounds of `sounds` in that category, in list order.
    pub fn get_sounds_in_category(&self, category: &str) -> Vec<usize> {
        self.sounds
            .iter()
            .copied()
            .filter(|&index| self.arena[index].category == category)
            .collect()
    }

    /// zombie.GameSounds.loadINI: `<cache dir>/sounds.ini` (`ConfigFile.read`); with a version of at most 1, every
    /// option named like a sound in `soundByName` sets that sound's user volume
    /// (`PZMath.tryParseFloat(value, 1.0F)`).
    pub fn load_ini(&mut self, cache_dir: &str) {
        let file_name = format!("{cache_dir}{}sounds.ini", std::path::MAIN_SEPARATOR);
        let mut config_file = ConfigFile::default();
        if config_file.read(&file_name) && config_file.get_version() <= VERSION {
            for option in config_file.get_options() {
                if let Some(&index) = self.sound_by_name.get(option.get_name()) {
                    self.arena[index].set_user_volume(pz_math::try_parse_float(
                        Some(option.get_value_as_string()),
                        1.0,
                    ));
                }
            }
        }
    }

    /// zombie.GameSounds.Reset (without the preview sound, which is FMOD's).
    pub fn reset(&mut self) {
        self.sounds.clear();
        self.sound_by_name.clear();
        self.arena.clear();
    }
}
