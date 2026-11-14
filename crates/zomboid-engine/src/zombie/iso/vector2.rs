//! zombie.iso.Vector2: the game's float 2D vector (only what the ported code uses so far).
use crate::zombie::core::math::pz_math;

/// zombie.iso.Vector2
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

impl Vector2 {
    pub fn new(x: f32, y: f32) -> Self {
        Vector2 { x, y }
    }
    /// zombie.iso.Vector2.getLength: `(float) Math.sqrt(x * x + y * y)`, the sum in float.
    pub fn get_length(&self) -> f32 {
        f64::from(self.x * self.x + self.y * self.y).sqrt() as f32
    }
    /// zombie.iso.Vector2.getLengthSquared
    pub fn get_length_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y
    }
    /// zombie.iso.Vector2.normalize: unchanged when the squared length is within 1e-5 of 1, zero stays zero, otherwise
    /// both parts divided by the float length. Returns the length.
    pub fn normalize(&mut self) -> f32 {
        let length_sq = self.get_length_squared();
        if pz_math::equal(length_sq, 1.0, 1.0e-5) {
            1.0
        } else if length_sq == 0.0 {
            self.x = 0.0;
            self.y = 0.0;
            0.0
        } else {
            let length = f64::from(length_sq).sqrt() as f32;
            self.x /= length;
            self.y /= length;
            length
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (3, 4) has length 5 and normalizes to (0.6, 0.8) exactly as float division gives them; a unit vector is left
    /// alone even when its squared length is not exactly 1.
    #[test]
    fn normalize_divides_by_the_float_length() {
        let mut v = Vector2::new(3.0, 4.0);
        assert_eq!(v.get_length(), 5.0);
        assert_eq!(v.normalize(), 5.0);
        assert_eq!(v, Vector2::new(3.0f32 / 5.0, 4.0f32 / 5.0));
        let mut u = Vector2::new(0.6, 0.800_001);
        assert_eq!(u.normalize(), 1.0);
        assert_eq!(u, Vector2::new(0.6, 0.800_001));
    }
}
