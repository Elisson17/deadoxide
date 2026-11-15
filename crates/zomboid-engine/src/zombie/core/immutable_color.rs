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
