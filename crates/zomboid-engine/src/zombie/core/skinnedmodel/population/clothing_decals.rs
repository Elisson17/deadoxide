//! zombie.core.skinnedmodel.population.ClothingDecals and ClothingDecalGroup: `media/clothing/clothingDecals.xml`,
//! the named groups of T-shirt prints a clothing item with a `decalGroup` picks one from.
use std::path::Path;

use super::outfit_rng::OutfitRng;
use crate::java::equals_ignore_case;
use crate::zombie::util::pz_xml_util::{parse_xml, XmlElement};
use crate::zombie::util::string_utils::is_null_or_whitespace;

/// zombie.core.skinnedmodel.population.ClothingDecalGroup
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClothingDecalGroup {
    pub name: Option<String>,
    pub decals: Vec<String>,
    pub groups: Vec<String>,
}

impl ClothingDecalGroup {
    /// JAXB's unmarshalling of `name`, `decal` and `group`; other elements are ignored.
    fn from_xml(e: &XmlElement) -> Self {
        let mut g = ClothingDecalGroup::default();
        for c in e.elements() {
            match c.name.as_str() {
                "name" => g.name = Some(c.text()),
                "decal" => g.decals.push(c.text()),
                "group" => g.groups.push(c.text()),
                _ => {}
            }
        }
        g
    }
}

/// zombie.core.skinnedmodel.population.ClothingDecals (the decal files' cache, `getDecal`, is not ported).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClothingDecals {
    pub groups: Vec<ClothingDecalGroup>,
}

impl ClothingDecals {
    /// JAXB's unmarshalling of the root `clothingDecals`: every `group` element; another root element is the
    /// original's `UnmarshalException`.
    pub fn from_xml(root: &XmlElement) -> Result<Self, String> {
        if root.name != "clothingDecals" {
            return Err(format!(
                "javax.xml.bind.UnmarshalException: unexpected element {}",
                root.name
            ));
        }
        Ok(ClothingDecals {
            groups: root
                .elements()
                .filter(|e| e.name == "group")
                .map(ClothingDecalGroup::from_xml)
                .collect(),
        })
    }

    /// zombie.core.skinnedmodel.population.ClothingDecals.init for the game alone: `Parse` of
    /// `media/clothing/clothingDecals.xml` (`None` where the original keeps a null instance: the file is missing or
    /// does not parse). Mods' groups are not ported.
    pub fn init(game_root: &Path) -> Option<Self> {
        let path = game_root.join("media/clothing/clothingDecals.xml");
        if !path.is_file() {
            return None;
        }
        parse_xml(&path)
            .and_then(|root| ClothingDecals::from_xml(&root))
            .ok()
    }

    /// zombie.core.skinnedmodel.population.ClothingDecals.FindGroup: the first group whose name equals `name`
    /// ignoring case. A group without a name is the original's `NullPointerException` (`Err`).
    pub fn find_group(&self, name: &str) -> Result<Option<&ClothingDecalGroup>, String> {
        if is_null_or_whitespace(Some(name)) {
            return Ok(None);
        }
        for group in &self.groups {
            let Some(group_name) = group.name.as_deref() else {
                return Err(
                    "java.lang.NullPointerException: ClothingDecalGroup without name".to_string(),
                );
            };
            if equals_ignore_case(group_name, name) {
                return Ok(Some(group));
            }
        }
        Ok(None)
    }

    /// zombie.core.skinnedmodel.population.ClothingDecalGroup.getDecals: the group's own decals, then each named
    /// group's, depth first. A group that reaches itself recurses until the original's `StackOverflowError`
    /// (`Err` here once the nesting passes the number of groups).
    pub fn get_decals(
        &self,
        group: &ClothingDecalGroup,
        decals: &mut Vec<String>,
    ) -> Result<(), String> {
        self.get_decals_depth(group, decals, 0)
    }

    fn get_decals_depth(
        &self,
        group: &ClothingDecalGroup,
        decals: &mut Vec<String>,
        depth: usize,
    ) -> Result<(), String> {
        if depth > self.groups.len() {
            return Err("java.lang.StackOverflowError: decal groups reach themselves".to_string());
        }
        decals.extend(group.decals.iter().cloned());
        for name in &group.groups {
            if let Some(g) = self.find_group(name)? {
                self.get_decals_depth(g, decals, depth + 1)?;
            }
        }
        Ok(())
    }

    /// zombie.core.skinnedmodel.population.ClothingDecals.getRandomDecal: `pickRandom` over the group's decals
    /// (`ClothingDecalGroup.getRandomDecal`); `None` for an unknown group or a group without decals.
    pub fn get_random_decal(
        &self,
        group_name: &str,
        rng: &mut OutfitRng,
    ) -> Result<Option<String>, String> {
        let Some(group) = self.find_group(group_name)? else {
            return Ok(None);
        };
        let mut temp_decals = Vec::new();
        self.get_decals(group, &mut temp_decals)?;
        Ok(rng.pick_random(&temp_decals))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zombie::util::pz_xml_util::parse_xml_str;

    /// Nested groups are expanded depth first and names match ignoring case.
    #[test]
    fn nested_groups() {
        let root = parse_xml_str(
            "<clothingDecals><group><name>A</name><decal>a1</decal></group>\
             <group><name>All</name><decal>x</decal><group>a</group><group>missing</group></group></clothingDecals>",
        )
        .unwrap();
        let d = ClothingDecals::from_xml(&root).unwrap();
        let all = d.find_group("ALL").unwrap().unwrap();
        let mut v = Vec::new();
        d.get_decals(all, &mut v).unwrap();
        assert_eq!(v, ["x", "a1"]);
        assert!(d.find_group(" ").unwrap().is_none());
    }
}
