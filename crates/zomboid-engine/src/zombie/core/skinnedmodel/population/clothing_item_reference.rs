//! zombie.core.skinnedmodel.population.ClothingItemReference: one clothing item of an outfit (`m_items` in
//! `clothing.xml`), with its chance and the alternatives one of which is worn instead.
use crate::zombie::util::pz_xml_util::{jaxb_parse_boolean, jaxb_parse_float, XmlElement};

/// zombie.core.skinnedmodel.population.ClothingItemReference: the fields `clothing.xml` sets (`randomData`, filled
/// when an outfit is randomized, is not ported yet).
#[derive(Clone, Debug, PartialEq)]
pub struct ClothingItemReference {
    pub probability: f32,
    pub item_guid: Option<String>,
    pub sub_items: Vec<ClothingItemReference>,
    pub randomized: bool,
    pub immutable: bool,
}

impl Default for ClothingItemReference {
    /// zombie.core.skinnedmodel.population.ClothingItemReference(): probability 1.
    fn default() -> Self {
        ClothingItemReference {
            probability: 1.0,
            item_guid: None,
            sub_items: Vec::new(),
            randomized: false,
            immutable: false,
        }
    }
}

impl ClothingItemReference {
    /// JAXB's unmarshalling of the `@XmlElement` fields: `probability`, `itemGUID`, `subItems` (each one more
    /// reference), `bRandomized`; other elements are ignored, and a repeated single field takes the last value.
    pub fn from_xml(e: &XmlElement) -> Self {
        let mut r = ClothingItemReference::default();
        for c in e.elements() {
            match c.name.as_str() {
                "probability" => {
                    if let Some(v) = jaxb_parse_float(&c.text()) {
                        r.probability = v;
                    }
                }
                "itemGUID" => r.item_guid = Some(c.text()),
                "subItems" => r.sub_items.push(ClothingItemReference::from_xml(c)),
                "bRandomized" => {
                    if let Some(v) = jaxb_parse_boolean(&c.text()) {
                        r.randomized = v;
                    }
                }
                _ => {}
            }
        }
        r
    }

    /// zombie.core.skinnedmodel.population.ClothingItemReference.setModID: the guid becomes "modID-guid" (a null
    /// guid becomes "modID-null", as Java's string concatenation does), here and in every sub item.
    pub fn set_mod_id(&mut self, mod_id: &str) {
        self.item_guid = Some(format!(
            "{mod_id}-{}",
            self.item_guid.as_deref().unwrap_or("null")
        ));
        for s in &mut self.sub_items {
            s.set_mod_id(mod_id);
        }
    }
}
