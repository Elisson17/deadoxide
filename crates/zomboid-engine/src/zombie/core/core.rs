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
