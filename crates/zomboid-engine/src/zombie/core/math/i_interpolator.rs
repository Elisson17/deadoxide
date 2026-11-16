//! zombie.core.math.IInterpolator
use super::interpolators::lerp_polyline::LerpPolyline;
use super::interpolators::lerp_sequence::LerpSequence;
use super::interpolators::{LerpEaseInQuad, LerpEaseOutInQuad, LerpEaseOutQuad, LerpLinear};

/// zombie.core.math.IInterpolator: the subclasses `InterpolatorSlot` can hold.
#[derive(Clone, Debug, PartialEq)]
pub enum IInterpolator {
    EaseInQuad(LerpEaseInQuad),
    EaseOutInQuad(LerpEaseOutInQuad),
    EaseOutQuad(LerpEaseOutQuad),
    Linear(LerpLinear),
    One,
    Polyline(LerpPolyline),
    Sequence(LerpSequence),
    Zero,
}

impl IInterpolator {
    /// java.lang.Class.getSimpleName of the instance.
    pub fn simple_name(&self) -> &'static str {
        match self {
            IInterpolator::EaseInQuad(_) => "LerpEaseInQuad",
            IInterpolator::EaseOutInQuad(_) => "LerpEaseOutInQuad",
            IInterpolator::EaseOutQuad(_) => "LerpEaseOutQuad",
            IInterpolator::Linear(_) => "LerpLinear",
            IInterpolator::One => "LerpOne",
            IInterpolator::Polyline(_) => "LerpPolyline",
            IInterpolator::Sequence(_) => "LerpSequence",
            IInterpolator::Zero => "LerpZero",
        }
    }
}
