//! zombie.core.skinnedmodel.advancedanimation.GrappleOffsetBehaviour
/// zombie.core.skinnedmodel.advancedanimation.GrappleOffsetBehaviour (`@XmlEnum` with `@XmlEnumValue` names).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrappleOffsetBehaviour {
    None,
    Grappled,
    GrappledTweenOutToNone,
    Grappler,
    NoneTweenInGrappler,
}

impl GrappleOffsetBehaviour {
    /// JAXB's `@XmlEnumValue` lookup: the constant whose value is exactly `s`, else null (`None`).
    pub fn from_xml(s: &str) -> Option<GrappleOffsetBehaviour> {
        match s {
            "None" => Some(GrappleOffsetBehaviour::None),
            "Grappled" => Some(GrappleOffsetBehaviour::Grappled),
            "Grappled_TweenOutToNone" => Some(GrappleOffsetBehaviour::GrappledTweenOutToNone),
            "Grappler" => Some(GrappleOffsetBehaviour::Grappler),
            "None_TweenInGrappler" => Some(GrappleOffsetBehaviour::NoneTweenInGrappler),
            _ => None,
        }
    }

    /// java.lang.Enum.name
    pub fn name(self) -> &'static str {
        match self {
            GrappleOffsetBehaviour::None => "NONE",
            GrappleOffsetBehaviour::Grappled => "GRAPPLED",
            GrappleOffsetBehaviour::GrappledTweenOutToNone => "GRAPPLED_TWEEN_OUT_TO_NONE",
            GrappleOffsetBehaviour::Grappler => "GRAPPLER",
            GrappleOffsetBehaviour::NoneTweenInGrappler => "NONE_TWEEN_IN_GRAPPLER",
        }
    }
}
