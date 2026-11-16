//! zombie.core.LanguageFile: a translation folder's `language.json`.
use crate::org::json::json_object::JsonObject;
use std::path::Path;

/// zombie.core.LanguageFile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageFile {
    pub name: String,
    pub language_name: Option<String>,
    pub base: Option<String>,
    pub azerty: bool,
}

impl LanguageFile {
    /// zombie.core.LanguageFile(String, Path): `new JSONObject(Files.readString(path))`; `version` (optInt with the
    /// default one) must be one and `language_name` (optString) not blank; `base` (optString) and `azerty`
    /// (optBoolean, default false) are optional. `Err` where the original throws.
    pub fn new(name: &str, path: &Path) -> Result<LanguageFile, String> {
        let bytes = crate::vfs::read(path).map_err(|e| format!("java.io.IOException: {e}"))?;
        let text = String::from_utf8(bytes)
            .map_err(|_| "java.nio.charset.MalformedInputException".to_string())?;
        let json = JsonObject::parse_text(&text).map_err(|e| e.to_string())?;
        let version = json.opt_int("version", 1);
        let file = LanguageFile {
            name: name.to_string(),
            language_name: json.opt_string("language_name", None),
            base: json.opt_string("base", None),
            azerty: json.opt_boolean("azerty", false),
        };
        if version != 1 {
            return Err("java.lang.RuntimeException: invalid or missing version".into());
        }
        if is_null_or_whitespace(file.language_name.as_deref()) {
            return Err(format!(
                "java.lang.RuntimeException: missing or empty value \"language_name\" for language {name}"
            ));
        }
        Ok(file)
    }
}

/// zombie.util.StringUtils.isNullOrWhitespace: null, empty, or every char `Character.isWhitespace`.
pub fn is_null_or_whitespace(s: Option<&str>) -> bool {
    match s {
        None => true,
        Some(s) => s.is_empty() || s.chars().all(crate::java::is_whitespace),
    }
}
