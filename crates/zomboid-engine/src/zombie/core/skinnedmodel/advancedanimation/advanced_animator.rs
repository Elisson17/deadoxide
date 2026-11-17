//! zombie.core.skinnedmodel.advancedanimation.AdvancedAnimator with the classes it drives: AnimLayer, LiveAnimNode,
//! SubLayerSlot, TransitionNodeProxy, and the track list of `AnimationPlayer` (`getMultiTrack()`).
//!
//! The original's objects point at each other (a live node at its layer, a track at its listeners, the animator at its
//! character). Here the animator owns everything and refers by id: layers, live nodes and tracks (`u32`), states and
//! nodes of the set by `StateRef` / `NodeRef`. The character is the `AnimatorOwner` passed to each call: it is where
//! the animator reads variables, writes the `FlagWhileAlive` variables and reports the animation events
//! (`IsoGameCharacter.OnAnimEvent`). The global `Rand` is passed in (`Env`).
//!
//! Not ported (UNKNOWN, each returns `Err` where the original would run it on state this port can reach): bone poses
//! and bone masks, the deferred (root) movement a track produces and `AnimationPlayer.DoAngles`, ragdoll simulation
//! and ragdoll nodes, grappling, node blend curves, the animation recorder and the debug monitor.
//!
//! The original's comparisons and branches are kept as written: `!(x <= 0.0)` differs from `x > 0.0` for NaN, and the
//! two branches of the percentage-event test are identical in the Java.
#![allow(clippy::neg_cmp_op_on_partial_ord, clippy::if_same_then_else)]
use std::collections::HashMap;
use std::rc::Rc;

use super::anim_condition::AnimCondition;
use super::anim_event::{AnimEvent, AnimEventKind, AnimEventTime};
use super::anim_node::AnimNode;
use super::anim_transition::AnimTransition;
use super::animation_set::AnimationSet;
use super::animation_variable_reference::AnimationVariableReference;
use super::i_animation_variable_source::IAnimationVariableSource;
use crate::java::{self, Random};
use crate::zombie::core::math::pz_math;
use crate::zombie::core::random::rand_abstract::RandAbstract;
use crate::zombie::core::skinnedmodel::animation::animation_clips::AnimationClips;
use crate::zombie::core::skinnedmodel::animation::animation_track::{
    AnimationTrack, ListenerRef, TrackEvent,
};
use crate::zombie::util::string_utils;

/// DebugOptions.instance.animation.allowEarlyTransitionOut: a debug-only option whose default is read when
/// `Core.debug` is false (a dedicated server, a release client).
const ALLOW_EARLY_TRANSITION_OUT: bool = true;
/// DebugOptions.instance.animation.disableAnimationBlends (debug-only, default false).
const DISABLE_ANIMATION_BLENDS: bool = false;
/// DebugOptions.instance.character.debug.ragdoll.physics.physicsHitReaction (debug-only, default true).
const PHYSICS_HIT_REACTION: bool = true;

/// An `AnimState` of the animator's set: its key in `AnimationSet.states` (the lower-cased folder name), or, for the
/// empty state `AnimationSet.GetState` makes for a name the set does not have (`new AnimState()`), `fresh` != 0 (each
/// such state is a new object, so no two are the same).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateRef {
    pub key: String,
    pub fresh: u32,
}

/// An `AnimNode` of the set: its state's key and its index in `AnimState.nodes`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeRef {
    pub state: String,
    pub index: usize,
}

fn node_of<'a>(set: &'a AnimationSet, r: &NodeRef) -> Result<&'a AnimNode, String> {
    set.states
        .get(&r.state)
        .and_then(|s| s.nodes.get(r.index))
        .ok_or_else(|| format!("AnimNode {}[{}] is not in the set", r.state, r.index))
}

/// What the animator needs of the character it animates (an `IAnimatable`: `IsoGameCharacter` and its
/// `OnAnimEvent`), over its variables (`IAnimationVariableSource`).
pub trait AnimatorOwner: IAnimationVariableSource {
    /// `getAnimationPlayer() != null && getAnimationPlayer().isReady()`
    fn is_player_ready(&self) -> bool;

    /// IsoGameCharacter.OnAnimEvent(layer, track, event): `layer_depth` is `AnimLayer.getDepth(sender)` and
    /// `layer_state_name` `AnimLayer.getCurrentStateName(sender)`.
    fn on_anim_event(
        &mut self,
        layer_depth: i32,
        layer_state_name: Option<&str>,
        track: Option<&AnimationTrack>,
        event: &AnimEvent,
    ) -> Result<(), String>;

    /// AnimationVariableReference.setVariable(character, boolean)
    fn set_variable_bool(
        &mut self,
        reference: &AnimationVariableReference,
        value: bool,
    ) -> Result<(), String>;

    /// IAnimationVariableSlot.setValue(float) of the existing slot named `key` (the `maxTwist` slot).
    fn set_variable_float(&mut self, key: &str, value: f32) -> Result<(), String>;

    /// IGrappleable.isGrappling of `getGrappleable()` (false without one).
    fn is_grappling(&self) -> bool {
        false
    }

    /// IGrappleable.isBeingGrappled of `getGrappleable()` (false without one).
    fn is_being_grappled(&self) -> bool {
        false
    }
}

/// The singletons and the character a call works with: the global `Rand` and the owner.
pub struct Env<'a> {
    pub owner: &'a mut dyn AnimatorOwner,
    pub rand: &'a mut RandAbstract<Random>,
}

/// LiveAnimNode.WhileAliveFlag, with the identity of the event whose reference it holds.
#[derive(Clone, Debug)]
struct WhileAliveFlag {
    event_key: (NodeRef, usize),
    variable_reference: AnimationVariableReference,
    while_alive_value: bool,
}

/// LiveAnimNode.TransitionIn
#[derive(Clone, Debug, Default)]
struct TransitionIn {
    time: f32,
    transitioned_from: Option<String>,
    active: bool,
    track: Option<u32>,
    data: Option<Rc<AnimTransition>>,
    weight: f32,
    raw_weight: f32,
    blending_in: bool,
    blending_out: bool,
}

/// zombie.core.skinnedmodel.advancedanimation.LiveAnimNode
#[derive(Clone, Debug)]
pub struct LiveAnimNode {
    pub id: u32,
    pub source: NodeRef,
    active: bool,
    was_active: bool,
    pub transitioning_out: bool,
    weight: f32,
    raw_weight: f32,
    is_new: bool,
    pub is_blend_field: bool,
    transition_in: TransitionIn,
    animation_tracks: Vec<u32>,
    pub node_anim_time: f32,
    pub prev_node_anim_time: f32,
    blending_in: bool,
    blending_out: bool,
    transition_out: Option<Rc<AnimTransition>>,
    while_alive_flags: Vec<WhileAliveFlag>,
    cached_random_anim: String,
}

