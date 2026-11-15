//! zombie.core.skinnedmodel.population.OutfitManager: the male and female outfits of `media/clothing/clothing.xml`.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::clothing_item::ClothingItem;
use super::outfit::Outfit;
use crate::java::equals_ignore_case;
use crate::zombie::file_guid_table::FileGuidTable;
use crate::zombie::util::pz_xml_util::{parse_xml, XmlElement};

/// A `TreeMap<String, Outfit>(String.CASE_INSENSITIVE_ORDER)` as name → index into the outfit list: a put whose
/// name equals an existing key ignoring case replaces the value and keeps the first key.
fn ci_put(map: &mut Vec<(String, usize)>, name: &str, index: usize) {
    match map.iter_mut().find(|(k, _)| equals_ignore_case(k, name)) {
        Some(e) => e.1 = index,
        None => map.push((name.to_string(), index)),
    }
}

fn ci_get(map: &[(String, usize)], name: &str) -> Option<usize> {
    map.iter()
        .find(|(k, _)| equals_ignore_case(k, name))
        .map(|(_, i)| *i)
}

/// zombie.core.skinnedmodel.population.OutfitManager
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OutfitManager {
    pub male_outfits: Vec<Outfit>,
    pub female_outfits: Vec<Outfit>,
    female_outfit_map: Vec<(String, usize)>,
    male_outfit_map: Vec<(String, usize)>,
    /// zombie.core.skinnedmodel.population.OutfitManager.cachedClothingItems (a `Hashtable` by exact guid).
    pub cached_clothing_items: HashMap<String, ClothingItemEntry>,
    pub clothing_items: ClothingItemAssets,
}

impl OutfitManager {
    /// JAXB's unmarshalling of the root `outfitManager` element: every `m_MaleOutfits` and `m_FemaleOutfits` is an
    /// outfit, other elements are ignored. Another root element is the original's `UnmarshalException`.
    pub fn from_xml(root: &XmlElement) -> Result<Self, String> {
        if root.name != "outfitManager" {
            return Err(format!(
                "javax.xml.bind.UnmarshalException: unexpected element {}",
                root.name
            ));
        }
        let mut m = OutfitManager::default();
        for c in root.elements() {
            match c.name.as_str() {
                "m_MaleOutfits" => m.male_outfits.push(Outfit::from_xml(c)),
                "m_FemaleOutfits" => m.female_outfits.push(Outfit::from_xml(c)),
                _ => {}
            }
        }
        Ok(m)
    }

    /// zombie.core.skinnedmodel.population.OutfitManager.parse: `None` when the file does not exist; otherwise every
    /// outfit gets the mod id (which also prefixes the item guids), then the name maps are filled in list order.
    pub fn parse(mod_id: &str, path: &Path) -> Result<Option<Self>, String> {
        if !path.exists() {
            return Ok(None);
        }
        let mut m = OutfitManager::from_xml(&parse_xml(path)?)?;
        for o in &mut m.male_outfits {
            o.set_mod_id(mod_id);
        }
        for o in &mut m.female_outfits {
            o.set_mod_id(mod_id);
        }
        for i in 0..m.male_outfits.len() {
            ci_put(&mut m.male_outfit_map, &m.male_outfits[i].name.clone(), i);
        }
        for i in 0..m.female_outfits.len() {
            ci_put(
                &mut m.female_outfit_map,
                &m.female_outfits[i].name.clone(),
                i,
            );
        }
        Ok(Some(m))
    }

    /// zombie.core.skinnedmodel.population.OutfitManager.init over the game folder: `tryParse("game", ...)` of
    /// `media/clothing/clothing.xml` (a parse error is logged by the original and leaves no manager: `Err` here),
    /// then `loaded()`: every outfit and item reference marked immutable. Not ported in `loaded()`: mods' clothing
    /// files, the file watcher, `loadAllClothingItems`, and `Collections.shuffle` of both lists (an unseeded
    /// `Random`, so the original's list order after init is random; the lists here keep the file order).
    pub fn init(game_root: &Path) -> Result<Option<Self>, String> {
        let mut m = match Self::parse("game", &game_root.join("media/clothing/clothing.xml"))? {
            Some(m) => m,
            None => return Ok(None),
        };
        fn mark(r: &mut super::clothing_item_reference::ClothingItemReference) {
            r.immutable = true;
        }
        for o in m.male_outfits.iter_mut().chain(m.female_outfits.iter_mut()) {
            o.immutable = true;
            o.items.iter_mut().for_each(mark);
        }
        Ok(Some(m))
    }

    /// zombie.core.skinnedmodel.population.OutfitManager.FindMaleOutfit (name compared ignoring case).
    pub fn find_male_outfit(&self, name: &str) -> Option<&Outfit> {
        ci_get(&self.male_outfit_map, name).map(|i| &self.male_outfits[i])
    }

    /// zombie.core.skinnedmodel.population.OutfitManager.FindFemaleOutfit (name compared ignoring case).
    pub fn find_female_outfit(&self, name: &str) -> Option<&Outfit> {
        ci_get(&self.female_outfit_map, name).map(|i| &self.female_outfits[i])
    }

    /// The number of distinct names (ignoring case) in the male map.
    pub fn male_outfit_map_len(&self) -> usize {
        self.male_outfit_map.len()
    }

