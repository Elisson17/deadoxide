//! org.joml.Vector2f (JOML, bundled in the game's jar; not decompiled): the two fields and the setters the vehicle
//! scripts use.

/// org.joml.Vector2f
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector2f {
    pub x: f32,
    pub y: f32,
}

impl Vector2f {
    /// org.joml.Vector2f.<init>(float, float)
    pub fn new(x: f32, y: f32) -> Vector2f {
        Vector2f { x, y }
    }

    /// org.joml.Vector2f.mul(float): each component times the scalar.
    pub fn mul(&mut self, scalar: f32) {
        self.x *= scalar;
        self.y *= scalar;
    }
}
