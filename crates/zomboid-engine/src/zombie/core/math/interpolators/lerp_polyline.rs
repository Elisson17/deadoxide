//! zombie.core.math.interpolators.LerpPolyline
use crate::zombie::core::math::pz_math;
use crate::zombie::util::pz_xml_util::{jaxb_parse_float, XmlElement};

/// zombie.core.math.interpolators.LerpPolyline.PolylinePoint (`x`, `y` attributes).
#[derive(Clone, Debug, PartialEq)]
pub struct PolylinePoint {
    pub x: f32,
    pub y: f32,
}

/// zombie.core.math.interpolators.LerpPolyline: points (`m_Points`) and their copy ordered by x (`points`).
#[derive(Clone, Debug, PartialEq)]
pub struct LerpPolyline {
    pub points_raw: Vec<PolylinePoint>,
    pub num_points: i32,
    pub points: Option<Vec<[f32; 2]>>,
    pub min_x: f32,
    pub max_x: f32,
}

impl LerpPolyline {
    /// JAXB's unmarshalling of the `m_Points` elements (each with `x`, `y`), then `afterUnmarshal` (`parse`).
    pub fn from_xml(e: &XmlElement) -> Self {
        let points_raw = e
            .elements()
            .filter(|c| c.name == "m_Points")
            .map(|c| {
                let mut p = PolylinePoint { x: 0.0, y: 0.0 };
                for (n, a) in &c.attributes {
                    match n.as_str() {
                        "x" => p.x = jaxb_parse_float(a).unwrap_or(p.x),
                        "y" => p.y = jaxb_parse_float(a).unwrap_or(p.y),
                        _ => {}
                    }
                }
                p
            })
            .collect();
        let mut l = LerpPolyline {
            points_raw,
            num_points: -1,
            points: None,
            min_x: 0.0,
            max_x: 0.0,
        };
        l.parse();
        l
    }

    /// zombie.core.math.interpolators.LerpPolyline.parse: as `LerpSequence.parse`, over copied coordinates.
    fn parse(&mut self) {
        self.num_points = self.points_raw.len() as i32;
        let mut points: Vec<[f32; 2]> = self.points_raw.iter().map(|p| [p.x, p.y]).collect();
        self.min_x = f32::MAX;
        self.max_x = f32::from_bits(1);
        let n = points.len();
        for point_idx in 0..n {
            let mut smallest_idx = point_idx;
            for other_idx in point_idx + 1..n {
                if points[other_idx][0] < points[point_idx][0] {
                    smallest_idx = other_idx;
                }
            }
            if smallest_idx != point_idx {
                points.swap(point_idx, smallest_idx);
            }
            self.min_x = pz_math::min(points[point_idx][0], self.min_x);
            self.max_x = pz_math::max(points[point_idx][0], self.max_x);
        }
        self.points = Some(points);
    }
}
