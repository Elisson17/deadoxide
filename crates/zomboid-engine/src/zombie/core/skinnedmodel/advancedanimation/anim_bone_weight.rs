//! zombie.core.skinnedmodel.advancedanimation.AnimBoneWeight
use crate::zombie::util::pz_xml_util::{jaxb_parse_boolean, jaxb_parse_float, XmlElement};

/// zombie.core.skinnedmodel.advancedanimation.AnimBoneWeight: a bone of a sub-state's mask and its weight.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimBoneWeight {
    pub bone_name: Option<String>,
    pub weight: f32,
    pub include_descendants: bool,
}

impl Default for AnimBoneWeight {
    /// zombie.core.skinnedmodel.advancedanimation.AnimBoneWeight(): the field initialisers.
    fn default() -> Self {
        AnimBoneWeight {
            bone_name: None,
            weight: 1.0,
            include_descendants: true,
        }
    }
}

impl AnimBoneWeight {
    /// zombie.core.skinnedmodel.advancedanimation.AnimBoneWeight(String, float)
    pub fn new(bone_name: &str, weight: f32) -> Self {
        AnimBoneWeight {
            bone_name: Some(bone_name.to_string()),
            weight,
            include_descendants: true,
        }
    }

    /// JAXB's unmarshalling of the `@XmlElement` fields `boneName`, `weight`, `includeDescendants` (a repeated element
    /// keeps the last value, a value that does not convert keeps the field, other elements are ignored).
    pub fn from_xml(e: &XmlElement) -> Self {
        let mut w = AnimBoneWeight::default();
        for c in e.elements() {
            match c.name.as_str() {
                "boneName" => w.bone_name = Some(c.text()),
                "weight" => {
                    if let Some(v) = jaxb_parse_float(&c.text()) {
                        w.weight = v;
                    }
                }
                "includeDescendants" => {
                    if let Some(v) = jaxb_parse_boolean(&c.text()) {
                        w.include_descendants = v;
                    }
                }
                _ => {}
            }
        }
        w
    }
}
