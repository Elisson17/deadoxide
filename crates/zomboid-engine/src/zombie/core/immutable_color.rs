//! zombie.core.ImmutableColor: an RGBA colour of floats that never changes once made.
use super::color::Color;

/// zombie.core.ImmutableColor
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImmutableColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl ImmutableColor {
    /// zombie.core.ImmutableColor.white: `new ImmutableColor(1, 1, 1, 1)`.
    pub const WHITE: ImmutableColor = ImmutableColor {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    /// zombie.core.ImmutableColor.<init>(float, float, float): each channel clamped to 0..1, alpha 1.
    pub fn new_rgb(r: f32, g: f32, b: f32) -> Self {
        use crate::zombie::core::math::pz_math::clamp;
        ImmutableColor {
            r: clamp(r, 0.0, 1.0),
            g: clamp(g, 0.0, 1.0),
            b: clamp(b, 0.0, 1.0),
            a: 1.0,
        }
    }

    /// zombie.core.ImmutableColor(Color): the colour's channels; a null colour is opaque black.
    pub fn from_color(color: Option<&Color>) -> Self {
        match color {
            Some(c) => ImmutableColor {
                r: c.r,
                g: c.g,
                b: c.b,
                a: c.a,
            },
            None => ImmutableColor {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        }
    }
}