    /// The number of distinct names (ignoring case) in the female map.
    pub fn female_outfit_map_len(&self) -> usize {
        self.female_outfit_map.len()
    }

    /// zombie.core.skinnedmodel.population.OutfitManager.GetSpecificOutfit
    pub fn get_specific_outfit(&self, female: bool, outfit_name: &str) -> Option<&Outfit> {
        if female {
            self.find_female_outfit(outfit_name)
        } else {
            self.find_male_outfit(outfit_name)
        }
    }
}

/// zombie.core.skinnedmodel.population.OutfitManager.ClothingItemEntry: the cache entry of one guid.
#[derive(Clone, Debug, PartialEq)]
pub struct ClothingItemEntry {
    pub file_path: String,
    /// Index into `ClothingItemAssets::items`.
    pub item: Option<usize>,
}

/// zombie.core.skinnedmodel.population.ClothingItemAssetManager's asset table: one clothing item per file path
/// (two guids naming the same file share the item, and each `getClothingItem` sets its `guid`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClothingItemAssets {
    pub items: Vec<ClothingItem>,
    by_path: HashMap<PathBuf, usize>,
}

impl ClothingItemAssets {
    /// AssetManager.load(AssetPath) and ClothingItemAssetManager.startLoading/onFileTaskFinished, done at once
    /// (the original parses on the file system's thread and fills the item when the main thread collects the
    /// result). A file that does not parse as a clothing item leaves a FAILED item with default fields.
    fn load(&mut self, absolute_path: &Path) -> usize {
        if let Some(&i) = self.by_path.get(absolute_path) {
            return i;
        }
        let item = parse_xml(absolute_path)
            .and_then(|root| ClothingItem::from_xml(&root))
            .unwrap_or_else(|_| ClothingItem {
                failed: true,
                ..ClothingItem::default()
            });
        self.items.push(item);
        self.by_path
            .insert(absolute_path.to_path_buf(), self.items.len() - 1);
        self.items.len() - 1
    }
}

impl OutfitManager {
    /// zombie.core.skinnedmodel.population.OutfitManager.getClothingItem: `None` for a guid the file guid table does
    /// not know; else the cached item (its `guid` set to this guid), or the item loaded from the file the guid names
    /// (`ZomboidFileSystem.resolveFileOrGUID`: here the file under the game folder; the original's map of mods'
    /// active files is not ported), named after the file (`mame`). The file watcher is not ported.
    pub fn get_clothing_item(
        &mut self,
        item_guid: &str,
        table: &FileGuidTable,
        game_root: &Path,
    ) -> Option<&ClothingItem> {
        let file_path = table.get_file_path_from_guid(item_guid)?.to_string();
        let entry = self
            .cached_clothing_items
            .entry(item_guid.to_string())
            .or_insert_with(|| ClothingItemEntry {
                file_path: file_path.clone(),
                item: None,
            });
        let index = match entry.item {
            Some(i) => i,
            None => {
                let resolved = table
                    .get_file_path_from_guid(&file_path)
                    .unwrap_or(&file_path)
                    .to_string();
                let i = self.clothing_items.load(&game_root.join(&resolved));
                self.clothing_items.items[i].mame = Some(extract_clothing_item_name(&file_path));
                entry.item = Some(i);
                i
            }
        };
        let item = &mut self.clothing_items.items[index];
        item.guid = Some(item_guid.to_string());
        Some(item)
    }
}

/// zombie.core.skinnedmodel.population.OutfitManager.extractClothingItemName: `trimPrefix` of
/// "media/clothing/clothingItems/" and `trimSuffix` of ".xml" (both case-sensitive).
fn extract_clothing_item_name(file_path: &str) -> String {
    let name = file_path
        .strip_prefix("media/clothing/clothingItems/")
        .unwrap_or(file_path);
    name.strip_suffix(".xml").unwrap_or(name).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zombie::util::pz_xml_util::parse_xml_str;

    /// Unknown elements are skipped, defaults stay where an element is absent, and lookups ignore case.
    #[test]
    fn outfits_from_xml() {
        let root = parse_xml_str(
            "<outfitManager><m_FemaleOutfits><m_Name>Agent</m_Name><m_Guid>g</m_Guid><m_Top>false</m_Top>\
             <m_items><probability>0.3</probability><itemGUID>a</itemGUID><subItems><itemGUID>b</itemGUID></subItems>\
             </m_items></m_FemaleOutfits><other/></outfitManager>",
        )
        .unwrap();
        let mut m = OutfitManager::from_xml(&root).unwrap();
        let o = &mut m.female_outfits[0];
        assert_eq!(
            (o.name.as_str(), o.top, o.pants, o.allow_pants_tint),
            ("Agent", false, true, false)
        );
        o.set_mod_id("game");
        let r = &o.items[0];
        assert_eq!(
            (r.probability, r.item_guid.as_deref()),
            (0.3, Some("game-a"))
        );
        assert_eq!(r.sub_items[0].item_guid.as_deref(), Some("game-b"));
        ci_put(&mut m.female_outfit_map, "Agent", 0);
        assert!(m.find_female_outfit("aGENT").is_some());
        assert!(m.find_male_outfit("Agent").is_none());
    }
}
