//! zombie.FileGuidTable: `media/fileGuidTable.xml`, the guid of every clothing item file (outfits name their items
//! by guid).
use std::collections::HashMap;
use std::path::Path;

use crate::java::case_insensitive_key;
use crate::zombie::util::pz_xml_util::{parse_xml, XmlElement};

/// zombie.FileGuidPair
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileGuidPair {
    pub path: Option<String>,
    pub guid: Option<String>,
}

/// A `TreeMap<String, String>(String.CASE_INSENSITIVE_ORDER)`: keyed by the case-folded key; a put whose key is
/// already there (ignoring case) replaces the value and keeps the first key.
#[derive(Clone, Debug, Default, PartialEq)]
struct CaseInsensitiveMap(HashMap<String, (String, String)>);

impl CaseInsensitiveMap {
    fn put(&mut self, key: &str, value: &str) {
        self.0
            .entry(case_insensitive_key(key))
            .and_modify(|e| e.1 = value.to_string())
            .or_insert_with(|| (key.to_string(), value.to_string()));
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.0
            .get(&case_insensitive_key(key))
            .map(|(_, v)| v.as_str())
    }
}

/// zombie.FileGuidTable
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileGuidTable {
    pub files: Vec<FileGuidPair>,
    guid_to_path: CaseInsensitiveMap,
    path_to_guid: CaseInsensitiveMap,
}

impl FileGuidTable {
    /// JAXB's unmarshalling of the root `fileGuidTable`: every `files` element is a pair of `path` and `guid`;
    /// other elements are ignored. Another root element is the original's `UnmarshalException`.
    pub fn from_xml(root: &XmlElement) -> Result<Self, String> {
        if root.name != "fileGuidTable" {
            return Err(format!(
                "javax.xml.bind.UnmarshalException: unexpected element {}",
                root.name
            ));
        }
        let mut t = FileGuidTable::default();
        for f in root.elements().filter(|e| e.name == "files") {
            let mut pair = FileGuidPair::default();
            for c in f.elements() {
                match c.name.as_str() {
                    "path" => pair.path = Some(c.text()),
                    "guid" => pair.guid = Some(c.text()),
                    _ => {}
                }
            }
            t.files.push(pair);
        }
        Ok(t)
    }

    /// zombie.FileGuidTable.setModID: every guid becomes "modID-guid" (a null guid "modID-null").
    pub fn set_mod_id(&mut self, mod_id: &str) {
        for pair in &mut self.files {
            pair.guid = Some(format!(
                "{mod_id}-{}",
                pair.guid.as_deref().unwrap_or("null")
            ));
        }
    }

    /// zombie.FileGuidTable.mergeFrom
    pub fn merge_from(&mut self, other: FileGuidTable) {
        self.files.extend(other.files);
    }

    /// zombie.FileGuidTable.loaded: both maps filled in file order (a later pair with the same key, ignoring
    /// case, wins). A null path or guid is the original's `NullPointerException` from the map's comparator.
    pub fn loaded(&mut self) -> Result<(), String> {
        for pair in &self.files {
            let (Some(path), Some(guid)) = (pair.path.as_deref(), pair.guid.as_deref()) else {
                return Err(
                    "java.lang.NullPointerException: FileGuidPair without path or guid".to_string(),
                );
            };
            self.guid_to_path.put(guid, path);
            self.path_to_guid.put(path, guid);
        }
        Ok(())
    }

    /// zombie.FileGuidTable.getFilePathFromGuid (ignoring case)
    pub fn get_file_path_from_guid(&self, guid: &str) -> Option<&str> {
        self.guid_to_path.get(guid)
    }

    /// zombie.FileGuidTable.getGuidFromFilePath (ignoring case)
    pub fn get_guid_from_file_path(&self, path: &str) -> Option<&str> {
        self.path_to_guid.get(path)
    }

    /// zombie.ZomboidFileSystem.loadFileGuidTable for the game alone: `media/fileGuidTable.xml`, guids prefixed
    /// "game-", then `loaded()`. Mods' tables (merged in by the original) are not ported. `Err` where the original
    /// logs "Failed to load file Guid table." and keeps no table.
    pub fn load_game(game_root: &Path) -> Result<Self, String> {
        let mut t =
            FileGuidTable::from_xml(&parse_xml(&game_root.join("media/fileGuidTable.xml"))?)?;
        t.set_mod_id("game");
        t.loaded()?;
        Ok(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zombie::util::pz_xml_util::parse_xml_str;

    /// Guids get the mod prefix; lookups ignore case both ways.
    #[test]
    fn guid_lookups_ignore_case() {
        let root = parse_xml_str(
            "<fileGuidTable><files><path>media/clothing/clothingItems/Hat.xml</path><guid>ab-C1</guid></files></fileGuidTable>",
        )
        .unwrap();
        let mut t = FileGuidTable::from_xml(&root).unwrap();
        t.set_mod_id("game");
        t.loaded().unwrap();
        assert_eq!(
            t.get_file_path_from_guid("GAME-AB-c1"),
            Some("media/clothing/clothingItems/Hat.xml")
        );
        assert_eq!(
            t.get_guid_from_file_path("MEDIA/clothing/clothingitems/hat.xml"),
            Some("game-ab-C1")
        );
        assert_eq!(t.get_file_path_from_guid("ab-C1"), None);
    }
}
