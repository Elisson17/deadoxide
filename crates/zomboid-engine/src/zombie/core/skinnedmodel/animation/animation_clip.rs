//! zombie.core.skinnedmodel.animation.AnimationClip: construction and the per-bone keyframe tables.
use super::keyframe::Keyframe;

/// AnimationClip.keyFramesByBoneIndex has this many slots (`new KeyframeByBoneIndexElement[60]`).
pub const BONE_INDEX_SLOTS: usize = 60;

/// zombie.core.skinnedmodel.animation.AnimationClip (the data the loaders fill; playback queries are not ported).
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationClip {
    pub name: String,
    pub is_ragdoll: bool,
    pub keep_last_frame: bool,
    pub duration: f32,
    /// The Java table holds the same `Keyframe` objects as `keyframeArray`; here indices into it.
    key_frames_by_bone_index: Vec<Vec<usize>>,
    keyframe_array: Vec<Keyframe>,
}

impl AnimationClip {
    /// AnimationClip(float duration, List<Keyframe> keyframes, String name, boolean bKeepLastFrame): `isRagdoll` false.
    pub fn new(duration: f32, keyframes: &[Keyframe], name: &str, keep_last_frame: bool) -> Self {
        Self::new_ragdoll(duration, keyframes, name, keep_last_frame, false)
    }

    /// AnimationClip(float, List<Keyframe>, String, boolean, boolean isRagdoll)
    pub fn new_ragdoll(
        duration: f32,
        keyframes: &[Keyframe],
        name: &str,
        keep_last_frame: bool,
        is_ragdoll: bool,
    ) -> Self {
        let mut clip = AnimationClip {
            name: name.to_string(),
            is_ragdoll,
            keep_last_frame,
            duration,
            key_frames_by_bone_index: Vec::new(),
            keyframe_array: keyframes.to_vec(),
        };
        clip.recalculate_keyframes_by_bone_index();
        clip
    }

    /// AnimationPlayer.getOrCreateRagdollAnimationClip: `new AnimationClip(1.0F, keyframeList, "RagdollAnimationClip",
    /// true, true)` (the animator only reads its name, duration and flags; the keyframe list is not ported).
    pub fn ragdoll() -> Self {
        Self::new_ragdoll(1.0, &[], "RagdollAnimationClip", true, true)
    }

    /// AnimationClip.getDuration
    pub fn get_duration(&self) -> f32 {
        self.duration
    }

    /// AnimationClip.getKeyframes
    pub fn get_keyframes(&self) -> &[Keyframe] {
        &self.keyframe_array
    }

    /// AnimationClip.getKeyframes: the array itself, for the loaders that renumber `Keyframe.bone` in place.
    pub fn get_keyframes_mut(&mut self) -> &mut [Keyframe] {
        &mut self.keyframe_array
    }

    /// AnimationClip.getBoneFramesAt(int idx): the frames grouped under bone `idx` (0..60) as of the last
    /// recalculation (the grouping is not redone when `Keyframe.bone` is renumbered afterwards).
    pub fn get_bone_frames_at(&self, idx: usize) -> Vec<&Keyframe> {
        self.key_frames_by_bone_index[idx]
            .iter()
            .map(|i| &self.keyframe_array[*i])
            .collect()
    }

    /// AnimationClip.recalculateKeyframesByBoneIndex: per bone 0..60 the keyframes of that bone in array order, the
    /// last one dropped when `keepLastFrame` is false and there is more than one.
    pub fn recalculate_keyframes_by_bone_index(&mut self) {
        let mut table = Vec::with_capacity(BONE_INDEX_SLOTS);
        for bone_idx in 0..BONE_INDEX_SLOTS {
            let mut bkf: Vec<usize> = (0..self.keyframe_array.len())
                .filter(|i| self.keyframe_array[*i].bone == bone_idx as i32)
                .collect();
            if !self.keep_last_frame && bkf.len() > 1 {
                bkf.pop();
            }
            table.push(bkf);
        }
        self.key_frames_by_bone_index = table;
    }
}
