//! zombie.core.math.interpolators.LerpType
/// zombie.core.math.interpolators.LerpType (no `@XmlEnum`: JAXB maps the constants by name).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LerpType {
    Linear,
    EaseOutQuad,
    EaseInQuad,
    EaseOutInQuad,
}

impl LerpType {
    /// JAXB's lookup by constant name, else null (`None`).
    pub fn from_xml(s: &str) -> Option<LerpType> {
        match s {
            "Linear" => Some(LerpType::Linear),
            "EaseOutQuad" => Some(LerpType::EaseOutQuad),
            "EaseInQuad" => Some(LerpType::EaseInQuad),
            "EaseOutInQuad" => Some(LerpType::EaseOutInQuad),
            _ => None,
        }
    }

    /// java.lang.Enum.name
    pub fn name(self) -> &'static str {
        match self {
            LerpType::Linear => "Linear",
            LerpType::EaseOutQuad => "EaseOutQuad",
            LerpType::EaseInQuad => "EaseInQuad",
            LerpType::EaseOutInQuad => "EaseOutInQuad",
        }
    }
}
