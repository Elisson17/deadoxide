//! zombie.core.skinnedmodel.population.Outfit: a named set of clothing items a character can be dressed in
//! (`m_MaleOutfits` / `m_FemaleOutfits` in `clothing.xml`).
use super::clothing_item_reference::ClothingItemReference;
use crate::zombie::util::pz_xml_util::{jaxb_parse_boolean, XmlElement};

/// zombie.core.skinnedmodel.population.Outfit: the fields `clothing.xml` sets (`randomData`, filled by
/// `Randomize`, is not ported yet).
#[derive(Clone, Debug, PartialEq)]
pub struct Outfit {
    pub name: String,
    pub top: bool,
    pub pants: bool,
    pub top_textures: Vec<String>,
    pub pants_textures: Vec<String>,
    pub items: Vec<ClothingItemReference>,
    pub allow_pants_hue: bool,
    pub allow_pants_tint: bool,
    pub allow_top_tint: bool,
    pub allow_tshirt_decal: bool,
    pub mod_id: Option<String>,
    pub immutable: bool,
}

impl Default for Outfit {
    /// zombie.core.skinnedmodel.population.Outfit(): the field initialisers.
    fn default() -> Self {
        Outfit {
            name: "Outfit".to_string(),
            top: true,
            pants: true,
            top_textures: Vec::new(),
            pants_textures: Vec::new(),
            items: Vec::new(),
            allow_pants_hue: true,
            allow_pants_tint: false,
            allow_top_tint: true,
            allow_tshirt_decal: true,
            mod_id: None,
            immutable: false,
        }
    }
}

impl Outfit {
    /// JAXB's unmarshalling of the `@XmlElement` fields (`m_Name`, `m_Top`, `m_Pants`, `m_TopTextures`,
    /// `m_PantsTextures`, `m_items`, `m_AllowPantsHue`, `m_AllowPantsTint`, `m_AllowTopTint`, `m_AllowTShirtDecal`);
    /// other elements (`m_Guid`) are ignored.
    pub fn from_xml(e: &XmlElement) -> Self {
        let mut o = Outfit::default();
        for c in e.elements() {
            let flag = |v: &mut bool| {
                if let Some(b) = jaxb_parse_boolean(&c.text()) {
                    *v = b;
                }
            };
            match c.name.as_str() {
                "m_Name" => o.name = c.text(),
                "m_Top" => flag(&mut o.top),
                "m_Pants" => flag(&mut o.pants),
                "m_TopTextures" => o.top_textures.push(c.text()),
                "m_PantsTextures" => o.pants_textures.push(c.text()),
                "m_items" => o.items.push(ClothingItemReference::from_xml(c)),
                "m_AllowPantsHue" => flag(&mut o.allow_pants_hue),
                "m_AllowPantsTint" => flag(&mut o.allow_pants_tint),
                "m_AllowTopTint" => flag(&mut o.allow_top_tint),
                "m_AllowTShirtDecal" => flag(&mut o.allow_tshirt_decal),
                _ => {}
            }
        }
        o
    }

    /// zombie.core.skinnedmodel.population.Outfit.setModID
    pub fn set_mod_id(&mut self, mod_id: &str) {
        self.mod_id = Some(mod_id.to_string());
        for r in &mut self.items {
            r.set_mod_id(mod_id);
        }
    }
}

impl Outfit {
    /// zombie.core.skinnedmodel.population.Outfit.clone: name, flags, both texture lists and cloned items; the
    /// mod id, `immutable` and the random data start fresh.
    pub fn clone_outfit(&self) -> Outfit {
        Outfit {
            name: self.name.clone(),
            top: self.top,
            pants: self.pants,
            pants_textures: self.pants_textures.clone(),
            top_textures: self.top_textures.clone(),
            items: self
                .items
                .iter()
                .map(ClothingItemReference::clone_reference)
                .collect(),
            allow_pants_hue: self.allow_pants_hue,
            allow_pants_tint: self.allow_pants_tint,
            allow_top_tint: self.allow_top_tint,
            allow_tshirt_decal: self.allow_tshirt_decal,
            ..Outfit::default()
        }
    }
}
