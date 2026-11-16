//! zombie.core.skinnedmodel.advancedanimation.AnimTransition
use super::anim_condition::AnimCondition;
use crate::zombie::core::skinnedmodel::animation::bone_axis::BoneAxis;
use crate::zombie::util::pz_xml_util::{jaxb_parse_boolean, jaxb_parse_float, XmlElement};

/// zombie.core.skinnedmodel.advancedanimation.AnimTransition: how a node blends into a target node.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimTransition {
    pub source: Option<String>,
    pub target: Option<String>,
    pub anim_name: Option<String>,
    pub deferred_bone_name: Option<String>,
    pub deferred_bone_axis: Option<BoneAxis>,
    pub use_defered_rotation: bool,
    pub use_deferred_movement: bool,
    pub deferred_rotation_scale: f32,
    pub sync_adjust_time: f32,
    pub blend_in_time: f32,
    pub blend_out_time: f32,
    pub speed_scale: f32,
    /// `@XmlTransient isParsed`: set by `parse`, which `AnimNode.findTransitionTo` runs (not at load).
    pub is_parsed: bool,
    pub conditions: Vec<AnimCondition>,
}

impl Default for AnimTransition {
    /// zombie.core.skinnedmodel.advancedanimation.AnimTransition(): the field initialisers.
    fn default() -> Self {
        AnimTransition {
            source: None,
            target: None,
            anim_name: None,
            deferred_bone_name: None,
            deferred_bone_axis: Some(BoneAxis::Y),
            use_defered_rotation: false,
            use_deferred_movement: true,
            deferred_rotation_scale: 1.0,
            sync_adjust_time: 0.0,
            blend_in_time: f32::INFINITY,
            blend_out_time: f32::INFINITY,
            speed_scale: f32::INFINITY,
            is_parsed: false,
            conditions: Vec::new(),
        }
    }
}

impl AnimTransition {
    /// JAXB's unmarshalling of the `@XmlElement` fields (`m_Source`, `m_Target`, `m_AnimName`, `m_DeferredBoneName`,
    /// `m_deferredBoneAxis`, `m_useDeferedRotation`, `m_useDeferredMovement`, `m_deferredRotationScale`,
    /// `m_SyncAdjustTime`, `m_blendInTime`, `m_blendOutTime`, `m_speedScale`, `m_Conditions` collected into the array).
    pub fn from_xml(e: &XmlElement) -> Self {
        let mut t = AnimTransition::default();
        let mut conditions = Vec::new();
        for c in e.elements() {
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
            match c.name.as_str() {
                "m_Source" => t.source = Some(c.text()),
                "m_Target" => t.target = Some(c.text()),
                "m_AnimName" => t.anim_name = Some(c.text()),
                "m_DeferredBoneName" => t.deferred_bone_name = Some(c.text()),
                "m_deferredBoneAxis" => t.deferred_bone_axis = BoneAxis::from_xml(&c.text()),
                "m_useDeferedRotation" => flag(&mut t.use_defered_rotation),
                "m_useDeferredMovement" => flag(&mut t.use_deferred_movement),
                "m_deferredRotationScale" => float(&mut t.deferred_rotation_scale),
                "m_SyncAdjustTime" => float(&mut t.sync_adjust_time),
                "m_blendInTime" => float(&mut t.blend_in_time),
                "m_blendOutTime" => float(&mut t.blend_out_time),
                "m_speedScale" => float(&mut t.speed_scale),
                "m_Conditions" => conditions.push(AnimCondition::from_xml(c)),
                _ => {}
            }
        }
        if !conditions.is_empty() {
            t.conditions = conditions;
        }
        t
    }
}
