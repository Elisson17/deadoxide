//! zombie.core.Languages: the list of translation languages (`Languages.instance`).
use super::language::Language;
use super::language_file::LanguageFile;
use std::path::Path;
use std::rc::Rc;

/// zombie.core.Languages: the languages in list order (EN first, then the folders in the order the file system
/// lists them) and the default language (EN).
#[derive(Clone, Debug)]
pub struct Languages {
    pub languages: Vec<Rc<Language>>,
    pub default_language: Rc<Language>,
}

impl Default for Languages {
    /// zombie.core.Languages(): only the built-in English.
    fn default() -> Self {
        let en = Rc::new(english());
        Languages {
            languages: vec![en.clone()],
            default_language: en,
        }
    }
}

/// `new Language("EN", "English", null, false)`.
fn english() -> Language {
    Language {
        name: "EN".into(),
        text: Some("English".into()),
        base: None,
        azerty: false,
    }
}

impl Languages {
    /// zombie.core.Languages.init: the built-in English, then `loadTranslateDirectory` of the game's
    /// `media/lua/shared/Translate` (`ZomboidFileSystem.getMediaPath`). Mods are not ported (no mod is active in the
    /// compared runs).
    pub fn init(&mut self, base: &Path) {
        self.languages.clear();
        self.default_language = Rc::new(english());
        self.languages.push(self.default_language.clone());
        self.load_translate_directory(&base.join("media/lua/shared/Translate"));
    }

    /// zombie.core.Languages.getDefaultLanguage.
    pub fn get_default_language(&self) -> Rc<Language> {
        self.default_language.clone()
    }

    /// zombie.core.Languages.getByIndex.
    pub fn get_by_index(&self, index: i32) -> Option<Rc<Language>> {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.languages.get(i).cloned())
    }

    /// zombie.core.Languages.getByName: the first language whose name equals `name` ignoring case (none for null).
    pub fn get_by_name(&self, name: Option<&str>) -> Option<Rc<Language>> {
        let name = name?;
        self.languages
            .iter()
            .find(|l| crate::java::equals_ignore_case(&l.name, name))
            .cloned()
    }

    /// zombie.core.Languages.getIndexByName.
    pub fn get_index_by_name(&self, name: &str) -> i32 {
        self.languages
            .iter()
            .position(|l| crate::java::equals_ignore_case(&l.name, name))
            .map_or(-1, |i| i as i32)
    }

    /// zombie.core.Languages.loadTranslateDirectory: every sub-folder with a `language.json`, in the directory
    /// stream's order (`vfs::read_dir`, the OS order like Java's); a readable `LanguageFile` adds a language or
    /// replaces the one of the same name (and the default when its name is "EN"); errors are logged by the original
    /// (`ExceptionLogger`) and skipped here.
    fn load_translate_directory(&mut self, dir: &Path) {
        if !crate::vfs::exists(dir) {
            return;
        }
        let Ok(names) = crate::vfs::read_dir(dir) else {
            return;
        };
        for name in names {
            let path = dir.join(&name);
            if !(crate::vfs::is_dir(&path) && crate::vfs::exists(&path.join("language.json"))) {
                continue;
            }
            // loadLanguageDirectory
            let Ok(data) = LanguageFile::new(&name, &path.join("language.json")) else {
                continue;
            };
            let lang = Rc::new(Language {
                name: data.name.clone(),
                text: data.language_name,
                base: data.base,
                azerty: data.azerty,
            });
            let index = self.get_index_by_name(&data.name);
            if index == -1 {
                self.languages.push(lang);
            } else {
                self.languages[index as usize] = lang.clone();
                if data.name == self.default_language.name {
                    self.default_language = lang;
                }
            }
        }
    }
}
