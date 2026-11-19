//! zombie.core.skinnedmodel.animation.AnimationTrack and AnimationMultiTrack: the time bookkeeping of the tracks an
//! animator plays (current/previous time, speed, looping, the finish events, the blend weight the layers set). The
//! bone poses and the bone masks the original also updates per tick need the skinned model and are not ported
//! (UNKNOWN). The deferred (root) movement of the deferred bone's keyframes is (`updateDeferredValues`).
use std::rc::Rc;

use super::animation_clip::AnimationClip;
use super::bone_axis::BoneAxis;
use crate::org::lwjgl::util::vector::quaternion::Quaternion;
use crate::org::lwjgl::util::vector::vector3f::Vector3f;
use crate::zombie::core::math::pz_math;
use crate::zombie::core::skinnedmodel::advancedanimation::i_animation_variable_source::IAnimationVariableSource;
use crate::zombie::core::skinnedmodel::helper_functions;
use crate::zombie::iso::vector2::Vector2;

/// A track's `IAnimListener`: the `AnimLayer` it plays on (added by `setAnimLayer`) or the `LiveAnimNode` that
/// started it (added by `addListener`), by id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListenerRef {
    Layer(u32),
    Node(u32),
}

/// The `IAnimListener` callbacks a track invokes while it ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackEvent {
    /// onAnimStarted
    AnimStarted,
    /// onLoopedAnim
    LoopedAnim,
    /// onNonLoopedAnimFadeOut
    NonLoopedAnimFadeOut,
    /// onNonLoopedAnimFinished
    NonLoopedAnimFinished,
}

/// zombie.core.skinnedmodel.animation.AnimationTrack. `blend_curve_set` is `blendCurve != null` (a node's blend curve:
/// not ported, the animator refuses to start such a track).
#[derive(Clone, Debug)]
pub struct AnimationTrack {
    pub id: u32,
    pub is_playing: bool,
    pub is_primary: bool,
    pub current_clip: Option<Rc<AnimationClip>>,
    pub priority: i32,
    pub ragdoll_start_time: f32,
    pub ragdoll_max_time: f32,
    current_time_value: f32,
    previous_time_value: f32,
    accumulated_time_value: f32,
    pub sync_tracking_enabled: bool,
    pub track_time_to_variable: Option<String>,
    pub reverse: bool,
    pub looping: bool,
    speed_delta: f32,
    blend_weight: f32,
    blend_field_weight: f32,
    pub blend_curve_set: bool,
    pub name: String,
    pub matching_grappled_anim_node: Option<String>,
    is_grappler: bool,
    pub early_blend_out_time: f32,
    pub trigger_on_non_looped_anim_fade_out_event: bool,
    pub anim_layer: Option<u32>,
    pub listeners: Vec<ListenerRef>,
    pub deferred_bone_name: String,
    /// `deferredBone.index` (`None`: the clip table has no such bone, or no keyframes are loaded for the clip)
    pub deferred_bone: Option<i32>,
    /// `deferredBoneAxis` (`None` is a null axis: an `m_deferredBoneAxis` the XML did not name)
    pub deferred_bone_axis: Option<BoneAxis>,
    pub use_deferred_movement: bool,
    pub use_deferred_rotation: bool,
    pub deferred_rotation_scale: f32,
    /// The track plays on a sub layer: its bone mask (`boneWeights`) is not ported, so neither is its deferred weight.
    pub bone_mask_unported: bool,
    deferred_motion: DeferredMotionData,
    deferred_pose_span: KeyframeSpan,
}

/// AnimationTrack.DeferredMotionData
#[derive(Clone, Copy, Debug, Default)]
struct DeferredMotionData {
    target_deferred_rotation: f32,
    prev_deferred_rotation: f32,
    target_deferred_rotation_q: Option<Quaternion>,
    target_deferred_position: Vector3f,
    prev_deferred_movement: Vector2,
    target_deferred_movement: Vector2,
    deferred_rotation_diff: f32,
    deferred_movement_diff: Vector2,
    counter_rotated_movement_diff: Vector2,
}

