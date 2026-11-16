//! Package `org.json` (version 20250517, inside the game's jar): the parser `zombie.core.Translator` and
//! `zombie.core.LanguageFile` read the translation files with. Only reading is ported.
pub mod json_array;
pub mod json_object;
pub mod json_tokener;

/// org.json.JSONException: its message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonException(pub String);

impl std::fmt::Display for JsonException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "org.json.JSONException: {}", self.0)
    }
}
