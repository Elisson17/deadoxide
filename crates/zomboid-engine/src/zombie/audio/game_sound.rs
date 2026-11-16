//! zombie.audio.GameSound and zombie.audio.GameSoundClip: the data a `sound` script fills (category, loop, 3D, master
//! volume, clips with event, file, distances, volume, pitch and reverb), their defaults and the values derived from
//! them. The FMOD side of both classes (`FMOD_STUDIO_EVENT_DESCRIPTION` handles, `hasSustainPoints`, `hasParameter`,
//! `numClipsUsingParameter`) needs the FMOD banks and is not ported: without FMOD the handles are never set (the
//! dedicated server never runs `GameSounds.initClipEvents`).
//!
//! The original's `GameSoundClip.gameSound` back reference is not kept (the owner passes itself where a method of the
//! clip reads it); `GameSoundClip.checkReloaded` (hot reload of scripts) is not ported.
use crate::java;
use crate::zombie::core::random::rand_abstract::RandAbstract;

/// zombie.audio.GameSound.MasterVolume
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MasterVolume {
    Primary,
    Ambient,
    Music,
    VehicleEngine,
}

impl MasterVolume {
    /// Enum.name()
    pub fn name(self) -> &'static str {
        match self {
            MasterVolume::Primary => "Primary",
            MasterVolume::Ambient => "Ambient",
            MasterVolume::Music => "Music",
            MasterVolume::VehicleEngine => "VehicleEngine",
        }
    }

    /// java.lang.Enum.valueOf of GameSound.MasterVolume (the implicit `valueOf`): the exact constant name; `Err` is the IllegalArgumentException.
    pub fn value_of(name: &str) -> Result<MasterVolume, String> {
        match name {
            "Primary" => Ok(MasterVolume::Primary),
            "Ambient" => Ok(MasterVolume::Ambient),
            "Music" => Ok(MasterVolume::Music),
            "VehicleEngine" => Ok(MasterVolume::VehicleEngine),
            _ => Err(format!(
                "java.lang.IllegalArgumentException: No enum constant zombie.audio.GameSound.MasterVolume.{name}"
            )),
        }
    }
}

/// zombie.audio.GameSoundClip.INIT_FLAG_DISTANCE_MIN
pub const INIT_FLAG_DISTANCE_MIN: i16 = 1;
/// zombie.audio.GameSoundClip.INIT_FLAG_DISTANCE_MAX
pub const INIT_FLAG_DISTANCE_MAX: i16 = 2;
/// zombie.audio.GameSoundClip.INIT_FLAG_STOP_IMMEDIATE
pub const INIT_FLAG_STOP_IMMEDIATE: i16 = 4;

/// zombie.audio.GameSoundClip, without the FMOD event descriptions.
#[derive(Clone, Debug, PartialEq)]
pub struct GameSoundClip {
    pub event: Option<String>,
    pub file: Option<String>,
    pub volume: f32,
    pub pitch: f32,
    pub distance_min: f32,
    pub distance_max: f32,
    pub reverb_max_range: f32,
    pub reverb_factor: f32,
    pub priority: i32,
    pub init_flags: i16,
    pub reload_epoch: i16,
}

impl GameSoundClip {
    /// zombie.audio.GameSoundClip(GameSound): the field defaults and the owner's current reload epoch.
    pub fn new(game_sound: &GameSound) -> GameSoundClip {
        GameSoundClip {
            event: None,
            file: None,
            volume: 1.0,
            pitch: 1.0,
            distance_min: 10.0,
            distance_max: 10.0,
            reverb_max_range: 10.0,
            reverb_factor: 0.0,
            priority: 5,
            init_flags: 0,
            reload_epoch: game_sound.reload_epoch,
        }
    }

    /// zombie.audio.GameSoundClip.hasMinDistance
    pub fn has_min_distance(&self) -> bool {
        (self.init_flags & INIT_FLAG_DISTANCE_MIN) != 0
    }

