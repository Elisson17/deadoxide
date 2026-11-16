//! zombie.core.skinnedmodel.advancedanimation.AnimNode: one node of an animation state, read from its XML file
//! through `PZXmlUtil.parse` (JAXB, `@XmlRootElement(name = "animNode")`).
use std::collections::HashMap;

use super::anim_2d_blend::Anim2DBlend;
use super::anim_2d_blend_picker::Anim2DBlendPicker;
use super::anim_2d_blend_triangle::Anim2DBlendTriangle;
use super::anim_bone_weight::AnimBoneWeight;
use super::anim_condition::{AnimCondition, Type};
use super::anim_event::AnimEvent;
use super::anim_transition::AnimTransition;
use super::grapple_offset_behaviour::GrappleOffsetBehaviour;
use super::i_animation_variable_source::IAnimationVariableSource;
use crate::java;
use crate::java::Random;
use crate::zombie::core::math::interpolators::xml::interpolator_slot::InterpolatorSlot;
use crate::zombie::core::random::rand_abstract::RandAbstract;
use crate::zombie::core::skinnedmodel::animation::bone_axis::BoneAxis;
use crate::zombie::util::pz_xml_util::{
    jaxb_parse_boolean, jaxb_parse_float, jaxb_parse_int, parse_xml_resolved, xml_trim, XmlElement,
};
use crate::zombie::util::string_utils;
use crate::zombie::zomboid_file_system::ZomboidFileSystem;

/// What one JAXB `unmarshal` call keeps while it reads a node: the document's ID table (`DefaultIDResolver`: id →
/// index of the `Anim2DBlend` in the node's list) and the IDREFs not yet bound (`UnmarshallingContext` patchers,
/// run at the end of the document).
#[derive(Default)]
pub struct JaxbContext {
    ids: HashMap<String, usize>,
    patchers: Vec<(String, usize, usize)>,
}

impl JaxbContext {
    /// com.sun.xml.bind.v2.runtime.unmarshaller.DefaultIDResolver.bind
    pub fn bind_id(&mut self, id: &str, blend: usize) {
        self.ids.insert(id.to_string(), blend);
    }

    /// com.sun.xml.bind.v2.runtime.reflect.TransducedAccessor.IDREFTransducedAccessorImpl.parse: the trimmed id's blend
    /// when it is bound already; else a patcher for field `slot` of triangle `triangle` (`None` now).
    pub fn idref(&mut self, text: &str, triangle: usize, slot: usize) -> Option<usize> {
        let id = xml_trim(text).to_string();
        match self.ids.get(&id) {
            Some(&b) => Some(b),
            None => {
                self.patchers.push((id, triangle, slot));
                None
            }
        }
    }
}

/// zombie.core.skinnedmodel.advancedanimation.AnimNode: the `@XmlElement` fields with their initialisers, and the
/// `@XmlTransient` ones `Parse` fills (`isIdleAnim`, `speedScaleF`, `blend2dPicker`, `transitionOut` as an index into
/// `transitions`). `parentState` is the state that holds the node; the events' `parentAnimNode` is the node.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimNode {
    pub name: String,
    pub is_idle_anim: bool,
    pub priority: i32,
    pub condition_priority: i32,
    pub anim_name: String,
    pub alternate_anims: Vec<String>,
    pub matching_grappled_anim_node: String,
    pub grapple_offset_forward: f32,
    pub grapple_offset_yaw: f32,
    pub grapple_tween_in_time: f32,
    pub grappler_offset_behaviour: Option<GrappleOffsetBehaviour>,
    pub is_ragdoll: bool,
    pub chance_to_ragdoll: f32,
    pub ragdoll_start_time_min: f32,
    pub ragdoll_start_time_max: f32,
    pub ragdoll_max_time: f32,
    pub deferred_bone_name: String,
    pub deferred_bone_axis: Option<BoneAxis>,
    pub use_defered_rotation: bool,
    pub use_deferred_movement: bool,
    pub deferred_rotation_scale: f32,
    pub is_looped: bool,
    pub blend_time: f32,
    pub blend_out_time: f32,
    pub blend_curve: Option<InterpolatorSlot>,
    pub stop_anim_on_exit: bool,
    pub early_transition_out: bool,
    pub speed_scale: String,
    pub speed_scale_variable: Option<String>,
    pub speed_scale_random_multiplier_min: f32,
    pub speed_scale_random_multiplier_max: f32,
    pub speed_scale_f: f32,
    pub random_advance_fraction: f32,
    pub max_torso_twist: f32,
    pub scalar: String,
    pub scalar2: String,
    pub is_anim_reverse: bool,
    pub sync_tracking_enabled: bool,
    pub track_time_to_variable: Option<String>,
    pub blends2d: Vec<Anim2DBlend>,
    pub conditions: Vec<AnimCondition>,
    pub events: Vec<AnimEvent>,
    pub blend_tris: Vec<Anim2DBlendTriangle>,
    pub transitions: Vec<AnimTransition>,
    pub sub_state_bone_weights: Vec<AnimBoneWeight>,
    pub blend2d_picker: Option<Anim2DBlendPicker>,
    pub transition_out: Option<usize>,
}

