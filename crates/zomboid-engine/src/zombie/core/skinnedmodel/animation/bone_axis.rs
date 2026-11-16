//! zombie.core.skinnedmodel.animation.BoneAxis
/// zombie.core.skinnedmodel.animation.BoneAxis (`@XmlEnum`: "Y", "Z").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoneAxis {
    Y,
    Z,
}

impl BoneAxis {
    /// JAXB's `@XmlEnumValue` lookup: the constant whose value is exactly `s`, else null (`None`).
    pub fn from_xml(s: &str) -> Option<BoneAxis> {
        match s {
            "Y" => Some(BoneAxis::Y),
            "Z" => Some(BoneAxis::Z),
            _ => None,
        }
    }

    /// java.lang.Enum.name
    pub fn name(self) -> &'static str {
        match self {
            BoneAxis::Y => "Y",
            BoneAxis::Z => "Z",
        }
    }
}
