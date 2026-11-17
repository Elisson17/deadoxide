//! org.lwjgl.util.vector.Vector4f.

/// org.lwjgl.util.vector.Vector4f: four single-precision components.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector4f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vector4f {
    /// Vector4f(float, float, float, float)
    pub fn of(x: f32, y: f32, z: f32, w: f32) -> Self {
        Vector4f { x, y, z, w }
    }
}