impl Default for AnimNode {
    /// zombie.core.skinnedmodel.advancedanimation.AnimNode(): the field initialisers.
    fn default() -> Self {
        AnimNode {
            name: String::new(),
            is_idle_anim: false,
            priority: 5,
            condition_priority: 0,
            anim_name: String::new(),
            alternate_anims: Vec::new(),
            matching_grappled_anim_node: String::new(),
            grapple_offset_forward: 0.0,
            grapple_offset_yaw: 0.0,
            grapple_tween_in_time: 0.25,
            grappler_offset_behaviour: Some(GrappleOffsetBehaviour::Grappled),
            is_ragdoll: false,
            chance_to_ragdoll: 1.0,
            ragdoll_start_time_min: 0.0,
            ragdoll_start_time_max: 0.0,
            ragdoll_max_time: 5.0,
            deferred_bone_name: "Translation_Data".to_string(),
            deferred_bone_axis: Some(BoneAxis::Y),
            use_defered_rotation: false,
            use_deferred_movement: true,
            deferred_rotation_scale: 1.0,
            is_looped: true,
            blend_time: 0.0,
            blend_out_time: -1.0,
            blend_curve: None,
            stop_anim_on_exit: false,
            early_transition_out: false,
            speed_scale: "1.00".to_string(),
            speed_scale_variable: None,
            speed_scale_random_multiplier_min: 1.0,
            speed_scale_random_multiplier_max: 1.0,
            speed_scale_f: f32::INFINITY,
            random_advance_fraction: 0.0,
            max_torso_twist: 15.0,
            scalar: String::new(),
            scalar2: String::new(),
            is_anim_reverse: false,
            sync_tracking_enabled: true,
            track_time_to_variable: None,
            blends2d: Vec::new(),
            conditions: Vec::new(),
            events: Vec::new(),
            blend_tris: Vec::new(),
            transitions: Vec::new(),
            sub_state_bone_weights: Vec::new(),
            blend2d_picker: None,
            transition_out: None,
        }
    }
}

/// java.lang.Float.compare: `<` and `>` first, then the bits with every NaN as the canonical NaN (so -0.0 < 0.0 and
/// NaN is above everything).
pub fn float_compare(a: f32, b: f32) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if a < b {
        return Ordering::Less;
    }
    if a > b {
        return Ordering::Greater;
    }
    let bits = |f: f32| {
        if f.is_nan() {
            0x7fc0_0000i32
        } else {
            f.to_bits() as i32
        }
    };
    bits(a).cmp(&bits(b))
}

