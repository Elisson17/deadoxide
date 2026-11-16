//! zombie.core.math.interpolators.xml.InterpolatorSlot
use crate::zombie::core::math::i_interpolator::IInterpolator;
use crate::zombie::core::math::interpolators::lerp_polyline::LerpPolyline;
use crate::zombie::core::math::interpolators::lerp_sequence::LerpSequence;
use crate::zombie::core::math::interpolators::{
    LerpEaseInQuad, LerpEaseOutInQuad, LerpEaseOutQuad, LerpLinear,
};
use crate::zombie::util::pz_xml_util::XmlElement;

/// zombie.core.math.interpolators.xml.InterpolatorSlot: one interpolator chosen by element name (`@XmlElements`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InterpolatorSlot {
    pub interpolator: Option<IInterpolator>,
}

impl InterpolatorSlot {
    /// JAXB's unmarshalling of the `@XmlElements` choice: `EaseInQuad`, `EaseOutInQuad`, `EaseOutQuad`, `Linear`,
    /// `One`, `Polyline`, `Sequence`, `Zero` (the last one present wins).
    pub fn from_xml(e: &XmlElement) -> Self {
        let mut s = InterpolatorSlot::default();
        for c in e.elements() {
            let i = match c.name.as_str() {
                "EaseInQuad" => IInterpolator::EaseInQuad(LerpEaseInQuad::from_xml(c)),
                "EaseOutInQuad" => IInterpolator::EaseOutInQuad(LerpEaseOutInQuad::from_xml(c)),
                "EaseOutQuad" => IInterpolator::EaseOutQuad(LerpEaseOutQuad::from_xml(c)),
                "Linear" => IInterpolator::Linear(LerpLinear::from_xml(c)),
                "One" => IInterpolator::One,
                "Polyline" => IInterpolator::Polyline(LerpPolyline::from_xml(c)),
                "Sequence" => IInterpolator::Sequence(LerpSequence::from_xml(c)),
                "Zero" => IInterpolator::Zero,
                _ => continue,
            };
            s.interpolator = Some(i);
        }
        s
    }
}