/// AnimationTrack.KeyframeSpan: the two keyframes (indices into the bone's frame list, -1 for null) around a time.
#[derive(Clone, Copy, Debug)]
struct KeyframeSpan {
    from: i32,
    to: i32,
}

impl KeyframeSpan {
    fn new() -> KeyframeSpan {
        KeyframeSpan { from: -1, to: -1 }
    }

    fn clear(&mut self) {
        self.from = -1;
        self.to = -1;
    }

    fn is_span(&self) -> bool {
        self.from != -1 && self.to != -1
    }
}

/// `new Keyframe(new Vector3f(), new Quaternion(), new Vector3f(1, 1, 1))` as AnimationTrack.L_updateDeferredValues
/// keeps them: the position and rotation a span's `lerp` leaves.
#[derive(Clone, Copy, Debug)]
struct PosRot {
    position: Vector3f,
    rotation: Quaternion,
}

impl AnimationTrack {
    /// AnimationTrack.alloc + resetInternal: the field defaults of a fresh track.
    pub fn alloc(id: u32) -> AnimationTrack {
        AnimationTrack {
            id,
            is_playing: false,
            is_primary: false,
            current_clip: None,
            priority: 0,
            ragdoll_start_time: 0.0,
            ragdoll_max_time: 5.0,
            current_time_value: 0.0,
            previous_time_value: 0.0,
            accumulated_time_value: 0.0,
            sync_tracking_enabled: true,
            track_time_to_variable: None,
            reverse: false,
            looping: false,
            speed_delta: 1.0,
            blend_weight: 0.0,
            blend_field_weight: 1.0,
            blend_curve_set: false,
            name: "!Empty!".to_string(),
            matching_grappled_anim_node: None,
            is_grappler: false,
            early_blend_out_time: 0.0,
            trigger_on_non_looped_anim_fade_out_event: false,
            anim_layer: None,
            listeners: Vec::new(),
            deferred_bone_name: String::new(),
            deferred_bone: None,
            deferred_bone_axis: Some(BoneAxis::Y),
            use_deferred_movement: true,
            use_deferred_rotation: false,
            deferred_rotation_scale: 1.0,
            bone_mask_unported: false,
            deferred_motion: DeferredMotionData::default(),
            deferred_pose_span: KeyframeSpan::new(),
        }
    }

    /// AnimationTrack.startClip: reset, then `isPlaying`, `looping`, the clip and the ragdoll max time. (`reset` has
    /// just cleared `trackTimeToVariable`, so the original's `getTrackTimeToVariable` branch is not taken.)
    pub fn start_clip(&mut self, clip: Rc<AnimationClip>, looped: bool, ragdoll_max_time: f32) {
        let id = self.id;
        *self = AnimationTrack::alloc(id);
        self.is_playing = true;
        self.looping = looped;
        self.current_clip = Some(clip);
        self.ragdoll_max_time = ragdoll_max_time;
    }

    /// AnimationTrack.isRagdoll
    pub fn is_ragdoll(&self) -> bool {
        self.current_clip.as_ref().is_some_and(|c| c.is_ragdoll)
    }

    /// AnimationTrack.hasClip
    pub fn has_clip(&self) -> bool {
        self.current_clip.is_some()
    }

    /// AnimationTrack.getDuration: the ragdoll max time of a ragdoll track, else the clip's duration (0 without one).
    pub fn get_duration(&self) -> f32 {
        if self.is_ragdoll() {
            self.ragdoll_max_time
        } else {
            self.current_clip.as_ref().map_or(0.0, |c| c.duration)
        }
    }

    fn get_reversible_time_value(&self, time_value: f32) -> f32 {
        if self.reverse {
            self.get_duration() - time_value
        } else {
            time_value
        }
    }

    /// AnimationTrack.getCurrentTrackTime
    pub fn get_current_track_time(&self) -> f32 {
        self.get_reversible_time_value(self.current_time_value)
    }

