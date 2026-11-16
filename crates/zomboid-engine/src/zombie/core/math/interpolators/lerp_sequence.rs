//! zombie.core.math.interpolators.LerpSequence
use super::lerp_type::LerpType;
use crate::zombie::core::math::pz_math;
use crate::zombie::util::pz_xml_util::{jaxb_parse_float, XmlElement};

/// zombie.core.math.interpolators.LerpSequence.LerpSequenceEntry: a point (`x`, `y` attributes) and the curve to it
/// (`@XmlElement(name = "out")`: an element, so the files' `out` attribute is not read and stays Linear).
#[derive(Clone, Debug, PartialEq)]
pub struct LerpSequenceEntry {
    pub lerp_type: Option<LerpType>,
    pub x: f32,
    pub y: f32,
}

impl LerpSequenceEntry {
    /// JAXB's unmarshalling of the `x` and `y` attributes and the `out` element.
    fn from_xml(e: &XmlElement) -> Self {
        let mut p = LerpSequenceEntry {
            lerp_type: Some(LerpType::Linear),
            x: 0.0,
            y: 0.0,
        };
        for (n, a) in &e.attributes {
            match n.as_str() {
                "x" => {
                    if let Some(v) = jaxb_parse_float(a) {
                        p.x = v;
                    }
                }
                "y" => {
                    if let Some(v) = jaxb_parse_float(a) {
                        p.y = v;
                    }
                }
                _ => {}
            }
        }
        for c in e.elements() {
            if c.name == "out" {
                p.lerp_type = LerpType::from_xml(&c.text());
            }
        }
        p
    }
}

/// zombie.core.math.interpolators.LerpSequence: points with a curve each; `sequence_entries` holds indices into
/// `sequence_raw` (the same objects in the original), ordered by `parse`.
#[derive(Clone, Debug, PartialEq)]
pub struct LerpSequence {
    pub sequence_raw: Vec<LerpSequenceEntry>,
    pub num_entries: i32,
    pub sequence_entries: Option<Vec<usize>>,
    pub min_x: f32,
    pub max_x: f32,
}

impl LerpSequence {
    /// JAXB's unmarshalling of the `point` elements, then `afterUnmarshal` (`parse`).
    pub fn from_xml(e: &XmlElement) -> Self {
        let mut s = LerpSequence {
            sequence_raw: e
                .elements()
                .filter(|c| c.name == "point")
                .map(LerpSequenceEntry::from_xml)
                .collect(),
            num_entries: -1,
            sequence_entries: None,
            min_x: 0.0,
            max_x: 0.0,
        };
        s.parse();
        s
    }

    /// zombie.core.math.interpolators.LerpSequence.parse: the entries copied, then a selection pass that compares each
    /// later entry with the entry at the current position (not with the smallest so far, as written) and swaps the
    /// last smaller one in; min and max x (from Float.MAX_VALUE and Float.MIN_VALUE).
    fn parse(&mut self) {
        self.num_entries = self.sequence_raw.len() as i32;
        let n = self.sequence_raw.len();
        let mut entries: Vec<usize> = (0..n).collect();
        self.min_x = f32::MAX;
        self.max_x = f32::from_bits(1);
        for entry_idx in 0..n {
            let mut smallest_idx = entry_idx;
            for other_idx in entry_idx + 1..n {
                if self.sequence_raw[entries[other_idx]].x < self.sequence_raw[entries[entry_idx]].x
                {
                    smallest_idx = other_idx;
                }
            }
            if smallest_idx != entry_idx {
                entries.swap(entry_idx, smallest_idx);
            }
            let x = self.sequence_raw[entries[entry_idx]].x;
            self.min_x = pz_math::min(x, self.min_x);
            self.max_x = pz_math::max(x, self.max_x);
        }
        self.sequence_entries = Some(entries);
    }
}
