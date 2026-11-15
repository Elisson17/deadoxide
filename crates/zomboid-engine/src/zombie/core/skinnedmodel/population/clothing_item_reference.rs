//! zombie.core.skinnedmodel.population.ClothingItemReference: one clothing item of an outfit (`m_items` in
//! `clothing.xml`), with its chance and the alternatives one of which is worn instead.
use std::path::Path;

use super::clothing_decals::ClothingDecals;
use super::outfit_manager::OutfitManager;
use super::outfit_rng::OutfitRng;
use crate::zombie::core::immutable_color::ImmutableColor;
use crate::zombie::file_guid_table::FileGuidTable;
use crate::zombie::util::pz_xml_util::{jaxb_parse_boolean, jaxb_parse_float, XmlElement};
use crate::zombie::util::string_utils::is_null_or_whitespace;

/// zombie.core.skinnedmodel.population.ClothingItemReference.RandomData: what `randomize` picked.
#[derive(Clone, Debug, PartialEq)]
pub struct RandomData {
    pub active: bool,
    pub hue: f32,
    pub tint: ImmutableColor,
    pub base_texture: Option<String>,
    /// `pickedItemRef` as a path of `sub_items` indices from this reference (empty: this reference itself).
    pub picked_item_ref: Option<Vec<usize>>,
    pub texture_choice: Option<String>,
    pub decal: Option<String>,
}

impl Default for RandomData {
    /// zombie.core.skinnedmodel.population.ClothingItemReference.RandomData(): active, white tint.
    fn default() -> Self {
        RandomData {
            active: true,
            hue: 0.0,
            tint: ImmutableColor::WHITE,
            base_texture: None,
            picked_item_ref: None,
            texture_choice: None,
            decal: None,
        }
    }
}

/// What `randomize` reads besides the reference: the clothing items, the decal groups and the sandbox option.
pub struct RandomizeContext<'a> {
    pub rng: &'a mut OutfitRng,
    pub outfits: &'a mut OutfitManager,
    pub table: &'a FileGuidTable,
    pub game_root: &'a Path,
    /// `ClothingDecals.instance` (`None` is the original's null instance).
    pub decals: Option<&'a ClothingDecals>,
    /// `SandboxOptions.instance.noBlackClothes`
    pub no_black_clothes: bool,
}

/// zombie.core.skinnedmodel.population.ClothingItemReference: the fields `clothing.xml` sets and `randomData`.
#[derive(Clone, Debug, PartialEq)]
pub struct ClothingItemReference {
    pub probability: f32,
    pub item_guid: Option<String>,
    pub sub_items: Vec<ClothingItemReference>,
    pub randomized: bool,
    pub immutable: bool,
    pub random_data: RandomData,
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
            random_data: RandomData::default(),
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

impl ClothingItemReference {
    /// The reference a `picked_item_ref` path names.
    pub fn picked(&self, path: &[usize]) -> Option<&ClothingItemReference> {
        match path.split_first() {
            None => Some(self),
            Some((&i, rest)) => self.sub_items.get(i)?.picked(rest),
        }
    }

    /// The guid `getClothingItem` looks up: the picked reference's, else this one's.
    pub fn picked_item_guid(&self) -> Option<&str> {
        match &self.random_data.picked_item_ref {
            Some(path) => self.picked(path).and_then(|r| r.item_guid.as_deref()),
            None => self.item_guid.as_deref(),
        }
    }

    /// zombie.core.skinnedmodel.population.ClothingItemReference.randomize: sub items first, then the pick among
    /// this item and its sub items' picks, then (for a clothing item the guid table knows) whether it is worn
    /// (`Next(0, 1) <= probability`), its hue, tint, base texture, texture choice and decal, in that order of draws.
    /// `Err` is the original's exception (an immutable reference, or a decal group lookup that throws).
    pub fn randomize(&mut self, cx: &mut RandomizeContext) -> Result<(), String> {
        if self.immutable {
            return Err(
                "java.lang.RuntimeException: trying to randomize an immutable ClothingItemReference"
                    .to_string(),
            );
        }
        self.random_data = RandomData::default();
        for item_ref in &mut self.sub_items {
            item_ref.randomize(cx)?;
        }
        self.random_data.picked_item_ref = Some(self.pick_random_item_internal(cx.rng));
        self.randomized = true;
        let guid = self.picked_item_guid().unwrap_or("null").to_string();
        let Some(item) = cx
            .outfits
            .get_clothing_item(&guid, cx.table, cx.game_root)
            .cloned()
        else {
            self.random_data.active = false;
            return Ok(());
        };
        self.random_data.active = cx.rng.next_float(0.0, 1.0) <= self.probability;
        if item.allow_random_hue {
            self.random_data.hue = cx.rng.next(200) as f32 / 100.0 - 1.0;
        }
        if item.allow_random_tint {
            self.random_data.tint = cx.rng.random_immutable_color(cx.no_black_clothes, false);
        } else {
            self.random_data.tint = ImmutableColor::WHITE;
        }
        self.random_data.base_texture = cx.rng.pick_random(&item.base_textures);
        self.random_data.texture_choice = cx.rng.pick_random(&item.texture_choices);
        if !is_null_or_whitespace(item.decal_group.as_deref()) {
            let Some(decals) = cx.decals else {
                return Err(
                    "java.lang.NullPointerException: ClothingDecals.instance is null".to_string(),
                );
            };
            self.random_data.decal =
                decals.get_random_decal(item.decal_group.as_deref().unwrap_or(""), cx.rng)?;
        }
        Ok(())
    }

    /// zombie.core.skinnedmodel.population.ClothingItemReference.pickRandomItemInternal: this reference when it
    /// has no sub items, else `Next(subItems + 1)`: 0 is this reference, i the pick of sub item i - 1.
    fn pick_random_item_internal(&self, rng: &mut OutfitRng) -> Vec<usize> {
        if self.sub_items.is_empty() {
            return Vec::new();
        }
        let len = i32::try_from(self.sub_items.len()).unwrap_or(i32::MAX - 1);
        let random_index = rng.next(len + 1);
        if random_index <= 0 {
            return Vec::new();
        }
        let i = (random_index - 1) as usize;
        let mut path = vec![i];
        if let Some(sub) = self.sub_items.get(i) {
            path.extend(sub.random_data.picked_item_ref.iter().flatten());
        }
        path
    }

    /// zombie.core.skinnedmodel.population.ClothingItemReference.clone: probability, guid and cloned sub items;
    /// `randomized`, `immutable` and the random data start fresh.
    pub fn clone_reference(&self) -> ClothingItemReference {
        ClothingItemReference {
            probability: self.probability,
            item_guid: self.item_guid.clone(),
            sub_items: self.sub_items.iter().map(Self::clone_reference).collect(),
            ..ClothingItemReference::default()
        }
    }
}