    /// AnimationTrack.getCurrentTimeValue
    pub fn get_current_time_value(&self) -> f32 {
        self.current_time_value
    }

    /// AnimationTrack.getPreviousTimeValue
    pub fn get_previous_time_value(&self) -> f32 {
        self.previous_time_value
    }

    /// AnimationTrack.getAccumulatedTimeValue
    pub fn get_accumulated_time_value(&self) -> f32 {
        self.accumulated_time_value
    }

    /// AnimationTrack.getSpeedDelta
    pub fn get_speed_delta(&self) -> f32 {
        self.speed_delta
    }

    /// AnimationTrack.setSpeedDelta
    pub fn set_speed_delta(&mut self, speed_delta: f32) {
        self.speed_delta = speed_delta;
    }

    /// AnimationTrack.getBlendWeight: `blendWeight * curveWeight * blendFieldWeight` (no curve: 1).
    pub fn get_blend_weight(&self) -> f32 {
        self.blend_weight * 1.0 * self.blend_field_weight
    }

    /// AnimationTrack.setBlendWeight
    pub fn set_blend_weight(&mut self, blend_weight: f32) {
        self.blend_weight = blend_weight;
    }

    /// AnimationTrack.getBlendFieldWeight
    pub fn get_blend_field_weight(&self) -> f32 {
        self.blend_field_weight
    }

    /// AnimationTrack.setBlendFieldWeight
    pub fn set_blend_field_weight(&mut self, blend_field_weight: f32) {
        self.blend_field_weight = blend_field_weight;
    }

    /// AnimationTrack.getCurrentTimeFraction
    pub fn get_current_time_fraction(&self) -> f32 {
        if self.has_clip() {
            self.current_time_value / self.get_duration()
        } else {
            0.0
        }
    }

    /// AnimationTrack.advance: both times wrap into `[0, duration]`, the accumulated time grows by the amount.
    pub fn advance(&mut self, advance_amount: f32) {
        self.current_time_value = pz_math::wrap(
            self.current_time_value + advance_amount,
            0.0,
            self.get_duration(),
        );
        self.previous_time_value = pz_math::wrap(
            self.previous_time_value + advance_amount,
            0.0,
            self.get_duration(),
        );
        self.accumulated_time_value += advance_amount;
    }

    /// AnimationTrack.scaledAdvance
    pub fn scaled_advance(&mut self, advance_amount: f32) {
        self.advance(advance_amount * self.speed_delta);
    }

    /// AnimationTrack.scaledRewind
    pub fn scaled_rewind(&mut self, rewind_amount: f32) {
        self.scaled_advance(-rewind_amount);
    }

    /// AnimationTrack.moveCurrentTimeValueTo
    pub fn move_current_time_value_to(&mut self, target: f32) {
        let diff = target - self.current_time_value;
        self.advance(diff);
    }

    /// AnimationTrack.moveCurrentTimeValueToFraction
    pub fn move_current_time_value_to_fraction(&mut self, fraction: f32) {
        let target_time = self.get_duration() * fraction;
        self.move_current_time_value_to(target_time);
    }

    /// AnimationTrack.isLooping
    pub fn is_looping(&self) -> bool {
        self.looping
    }

    /// AnimationTrack.isGrappler
    pub fn is_grappler(&self) -> bool {
        self.is_grappler
    }

    /// AnimationTrack.setMatchingGrappledAnimNode
    pub fn set_matching_grappled_anim_node(&mut self, node: &str) {
        self.matching_grappled_anim_node = Some(node.to_string());
        self.is_grappler = !crate::zombie::util::string_utils::is_null_or_whitespace(Some(node));
    }

    /// AnimationTrack.setAnimLayer: the previous layer stops listening, the new one starts (at the end of the list).
    pub fn set_anim_layer(&mut self, anim_layer: Option<u32>) {
        if self.anim_layer != anim_layer {
            if let Some(old) = self.anim_layer {
                self.listeners.retain(|l| *l != ListenerRef::Layer(old));
            }
            self.anim_layer = anim_layer;
            if let Some(new) = self.anim_layer {
                self.listeners.push(ListenerRef::Layer(new));
            }
        }
    }

