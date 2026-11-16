//! zombie.core.skinnedmodel.model.ModelTxt
use super::vertex_buffer_object::VertexArray;
use crate::java::JavaHashMap;
use crate::org::lwjgl::util::vector::matrix4f::Matrix4f;
use crate::zombie::core::skinnedmodel::animation::animation_clip::AnimationClip;

/// zombie.core.skinnedmodel.model.ModelTxt: what `ModelLoader.loadTxt` returns.
#[derive(Clone, Debug)]
pub struct ModelTxt {
    pub is_static: bool,
    pub reverse: bool,
    pub vertices: Option<VertexArray>,
    pub elements: Option<Vec<i32>>,
    pub bone_indices: JavaHashMap<i32>,
    pub skeleton_hierarchy: Vec<i32>,
    pub bind_pose: Vec<Matrix4f>,
    pub skin_offset_matrices: Vec<Matrix4f>,
    pub inv_bind_pose: Vec<Matrix4f>,
    pub clips: JavaHashMap<AnimationClip>,
}

impl ModelTxt {
    /// ModelTxt()
    pub fn new() -> Self {
        ModelTxt {
            is_static: false,
            reverse: false,
            vertices: None,
            elements: None,
            bone_indices: JavaHashMap::default(),
            skeleton_hierarchy: Vec::new(),
            bind_pose: Vec::new(),
            skin_offset_matrices: Vec::new(),
            inv_bind_pose: Vec::new(),
            clips: JavaHashMap::default(),
        }
    }
}

impl Default for ModelTxt {
    fn default() -> Self {
        ModelTxt::new()
    }
}
