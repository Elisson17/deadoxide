//! zombie.radio.RadioTranslationData: a community translation of a radio data file (`RadioData_<language>.txt`): an
//! `[Info]` header (guid, language, version, translators), then `[Translations]` with `id = text` lines and
//! `[Collection]` blocks that give one text to several ids.
use std::collections::HashMap;

use crate::java;
use crate::zombie::core::translator::Translator;

/// zombie.radio.RadioTranslationData
#[derive(Clone, Debug, PartialEq)]
pub struct RadioTranslationData {
    pub file_path: String,
    pub guid: Option<String>,
    pub language: Option<String>,
    /// `languageEnum`: the name of the `Language` the header names
    pub language_enum: Option<String>,
    pub version: i32,
    pub translators: Vec<String>,
    pub translations: HashMap<String, String>,
}

/// `line.split("=", 2)`: at most two pieces, the first cut at the first `=`; a line without one is one piece.
fn split_eq_2(line: &str) -> Vec<&str> {
    match line.find('=') {
        Some(i) => vec![&line[..i], &line[i + 1..]],
        None => vec![line],
    }
}

impl RadioTranslationData {
    /// `RadioTranslationData(file)`
    pub fn new(file: &str) -> Self {
        RadioTranslationData {
            file_path: file.to_string(),
            guid: None,
            language: None,
            language_enum: None,
            version: -1,
            translators: Vec::new(),
            translations: HashMap::new(),
        }
    }

    /// `getTranslationCount`
    pub fn translation_count(&self) -> usize {
        self.translations.len()
    }

    /// `validate`
    pub fn validate(&self) -> bool {
        self.guid.is_some() && self.language.is_some() && self.version >= 0
    }

    /// `getTranslation(guid)`
    pub fn get_translation(&self, guid: &str) -> Option<&String> {
        self.translations.get(guid)
    }

    /// `ReadFile(file)`: `text` is the file (`None` when it does not exist). The header lines up to `[/Info]` are
    /// read with `split("=")` (everything after the first piece is joined back without its `=`), a number that does
    /// not parse stops the header there (the original logs it), then the language is looked up among the available
    /// ones; `None` is the original's null (language not found, or no guid, language or version).
    pub fn read_file(file: &str, text: Option<&[u8]>, tr: &mut Translator) -> Option<Self> {
        let mut d = RadioTranslationData::new(file);
        if let Some(text) = text {
            let text = String::from_utf8_lossy(text);
            for line in java::buffered_reader_lines(&text) {
                let parts = java::split_char(line, '=');
                if parts.len() > 1 {
                    let id = java::trim(parts[0]);
                    let mut txt = String::new();
                    for p in &parts[1..] {
                        txt.push_str(p);
                    }
                    let txt = java::trim(&txt).to_string();
                    if id == "guid" {
                        d.guid = Some(txt);
                    } else if id == "language" {
                        d.language = Some(txt);
                    } else if id == "version" {
                        match java::parse_int(&txt) {
                            Some(v) => d.version = v,
                            None => break,
                        }
                    } else if id == "translator" {
                        for n in java::split_char(&txt, ',') {
                            d.translators.push(n.to_string());
                        }
                    }
                }
                if java::trim(line) == "[/Info]" {
                    break;
                }
            }
        }
        let mut found = false;
        if let Some(l) = &d.language {
            for lang in tr.get_available_language() {
                if lang.name == *l {
                    d.language_enum = Some(lang.name.clone());
                    found = true;
                    break;
                }
            }
        }
        if !found && d.language.is_some() {
            return None;
        }
        if d.guid.is_some() && d.language.is_some() && d.version >= 0 {
            Some(d)
        } else {
            None
        }
    }

    /// `loadTranslations()`: false unless the file's language is the current one and the `[Translations]` section is
    /// closed by `[/Translations]`. Entries read before a missing close stay in the table.
    pub fn load_translations(&mut self, text: Option<&[u8]>, tr: &mut Translator) -> bool {
        let current = tr.get_language();
        if self.language_enum.as_deref() != Some(current.name.as_str()) {
            return false;
        }
        let mut valid = false;
        let Some(text) = text else {
            return false;
        };
        let text = String::from_utf8_lossy(text);
        let mut lines = java::buffered_reader_lines(&text).into_iter();
        let mut open = false;
        let mut member_keys: Vec<String> = Vec::new();
        while let Some(line) = lines.next() {
            let line = java::trim(line);
            if line == "[Translations]" {
                open = true;
            } else if open {
                if line != "[Collection]" {
                    if line == "[/Translations]" {
                        valid = true;
                        break;
                    }
                    let parts = split_eq_2(line);
                    if parts.len() == 2 {
                        let key = java::trim(parts[0]).to_string();
                        let val = java::trim(parts[1]).to_string();
                        self.translations.insert(key, val);
                    }
                } else {
                    let mut text: Option<String> = None;
                    for line in lines.by_ref() {
                        let line = java::trim(line);
                        if line == "[/Collection]" {
                            break;
                        }
                        let parts = split_eq_2(line);
                        if parts.len() == 2 {
                            let key = java::trim(parts[0]);
                            let val = java::trim(parts[1]).to_string();
                            if key == "text" {
                                text = Some(val);
                            } else if key == "member" {
                                member_keys.push(val);
                            }
                        }
                    }
                    if let Some(t) = &text {
                        if !member_keys.is_empty() {
                            for g in &member_keys {
                                self.translations.insert(g.clone(), t.clone());
                            }
                        }
                    }
                    member_keys.clear();
                }
            }
        }
        valid
    }
}