impl AnimNode {
    /// JAXB's unmarshalling of an `animNode` element (`PZXmlUtil.unmarshall`): each known child element sets its
    /// field (strings as the element text; ints, floats and booleans through the JAXB conversions, a value that does
    /// not convert keeping the field; an unknown enum constant null; a repeated single element keeps the last), list
    /// and array fields collect their elements in order, and unknown elements and attributes are ignored. The ID table
    /// of the `m_2DBlends` and the IDREFs of the `m_2DBlendTri` are resolved as JAXB does (now or at the end).
    pub fn from_xml(e: &XmlElement) -> Self {
        let mut n = AnimNode::default();
        let mut ctx = JaxbContext::default();
        let mut conditions = Vec::new();
        for c in e.elements() {
            let text = || c.text();
            let float = |f: &mut f32| {
                if let Some(v) = jaxb_parse_float(&c.text()) {
                    *f = v;
                }
            };
            let flag = |f: &mut bool| {
                if let Some(v) = jaxb_parse_boolean(&c.text()) {
                    *f = v;
                }
            };
            let int = |f: &mut i32| {
                if let Some(v) = jaxb_parse_int(&c.text()) {
                    *f = v;
                }
            };
            match c.name.as_str() {
                "m_Name" => n.name = text(),
                "m_Priority" => int(&mut n.priority),
                "m_ConditionPriority" => int(&mut n.condition_priority),
                "m_AnimName" => n.anim_name = text(),
                "m_AlternateAnims" => n.alternate_anims.push(text()),
                "m_MatchingGrappledAnimNode" => n.matching_grappled_anim_node = text(),
                "m_GrappleOffsetForward" => float(&mut n.grapple_offset_forward),
                "m_GrappleOffsetYaw" => float(&mut n.grapple_offset_yaw),
                "m_GrappleTweenInTime" => float(&mut n.grapple_tween_in_time),
                "m_GrapplerOffsetBehaviour" => {
                    n.grappler_offset_behaviour = GrappleOffsetBehaviour::from_xml(&text());
                }
                "m_isRagdoll" => flag(&mut n.is_ragdoll),
                "m_chanceToRagdoll" => float(&mut n.chance_to_ragdoll),
                "m_ragdollStartTimeMin" => float(&mut n.ragdoll_start_time_min),
                "m_ragdollStartTimeMax" => float(&mut n.ragdoll_start_time_max),
                "m_ragdollMaxTime" => float(&mut n.ragdoll_max_time),
                "m_DeferredBoneName" => n.deferred_bone_name = text(),
                "m_deferredBoneAxis" => n.deferred_bone_axis = BoneAxis::from_xml(&text()),
                "m_useDeferedRotation" => flag(&mut n.use_defered_rotation),
                "m_useDeferredMovement" => flag(&mut n.use_deferred_movement),
                "m_deferredRotationScale" => float(&mut n.deferred_rotation_scale),
                "m_Looped" => flag(&mut n.is_looped),
                "m_BlendTime" => float(&mut n.blend_time),
                "m_BlendOutTime" => float(&mut n.blend_out_time),
                "m_BlendCurve" => n.blend_curve = Some(InterpolatorSlot::from_xml(c)),
                "m_StopAnimOnExit" => flag(&mut n.stop_anim_on_exit),
                "m_EarlyTransitionOut" => flag(&mut n.early_transition_out),
                "m_SpeedScale" => n.speed_scale = text(),
                "m_SpeedScaleVariable" => n.speed_scale_variable = Some(text()),
                "m_SpeedScaleRandomMultiplierMin" => {
                    float(&mut n.speed_scale_random_multiplier_min);
                }
                "m_SpeedScaleRandomMultiplierMax" => {
                    float(&mut n.speed_scale_random_multiplier_max);
                }
                "m_randomAdvanceFraction" => float(&mut n.random_advance_fraction),
                "m_maxTorsoTwist" => float(&mut n.max_torso_twist),
                "m_Scalar" => n.scalar = text(),
                "m_Scalar2" => n.scalar2 = text(),
                "m_AnimReverse" => flag(&mut n.is_anim_reverse),
                "m_SyncTrackingEnabled" => flag(&mut n.sync_tracking_enabled),
                "m_TrackTimeToVariable" => n.track_time_to_variable = Some(text()),
                "m_2DBlends" => {
                    let b = Anim2DBlend::from_xml(c, n.blends2d.len(), &mut ctx);
                    n.blends2d.push(b);
                }
                "m_Conditions" => conditions.push(AnimCondition::from_xml(c)),
                "m_Events" => n.events.push(AnimEvent::from_xml(c)),
                "m_2DBlendTri" => {
                    let t = Anim2DBlendTriangle::from_xml(c, n.blend_tris.len(), &mut ctx);
                    n.blend_tris.push(t);
                }
                "m_Transitions" => n.transitions.push(AnimTransition::from_xml(c)),
                "m_SubStateBoneWeights" => {
                    n.sub_state_bone_weights.push(AnimBoneWeight::from_xml(c));
                }
                _ => {}
            }
        }
        if !conditions.is_empty() {
            n.conditions = conditions;
        }
        // the patchers, at the end of the document
        for (id, triangle, slot) in std::mem::take(&mut ctx.patchers) {
            if let Some(&b) = ctx.ids.get(&id) {
                n.blend_tris[triangle].set(slot, Some(b));
            }
        }
        n
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimNode.Parse: the file (`PZXmlUtil.parse`), then `isIdleAnim`
    /// (the name contains "Idle"), the 2D blend picker over the triangles, a `canRagdoll` BOOL true condition added to
    /// a ragdoll node, every condition parsed against this node, the `SetVariable` / `FlagWhileAlive` events (names
    /// ignoring case) replaced by their subclasses, the events sorted by time (stable, `Float.compare`), `speedScaleF`
    /// (`Float.parseFloat`, else the speed scale text becomes `speedScaleVariable`), the five default bone weights when
    /// there are none, and `transitionOut` (the last transition without a target). `Ok(None)` is the original's null
    /// (the PZXmlParserException it logs); `Err` is an exception it does not catch.
    pub fn parse(fs: &ZomboidFileSystem, source: &str) -> Result<Option<AnimNode>, String> {
        let root = match parse_xml_resolved(fs, source) {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };
        if root.name != "animNode" {
            // JAXB's answer to another root element is not measured; taken as the parse failure here.
            return Ok(None);
        }
        let mut n = AnimNode::from_xml(&root);
        n.is_idle_anim = n.name.contains("Idle");
        if !n.blend_tris.is_empty() {
            n.blend2d_picker = Some(Anim2DBlendPicker::set_pick_triangles(
                &n.blend_tris,
                &n.blends2d,
            )?);
        }
        if n.is_ragdoll {
            n.conditions.push(AnimCondition {
                type_: Some(Type::Bool),
                bool_value: true,
                name: "canRagdoll".to_string(),
                ..AnimCondition::default()
            });
        }
        let name = n.name.clone();
        for c in &mut n.conditions {
            c.parse(&name, None)?;
        }
        let events = std::mem::take(&mut n.events);
        for ev in events {
            let is = |s: &str| {
                ev.event_name
                    .as_deref()
                    .is_some_and(|n| java::equals_ignore_case(s, n))
            };
            let ev = if is("SetVariable") {
                ev.into_set_variable()?
            } else if is("FlagWhileAlive") {
                ev.into_flag_while_alive()?
            } else {
                ev
            };
            n.events.push(ev);
        }
        n.events.sort_by(|a, b| float_compare(a.time_pc, b.time_pc));
        match java::parse_float(&n.speed_scale) {
            Some(v) => n.speed_scale_f = v,
            None => n.speed_scale_variable = Some(n.speed_scale.clone()),
        }
        if n.sub_state_bone_weights.is_empty() {
            n.sub_state_bone_weights = vec![
                AnimBoneWeight::new("Bip01_Spine1", 0.5),
                AnimBoneWeight::new("Bip01_Neck", 1.0),
                AnimBoneWeight::new("Bip01_BackPack", 1.0),
                AnimBoneWeight::new("Bip01_Prop1", 1.0),
                AnimBoneWeight::new("Bip01_Prop2", 1.0),
            ];
        }
        n.transition_out = None;
        for (i, t) in n.transitions.iter().enumerate() {
            if string_utils::is_null_or_whitespace(t.target.as_deref()) {
                n.transition_out = Some(i);
            }
        }
        Ok(Some(n))
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimNode.isAbstract: not a ragdoll, no animation name, no alternates
    /// and no 2D blends.
    pub fn is_abstract(&self) -> bool {
        if self.is_ragdoll {
            return false;
        }
        if !string_utils::is_null_or_whitespace(Some(&self.anim_name)) {
            return false;
        }
        if !self.alternate_anims.is_empty() {
            return false;
        }
        self.blends2d.is_empty()
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimNode.compareSelectionConditions(AnimNode, AnimNode): abstract
    /// first, then by condition priority, then by number of conditions.
    pub fn compare_selection_conditions(a: &AnimNode, b: &AnimNode) -> i32 {
        if a.is_abstract() != b.is_abstract() {
            return if a.is_abstract() { -1 } else { 1 };
        }
        if a.condition_priority < b.condition_priority {
            return -1;
        }
        if a.condition_priority > b.condition_priority {
            return 1;
        }
        if a.conditions.len() < b.conditions.len() {
            return -1;
        }
        if a.conditions.len() > b.conditions.len() {
            1
        } else {
            0
        }
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimNode.checkConditions: `AnimCondition.pass` of the node's
    /// conditions.
    pub fn check_conditions(
        &self,
        var_source: &dyn IAnimationVariableSource,
    ) -> Result<bool, String> {
        AnimCondition::pass(var_source, &self.conditions)
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimNode.getSpeedScale: the parsed number, else the variable named
    /// by the speed scale text (1.0 when the owner has none).
    pub fn get_speed_scale(&self, var_source: &dyn IAnimationVariableSource) -> f32 {
        if self.speed_scale_f != f32::INFINITY {
            self.speed_scale_f
        } else {
            var_source.get_variable_float(&self.speed_scale, 1.0)
        }
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimNode.getBlendOutTime: the out-transition's blend-out time when
    /// the node has one, else `blendOutTime` when not negative, else `blendTime`.
    pub fn get_blend_out_time(&self) -> f32 {
        if let Some(t) = self.transition_out {
            return self.transitions[t].blend_out_time;
        }
        if self.blend_out_time >= 0.0 {
            self.blend_out_time
        } else {
            self.blend_time
        }
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimNode.getDeferredBoneName: "Translation_Data" when blank.
    pub fn get_deferred_bone_name(&self) -> &str {
        if string_utils::is_null_or_whitespace(Some(&self.deferred_bone_name)) {
            "Translation_Data"
        } else {
            &self.deferred_bone_name
        }
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimNode.isGrappler: a matching grappled node is named.
    pub fn is_grappler(&self) -> bool {
        !string_utils::is_null_or_whitespace(Some(&self.matching_grappled_anim_node))
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimNode.getRandomAnim: the animation name, or an alternate: with
    /// both, `Rand.Next(1 + alternates)` picks the name on 0, else `PZArrayUtil.pickRandom` draws again.
    pub fn get_random_anim(&self, rand: &mut RandAbstract<Random>) -> String {
        let empty_name = self.anim_name.is_empty();
        if empty_name && self.alternate_anims.is_empty() {
            return String::new();
        }
        if empty_name {
            return pick_random(&self.alternate_anims, rand);
        }
        if self.alternate_anims.is_empty() {
            return self.anim_name.clone();
        }
        let idx = rand.next(1 + self.alternate_anims.len() as i32);
        if idx == 0 {
            self.anim_name.clone()
        } else {
            pick_random(&self.alternate_anims, rand)
        }
    }
}

/// zombie.util.list.PZArrayUtil.pickRandom(List): `Rand.Next(size)` (no draw for an empty list, which gives null).
fn pick_random(list: &[String], rand: &mut RandAbstract<Random>) -> String {
    if list.is_empty() {
        return String::new();
    }
    let i = rand.next(list.len() as i32);
    list[i as usize].clone()
}
