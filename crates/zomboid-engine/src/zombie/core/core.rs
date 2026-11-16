//! zombie.core.Core: so far the font size option (the rest of the class is not ported).
use crate::zombie::core::math::pz_math;

/// zombie.core.Core.getOptionFontSizeReal: the font size option (1..6), where 6 ("auto") picks 1..5 from the screen
/// height: `floor((height - 1080) / 132f) + 1` clamped to 0..10, then 0 → 1, 1 → 2, 2-3 → 3, 4-6 → 4, 7-10 → 5.
pub fn get_option_font_size_real(option_font_size: i32, screen_height: i32) -> i32 {
    let mut size = option_font_size;
    if size == 6 {
        let scale =
            pz_math::floor(screen_height.wrapping_sub(1080) as f32 / 132.0) as i32 + 1;
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
