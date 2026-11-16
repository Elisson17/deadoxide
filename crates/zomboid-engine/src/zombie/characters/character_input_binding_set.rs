//! zombie.characters.CharacterInputBindingSet: the controller binding presets saved as XML in the cache dir's
//! `InputBindings` folder. Reading a preset file (JAXB through PZXmlUtil.parse) is not ported yet: a folder with
//! a preset in it is an error here.
use std::path::Path;

use super::character_joypad_axis2d_binding::CharacterJoypadAxis2dBindings;
use super::character_joypad_button_binding::CharacterJoypadButtonBindings;
use crate::se::krka::kahlua::vm::object::{Throwable, R};

/// zombie.characters.CharacterInputBindingSet (the fields a loaded set has; none is loaded so far).
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterInputBindingSet {
    pub name: String,
    pub description: Option<String>,
}

/// CharacterInputBindingSet's statics and the enum statics its methods change.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputBindings {
    /// CharacterInputBindingSet.loadedBindingSets (null until getLoadedBindingSets/reloadAll).
    pub loaded_binding_sets: Option<Vec<CharacterInputBindingSet>>,
    pub button_bindings: CharacterJoypadButtonBindings,
    pub axis2d_bindings: CharacterJoypadAxis2dBindings,
}

impl InputBindings {
    /// zombie.characters.CharacterInputBindingSet.getLoadedBindingSets: reloadAll() on first use.
    pub fn get_loaded_binding_sets(&mut self, cache_dir: &str) -> R<&[CharacterInputBindingSet]> {
        if self.loaded_binding_sets.is_none() {
            self.reload_all(cache_dir)?;
        }
        Ok(self.loaded_binding_sets.as_deref().unwrap_or_default())
    }

    /// zombie.characters.CharacterInputBindingSet.reloadAll: every `*.xml` file directly in
    /// `getCacheDirSub("InputBindings")` (ZomboidFileSystem.listAllFiles, not recursive, in the directory's listing
    /// order) is a set; none gives an empty array (PZArrayUtil.toArray gives null for an empty list, replaced by
    /// `new CharacterInputBindingSet[0]`).
    pub fn reload_all(&mut self, cache_dir: &str) -> R<&[CharacterInputBindingSet]> {
        let all_sets: Vec<CharacterInputBindingSet> = Vec::new();
        // ZomboidFileSystem.getCacheDirSub: cacheDir + File.separator + subPath
        let bindings_dir_path = format!("{cache_dir}{}InputBindings", std::path::MAIN_SEPARATOR);
        if let Some(bindings_file) = list_all_files(Path::new(&bindings_dir_path)).first() {
            return Err(Throwable::runtime(format!(
                "not ported: CharacterInputBindingSet.loadFromFile {}",
                bindings_file.display()
            )));
        }
        self.loaded_binding_sets = Some(all_sets);
        Ok(self.loaded_binding_sets.as_deref().unwrap_or_default())
    }

    /// zombie.characters.CharacterInputBindingSet.resetAllToDefault
    pub fn reset_all_to_default(&mut self) {
        self.button_bindings.set_all_to_default();
        self.axis2d_bindings.set_all_to_default();
    }
}

/// ZomboidFileSystem.listAllFiles(folderPath, CharacterInputBindingSet::isInputBindingFile, false): the files of an
/// existing folder (`File.listFiles` order) whose name ends with ".xml"; nothing for a missing folder.
fn list_all_files(folder: &Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with(".xml"))
        })
        .collect()
}