    /// AnimationTrack.addListener
    pub fn add_listener(&mut self, listener: ListenerRef) {
        self.listeners.push(listener);
    }

    /// AnimationTrack.removeListener: the first equal entry (`ArrayList.remove(Object)`).
    pub fn remove_listener(&mut self, listener: ListenerRef) {
        if let Some(i) = self.listeners.iter().position(|l| *l == listener) {
            self.listeners.remove(i);
        }
    }

    fn get_track_time_to_variable(
        &self,
        default_time: f32,
        source: &dyn IAnimationVariableSource,
    ) -> f32 {
        let Some(name) = &self.track_time_to_variable else {
            return default_time;
        };
        if self.anim_layer.is_none() {
            return default_time;
        }
        let anim_length = self.get_duration();
        let time_fraction = source.get_variable_float(name, default_time / anim_length);
        anim_length * time_fraction
    }

    /// AnimationTrack.tickCurrentTimeInternal: the callbacks it would invoke, in order. The original invokes each one
    /// where it is raised, before the rest of the tick; the callbacks here are run by the animator after the tick, so
    /// they see the track's final time (none of the ported listeners reads the track).
    pub fn tick_current_time(
        &mut self,
        mut time: f32,
        source: &dyn IAnimationVariableSource,
    ) -> Vec<TrackEvent> {
        let mut events = Vec::new();
        time *= self.speed_delta;
        if !self.is_playing {
            time = 0.0;
        }
        let end_duration = self.get_duration();
        self.previous_time_value = self.current_time_value;
        let ttv_empty = self
            .track_time_to_variable
            .as_deref()
            .is_none_or(str::is_empty);
        if ttv_empty {
            self.current_time_value += time;
            self.accumulated_time_value += time;
        } else {
            self.current_time_value =
                self.get_track_time_to_variable(self.current_time_value + time, source);
        }
        if self.looping {
            if self.previous_time_value == 0.0 && self.current_time_value > 0.0 {
                events.push(TrackEvent::AnimStarted);
            }
            if self.previous_time_value > self.current_time_value
                || self.current_time_value >= end_duration
            {
                events.push(TrackEvent::LoopedAnim);
                // Java's float `%` is C's fmod
                self.current_time_value %= end_duration;
                events.push(TrackEvent::AnimStarted);
            }
        } else {
            if self.current_time_value < 0.0 {
                self.current_time_value = 0.0;
            }
            if self.previous_time_value == 0.0 && self.current_time_value > 0.0 {
                events.push(TrackEvent::AnimStarted);
            }
            if self.trigger_on_non_looped_anim_fade_out_event {
                let early_blend_out_at_time =
                    end_duration - self.early_blend_out_time * self.speed_delta;
                if self.previous_time_value < early_blend_out_at_time
                    && early_blend_out_at_time <= self.current_time_value
                {
                    events.push(TrackEvent::NonLoopedAnimFadeOut);
                }
            }
            if self.current_time_value > end_duration {
                self.current_time_value = end_duration;
            }
            let is_ragdoll = self.is_ragdoll();
            if !is_ragdoll
                && self.previous_time_value < end_duration
                && self.current_time_value >= end_duration
            {
                // (`if (this.looping)` is false in this branch)
                events.push(TrackEvent::NonLoopedAnimFinished);
            }
        }
        events
    }