    /// zombie.audio.GameSoundClip.hasMaxDistance
    pub fn has_max_distance(&self) -> bool {
        (self.init_flags & INIT_FLAG_DISTANCE_MAX) != 0
    }

    /// zombie.audio.GameSoundClip.isStopImmediate
    pub fn is_stop_immediate(&self) -> bool {
        (self.init_flags & INIT_FLAG_STOP_IMMEDIATE) != 0
    }

    /// zombie.audio.GameSoundClip.getEffectiveVolume (and getEffectiveVolumeInMenu, the same expression):
    /// `volume * gameSound.getUserVolume()`.
    pub fn get_effective_volume(
        &self,
        game_sound: &GameSound,
        enable_advanced_sound_options: bool,
    ) -> f32 {
        self.volume * game_sound.get_user_volume(enable_advanced_sound_options)
    }
}

/// zombie.audio.GameSound
#[derive(Clone, Debug, PartialEq)]
pub struct GameSound {
    pub name: Option<String>,
    pub category: String,
    pub r#loop: bool,
    pub is_3d: bool,
    pub clips: Vec<GameSoundClip>,
    user_volume: f32,
    pub master: MasterVolume,
    pub max_instances_per_emitter: i32,
    pub reload_epoch: i16,
}

impl Default for GameSound {
    /// zombie.audio.GameSound(): no name, category "General", not looping, 3D, no clips, user volume 1, master
    /// volume Primary, no instance limit.
    fn default() -> Self {
        GameSound {
            name: None,
            category: "General".to_string(),
            r#loop: false,
            is_3d: true,
            clips: Vec::new(),
            user_volume: 1.0,
            master: MasterVolume::Primary,
            max_instances_per_emitter: -1,
            reload_epoch: 0,
        }
    }
}

impl GameSound {
    /// zombie.audio.GameSound.setUserVolume: `Math.max(0.0F, Math.min(2.0F, gain))`.
    pub fn set_user_volume(&mut self, gain: f32) {
        self.user_volume = java::math_max_f32(0.0, java::math_min_f32(2.0, gain));
    }

    /// The private `userVolume` field.
    pub fn user_volume(&self) -> f32 {
        self.user_volume
    }

    /// zombie.audio.GameSound.getUserVolume: 1 unless `SystemDisabler.getEnableAdvancedSoundOptions()`.
    pub fn get_user_volume(&self, enable_advanced_sound_options: bool) -> f32 {
        if !enable_advanced_sound_options {
            1.0
        } else {
            self.user_volume
        }
    }

    /// zombie.audio.GameSound.getRandomClip: `clips.get(Rand.Next(clips.size()))`. `Err` is the
    /// IndexOutOfBoundsException of an empty list (`Rand.Next(0)` is 0).
    pub fn get_random_clip<R: java::JavaRandom>(
        &self,
        rand: &mut RandAbstract<R>,
    ) -> Result<usize, String> {
        let index = rand.next(self.clips.len() as i32);
        if index < 0 || index as usize >= self.clips.len() {
            return Err(format!(
                "java.lang.IndexOutOfBoundsException: Index {index} out of bounds for length {}",
                self.clips.len()
            ));
        }
        Ok(index as usize)
    }

    /// zombie.audio.GameSound.getMaxDistanceOfClips: the largest `distanceMax` among the clips that have one.
    pub fn get_max_distance_of_clips(&self) -> f32 {
        let mut max = 0.0f32;
        for clip in &self.clips {
            if clip.has_max_distance() {
                max = crate::zombie::core::math::pz_math::max(max, clip.distance_max);
            }
        }
        max
    }

    /// zombie.audio.GameSound.reset
    pub fn reset(&mut self) {
        self.name = None;
        self.category = "General".to_string();
        self.r#loop = false;
        self.is_3d = true;
        self.clips.clear();
        self.user_volume = 1.0;
        self.master = MasterVolume::Primary;
        self.max_instances_per_emitter = -1;
        self.reload_epoch = self.reload_epoch.wrapping_add(1);
    }
}
