//! zombie.core.Core: the font size option and the version strings (the rest of the class is not ported).
use crate::zombie::core::math::pz_math;

/// zombie.core.Core.getOptionFontSizeReal: the font size option (1..6), where 6 ("auto") picks 1..5 from the screen
/// height: `floor((height - 1080) / 132f) + 1` clamped to 0..10, then 0 → 1, 1 → 2, 2-3 → 3, 4-6 → 4, 7-10 → 5.
pub fn get_option_font_size_real(option_font_size: i32, screen_height: i32) -> i32 {
    let mut size = option_font_size;
    if size == 6 {
        let scale = pz_math::floor(screen_height.wrapping_sub(1080) as f32 / 132.0) as i32 + 1;
        let scale_clamp = pz_math::clamp_i32(scale, 0, 10);
        match scale_clamp {
            0 => size = 1,
            1 => size = 2,
            2 | 3 => size = 3,
            4..=6 => size = 4,
            7..=10 => size = 5,
            _ => {}
        }
    }
    size
}

/// zombie.core.Core.gameVersion: `new GameVersion(42, 21, "")`; its `toString()` is `String.format("%d.%d%s", major,
/// minor, suffix)`.
pub const GAME_VERSION: &str = "42.21";

/// The git revision `Core.getGitRevisionString()` and `getGitSha()` return when `Core.IS_DEV` is false (the
/// game's own value).
pub const GIT_REVISION: &str = "4a0e9546ec";

/// zombie.core.Core.getGameAndBuildVersion: `gameVersion + ".0"`.
pub fn get_game_and_build_version() -> String {
    format!("{GAME_VERSION}.0")
}

/// zombie.core.Core.getVersion: `getGameAndBuildVersion() + " " + getGitRevisionString()`.
pub fn get_version() -> String {
    format!("{} {GIT_REVISION}", get_game_and_build_version())
}

/// zombie.core.Core.getBulletVersion: `Bullet.getPZBulletVersion()`, a native of the game's libPZBullet. MEASURED
/// (LuaGameProbe's MainScreen.version "pzBullet(1.0.0.28)"), not computed: the library is not ported.
pub const BULLET_VERSION: &str = "1.0.0.28";

/// zombie.core.Core.KeyBinding: `record KeyBinding(String name, int keyValue, int altKey, boolean shift, boolean ctrl,
/// boolean alt)`; the derived `equals` compares every component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyBinding {
    pub name: String,
    pub key_value: i32,
    pub alt_key: i32,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl KeyBinding {
    /// zombie.core.Core.KEYBINDING_EMPTY: `new KeyBinding("empty", 0, 0, false, false, false)`.
    pub fn empty() -> KeyBinding {
        KeyBinding {
            name: "empty".into(),
            key_value: 0,
            alt_key: 0,
            shift: false,
            ctrl: false,
            alt: false,
        }
    }
}

/// The key binding tables of zombie.core.Core: `keyMaps` and `keyBindingByKeyValue`.
#[derive(Default)]
pub struct KeyMaps {
    key_maps: std::collections::HashMap<Option<String>, KeyBinding>,
    key_binding_by_key_value: std::collections::HashMap<i32, Vec<KeyBinding>>,
}

impl KeyMaps {
    /// zombie.core.Core.reinitKeyMaps.
    pub fn reinit(&mut self) {
        self.key_maps = std::collections::HashMap::new();
        self.key_binding_by_key_value.clear();
    }

    /// zombie.core.Core.getKeyBinding(String): `keyMaps.getOrDefault(keyName, KEYBINDING_EMPTY)`.
    pub fn get_key_binding(&self, key_name: Option<&str>) -> KeyBinding {
        self.key_maps
            .get(&key_name.map(str::to_string))
            .cloned()
            .unwrap_or_else(KeyBinding::empty)
    }

    /// zombie.core.Core.addKeyBinding(String, int, int, boolean, boolean, boolean).
    pub fn add_key_binding(
        &mut self,
        key_name: Option<&str>,
        key: i32,
        alt_key: i32,
        shift: bool,
        ctrl: bool,
        alt: bool,
    ) {
        let binding = KeyBinding {
            name: key_name.unwrap_or("null").to_string(),
            key_value: key,
            alt_key,
            shift,
            ctrl,
            alt,
        };
        let previous = self
            .key_maps
            .insert(key_name.map(str::to_string), binding.clone());
        if let Some(previous) = previous {
            if let Some(list) = self.key_binding_by_key_value.get_mut(&previous.key_value) {
                if let Some(i) = list.iter().position(|b| *b == previous) {
                    list.remove(i);
                }
            }
        }
        let list = self.key_binding_by_key_value.entry(key).or_default();
        if !list.contains(&binding) {
            list.push(binding);
        }
    }
}