    /// AnimationTrack.updateKeyframesInternal: `if (dt > 0) TickCurrentTime(dt)`, then `updateDeferredValues` (`server`
    /// is `GameServer.server`). The pose of the bones `updatePose` fills is not ported (it only feeds the drawn skeleton).
    pub fn update_keyframes(
        &mut self,
        dt: f32,
        source: &dyn IAnimationVariableSource,
        server: bool,
    ) -> Result<Vec<TrackEvent>, String> {
        if self.current_clip.is_none() {
            return Err(
                "java.lang.RuntimeException: AnimationPlayer.Update was called before startClip"
                    .into(),
            );
        }
        let events = if dt > 0.0 {
            self.tick_current_time(dt, source)
        } else {
            Vec::new()
        };
        self.update_deferred_values(server)?;
        Ok(events)
    }

    /// AnimationTrack.getPreviousAnimationTime: 0 for a ragdoll, else the previous track time.
    fn get_previous_animation_time(&self) -> f32 {
        if self.is_ragdoll() {
            0.0
        } else {
            self.get_reversible_time_value(self.previous_time_value)
        }
    }

    /// AnimationTrack.getCurrentAnimationTime: the clip's duration for a ragdoll, else the current track time.
    fn get_current_animation_time(&self) -> f32 {
        if self.is_ragdoll() {
            self.current_clip.as_ref().map_or(0.0, |c| c.get_duration())
        } else {
            self.get_current_track_time()
        }
    }

    /// AnimationTrack.updateDeferredValuesInternal: the deferred movement and rotation between the previous and the
    /// current animation time (a looping track that wrapped is split in two spans).
    fn update_deferred_values(&mut self, server: bool) -> Result<(), String> {
        if self.deferred_bone.is_none() {
            return Ok(());
        }
        let mut dm = self.deferred_motion;
        dm.deferred_rotation_diff = 0.0;
        dm.deferred_movement_diff.set(0.0, 0.0);
        dm.counter_rotated_movement_diff.set(0.0, 0.0);
        let mut prev_time = self.get_previous_animation_time();
        let current_time = self.get_current_animation_time();
        if self.is_looping() && prev_time > current_time {
            let end_time = self.get_duration();
            self.append_deferred_values(&mut dm, prev_time, end_time, server)?;
            prev_time = 0.0;
        }
        self.append_deferred_values(&mut dm, prev_time, current_time, server)?;
        self.deferred_motion = dm;
        Ok(())
    }

    /// AnimationTrack.appendDeferredValues
    fn append_deferred_values(
        &mut self,
        dm: &mut DeferredMotionData,
        prev_time: f32,
        current_time: f32,
        server: bool,
    ) -> Result<(), String> {
        let bone = self.get_deferred_movement_bone_idx();
        let mut prev_key_frame = self.get_deferred_movement_frame_at(bone, prev_time)?;
        let mut key_frame = self.get_deferred_movement_frame_at(bone, current_time)?;
        if !server {
            dm.prev_deferred_rotation =
                self.get_deferred_twist_rotation(&mut prev_key_frame.rotation)?;
            dm.target_deferred_rotation_q = Some(key_frame.rotation);
            dm.target_deferred_rotation =
                self.get_deferred_twist_rotation(&mut key_frame.rotation)?;
            let angle_diff =
                pz_math::get_closest_angle(dm.prev_deferred_rotation, dm.target_deferred_rotation);
            dm.deferred_rotation_diff += angle_diff * self.deferred_rotation_scale;
        }
        dm.prev_deferred_movement = self.deferred_movement_of(&prev_key_frame.position);
        dm.target_deferred_position = key_frame.position;
        dm.target_deferred_movement = self.deferred_movement_of(&key_frame.position);
        let diff = Vector2::new(
            dm.target_deferred_movement.x - dm.prev_deferred_movement.x,
            dm.target_deferred_movement.y - dm.prev_deferred_movement.y,
        );
        let mut cr_diff = diff;
        if self.use_deferred_rotation && !self.is_ragdoll() {
            let len = cr_diff.normalize();
            cr_diff.rotate(-(dm.target_deferred_rotation + (std::f64::consts::PI / 2.0) as f32));
            cr_diff.scale(-len);
        }
        dm.deferred_movement_diff.x += diff.x;
        dm.deferred_movement_diff.y += diff.y;
        dm.counter_rotated_movement_diff.x += cr_diff.x;
        dm.counter_rotated_movement_diff.y += cr_diff.y;
        Ok(())
    }

