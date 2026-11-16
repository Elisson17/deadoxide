//! zombie.core.skinnedmodel.animation.AnimationTrack and AnimationMultiTrack: the time bookkeeping of the tracks an
//! animator plays (current/previous time, speed, looping, the finish events, the blend weight the layers set). The
//! bone poses, the bone weights and the deferred (root) movement the original also updates per tick need the skinned
//! model and are not ported (UNKNOWN).
use std::rc::Rc;

use super::animation_clip::AnimationClip;
use crate::zombie::core::math::pz_math;
use crate::zombie::core::skinnedmodel::advancedanimation::i_animation_variable_source::IAnimationVariableSource;

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
    pub use_deferred_movement: bool,
    pub use_deferred_rotation: bool,
    pub deferred_rotation_scale: f32,
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
            use_deferred_movement: true,
            use_deferred_rotation: false,
            deferred_rotation_scale: 1.0,
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

    /// AnimationTrack.updateKeyframesInternal's time part: `if (dt > 0) TickCurrentTime(dt)`.
    pub fn update_keyframes(
        &mut self,
        dt: f32,
        source: &dyn IAnimationVariableSource,
    ) -> Result<Vec<TrackEvent>, String> {
        if self.current_clip.is_none() {
            return Err(
                "java.lang.RuntimeException: AnimationPlayer.Update was called before startClip"
                    .into(),
            );
        }
        if dt > 0.0 {
            Ok(self.tick_current_time(dt, source))
        } else {
            Ok(Vec::new())
        }
    }
}
