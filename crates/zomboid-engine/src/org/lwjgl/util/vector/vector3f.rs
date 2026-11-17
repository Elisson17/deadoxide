//! org.lwjgl.util.vector.Vector3f.

/// org.lwjgl.util.vector.Vector3f: three single-precision components.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3f {
    /// Vector3f(): zero.
    pub fn new() -> Self {
        Vector3f {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    /// Vector3f(float, float, float)
    pub fn of(x: f32, y: f32, z: f32) -> Self {
        Vector3f { x, y, z }
    }

    /// Vector3f.set(float, float, float)
    pub fn set(&mut self, x: f32, y: f32, z: f32) {
        self.x = x;
        self.y = y;
        self.z = z;
    }

    /// Vector3f.lengthSquared
    pub fn length_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    /// Vector.length: `(float)Math.sqrt(lengthSquared())`.
    pub fn length(&self) -> f32 {
        f64::from(self.length_squared()).sqrt() as f32
    }
}