    /// AnimationTrack.getDeferredTwistRotation: the bone rotation's twist about the deferred bone's axis (the Y one
    /// normalises the quaternion in place).
    fn get_deferred_twist_rotation(&self, bone_rotation: &mut Quaternion) -> Result<f32, String> {
        match self.deferred_bone_axis {
            Some(BoneAxis::Z) => Ok(helper_functions::get_rotation_z(bone_rotation)),
            Some(BoneAxis::Y) => helper_functions::get_rotation_y(bone_rotation),
            // DebugType.Animation.error("BoneAxis unhandled: ...")
            None => Ok(0.0),
        }
    }

    /// AnimationTrack.getDeferredMovement(bonePos, deferredPos): `(x, -z)` for the Y axis, else `(x, y)`.
    fn deferred_movement_of(&self, bone_pos: &Vector3f) -> Vector2 {
        if self.deferred_bone_axis == Some(BoneAxis::Y) {
            Vector2::new(bone_pos.x, -bone_pos.z)
        } else {
            Vector2::new(bone_pos.x, bone_pos.y)
        }
    }

    /// AnimationTrack.getDeferredMovementBoneIdx
    pub fn get_deferred_movement_bone_idx(&self) -> i32 {
        self.deferred_bone.unwrap_or(-1)
    }

    /// AnimationTrack.getDeferredMovementFrameAt: the interpolated keyframe of the deferred bone at `time`.
    fn get_deferred_movement_frame_at(
        &mut self,
        bone_idx: i32,
        time: f32,
    ) -> Result<PosRot, String> {
        let clip = self
            .current_clip
            .clone()
            .ok_or("AnimationTrack.getKeyframeSpan: no clip")?;
        let bone = usize::try_from(bone_idx)
            .ok()
            .filter(|b| *b < super::animation_clip::BONE_INDEX_SLOTS)
            .ok_or("java.lang.ArrayIndexOutOfBoundsException: the deferred bone is outside the clip's bone table")?;
        let span = self.get_keyframe_span(&clip, bone, time);
        span_lerp(&clip, bone, span, time)
    }

    /// AnimationTrack.getKeyframeSpan(boneIdx, time, deferredPoseSpan): the cached span when it still holds `time`,
    /// else the span of the bone's frames around `time` (searching on from the cached one when `time` has moved
    /// past it).
    fn get_keyframe_span(&mut self, clip: &AnimationClip, bone: usize, time: f32) -> KeyframeSpan {
        let mut result = self.deferred_pose_span;
        let span = Self::keyframe_span(clip, bone, time, &mut result);
        self.deferred_pose_span = span;
        span
    }

    fn keyframe_span(
        clip: &AnimationClip,
        bone: usize,
        time: f32,
        result: &mut KeyframeSpan,
    ) -> KeyframeSpan {
        let is_bone =
            |idx: i32| idx != -1 && clip.bone_frame(bone, idx as usize).bone == bone as i32;
        if !(is_bone(result.from) || is_bone(result.to)) {
            result.clear();
        }
        let num_frames = clip.bone_frame_count(bone);
        if num_frames == 0 {
            result.clear();
            return *result;
        }
        let frame_time = |idx: i32| clip.bone_frame(bone, idx as usize).time;
        // containsTime: a span whose two frames bracket `time`
        if result.is_span() && frame_time(result.from) <= time && time <= frame_time(result.to) {
            return *result;
        }
        let last = num_frames as i32 - 1;
        if time >= frame_time(last) {
            result.from = if num_frames > 1 { last - 1 } else { 0 };
            result.to = last;
            return *result;
        }
        if time <= frame_time(0) {
            result.clear();
            result.to = 0;
            return *result;
        }
        let mut start_idx = 0;
        if result.is_span() && frame_time(result.to) <= time {
            start_idx = result.to;
        }
        result.clear();
        let mut idx = start_idx;
        while idx < last {
            let (k_curr, k_next) = (frame_time(idx), frame_time(idx + 1));
            if k_curr <= time && time <= k_next {
                result.from = idx;
                result.to = idx + 1;
                break;
            }
            idx += 1;
        }
        *result
    }

