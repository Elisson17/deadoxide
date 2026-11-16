//! zombie.core.skinnedmodel.model.SkinningData: the data fields the loaders fill (the bone hierarchy views and the GL
//! buffers are not ported).
use crate::java::JavaHashMap;
use crate::org::lwjgl::util::vector::matrix4f::Matrix4f;
use crate::zombie::core::skinnedmodel::animation::animation_clip::AnimationClip;

/// zombie.core.skinnedmodel.model.SkinningData
#[derive(Clone, Debug)]
pub struct SkinningData {
    pub animation_clips: JavaHashMap<AnimationClip>,
    pub bind_pose: Vec<Matrix4f>,
    pub inverse_bind_pose: Vec<Matrix4f>,
    pub bone_offset: Vec<Matrix4f>,
    pub skeleton_hierarchy: Vec<i32>,
    pub bone_indices: JavaHashMap<i32>,
}

impl SkinningData {
    /// SkinningData(animationClips, bindPose, inverseBindPose, skinOffset, skeletonHierarchy, boneIndices): `boneOffset`
    /// takes `skinOffset.get(n)` for `n < skeletonHierarchy.size()`; `Err` is the IndexOutOfBoundsException.
    pub fn new(
        animation_clips: JavaHashMap<AnimationClip>,
        bind_pose: Vec<Matrix4f>,
        inverse_bind_pose: Vec<Matrix4f>,
        skin_offset: &[Matrix4f],
        skeleton_hierarchy: Vec<i32>,
        bone_indices: JavaHashMap<i32>,
    ) -> Result<Self, String> {
        let mut bone_offset = Vec::new();
        for n in 0..skeleton_hierarchy.len() {
            bone_offset.push(
                *skin_offset
                    .get(n)
                    .ok_or_else(|| "java.lang.IndexOutOfBoundsException".to_string())?,
            );
        }
        Ok(SkinningData {
            animation_clips,
            bind_pose,
            inverse_bind_pose,
            bone_offset,
            skeleton_hierarchy,
            bone_indices,
        })
    }

    /// SkinningData.numBones
    pub fn num_bones(&self) -> usize {
        self.skeleton_hierarchy.len()
    }
}