impl LiveAnimNode {
    /// LiveAnimNode.alloc: `reset()` as it leaves the node.
    fn alloc(id: u32, source: NodeRef) -> LiveAnimNode {
        LiveAnimNode {
            id,
            source,
            active: false,
            was_active: false,
            transitioning_out: false,
            weight: 0.0,
            raw_weight: 0.0,
            is_new: true,
            is_blend_field: false,
            transition_in: TransitionIn::default(),
            animation_tracks: Vec::new(),
            node_anim_time: 0.0,
            prev_node_anim_time: 0.0,
            blending_in: false,
            blending_out: false,
            transition_out: None,
            while_alive_flags: Vec::new(),
            cached_random_anim: String::new(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn get_weight(&self) -> f32 {
        self.weight
    }

    pub fn get_raw_weight(&self) -> f32 {
        self.raw_weight
    }

    pub fn is_blending_in(&self) -> bool {
        self.blending_in
    }

    pub fn is_blending_out(&self) -> bool {
        self.blending_out
    }

    pub fn get_transition_in_weight(&self) -> f32 {
        self.transition_in.weight
    }

    pub fn get_transition_in_track(&self) -> Option<u32> {
        self.transition_in.track
    }

    pub fn get_transition_from(&self) -> Option<&str> {
        self.transition_in.transitioned_from.as_deref()
    }

    pub fn is_new(&self) -> bool {
        self.is_new
    }

    pub fn main_tracks(&self) -> &[u32] {
        &self.animation_tracks
    }

    pub fn while_alive_flag_count(&self) -> usize {
        self.while_alive_flags.len()
    }

    /// LiveAnimNode.isTransitioningIn
    pub fn is_transitioning_in(&self) -> bool {
        self.transition_in.active && self.transition_in.track.is_some()
    }

    /// LiveAnimNode.isMainAnimActive
    pub fn is_main_anim_active(&self) -> bool {
        !self.is_transitioning_in() || self.transition_in.blending_out
    }

    /// LiveAnimNode.wasActivated
    fn was_activated(&self) -> bool {
        self.active != self.was_active && self.active
    }

    /// LiveAnimNode.wasDeactivated
    fn was_deactivated(&self) -> bool {
        self.active != self.was_active && self.was_active
    }

    /// LiveAnimNode.getPlayingTrackAt over `0 .. getPlayingTrackCount()`: the main tracks while the main animation is
    /// active, then the transition-in track (no ragdoll track is ever running: ragdoll nodes are not ported).
    fn playing_tracks(&self) -> Vec<u32> {
        let mut v = Vec::new();
        if self.is_main_anim_active() {
            v.extend(self.animation_tracks.iter().copied());
        }
        if self.is_transitioning_in() {
            if let Some(t) = self.transition_in.track {
                v.push(t);
            }
        }
        v
    }

    /// LiveAnimNode.getAnimName
    fn get_anim_name<'a>(&'a self, src: &'a AnimNode) -> &'a str {
        if !string_utils::is_null_or_whitespace(Some(&self.cached_random_anim)) {
            &self.cached_random_anim
        } else {
            &src.anim_name
        }
    }

    /// LiveAnimNode.getBlendInTime (= getTransitionInBlendOutTime)
    fn get_blend_in_time(&self, src: &AnimNode) -> f32 {
        let Some(data) = &self.transition_in.data else {
            return src.blend_time;
        };
        if self.transition_in.track.is_some() && data.blend_out_time != f32::INFINITY {
            return data.blend_out_time;
        }
        if self.transition_in.track.is_none() {
            if data.blend_in_time != f32::INFINITY {
                return data.blend_in_time;
            }
            if data.blend_out_time != f32::INFINITY {
                return data.blend_out_time;
            }
        }
        src.blend_time
    }

    /// LiveAnimNode.getTransitionInBlendInTime
    fn get_transition_in_blend_in_time(&self) -> f32 {
        match &self.transition_in.data {
            Some(d) if d.blend_in_time != f32::INFINITY => d.blend_in_time,
            _ => 0.0,
        }
    }

    /// LiveAnimNode.getBlendOutTime
    fn get_blend_out_time(&self, src: &AnimNode) -> f32 {
        let Some(t) = &self.transition_out else {
            return src.get_blend_out_time();
        };
        let has_anim = !string_utils::is_null_or_whitespace(t.anim_name.as_deref());
        if has_anim && t.blend_in_time != f32::INFINITY {
            return t.blend_in_time;
        }
        if !has_anim {
            if t.blend_out_time != f32::INFINITY {
                return t.blend_out_time;
            }
            if t.blend_in_time != f32::INFINITY {
                return t.blend_in_time;
            }
        }
        src.get_blend_out_time()
    }

    /// LiveAnimNode.incrementBlendTime
    fn increment_blend_time(initial_weight: f32, blend_time_max: f32, time_delta: f32) -> f32 {
        let prev_time = initial_weight * blend_time_max;
        prev_time + time_delta
    }

    /// LiveAnimNode.updateBlendingIn
    fn update_blending_in(&mut self, src: &AnimNode, time_delta: f32) {
        let blend_time_max = self.get_blend_in_time(src);
        if !(blend_time_max <= 0.0) && !DISABLE_ANIMATION_BLENDS {
            let next_time = Self::increment_blend_time(self.raw_weight, blend_time_max, time_delta);
            let raw_weight = pz_math::clamp(next_time / blend_time_max, 0.0, 1.0);
            self.raw_weight = raw_weight;
            self.weight = pz_math::lerp_func_ease_out_in_quad(raw_weight);
            self.blending_in = next_time < blend_time_max;
        } else {
            // stopBlendingIn: setWeightsToFull
            self.weight = 1.0;
            self.raw_weight = 1.0;
            self.blending_in = false;
        }
    }

    /// LiveAnimNode.updateBlendingOut
    fn update_blending_out(&mut self, src: &AnimNode, time_delta: f32) {
        let blend_time_max = self.get_blend_out_time(src);
        if !(blend_time_max <= 0.0) && !DISABLE_ANIMATION_BLENDS {
            let next_time =
                Self::increment_blend_time(1.0 - self.raw_weight, blend_time_max, time_delta);
            let raw_weight = pz_math::clamp(1.0 - next_time / blend_time_max, 0.0, 1.0);
            self.raw_weight = raw_weight;
            self.weight = pz_math::lerp_func_ease_out_in_quad(raw_weight);
            self.blending_out = next_time < blend_time_max;
        } else {
            // stopBlendingOut: setWeightsToZero
            self.weight = 0.0;
            self.raw_weight = 0.0;
            self.blending_out = false;
        }
    }
}

/// zombie.core.skinnedmodel.advancedanimation.AnimLayer
#[derive(Clone, Debug)]
pub struct AnimLayer {
    pub id: u32,
    parent_layer: Option<u32>,
    state: Option<StateRef>,
    current_sync_track: Option<u32>,
    live_anim_nodes: Vec<LiveAnimNode>,
    no_anim_conditions_event_sent: bool,
}

impl AnimLayer {
    fn alloc(id: u32) -> AnimLayer {
        AnimLayer {
            id,
            parent_layer: None,
            state: None,
            current_sync_track: None,
            live_anim_nodes: Vec::new(),
            no_anim_conditions_event_sent: false,
        }
    }

    pub fn get_live_anim_nodes(&self) -> &[LiveAnimNode] {
        &self.live_anim_nodes
    }

    pub fn get_state(&self) -> Option<&StateRef> {
        self.state.as_ref()
    }

    /// AnimLayer.hasState
    pub fn has_state(&self) -> bool {
        self.state.is_some()
    }

    /// AnimLayer.isSubLayer
    pub fn is_sub_layer(&self) -> bool {
        self.parent_layer.is_some()
    }
}

/// zombie.core.skinnedmodel.advancedanimation.SubLayerSlot
#[derive(Clone, Debug)]
struct SubLayerSlot {
    should_be_active: bool,
    desired_layer: i32,
    anim_layer: u32,
    next_state: Option<StateRef>,
}

/// zombie.core.skinnedmodel.advancedanimation.events.LocalAnimEvent
#[derive(Clone, Copy, Debug)]
enum LocalAnimEvent {
    ActiveAnimLooped,
    ActiveNonLoopedAnimFadeOut,
    ActiveAnimFinishing,
    ActiveNonLoopedAnimFinished,
    NoAnimConditionsPass,
}

impl LocalAnimEvent {
    /// `LocalAnimEvent.getAnimEvent()`: an `AnimEvent()` with the name and either the percentage or the time.
    fn anim_event(self) -> AnimEvent {
        let mut e = AnimEvent::default();
        match self {
            LocalAnimEvent::ActiveAnimLooped => {
                e.time_pc = 1.0;
                e.event_name = Some("ActiveAnimLooped".to_string());
            }
            LocalAnimEvent::ActiveNonLoopedAnimFadeOut => {
                e.time_pc = 1.0;
                e.event_name = Some("NonLoopedAnimFadeOut".to_string());
            }
            LocalAnimEvent::ActiveAnimFinishing => {
                e.time = Some(AnimEventTime::End);
                e.event_name = Some("ActiveAnimFinishing".to_string());
            }
            LocalAnimEvent::ActiveNonLoopedAnimFinished => {
                e.time = Some(AnimEventTime::End);
                e.event_name = Some("ActiveAnimFinished".to_string());
            }
            LocalAnimEvent::NoAnimConditionsPass => {
                e.time = Some(AnimEventTime::End);
                e.event_name = Some("NoAnimConditionsPass".to_string());
            }
        }
        e
    }
}

/// zombie.core.utils.TransitionNodeProxy.TransitionNodeProxyData
struct TransitionData {
    new_node: u32,
    old_node: u32,
    transition_out: Rc<AnimTransition>,
    layer_out: u32,
}

/// zombie.core.utils.TransitionNodeProxy: (live node, layer) pairs.
#[derive(Default)]
struct TransitionNodeProxy {
    all_new_nodes: Vec<(u32, u32)>,
    all_outgoing_nodes: Vec<(u32, u32)>,
    found_transitions: Vec<TransitionData>,
}

/// zombie.core.skinnedmodel.advancedanimation.AdvancedAnimator (with the `AnimationPlayer` track list).
pub struct AdvancedAnimator {
    pub anim_set: Option<Rc<AnimationSet>>,
    layers: Vec<AnimLayer>,
    root_layer: u32,
    sub_layers: Vec<SubLayerSlot>,
    /// `rootState` and `subStates` of the last `setState(String, List)`.
    pub root_state: Option<StateRef>,
    pub sub_states: Vec<StateRef>,
    /// `setFlagCounters`: the variable and its count.
    set_flag_counters: Vec<(AnimationVariableReference, i32)>,
    /// `AnimationPlayer.multiTrack`
    tracks: Vec<AnimationTrack>,
    /// `SkinningData.animationClips` of the character's `AnimationPlayer`.
    clips: Rc<AnimationClips>,
    /// The transitions `AnimNode.findTransitionTo` has parsed (`AnimTransition.parse` keeps its result in the shared
    /// object), by (from node, index in its `transitions`).
    parsed_transitions: HashMap<(NodeRef, usize), Rc<AnimTransition>>,
    next_layer_id: u32,
    next_node_id: u32,
    next_track_id: u32,
    next_fresh_state: u32,
}

fn ref_equals(a: &AnimationVariableReference, b: &AnimationVariableReference) -> bool {
    string_utils::equals_ignore_case(
        a.sub_variable_source_name.as_deref(),
        b.sub_variable_source_name.as_deref(),
    ) && java::equals_ignore_case(&a.name, &b.name)
}

/// The parameters of `AnimationPlayer.play(StartAnimTrackParameters, AnimLayer)` that differ between the callers (the
/// `StartAnimTrackParameters` defaults for the rest).
struct PlayParams<'a> {
    anim_name: &'a str,
    looped: bool,
    is_primary: bool,
    speed_scale: f32,
    sync_tracking_enabled: bool,
    track_time_to_variable: Option<String>,
    initial_weight: f32,
    reverse: bool,
    priority: i32,
    deferred_bone_name: &'a str,
    use_deferred_rotation: bool,
    deferred_rotation_scale: f32,
    matching_grappled_anim_node: &'a str,
}

impl AdvancedAnimator {
    /// AdvancedAnimator.init(character, animationSetName): the root layer; the set is `AnimationSet.GetAnimationSet`'s
    /// (passed in), the callback is the owner of every call.
    pub fn init(anim_set: Option<Rc<AnimationSet>>, clips: Rc<AnimationClips>) -> AdvancedAnimator {
        AdvancedAnimator {
            anim_set,
            layers: vec![AnimLayer::alloc(1)],
            root_layer: 1,
            sub_layers: Vec::new(),
            root_state: None,
            sub_states: Vec::new(),
            set_flag_counters: Vec::new(),
            tracks: Vec::new(),
            clips,
            parsed_transitions: HashMap::new(),
            next_layer_id: 2,
            next_node_id: 1,
            next_track_id: 1,
            next_fresh_state: 1,
        }
    }

    pub fn root_layer(&self) -> &AnimLayer {
        &self.layers[0]
    }

    /// The sub layers in slot order.
    pub fn sub_layer_ids(&self) -> Vec<u32> {
        self.sub_layers.iter().map(|s| s.anim_layer).collect()
    }

    pub fn layer(&self, id: u32) -> Option<&AnimLayer> {
        self.layers.iter().find(|l| l.id == id)
    }

    pub fn track(&self, id: u32) -> Option<&AnimationTrack> {
        self.tracks.iter().find(|t| t.id == id)
    }

    /// `AnimationPlayer.getMultiTrack().getTracks()`
    pub fn tracks(&self) -> &[AnimationTrack] {
        &self.tracks
    }

    /// AdvancedAnimator.getCurrentStateName
    pub fn get_current_state_name(&self) -> Option<String> {
        self.state_name_of_layer(self.root_layer)
    }

    /// AnimLayer.getCurrentStateName of a layer.
    pub fn state_name_of_layer(&self, layer: u32) -> Option<String> {
        let l = self.layer(layer)?;
        let s = l.state.as_ref()?;
        Some(self.state_name(s))
    }

    /// AnimState.name (a state made for an unknown name has none: "")
    pub fn state_name(&self, s: &StateRef) -> String {
        if s.fresh != 0 {
            return String::new();
        }
        self.anim_set
            .as_ref()
            .and_then(|set| set.states.get(&s.key))
            .map_or_else(String::new, |st| st.name.clone())
    }

    /// LiveAnimNode.getName
    pub fn node_name(&self, n: &LiveAnimNode) -> String {
        self.anim_set
            .as_ref()
            .and_then(|set| node_of(set, &n.source).ok())
            .map_or_else(String::new, |a| a.name.clone())
    }

    /// LiveAnimNode.getAnimName
    pub fn node_anim_name(&self, n: &LiveAnimNode) -> String {
        self.anim_set
            .as_ref()
            .and_then(|set| node_of(set, &n.source).ok())
            .map_or_else(String::new, |a| n.get_anim_name(a).to_string())
    }

    /// AnimLayer.getDepth
    pub fn get_depth(&self, layer: u32) -> i32 {
        match self.layer(layer).and_then(|l| l.parent_layer) {
            Some(p) => self.get_depth(p) + 1,
            None => 0,
        }
    }

    fn set(&self) -> Result<Rc<AnimationSet>, String> {
        self.anim_set
            .clone()
            .ok_or_else(|| "AdvancedAnimator: the AnimSet is null".to_string())
    }

    fn li(&self, id: u32) -> Result<usize, String> {
        self.layers
            .iter()
            .position(|l| l.id == id)
            .ok_or_else(|| format!("AnimLayer {id} is not in the animator"))
    }

    fn find_node(&self, node_id: u32) -> Option<(usize, usize)> {
        for (li, l) in self.layers.iter().enumerate() {
            if let Some(ni) = l.live_anim_nodes.iter().position(|n| n.id == node_id) {
                return Some((li, ni));
            }
        }
        None
    }

    fn node(&self, id: u32) -> Result<&LiveAnimNode, String> {
        let (li, ni) = self
            .find_node(id)
            .ok_or_else(|| format!("LiveAnimNode {id} is not in the animator"))?;
        Ok(&self.layers[li].live_anim_nodes[ni])
    }

    fn node_mut(&mut self, id: u32) -> Result<&mut LiveAnimNode, String> {
        let (li, ni) = self
            .find_node(id)
            .ok_or_else(|| format!("LiveAnimNode {id} is not in the animator"))?;
        Ok(&mut self.layers[li].live_anim_nodes[ni])
    }

    /// The layer a live node is in (`LiveAnimNode.animLayer`).
    fn node_layer(&self, id: u32) -> Result<u32, String> {
        let (li, _) = self
            .find_node(id)
            .ok_or_else(|| format!("LiveAnimNode {id} is not in the animator"))?;
        Ok(self.layers[li].id)
    }

    fn ti(&self, id: u32) -> Option<usize> {
        self.tracks.iter().position(|t| t.id == id)
    }

    fn track_mut(&mut self, id: u32) -> Option<&mut AnimationTrack> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    /// AnimationSet.GetState: the set's state, else a new empty `AnimState`.
    fn get_state(&mut self, name: &str) -> Result<StateRef, String> {
        let set = self.set()?;
        let key = java::to_lower_case(name);
        if set.states.contains_key(&key) {
            Ok(StateRef { key, fresh: 0 })
        } else {
            let fresh = self.next_fresh_state;
            self.next_fresh_state += 1;
            Ok(StateRef {
                key: String::new(),
                fresh,
            })
        }
    }

    /// AdvancedAnimator.containsState
    pub fn contains_state(&self, state_name: &str) -> bool {
        self.anim_set
            .as_ref()
            .is_some_and(|s| s.contains_state(state_name))
    }

    // ---------------------------------------------------------------------------------------------------------
    // AdvancedAnimator: states and layers
    // ---------------------------------------------------------------------------------------------------------

    /// AdvancedAnimator.setState(String, List&lt;String&gt;): without an AnimSet the original logs an error and sets
    /// nothing.
    pub fn set_state_names(
        &mut self,
        state_name: &str,
        sub_state_names: &[String],
    ) -> Result<(), String> {
        if self.anim_set.is_none() {
            return Ok(());
        }
        let root_state = self.get_state(state_name)?;
        self.root_state = Some(root_state.clone());
        let mut sub_states = Vec::new();
        for n in sub_state_names {
            sub_states.push(self.get_state(n)?);
        }
        self.sub_states = sub_states.clone();
        self.set_state(root_state, &sub_states)
    }

    /// AdvancedAnimator.setState(AnimState, List&lt;AnimState&gt;)
    pub fn set_state(
        &mut self,
        root_state: StateRef,
        sub_states: &[StateRef],
    ) -> Result<(), String> {
        if self.is_current_state(&root_state, sub_states)? {
            return Ok(());
        }
        let root = self.root_layer;
        if self.layers[self.li(root)?].state.as_ref() != Some(&root_state) {
            self.layer_transition_to(root, Some(root_state), false)?;
        }
        self.clean_up_empty_layers()?;
        for s in &mut self.sub_layers {
            s.should_be_active = false;
        }
        for (i, sub) in sub_states.iter().enumerate() {
            if let Some(slot) = self.find_sub_layer_with_state(sub) {
                self.sub_layers[slot].should_be_active = true;
                self.sub_layers[slot].desired_layer = i as i32;
            }
        }
        for (i, sub_state) in sub_states.iter().enumerate() {
            if self.find_sub_layer_with_state(sub_state).is_none() {
                if let Some(inactive) = self.find_inactive_sub_layer_with_desired_layer(i as i32) {
                    self.sub_layers[inactive].next_state = Some(sub_state.clone());
                    self.sub_layers[inactive].should_be_active = true;
                } else {
                    let layer_id = self.next_layer_id;
                    self.next_layer_id += 1;
                    self.layers.push(AnimLayer::alloc(layer_id));
                    self.sub_layers.push(SubLayerSlot {
                        should_be_active: true,
                        desired_layer: i as i32,
                        anim_layer: layer_id,
                        next_state: Some(sub_state.clone()),
                    });
                }
            }
        }
        // PZArrayUtil.sort(subLayers, SubLayerSlot::compare): a stable sort (the comparer reads the current depths)
        let depths: HashMap<u32, i32> = self
            .sub_layers
            .iter()
            .map(|s| (s.anim_layer, self.get_depth(s.anim_layer)))
            .collect();
        self.sub_layers.sort_by(|a, b| {
            if a.desired_layer != b.desired_layer {
                a.desired_layer.cmp(&b.desired_layer)
            } else if a.should_be_active != b.should_be_active {
                a.should_be_active.cmp(&b.should_be_active)
            } else {
                depths[&a.anim_layer].cmp(&depths[&b.anim_layer])
            }
        });
        self.ensure_layer_parents()?;
        // forEach(SubLayerSlot::applyNextTransition)
        for i in 0..self.sub_layers.len() {
            let layer = self.sub_layers[i].anim_layer;
            if !self.sub_layers[i].should_be_active {
                self.layer_transition_to(layer, None, false)?;
                self.sub_layers[i].next_state = None;
            } else if let Some(next) = self.sub_layers[i].next_state.take() {
                self.layer_transition_to(layer, Some(next), false)?;
            }
        }
        self.clean_up_empty_layers()
    }

    /// SubLayerSlot.isNextOrCurrentState
    fn slot_is_next_or_current_state(&self, slot: &SubLayerSlot, state: &StateRef) -> bool {
        match &slot.next_state {
            Some(n) => n == state,
            None => self
                .layer(slot.anim_layer)
                .is_some_and(|l| l.state.as_ref() == Some(state)),
        }
    }

    /// AdvancedAnimator.findSubLayerWithState
    fn find_sub_layer_with_state(&self, state: &StateRef) -> Option<usize> {
        self.sub_layers
            .iter()
            .position(|s| self.slot_is_next_or_current_state(s, state))
    }

    /// AdvancedAnimator.findInactiveSubLayerWithDesiredLayer: the last one.
    fn find_inactive_sub_layer_with_desired_layer(&self, desired_layer: i32) -> Option<usize> {
        self.sub_layers
            .iter()
            .rposition(|s| !s.should_be_active && s.desired_layer == desired_layer)
    }

    /// AdvancedAnimator.isCurrentState (its last line compares the highest desired layer with the number of sub
    /// states, so with sub states it is never true: kept as it is).
    fn is_current_state(
        &self,
        root_state: &StateRef,
        sub_states: &[StateRef],
    ) -> Result<bool, String> {
        if self.layers[self.li(self.root_layer)?].state.as_ref() != Some(root_state) {
            return Ok(false);
        }
        let mut max_desired_layer = 0i32;
        for slot in &self.sub_layers {
            if slot.should_be_active {
                if slot.desired_layer >= sub_states.len() as i32 {
                    return Ok(false);
                }
                let wanted = sub_states
                    .get(slot.desired_layer.max(0) as usize)
                    .ok_or("java.lang.IndexOutOfBoundsException: SubLayerSlot.desiredLayer")?;
                if !self.slot_is_next_or_current_state(slot, wanted) {
                    return Ok(false);
                }
                max_desired_layer = max_desired_layer.max(slot.desired_layer);
            }
        }
        Ok(max_desired_layer == sub_states.len() as i32)
    }

    /// AdvancedAnimator.ensureLayerParents
    fn ensure_layer_parents(&mut self) -> Result<(), String> {
        let mut parent = self.root_layer;
        for i in 0..self.sub_layers.len() {
            let layer = self.sub_layers[i].anim_layer;
            self.layer_set_parent(layer, Some(parent))?;
            parent = layer;
        }
        Ok(())
    }

    /// AdvancedAnimator.cleanUpEmptyLayers: a slot that should not be active, whose layer has no state and no live
    /// node, is dropped (with its layer).
    fn clean_up_empty_layers(&mut self) -> Result<(), String> {
        let mut i = 0;
        while i < self.sub_layers.len() {
            let slot = &self.sub_layers[i];
            let layer = self.layer(slot.anim_layer);
            let stateless = layer.is_none_or(|l| l.state.is_none());
            let has_running = layer.is_some_and(|l| !l.live_anim_nodes.is_empty());
            if !slot.should_be_active && stateless && !has_running {
                let id = slot.anim_layer;
                self.sub_layers.remove(i);
                self.layers.retain(|l| l.id != id);
            } else {
                i += 1;
            }
        }
        self.ensure_layer_parents()
    }

    /// AnimLayer.setParentLayer. A changed parent calls `onTransferredToLayer` on each live node, which sets the same
    /// layer on its tracks again (no change) and installs the bone masks of a sub layer (UNKNOWN: bone weights are
    /// not ported; no timing or weight depends on them).
    fn layer_set_parent(&mut self, layer: u32, parent: Option<u32>) -> Result<(), String> {
        if Some(layer) == parent {
            return Err(
                "IllegalArgumentException: Attempting to set layer's parent to itself.".into(),
            );
        }
        let li = self.li(layer)?;
        self.layers[li].parent_layer = parent;
        Ok(())
    }

    /// AnimLayer.transitionTo(newState, force)
    fn layer_transition_to(
        &mut self,
        layer: u32,
        new_state: Option<StateRef>,
        force: bool,
    ) -> Result<bool, String> {
        let li = self.li(layer)?;
        if !force && new_state == self.layers[li].state {
            return Ok(true);
        }
        let l = &mut self.layers[li];
        l.state = new_state;
        l.no_anim_conditions_event_sent = false;
        for an in &mut l.live_anim_nodes {
            an.transitioning_out = true;
        }
        Ok(true)
    }

    /// AdvancedAnimator.reset: every layer forgets its state and releases its nodes.
    pub fn reset(&mut self, env: &mut Env) -> Result<(), String> {
        let ids: Vec<u32> = self.layers.iter().map(|l| l.id).collect();
        for id in ids {
            self.layer_reset(id, env)?;
        }
        Ok(())
    }

    /// AnimLayer.reset
    fn layer_reset(&mut self, layer: u32, env: &mut Env) -> Result<(), String> {
        let li = self.li(layer)?;
        let ids: Vec<u32> = self.layers[li]
            .live_anim_nodes
            .iter()
            .map(|n| n.id)
            .collect();
        for id in ids {
            self.release_live_node(id, env)?;
        }
        let l = &mut self.layers[li];
        l.live_anim_nodes.clear();
        l.no_anim_conditions_event_sent = false;
        l.state = None;
        Ok(())
    }

    // ---------------------------------------------------------------------------------------------------------
    // AdvancedAnimator.update
    // ---------------------------------------------------------------------------------------------------------

    /// AdvancedAnimator.update(deltaT) -> updateInternal
    pub fn update(&mut self, delta_t: f32, env: &mut Env) -> Result<(), String> {
        if !env.owner.is_player_ready() {
            return Ok(());
        }
        if self.anim_set.is_none() {
            return Ok(());
        }
        let root = self.root_layer;
        if !self.layers[self.li(root)?].has_state() {
            let idle = self.get_state("Idle")?;
            self.layer_transition_to(root, Some(idle), true)?;
        }
        self.layer_update_live_anim_nodes(root, env)?;
        for i in 0..self.sub_layers.len() {
            let layer = self.sub_layers[i].anim_layer;
            self.layer_update_live_anim_nodes(layer, env)?;
        }
        self.generate_transition_data(env)?;
        self.layer_update(root, delta_t, env)?;
        for i in 0..self.sub_layers.len() {
            let layer = self.sub_layers[i].anim_layer;
            self.layer_update(layer, delta_t, env)?;
        }
        self.clean_up_empty_layers()
    }

    /// AnimationPlayer.Update(deltaT) with `updateBones` false (`IsoGameCharacter.updateAnimPlayer` of a character
    /// that is not drawn): `multiTrack.Update(deltaT)`, each track ticking its time and invoking its listeners. The
    /// ragdoll controller, `updateAnimation_NonVisualOnly` (the deferred movement, `DoAngles`) and the bone transforms
    /// the original also runs are not ported (UNKNOWN).
    pub fn update_animation_player(&mut self, delta_t: f32, env: &mut Env) -> Result<(), String> {
        if !env.owner.is_player_ready() {
            return Ok(());
        }
        let mut n = 0;
        while n < self.tracks.len() {
            let id = self.tracks[n].id;
            let events = self.tracks[n].update_keyframes(delta_t, &*env.owner)?;
            for e in events {
                self.dispatch_track_event(id, e, env)?;
            }
            // AnimationMultiTrack.Update removes a track whose clip is gone; a listener that released the track while
            // it was ticking would make the original drop the track now at this index (UNKNOWN: not reproduced)
            if self.tracks.get(n).map(|t| t.id) != Some(id) {
                return Err(
                    "AnimationMultiTrack.Update: a listener released the track being updated (not ported)"
                        .into(),
                );
            }
            n += 1;
        }
        Ok(())
    }

    /// AnimationTrack.invokeListeners: the listeners as they were when the callback was raised, in the order added.
    /// (The original invokes each callback where it is raised, in the middle of the tick; here they run after the
    /// tick, so a listener sees the track's final time of the tick. None of the ported listeners reads it.)
    fn dispatch_track_event(
        &mut self,
        track: u32,
        event: TrackEvent,
        env: &mut Env,
    ) -> Result<(), String> {
        let Some(ti) = self.ti(track) else {
            return Ok(());
        };
        let listeners = self.tracks[ti].listeners.clone();
        for l in listeners {
            match l {
                ListenerRef::Layer(layer) => self.layer_on_track_event(layer, track, event, env)?,
                ListenerRef::Node(node) => self.node_on_track_event(node, track, event, env)?,
            }
        }
        Ok(())
    }

    /// AnimLayer.onLoopedAnim / onNonLoopedAnimFadeOut / onNonLoopedAnimFinished (onAnimStarted does nothing).
    fn layer_on_track_event(
        &mut self,
        layer: u32,
        track: u32,
        event: TrackEvent,
        env: &mut Env,
    ) -> Result<(), String> {
        // A track keeps the layer that started it as a listener even when the layer was released since (a transition
        // track outlives the sub layer of the node it blends out of). AnimLayer.onReleased clears `parentAnimator`, so
        // `invokeAnimEvent` does nothing. (UNKNOWN: the original pools its layers, so a released layer that was
        // allocated again for another slot would receive these events; that depends on the global pool's content.)
        if self.li(layer).is_err() {
            return Ok(());
        }
        match event {
            TrackEvent::AnimStarted => Ok(()),
            TrackEvent::LoopedAnim => self.layer_invoke_local_event(
                layer,
                track,
                LocalAnimEvent::ActiveAnimLooped,
                false,
                env,
            ),
            TrackEvent::NonLoopedAnimFadeOut => {
                self.layer_invoke_local_event(
                    layer,
                    track,
                    LocalAnimEvent::ActiveAnimFinishing,
                    true,
                    env,
                )?;
                self.layer_invoke_local_event(
                    layer,
                    track,
                    LocalAnimEvent::ActiveNonLoopedAnimFadeOut,
                    true,
                    env,
                )
            }
            TrackEvent::NonLoopedAnimFinished => {
                self.layer_invoke_local_event(
                    layer,
                    track,
                    LocalAnimEvent::ActiveAnimFinishing,
                    false,
                    env,
                )?;
                self.layer_invoke_local_event(
                    layer,
                    track,
                    LocalAnimEvent::ActiveNonLoopedAnimFinished,
                    true,
                    env,
                )
            }
        }
    }

    /// AnimLayer.invokeAnimEvent(track, localEvent, includeTransitioningOut): the first live node of the layer's
    /// current state that plays the track reports it.
    fn layer_invoke_local_event(
        &mut self,
        layer: u32,
        track: u32,
        local: LocalAnimEvent,
        include_transitioning_out: bool,
        env: &mut Env,
    ) -> Result<(), String> {
        let li = self.li(layer)?;
        let state = self.layers[li].state.clone();
        let mut found = None;
        for node in &self.layers[li].live_anim_nodes {
            if (!node.transitioning_out || include_transitioning_out)
                && state
                    .as_ref()
                    .is_some_and(|s| s.fresh == 0 && s.key == node.source.state)
                && node.animation_tracks.contains(&track)
            {
                found = Some(node.id);
                break;
            }
        }
        if let Some(node) = found {
            let ev = local.anim_event();
            self.layer_invoke_anim_event(layer, Some(node), Some(track), &ev, None, env)?;
        }
        Ok(())
    }

    /// AnimLayer.invokeAnimEvent(node, track, animEvent): a `FlagWhileAlive` event raises its variable once per live
    /// node, then the animator passes the event on to the character. `event_key` is the event's place in its node
    /// (the original compares the events' variable reference objects).
    fn layer_invoke_anim_event(
        &mut self,
        layer: u32,
        node: Option<u32>,
        track: Option<u32>,
        event: &AnimEvent,
        event_key: Option<(NodeRef, usize)>,
        env: &mut Env,
    ) -> Result<(), String> {
        if let AnimEventKind::FlagWhileAlive {
            variable_reference,
            flag_value,
        } = &event.kind
        {
            let node_id =
                node.ok_or("NullPointerException: FlagWhileAlive event without a node")?;
            let key = event_key.ok_or("FlagWhileAlive event without its place in the node")?;
            let n = self.node_mut(node_id)?;
            // LiveAnimNode.incrementWhileAliveFlagOnce
            if !n.while_alive_flags.iter().any(|f| f.event_key == key) {
                n.while_alive_flags.push(WhileAliveFlag {
                    event_key: key,
                    variable_reference: variable_reference.clone(),
                    while_alive_value: *flag_value,
                });
                self.increment_while_alive_flag(variable_reference, *flag_value, env)?;
            }
        }
        // AdvancedAnimator.OnAnimEvent -> the character's OnAnimEvent(sender, track, event)
        let depth = self.get_depth(layer);
        let state_name = self.state_name_of_layer(layer);
        let track_copy = track.and_then(|t| self.track(t).cloned());
        env.owner
            .on_anim_event(depth, state_name.as_deref(), track_copy.as_ref(), event)
    }

    /// AdvancedAnimator.incrementWhileAliveFlag
    fn increment_while_alive_flag(
        &mut self,
        variable_reference: &AnimationVariableReference,
        while_alive_value: bool,
        env: &mut Env,
    ) -> Result<(), String> {
        let i = match self
            .set_flag_counters
            .iter()
            .position(|(r, _)| ref_equals(r, variable_reference))
        {
            Some(i) => i,
            None => {
                self.set_flag_counters.push((variable_reference.clone(), 0));
                self.set_flag_counters.len() - 1
            }
        };
        self.set_flag_counters[i].1 += 1;
        let still_alive = self.set_flag_counters[i].1;
        env.owner.set_variable_bool(
            variable_reference,
            if still_alive > 0 {
                while_alive_value
            } else {
                !while_alive_value
            },
        )
    }

    /// AdvancedAnimator.decrementWhileAliveFlag
    fn decrement_while_alive_flag(
        &mut self,
        variable_reference: &AnimationVariableReference,
        while_alive_value: bool,
        env: &mut Env,
    ) -> Result<(), String> {
        let i = self
            .set_flag_counters
            .iter()
            .position(|(r, _)| ref_equals(r, variable_reference))
            .ok_or_else(|| {
                format!("java.lang.NullPointerException: No counter found for variable: {variable_reference:?}")
            })?;
        if self.set_flag_counters[i].1 == 0 {
            return Err(format!(
                "java.lang.IndexOutOfBoundsException: Too many decrements. var: {variable_reference:?}"
            ));
        }
        self.set_flag_counters[i].1 -= 1;
        let still_alive = self.set_flag_counters[i].1;
        env.owner.set_variable_bool(
            variable_reference,
            if still_alive > 0 {
                while_alive_value
            } else {
                !while_alive_value
            },
        )
    }

    /// LiveAnimNode.decrementWhileAliveFlags -> AnimLayer.decrementWhileAliveFlags(animNode): each flag of the node,
    /// then the node's list is cleared.
    fn node_decrement_while_alive_flags(
        &mut self,
        node_id: u32,
        env: &mut Env,
    ) -> Result<(), String> {
        let Ok(n) = self.node(node_id) else {
            return Ok(());
        };
        let flags = n.while_alive_flags.clone();
        for f in &flags {
            self.decrement_while_alive_flag(&f.variable_reference, f.while_alive_value, env)?;
        }
        self.node_mut(node_id)?.while_alive_flags.clear();
        Ok(())
    }

    // ---------------------------------------------------------------------------------------------------------
    // LiveAnimNode's IAnimListener callbacks
    // ---------------------------------------------------------------------------------------------------------

    /// LiveAnimNode.onAnimStarted / onLoopedAnim / onNonLoopedAnimFadeOut / onNonLoopedAnimFinished
    fn node_on_track_event(
        &mut self,
        node_id: u32,
        track: u32,
        event: TrackEvent,
        env: &mut Env,
    ) -> Result<(), String> {
        let Ok(n) = self.node(node_id) else {
            return Ok(());
        };
        let transitioning_out = n.transitioning_out;
        match event {
            TrackEvent::AnimStarted => {
                self.node_invoke_anim_time_event(node_id, track, AnimEventTime::Start, env)
            }
            TrackEvent::LoopedAnim => {
                if !transitioning_out {
                    self.node_invoke_anim_time_event(node_id, track, AnimEventTime::End, env)?;
                }
                Ok(())
            }
            TrackEvent::NonLoopedAnimFadeOut => {
                if ALLOW_EARLY_TRANSITION_OUT {
                    self.node_invoke_anim_time_event(node_id, track, AnimEventTime::End, env)?;
                    self.node_mut(node_id)?.transitioning_out = true;
                    self.node_decrement_while_alive_flags(node_id, env)?;
                }
                Ok(())
            }
            TrackEvent::NonLoopedAnimFinished => {
                if !transitioning_out {
                    self.node_invoke_anim_time_event(node_id, track, AnimEventTime::End, env)?;
                    self.node_decrement_while_alive_flags(node_id, env)?;
                }
                Ok(())
            }
        }
    }

    /// LiveAnimNode.invokeAnimTimeEvent: the node's events of that time, in order.
    fn node_invoke_anim_time_event(
        &mut self,
        node_id: u32,
        track: u32,
        time: AnimEventTime,
        env: &mut Env,
    ) -> Result<(), String> {
        let set = self.set()?;
        let source = self.node(node_id)?.source.clone();
        let layer = self.node_layer(node_id)?;
        let src = node_of(&set, &source)?;
        for (i, event) in src.events.iter().enumerate() {
            if event.time == Some(time) {
                self.layer_invoke_anim_event(
                    layer,
                    Some(node_id),
                    Some(track),
                    event,
                    Some((source.clone(), i)),
                    env,
                )?;
            }
        }
        Ok(())
    }

    /// LiveAnimNode.onTrackDestroyed
    fn node_on_track_destroyed(&mut self, node_id: u32, track: u32) {
        let Ok(n) = self.node_mut(node_id) else {
            return;
        };
        n.animation_tracks.retain(|t| *t != track);
        if n.transition_in.track == Some(track) {
            n.transition_in.track = None;
            n.transition_in.active = false;
            n.transition_in.weight = 0.0;
            self.set_main_tracks_playing(node_id, true);
        }
    }

    /// LiveAnimNode.setMainTracksPlaying
    fn set_main_tracks_playing(&mut self, node_id: u32, are_playing: bool) {
        let Ok(n) = self.node(node_id) else {
            return;
        };
        let ids = n.animation_tracks.clone();
        for id in ids {
            if let Some(t) = self.track_mut(id) {
                t.is_playing = are_playing;
            }
        }
    }

    // ---------------------------------------------------------------------------------------------------------
    // Tracks
    // ---------------------------------------------------------------------------------------------------------

    /// AnimationMultiTrack.removeTrack: the track leaves the list and is released, which tells its listeners.
    fn remove_track(&mut self, track: Option<u32>) {
        let Some(id) = track else {
            return;
        };
        let Some(idx) = self.ti(id) else {
            return;
        };
        let t = self.tracks.remove(idx);
        for l in t.listeners {
            // (AnimLayer.onTrackDestroyed does nothing)
            if let ListenerRef::Node(n) = l {
                self.node_on_track_destroyed(n, id);
            }
        }
    }

    /// AnimationPlayer.play(StartAnimTrackParameters, AnimLayer): the track of the clip `anim_name`, or `None` when the
    /// clip is not there (the original warns).
    fn player_play(&mut self, layer: u32, p: PlayParams) -> Option<u32> {
        // AnimationPlayer.play(String, looped, isRagdoll false, ragdollMaxTime -1): the clip by name
        let clip = self.clips.get(p.anim_name)?.clone();
        let id = self.next_track_id;
        self.next_track_id += 1;
        let mut track = AnimationTrack::alloc(id);
        track.start_clip(clip.clone(), p.looped, -1.0);
        track.name = clip.name.clone();
        track.is_playing = true;
        track.is_primary = p.is_primary;
        // (the bone masks of a sub layer and the deferred bone's lookup: not ported)
        track.set_speed_delta(p.speed_scale);
        track.sync_tracking_enabled = p.sync_tracking_enabled;
        track.track_time_to_variable = p.track_time_to_variable;
        track.deferred_bone_name = p.deferred_bone_name.to_string();
        track.use_deferred_rotation = p.use_deferred_rotation;
        track.deferred_rotation_scale = p.deferred_rotation_scale;
        track.set_blend_weight(p.initial_weight);
        track.reverse = p.reverse;
        track.priority = p.priority;
        track.ragdoll_start_time = -1.0;
        track.set_matching_grappled_anim_node(p.matching_grappled_anim_node);
        track.set_anim_layer(Some(layer));
        self.tracks.push(track);
        Some(id)
    }

    /// AnimLayer.startTrackGeneric(liveAnimNode, animName, isPrimaryTrack, speedScale, isRagdoll false, ...): the node
    /// is not yet in its layer's list (the original adds it after `startLiveNodeTracks`).
    fn start_track_generic(
        &mut self,
        layer: u32,
        node: &mut LiveAnimNode,
        src: &AnimNode,
        anim_name: &str,
        is_primary_track: bool,
        speed_scale: f32,
    ) -> Result<(), String> {
        let track = self.player_play(
            layer,
            PlayParams {
                anim_name,
                looped: src.is_looped,
                is_primary: is_primary_track,
                speed_scale,
                sync_tracking_enabled: src.sync_tracking_enabled,
                track_time_to_variable: src.track_time_to_variable.clone(),
                initial_weight: node.weight,
                reverse: src.is_anim_reverse,
                priority: src.priority,
                deferred_bone_name: src.get_deferred_bone_name(),
                use_deferred_rotation: src.use_defered_rotation,
                deferred_rotation_scale: src.deferred_rotation_scale,
                matching_grappled_anim_node: &src.matching_grappled_anim_node,
            },
        );
        if let Some(id) = track {
            if src.blend_curve.is_some() {
                return Err(
                    "AnimLayer.startTrackGeneric: a node with a blend curve (track.blendCurve) is not ported"
                        .into(),
                );
            }
            let t = self.track_mut(id).ok_or("track just started is gone")?;
            t.add_listener(ListenerRef::Node(node.id));
            // LiveAnimNode.addMainTrack
            if !src.is_looped && !src.stop_anim_on_exit && src.early_transition_out {
                let blend_out_time = node.get_blend_out_time(src);
                if blend_out_time > 0.0 && blend_out_time.is_finite() {
                    t.early_blend_out_time = blend_out_time;
                    t.trigger_on_non_looped_anim_fade_out_event = true;
                }
            }
            node.animation_tracks.push(id);
        }
        Ok(())
    }

    /// LiveAnimNode.release -> onReleased: its tracks leave the multi track, then `reset()` lets go of its
    /// while-alive flags. The node stays in its layer's list until the caller removes it.
    fn release_live_node(&mut self, node_id: u32, env: &mut Env) -> Result<(), String> {
        let n = self.node(node_id)?;
        let tracks = n.animation_tracks.clone();
        // removeAllTracks: multiTrack.removeTracks(animationTracks), removeTrack(getTransitionInTrack())
        for t in tracks {
            self.remove_track(Some(t));
        }
        let tin = self.node(node_id)?.transition_in.track;
        self.remove_track(tin);
        self.node_decrement_while_alive_flags(node_id, env)
    }

    /// LiveAnimNode.stopTransitionIn
    fn stop_transition_in(&mut self, node_id: u32) -> Result<(), String> {
        let t = self.node(node_id)?.transition_in.track;
        self.remove_track(t);
        self.node_mut(node_id)?.transition_in = TransitionIn::default();
        Ok(())
    }

    /// LiveAnimNode.startTransitionIn(AnimNode transitionFrom, AnimTransition, AnimationTrack)
    fn start_transition_in(
        &mut self,
        node_id: u32,
        from_name: String,
        transition_in: Rc<AnimTransition>,
        track: Option<u32>,
    ) -> Result<(), String> {
        if self.node(node_id)?.transition_in.track.is_some() {
            self.stop_transition_in(node_id)?;
        }
        let n = self.node_mut(node_id)?;
        n.transition_in.active = track.is_some();
        n.transition_in.transitioned_from = Some(from_name);
        n.transition_in.data = Some(transition_in);
        n.transition_in.track = track;
        n.transition_in.weight = 0.0;
        n.transition_in.raw_weight = 0.0;
        n.transition_in.blending_in = true;
        n.transition_in.blending_out = false;
        n.transition_in.time = 0.0;
        if let Some(t) = track {
            if let Some(tr) = self.track_mut(t) {
                tr.add_listener(ListenerRef::Node(node_id));
            }
        }
        self.set_main_tracks_playing(node_id, false);
        Ok(())
    }

    // ---------------------------------------------------------------------------------------------------------
    // AnimLayer: node selection and update
    // ---------------------------------------------------------------------------------------------------------

    /// AnimLayer.updateLiveAnimNodes
    fn layer_update_live_anim_nodes(&mut self, layer: u32, env: &mut Env) -> Result<(), String> {
        self.layer_remove_faded_out_nodes(layer, env)?;
        self.layer_update_node_active_flags(layer, env)
    }

    /// AnimLayer.removeFadedOutNodes
    fn layer_remove_faded_out_nodes(&mut self, layer: u32, env: &mut Env) -> Result<(), String> {
        let li = self.li(layer)?;
        let mut an_idx = self.layers[li].live_anim_nodes.len();
        while an_idx > 0 {
            an_idx -= 1;
            let n = &self.layers[li].live_anim_nodes[an_idx];
            if !n.active
                && (!n.is_transitioning_in() || !(n.transition_in.weight > 0.01))
                && !(n.weight > 0.01)
            {
                let id = n.id;
                // removeLiveNodeAt: the node leaves the list, then is released
                self.release_live_node(id, env)?;
                let li = self.li(layer)?;
                let pos = self.layers[li]
                    .live_anim_nodes
                    .iter()
                    .position(|n| n.id == id)
                    .ok_or("live node vanished while it was released")?;
                self.layers[li].live_anim_nodes.remove(pos);
            }
        }
        Ok(())
    }

    /// AnimLayer.updateNodeActiveFlags
    fn layer_update_node_active_flags(&mut self, layer: u32, env: &mut Env) -> Result<(), String> {
        let li = self.li(layer)?;
        for n in &mut self.layers[li].live_anim_nodes {
            n.active = false;
        }
        let Some(state) = self.layers[li].state.clone() else {
            return Ok(());
        };
        if env.owner.get_variable_boolean("AnimLocked") {
            return Ok(());
        }
        let set = self.set()?;
        let nodes: Vec<usize> = if state.fresh == 0 {
            match set.states.get(&state.key) {
                Some(st) => st.get_anim_nodes(&*env.owner)?,
                None => Vec::new(),
            }
        } else {
            Vec::new()
        };
        if nodes.is_empty() {
            // AnimLayer.onNoAnimConditionsPass
            if !self.layers[li].no_anim_conditions_event_sent {
                let ev = LocalAnimEvent::NoAnimConditionsPass.anim_event();
                self.layer_invoke_anim_event(layer, None, None, &ev, None, env)?;
                let li = self.li(layer)?;
                self.layers[li].no_anim_conditions_event_sent = true;
            }
        } else {
            self.layers[li].no_anim_conditions_event_sent = false;
        }
        for idx in nodes {
            self.layer_get_or_create_live_node(
                layer,
                NodeRef {
                    state: state.key.clone(),
                    index: idx,
                },
                env,
            )?;
        }
        Ok(())
    }

    /// AnimLayer.getOrCreateLiveNode / findLiveNode
    fn layer_get_or_create_live_node(
        &mut self,
        layer: u32,
        node: NodeRef,
        env: &mut Env,
    ) -> Result<(), String> {
        let set = self.set()?;
        let src = node_of(&set, &node)?;
        let li = self.li(layer)?;
        let mut found = None;
        for an in &self.layers[li].live_anim_nodes {
            if !an.transitioning_out {
                if an.source == node {
                    found = Some(an.id);
                    break;
                }
                let other = node_of(&set, &an.source)?;
                if an.source.state == node.state && other.name == src.name {
                    found = Some(an.id);
                    break;
                }
            }
        }
        if let Some(id) = found {
            self.node_mut(id)?.active = true;
            return Ok(());
        }
        let id = self.next_node_id;
        self.next_node_id += 1;
        let mut live = LiveAnimNode::alloc(id, node);
        self.start_live_node_tracks(layer, &mut live, src, env)?;
        live.active = true;
        let li = self.li(layer)?;
        self.layers[li].live_anim_nodes.push(live);
        Ok(())
    }

    /// AnimLayer.startLiveNodeTracks
    fn start_live_node_tracks(
        &mut self,
        layer: u32,
        live: &mut LiveAnimNode,
        node: &AnimNode,
        env: &mut Env,
    ) -> Result<(), String> {
        let speed_scale_real = node.get_speed_scale(&*env.owner);
        let random_alpha = env.rand.next_float(0.0, 1.0);
        let speed_scale_multiplier = pz_math::lerp(
            node.speed_scale_random_multiplier_min,
            node.speed_scale_random_multiplier_max,
            random_alpha,
        );
        let speed_scale = speed_scale_real * speed_scale_multiplier;
        let is_ragdoll = node.is_ragdoll;
        let ragdoll_start_time_min = pz_math::max(node.ragdoll_start_time_min, 0.0);
        let ragdoll_start_time_max =
            pz_math::max(node.ragdoll_start_time_max, ragdoll_start_time_min);
        let mut ragdoll_start_time = env
            .rand
            .next_float(ragdoll_start_time_min, ragdoll_start_time_max);
        if PHYSICS_HIT_REACTION {
            ragdoll_start_time = 0.0;
        }
        let start_as_ragdoll = is_ragdoll && pz_math::equal(ragdoll_start_time, 0.0, 0.01);
        let is_blend_field = !start_as_ragdoll && !node.blends2d.is_empty();
        live.is_blend_field = is_blend_field;
        if is_ragdoll {
            return Err(format!(
                "AnimLayer.startRagdollTrack: the ragdoll node '{}' is not ported (ragdoll simulation)",
                node.name
            ));
        }
        if !is_blend_field {
            self.start_primary_animation_track(layer, live, node, speed_scale, env)?;
        } else {
            for ab in &node.blends2d {
                if java::equals_ignore_case(&ab.anim_name, &node.anim_name) {
                    self.start_primary_animation_track(layer, live, node, speed_scale, env)?;
                } else {
                    // startBlendFieldTrack
                    self.start_track_generic(layer, live, node, &ab.anim_name, false, speed_scale)?;
                }
            }
        }
        Ok(())
    }

    /// AnimLayer.startPrimaryAnimationTrack: `selectRandomAnim()` (the draws), then the track.
    fn start_primary_animation_track(
        &mut self,
        layer: u32,
        live: &mut LiveAnimNode,
        node: &AnimNode,
        speed_scale: f32,
        env: &mut Env,
    ) -> Result<(), String> {
        live.cached_random_anim = node.get_random_anim(env.rand);
        let anim_name = live.get_anim_name(node).to_string();
        self.start_track_generic(layer, live, node, &anim_name, true, speed_scale)
    }

    /// AnimLayer.getHighestLiveNode
    fn get_highest_live_node(&self, li: usize) -> Option<u32> {
        let nodes = &self.layers[li].live_anim_nodes;
        let mut highest = nodes.first()?;
        for an in nodes.iter().rev() {
            if an.weight > highest.weight {
                highest = an;
            }
        }
        Some(highest.id)
    }

    /// AnimLayer.findSyncTrack
    fn find_sync_track(&self, li: usize, node: u32) -> Result<Option<u32>, String> {
        if let Some(p) = self.layers[li].parent_layer {
            let pl = self.layer(p).ok_or("parent layer missing")?;
            if let Some(t) = pl.current_sync_track {
                return Ok(Some(t));
            }
        }
        let mut sync: Option<u32> = None;
        for tid in self.node(node)?.playing_tracks() {
            let Some(t) = self.track(tid) else { continue };
            if t.sync_tracking_enabled && t.has_clip() {
                let better = match sync.and_then(|s| self.track(s)) {
                    None => true,
                    Some(s) => t.get_blend_weight() > s.get_blend_weight(),
                };
                if better {
                    sync = Some(tid);
                }
            }
        }
        Ok(sync)
    }

    /// AnimLayer.getMaximumTwist
    fn get_maximum_twist(
        &self,
        layer: u32,
        char_prev_twist: f32,
        set: &AnimationSet,
    ) -> Result<f32, String> {
        let li = self.li(layer)?;
        let maximum_possible_twist = match self.layers[li].parent_layer {
            Some(p) => self.get_maximum_twist(p, char_prev_twist, set)?,
            None => 70.0,
        };
        let mut char_twist = 0.0f32;
        let mut remaining_weight = 1.0f32;
        for ln in self.layers[li].live_anim_nodes.iter().rev() {
            let node_weight = ln.weight;
            if remaining_weight <= 0.0 {
                break;
            }
            let clamped_weight = pz_math::clamp(node_weight, 0.0, remaining_weight);
            remaining_weight -= clamped_weight;
            let src = node_of(set, &ln.source)?;
            let node_max_twist = pz_math::clamp(src.max_torso_twist, 0.0, maximum_possible_twist);
            char_twist += node_max_twist * clamped_weight;
        }
        if remaining_weight > 0.0 {
            char_twist += char_prev_twist * remaining_weight;
        }
        Ok(char_twist)
    }

    /// AnimLayer.Update -> updateInternal
    fn layer_update(&mut self, layer: u32, delta_t: f32, env: &mut Env) -> Result<(), String> {
        let set = self.set()?;
        let li = self.li(layer)?;
        let highest = self.get_highest_live_node(li);
        self.layers[li].current_sync_track = None;
        let Some(highest) = highest else {
            return Ok(());
        };
        let ids: Vec<u32> = self.layers[li]
            .live_anim_nodes
            .iter()
            .map(|n| n.id)
            .collect();
        for id in &ids {
            self.node_update(*id, delta_t, env)?;
        }
        // updateMaximumTwist: only when the character has the `maxTwist` variable
        if let Some(slot) = env.owner.get_variable("maxTwist") {
            let maximum_twist = self.get_maximum_twist(layer, slot.get_value_float(), &set)?;
            env.owner.set_variable_float("maxTwist", maximum_twist)?;
        }
        let li = self.li(layer)?;
        let sync_track = self.find_sync_track(li, highest)?;
        self.layers[li].current_sync_track = sync_track;
        let sync_value = match sync_track.and_then(|t| self.track(t)) {
            Some(t) => t.get_current_time_fraction(),
            None => -1.0,
        };
        let grappling = env.owner.is_grappling();
        let being_grappled = env.owner.is_being_grappled();
        for id in ids {
            let Ok(node) = self.node(id) else { continue };
            let source = node.source.clone();
            let src = node_of(&set, &source)?;
            let mut duration = 1.0f32;
            for tid in self.node(id)?.playing_tracks() {
                let Some(t) = self.track(tid) else { continue };
                if t.is_playing {
                    let move_to_sync = sync_track.is_some()
                        && t.sync_tracking_enabled
                        && t.is_looping()
                        && Some(tid) != sync_track;
                    let is_primary = t.is_primary;
                    if move_to_sync {
                        if let Some(t) = self.track_mut(tid) {
                            t.move_current_time_value_to_fraction(sync_value);
                        }
                    }
                    if is_primary {
                        let t = self.track(tid).ok_or("track vanished")?;
                        let (d, cur) = (t.get_duration(), t.get_current_time_value());
                        duration = d;
                        self.node_mut(id)?.node_anim_time = cur;
                    }
                }
            }
            // updateInternalGrapple
            if (grappling && src.is_grappler()) || being_grappled {
                return Err("AnimLayer.updateInternalGrapple: grappling is not ported".into());
            }
            if !src.events.is_empty() {
                let n = self.node(id)?;
                let anim_percent = n.node_anim_time / duration;
                let anim_prev_percent = n.prev_node_anim_time / duration;
                let looped = src.is_looped;
                for (i, event) in src.events.iter().enumerate() {
                    if event.time == Some(AnimEventTime::Percentage) {
                        let event_time_pc = event.time_pc;
                        let key = Some((source.clone(), i));
                        if anim_prev_percent < event_time_pc && event_time_pc <= anim_percent {
                            self.layer_invoke_anim_event(layer, Some(id), None, event, key, env)?;
                        } else {
                            if !looped && anim_percent < event_time_pc {
                                break;
                            }
                            if looped && anim_prev_percent > anim_percent {
                                if anim_prev_percent < event_time_pc
                                    && event_time_pc <= anim_percent + 1.0
                                {
                                    self.layer_invoke_anim_event(
                                        layer,
                                        Some(id),
                                        None,
                                        event,
                                        key,
                                        env,
                                    )?;
                                } else if anim_prev_percent > event_time_pc
                                    && event_time_pc <= anim_percent
                                {
                                    self.layer_invoke_anim_event(
                                        layer,
                                        Some(id),
                                        None,
                                        event,
                                        key,
                                        env,
                                    )?;
                                }
                            }
                        }
                    }
                }
            }
            if !self.node(id)?.playing_tracks().is_empty() {
                let scalar_name = &src.scalar;
                let scalar2_name = &src.scalar2;
                let n = self.node(id)?;
                let transition_in_weight = n.transition_in.weight;
                // setTransitionInBlendDelta
                if let Some(tt) = n.transition_in.track {
                    if let Some(t) = self.track_mut(tt) {
                        t.set_blend_weight(transition_in_weight);
                    }
                }
                let n = self.node(id)?;
                let node_weight = n.weight;
                let remaining_node_weight = node_weight;
                let main = n.animation_tracks.clone();
                let is_blend_field = n.is_blend_field;
                if !main.is_empty() {
                    if is_blend_field {
                        let x = env.owner.get_variable_float(scalar_name, 0.0);
                        let y = env.owner.get_variable_float(scalar2_name, 0.0);
                        self.apply_blend_field(id, src, remaining_node_weight, x, y)?;
                    } else {
                        let weight_scalar = pz_math::clamp(
                            env.owner.get_variable_float(scalar_name, 1.0),
                            0.0,
                            1.0,
                        );
                        let main_track_weight = remaining_node_weight * weight_scalar;
                        let main_track_portion_weight = main_track_weight / main.len() as f32;
                        for t in main {
                            if let Some(t) = self.track_mut(t) {
                                t.set_blend_weight(main_track_portion_weight);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// AnimLayer.applyBlendField
    fn apply_blend_field(
        &mut self,
        node_id: u32,
        src: &AnimNode,
        node_weight: f32,
        x: f32,
        y: f32,
    ) -> Result<(), String> {
        let n = self.node(node_id)?;
        let main = n.animation_tracks.clone();
        if n.active {
            let picker = src
                .blend2d_picker
                .as_ref()
                .ok_or("java.lang.NullPointerException: AnimNode.blend2dPicker")?;
            let mut pick = picker.pick(x, y, &src.blend_tris, &src.blends2d)?;
            if pick.scale1.is_nan() {
                pick.scale1 = 0.5;
            }
            if pick.scale2.is_nan() {
                pick.scale2 = 0.5;
            }
            if pick.scale3.is_nan() {
                pick.scale3 = 0.5;
            }
            for (i, tid) in main.iter().enumerate() {
                if i >= src.blends2d.len() {
                    return Err("java.lang.IndexOutOfBoundsException: AnimNode.blends2d".into());
                }
                let mut blend_field_weight = if Some(i) == pick.node1 {
                    pick.scale1
                } else if Some(i) == pick.node2 {
                    pick.scale2
                } else if Some(i) == pick.node3 {
                    pick.scale3
                } else {
                    0.0
                };
                if blend_field_weight < 1.0e-4 {
                    blend_field_weight = 0.0;
                }
                blend_field_weight = pz_math::clamp(blend_field_weight, 0.0, 1.0);
                if let Some(t) = self.track_mut(*tid) {
                    t.set_blend_field_weight(blend_field_weight);
                }
            }
        }
        for tid in main {
            if let Some(t) = self.track_mut(tid) {
                t.set_blend_weight(node_weight);
            }
        }
        Ok(())
    }

    /// LiveAnimNode.getMaxDuration
    fn node_max_duration(&self, node_id: u32) -> Result<f32, String> {
        let mut max_duration = 0.0f32;
        for t in &self.node(node_id)?.animation_tracks {
            if let Some(t) = self.track(*t) {
                max_duration = pz_math::max(t.get_duration(), max_duration);
            }
        }
        Ok(max_duration)
    }

    /// LiveAnimNode.getMainInitialRewindTime
    fn node_main_initial_rewind_time(
        &self,
        node_id: u32,
        src: &AnimNode,
        env: &mut Env,
    ) -> Result<f32, String> {
        let mut random_advance_time = 0.0f32;
        if src.random_advance_fraction > 0.0 {
            let advance_frac = env.rand.next_float(0.0, src.random_advance_fraction);
            random_advance_time = advance_frac * self.node_max_duration(node_id)?;
        }
        let n = self.node(node_id)?;
        match &n.transition_in.data {
            None => Ok(0.0 - random_advance_time),
            Some(data) => {
                let blend_in_time = n.get_blend_in_time(src);
                let sync_adjust_time = data.sync_adjust_time;
                Ok(if n.transition_in.track.is_some() {
                    blend_in_time - sync_adjust_time
                } else {
                    blend_in_time - sync_adjust_time - random_advance_time
                })
            }
        }
    }

    /// LiveAnimNode.getTrackTimeToVariable
    fn node_track_time_to_variable(
        &self,
        node_id: u32,
        src: &AnimNode,
        default_time: f32,
        env: &Env,
    ) -> Result<f32, String> {
        let anim_length = self.node_max_duration(node_id)?;
        let name = src.track_time_to_variable.as_deref().unwrap_or("");
        let time_fraction = env
            .owner
            .get_variable_float(name, default_time / anim_length);
        Ok(anim_length * time_fraction)
    }

    /// LiveAnimNode.update
    fn node_update(&mut self, id: u32, time_delta: f32, env: &mut Env) -> Result<(), String> {
        let set = self.set()?;
        let source = self.node(id)?.source.clone();
        let src = node_of(&set, &source)?;
        {
            let n = self.node_mut(id)?;
            n.is_new = false;
            if n.active != n.was_active {
                n.blending_in = n.active;
                n.blending_out = !n.active;
                if n.transition_in.active {
                    n.transition_in.blending_in = n.active;
                    n.transition_in.blending_out = !n.active;
                }
                n.was_active = n.active;
            }
        }
        let was_main_anim_active = self.node(id)?.is_main_anim_active();
        if self.node(id)?.is_transitioning_in() {
            self.update_transitioning_in(id, src, time_delta, env)?;
        }
        let main_anim_active = self.node(id)?.is_main_anim_active();
        if main_anim_active {
            if self.node(id)?.blending_out && src.stop_anim_on_exit {
                self.set_main_tracks_playing(id, false);
            } else {
                self.set_main_tracks_playing(id, true);
            }
        } else {
            self.set_main_tracks_playing(id, false);
        }
        if main_anim_active {
            let main_anim_started = !was_main_anim_active;
            if main_anim_started && src.is_looped {
                let rewind_amount = self.node_main_initial_rewind_time(id, src, env)?;
                let ids = self.node(id)?.animation_tracks.clone();
                for t in ids {
                    if let Some(t) = self.track_mut(t) {
                        t.scaled_rewind(rewind_amount);
                    }
                }
            }
            let n = self.node_mut(id)?;
            if n.blending_in {
                n.update_blending_in(src, time_delta);
            } else if n.blending_out {
                n.update_blending_out(src, time_delta);
            }
            n.prev_node_anim_time = n.node_anim_time;
            if src
                .track_time_to_variable
                .as_deref()
                .is_none_or(str::is_empty)
            {
                n.node_anim_time += time_delta;
            } else {
                let default_time = n.node_anim_time + time_delta;
                let t = self.node_track_time_to_variable(id, src, default_time, env)?;
                let n = self.node_mut(id)?;
                n.node_anim_time = t;
                if n.prev_node_anim_time < n.node_anim_time - time_delta * 1.1 {
                    n.prev_node_anim_time = n.node_anim_time;
                }
            }
            let n = self.node(id)?;
            if !n.transition_in.active {
                if let Some(tt) = n.transition_in.track {
                    if self.track(tt).is_some_and(|t| t.get_blend_weight() <= 0.0) {
                        self.stop_transition_in(id)?;
                    }
                }
            }
            // updateRagdollTracks: no ragdoll tracks exist (ragdoll nodes are not ported)
        }
        Ok(())
    }

    /// LiveAnimNode.updateTransitioningIn
    fn update_transitioning_in(
        &mut self,
        id: u32,
        src: &AnimNode,
        time_delta: f32,
        env: &mut Env,
    ) -> Result<(), String> {
        let n = self.node(id)?;
        let tt = n.transition_in.track.ok_or("transition-in track missing")?;
        let (speed_delta, transition_duration, time) = {
            let t = self
                .track(tt)
                .ok_or("transition-in track is not in the multi track")?;
            (
                t.get_speed_delta(),
                t.get_duration(),
                t.get_current_time_value(),
            )
        };
        self.node_mut(id)?.transition_in.time = time;
        if !(time >= transition_duration) && !DISABLE_ANIMATION_BLENDS {
            if !self.node(id)?.transition_in.blending_out {
                let data = self
                    .node(id)?
                    .transition_in
                    .data
                    .clone()
                    .ok_or("transition-in data missing")?;
                let conditions_pass = AnimCondition::pass(&*env.owner, &data.conditions)?;
                if !conditions_pass {
                    let n = self.node_mut(id)?;
                    n.transition_in.blending_in = false;
                    n.transition_in.blending_out = true;
                }
            }
            let blend_out_time = self.node(id)?.get_blend_in_time(src) * speed_delta;
            if time >= transition_duration - blend_out_time {
                let n = self.node_mut(id)?;
                n.transition_in.blending_in = false;
                n.transition_in.blending_out = true;
            }
            if self.node(id)?.transition_in.blending_in {
                let n = self.node_mut(id)?;
                let blend_time_max = n.get_transition_in_blend_in_time() * speed_delta;
                let next_time = LiveAnimNode::increment_blend_time(
                    n.transition_in.raw_weight,
                    blend_time_max,
                    time_delta * speed_delta,
                );
                let raw_weight = pz_math::clamp(next_time / blend_time_max, 0.0, 1.0);
                n.transition_in.raw_weight = raw_weight;
                n.transition_in.weight = pz_math::lerp_func_ease_out_in_quad(raw_weight);
                n.transition_in.blending_in = next_time < blend_time_max;
                n.transition_in.active = next_time < transition_duration;
            }
            if self.node(id)?.transition_in.blending_out {
                let n = self.node_mut(id)?;
                let blend_time_max = n.get_blend_in_time(src) * speed_delta;
                let next_time = LiveAnimNode::increment_blend_time(
                    1.0 - n.transition_in.raw_weight,
                    blend_time_max,
                    time_delta * speed_delta,
                );
                let raw_weight = pz_math::clamp(1.0 - next_time / blend_time_max, 0.0, 1.0);
                n.transition_in.raw_weight = raw_weight;
                n.transition_in.weight = pz_math::lerp_func_ease_out_in_quad(raw_weight);
                n.transition_in.blending_out = next_time < blend_time_max;
                n.transition_in.active = n.transition_in.blending_out;
            }
        } else {
            self.stop_transition_in(id)?;
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------------------------------------
    // AdvancedAnimator.GenerateTransitionData
    // ---------------------------------------------------------------------------------------------------------

    /// AnimLayer.FindTransitioningLiveAnimNode
    fn layer_find_transitioning_live_anim_node(
        &self,
        layer: u32,
        proxy: &mut TransitionNodeProxy,
        is_root_layer: bool,
    ) -> Result<(), String> {
        let li = self.li(layer)?;
        for n in &self.layers[li].live_anim_nodes {
            if n.is_new && n.was_activated() {
                let already_started = proxy
                    .all_new_nodes
                    .iter()
                    .any(|(other, _)| self.node(*other).is_ok_and(|o| o.source == n.source));
                if !already_started {
                    proxy.all_new_nodes.push((n.id, layer));
                }
            } else if (n.was_deactivated() && is_root_layer)
                || (!is_root_layer && n.transitioning_out)
            {
                let already_started = proxy
                    .all_outgoing_nodes
                    .iter()
                    .any(|(other, _)| self.node(*other).is_ok_and(|o| o.source == n.source));
                if !already_started {
                    proxy.all_outgoing_nodes.push((n.id, layer));
                }
            }
        }
        Ok(())
    }

    /// AnimNode.findTransitionTo: the first transition of `from` to a node of that name whose conditions pass.
    fn find_transition_to(
        &mut self,
        owner: &dyn IAnimationVariableSource,
        from: &NodeRef,
        to: &NodeRef,
    ) -> Result<Option<Rc<AnimTransition>>, String> {
        let set = self.set()?;
        let from_node = node_of(&set, from)?;
        let to_node = node_of(&set, to)?;
        for (i, trans) in from_node.transitions.iter().enumerate() {
            if string_utils::equals_ignore_case(trans.target.as_deref(), Some(&to_node.name)) {
                let key = (from.clone(), i);
                let parsed = match self.parsed_transitions.get(&key) {
                    Some(p) => p.clone(),
                    None => {
                        let mut t = trans.clone();
                        t.parse(&from_node.name, &to_node.name)?;
                        let rc = Rc::new(t);
                        self.parsed_transitions.insert(key, rc.clone());
                        rc
                    }
                };
                if AnimCondition::pass(owner, &parsed.conditions)? {
                    return Ok(Some(parsed));
                }
            }
        }
        Ok(None)
    }

    /// AdvancedAnimator.GenerateTransitionData
    fn generate_transition_data(&mut self, env: &mut Env) -> Result<(), String> {
        let mut proxy = TransitionNodeProxy::default();
        self.layer_find_transitioning_live_anim_node(self.root_layer, &mut proxy, true)?;
        for i in 0..self.sub_layers.len() {
            let layer = self.sub_layers[i].anim_layer;
            self.layer_find_transitioning_live_anim_node(layer, &mut proxy, false)?;
        }
        if !proxy.all_new_nodes.is_empty() && !proxy.all_outgoing_nodes.is_empty() {
            // FindTransitionsFromProxy
            for &(new_node, _layer_in) in &proxy.all_new_nodes.clone() {
                let to = self.node(new_node)?.source.clone();
                for &(old_node, layer_out) in &proxy.all_outgoing_nodes.clone() {
                    let from = self.node(old_node)?.source.clone();
                    if to != from {
                        if let Some(t) = self.find_transition_to(&*env.owner, &from, &to)? {
                            proxy.found_transitions.push(TransitionData {
                                new_node,
                                old_node,
                                transition_out: t,
                                layer_out,
                            });
                        }
                    }
                }
            }
            // ProcessTransitions
            for t in &proxy.found_transitions {
                let track = self.start_transition_animation(t, env)?;
                let from_name = {
                    let set = self.set()?;
                    node_of(&set, &self.node(t.old_node)?.source)?.name.clone()
                };
                self.start_transition_in(t.new_node, from_name, t.transition_out.clone(), track)?;
                self.node_mut(t.old_node)?.transition_out = Some(t.transition_out.clone());
            }
        }
        Ok(())
    }

    /// AnimLayer.startTransitionAnimation
    fn start_transition_animation(
        &mut self,
        t: &TransitionData,
        env: &mut Env,
    ) -> Result<Option<u32>, String> {
        let anim_name = match &t.transition_out.anim_name {
            Some(n) if !string_utils::is_null_or_whitespace(Some(n)) => n.clone(),
            _ => return Ok(None),
        };
        let set = self.set()?;
        let new_src = node_of(&set, &self.node(t.new_node)?.source)?;
        let old_src = node_of(&set, &self.node(t.old_node)?.source)?;
        let mut speed_scale = t.transition_out.speed_scale;
        if speed_scale == f32::INFINITY {
            speed_scale = new_src.get_speed_scale(&*env.owner);
        }
        // TransitionNodeProxyData.getDeferred*: the transition's own when it names a bone, else the old node's
        let own =
            !string_utils::is_null_or_whitespace(t.transition_out.deferred_bone_name.as_deref());
        let (use_rotation, rotation_scale, bone) = if own {
            (
                t.transition_out.use_defered_rotation,
                t.transition_out.deferred_rotation_scale,
                t.transition_out
                    .deferred_bone_name
                    .clone()
                    .unwrap_or_default(),
            )
        } else {
            (
                old_src.use_defered_rotation,
                old_src.deferred_rotation_scale,
                old_src.get_deferred_bone_name().to_string(),
            )
        };
        // the StartAnimTrackParameters defaults: not looped, not primary, no sync tracking, initial weight 0
        Ok(self.player_play(
            t.layer_out,
            PlayParams {
                anim_name: &anim_name,
                looped: false,
                is_primary: false,
                speed_scale,
                sync_tracking_enabled: false,
                track_time_to_variable: None,
                initial_weight: 0.0,
                reverse: false,
                priority: old_src.priority,
                deferred_bone_name: &bone,
                use_deferred_rotation: use_rotation,
                deferred_rotation_scale: rotation_scale,
                matching_grappled_anim_node: "",
            },
        ))
    }
}