    /// AnimationTrack.getDeferredMovementDiff: the counter rotated movement of the last update.
    pub fn get_deferred_movement_diff(&self) -> Vector2 {
        self.deferred_motion.counter_rotated_movement_diff
    }

    /// AnimationTrack.getDeferredRotationDiff
    pub fn get_deferred_rotation_diff(&self) -> f32 {
        self.deferred_motion.deferred_rotation_diff
    }

    /// AnimationTrack.getDeferredBoneWeight: 0 without a deferred bone, else the bone mask's weight (1 without a mask).
    pub fn get_deferred_bone_weight(&self) -> Result<f32, String> {
        if self.deferred_bone.is_none() {
            Ok(0.0)
        } else if self.bone_mask_unported {
            Err(
                "AnimationTrack.getBoneWeight: the bone mask of a sub layer track is not ported"
                    .into(),
            )
        } else {
            Ok(1.0)
        }
    }

    /// AnimationTrack.isFinished
    pub fn is_finished(&self) -> bool {
        if self.is_ragdoll() {
            // !isRagdollSimulationActive(): no ragdoll controller is ported
            true
        } else {
            !self.looping
                && self.get_duration() > 0.0
                && self.current_time_value >= self.get_duration()
        }
    }
}

/// KeyframeSpan.lerp(time, result): the span's keyframe at `time`: identity for an empty span, the one frame for a
/// half span, else `Keyframe.lerp(from, to, time)` (`b.time == a.time` takes `b`).
fn span_lerp(
    clip: &AnimationClip,
    bone: usize,
    span: KeyframeSpan,
    time: f32,
) -> Result<PosRot, String> {
    const NPE: &str = "java.lang.NullPointerException";
    let frame = |i: i32| clip.bone_frame(bone, i as usize);
    // Keyframe.set(keyframe): the position and rotation as they are (a null one stays null: the deferred code then
    // dereferences it)
    let take = |k: &super::keyframe::Keyframe| -> Result<PosRot, String> {
        Ok(PosRot {
            position: k.position.ok_or(NPE)?,
            rotation: k.rotation.ok_or(NPE)?,
        })
    };
    if span.from == -1 && span.to == -1 {
        return Ok(PosRot {
            position: Vector3f::of(0.0, 0.0, 0.0),
            rotation: Quaternion::new(),
        });
    }
    if span.to == -1 {
        return take(frame(span.from));
    }
    if span.from == -1 || span.from == span.to {
        return take(frame(span.to));
    }
    let (a, b) = (frame(span.from), frame(span.to));
    if b.time == a.time {
        // Keyframe.get: the defaults for a null part
        return Ok(PosRot {
            position: b.position.unwrap_or(Vector3f::of(0.0, 0.0, 0.0)),
            rotation: b.rotation.unwrap_or_default(),
        });
    }
    let del = (time - a.time) / (b.time - a.time);
    let (pa, pb) = (a.position.ok_or(NPE)?, b.position.ok_or(NPE)?);
    let (qa, qb) = (a.rotation.ok_or(NPE)?, b.rotation.ok_or(NPE)?);
    let (x, y, z) = helper_functions::lerp3((pa.x, pa.y, pa.z), (pb.x, pb.y, pb.z), del);
    Ok(PosRot {
        position: Vector3f::of(x, y, z),
        rotation: helper_functions::slerp(&qa, &qb, del),
    })
}

impl AnimationTrack {
    /// AnimationTrack.reset of the deferred part (the field defaults `resetInternal` sets).
    #[allow(dead_code)]
    fn reset_deferred(&mut self) {
        self.deferred_motion = DeferredMotionData::default();
        self.deferred_pose_span.clear();
    }
}
