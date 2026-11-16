//! zombie.core.skinnedmodel.animation.Keyframe
use crate::org::lwjgl::util::vector::quaternion::Quaternion;
use crate::org::lwjgl::util::vector::vector3f::Vector3f;

/// zombie.core.skinnedmodel.animation.Keyframe: one bone's transform at a time. `rotation`/`position`/`scale` are
/// nullable in the Java; `Keyframe()` leaves rotation and position null and the scale at (1, 1, 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Keyframe {
    pub rotation: Option<Quaternion>,
    pub position: Option<Vector3f>,
    pub scale: Option<Vector3f>,
    pub bone: i32,
    pub bone_name: Option<String>,
    pub time: f32,
}

impl Default for Keyframe {
    fn default() -> Self {
        Keyframe::new()
    }
}

impl Keyframe {
    /// Keyframe()
    pub fn new() -> Self {
        Keyframe {
            rotation: None,
            position: None,
            scale: Some(Vector3f::of(1.0, 1.0, 1.0)),
            bone: 0,
            bone_name: None,
            time: -1.0,
        }
    }

    /// Keyframe(Vector3f pos, Quaternion rotation, Vector3f scale): copies of the three.
    pub fn with(pos: &Vector3f, rotation: &Quaternion, scale: &Vector3f) -> Self {
        Keyframe {
            rotation: Some(*rotation),
            position: Some(*pos),
            scale: Some(*scale),
            bone: 0,
            bone_name: None,
            time: -1.0,
        }
    }
}
