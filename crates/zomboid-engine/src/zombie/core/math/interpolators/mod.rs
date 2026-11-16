//! Package `zombie.core.math.interpolators`: the curves an animation node's `m_BlendCurve` can name. The four
//! two-value curves (`y0`, `y1` attributes) are here; `LerpOne` and `LerpZero` have no fields.
use crate::zombie::util::pz_xml_util::{jaxb_parse_float, XmlElement};

pub mod lerp_polyline;
pub mod lerp_sequence;
pub mod lerp_type;
pub mod xml;

/// The `@XmlAttribute` floats `y0` (startValue, 0) and `y1` (endValue, 1) of the two-value curves.
fn y0_y1(e: &XmlElement) -> (f32, f32) {
    let mut v = (0.0f32, 1.0f32);
    for (n, a) in &e.attributes {
        match n.as_str() {
            "y0" => {
                if let Some(x) = jaxb_parse_float(a) {
                    v.0 = x;
                }
            }
            "y1" => {
                if let Some(x) = jaxb_parse_float(a) {
                    v.1 = x;
                }
            }
            _ => {}
        }
    }
    v
}

macro_rules! two_value_curve {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq)]
        pub struct $name {
            pub start_value: f32,
            pub end_value: f32,
        }

        impl $name {
            /// JAXB's unmarshalling of the `y0` and `y1` attributes.
            pub fn from_xml(e: &XmlElement) -> Self {
                let (start_value, end_value) = y0_y1(e);
                $name {
                    start_value,
                    end_value,
                }
            }
        }
    };
}

two_value_curve!(
    /// zombie.core.math.interpolators.LerpLinear
    LerpLinear
);
two_value_curve!(
    /// zombie.core.math.interpolators.LerpEaseInQuad
    LerpEaseInQuad
);
two_value_curve!(
    /// zombie.core.math.interpolators.LerpEaseOutInQuad
    LerpEaseOutInQuad
);
two_value_curve!(
    /// zombie.core.math.interpolators.LerpEaseOutQuad
    LerpEaseOutQuad
);
