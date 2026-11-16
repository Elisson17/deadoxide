//! zombie.core.Translator: the translation tables of `media/lua/shared/Translate/<LANG>/*.json` and the lookups the
//! game and its Lua make (`getText`, `getTextOrNull`, item, recipe, moveable, media, attribute names).
//! The original's statics are one `Translator` value; `Languages.instance` is its `languages` field and the inputs
//! the original reads from elsewhere (`Core`'s language option, the JVM's `user.language`, `Core.debug`) are fields.
//! Not ported: the mods' tables (`tryFillMapFromMods`; no mod is active in the compared runs; `readModTranslation`
//! is ported),
//! `Translator.debug` (the translationProblems.txt report, off in the game), the `Core.IS_DEV` checks (off in the
//! game: the strict JSON mode, the look-alike character check, the placeholder report), `readMapTranslation`,
//! `getMoveableDisplayName(IsoObject)`, and `PerkFactory.initTranslations` at the end of `loadFiles` (standalone its
//! perk list is empty, measured by `TranslatorProbe`; with perks it renames them through `getText`).
use super::language::Language;
use super::language_file::is_null_or_whitespace;
use super::languages::Languages;
use crate::java_formatter::{self, FormatArg, FormatError};
use crate::org::json::json_object::{JsonObject, JsonValue};
use crate::org::json::JsonException;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// zombie.core.Translator.BY_NAME: the file name of each table, in the original's (LinkedHashMap) order, which is the
/// order `loadFiles` fills them in.
pub const BY_NAME: [&str; 30] = [
    "Tooltip",
    "IG_UI",
    "Recipes",
    "RecipeGroups",
    "Farming",
    "ContextMenu",
    "SurvivalGuide",
    "UI",
    "Items",
    "ItemName",
    "Moodles",
    "Sandbox",
    "Challenge",
    "Stash",
    "Moveables",
    "MakeUp",
    "GameSound",
    "DynamicRadio",
    "EvolvedRecipeName",
    "Recorded_Media",
    "SurvivorNames",
    "Attributes",
    "Fluids",
    "Print_Media",
    "Print_Text",
    "Entity",
    "RadioData",
    "BodyParts",
    "MapLabel",
    "Credits",
];

// Indices into BY_NAME / Translator::maps (the original's field names).
const TOOLTIP: usize = 0;
const IGUI: usize = 1;
const RECIPE: usize = 2;
const RECIPE_GROUPS: usize = 3;
const FARMING: usize = 4;
const CONTEXT_MENU: usize = 5;
const SURVIVAL_GUIDE: usize = 6;
const UI: usize = 7;
const ITEMS: usize = 8;
const ITEM_NAME: usize = 9;
const MOODLES: usize = 10;
const SANDBOX: usize = 11;
const CHALLENGE: usize = 12;
const STASH: usize = 13;
const MOVEABLES: usize = 14;
const MAKEUP: usize = 15;
const GAME_SOUND: usize = 16;
const DYNAMIC_RADIO: usize = 17;
const ITEM_EVOLVED_RECIPE_NAME: usize = 18;
const RECORDED_MEDIA: usize = 19;
const SURVIVOR_NAMES: usize = 20;
const ATTRIBUTES: usize = 21;
const FLUIDS: usize = 22;
const PRINT_MEDIA: usize = 23;
const PRINT_TEXT: usize = 24;
const ENTITY: usize = 25;
const RADIO_DATA: usize = 26;
const BODY_PARTS: usize = 27;
const MAP_LABEL: usize = 28;
const CREDITS: usize = 29;

/// What reading one translation file throws: `RuntimeException("JSON Error in: <file>")` with this cause.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FillError {
    /// java.io.IOException from Files.readString
    Io(String),
    /// java.nio.charset.MalformedInputException: the file is not UTF-8
    Malformed,
    /// org.json.JSONException
    Json(JsonException),
    /// java.lang.ClassCastException: a value that is not a string
    ClassCast,
    /// java.lang.NullPointerException: a null value
    NullPointer,
}

impl FillError {
    /// The cause as `TranslatorProbe` prints it: class, and the message of a JSONException.
    pub fn java_text(&self) -> String {
        match self {
            FillError::Io(_) => "java.io.IOException".into(),
            FillError::Malformed => "java.nio.charset.MalformedInputException".into(),
            FillError::Json(e) => e.to_string(),
            FillError::ClassCast => "java.lang.ClassCastException".into(),
            FillError::NullPointer => "java.lang.NullPointerException".into(),
        }
    }
}

/// java.lang.RuntimeException("JSON Error in: %s") out of `tryFillMapFromFile` (and so out of `loadFiles`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranslatorError {
    pub file: String,
    pub cause: FillError,
}

impl std::fmt::Display for TranslatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "java.lang.RuntimeException: JSON Error in: {}: {}",
            self.file,
            self.cause.java_text()
        )
    }
}

/// zombie.core.Translator (its static state).
#[derive(Clone, Debug)]
pub struct Translator {
    /// ZomboidFileSystem.instance.base.canonicalFile: the game folder
    pub base: PathBuf,
    /// Languages.instance
    pub languages: Languages,
    /// Core.getInstance().getOptionLanguageName()
    pub option_language_name: String,
    /// System.getProperty("user.language"): the JVM's, from the OS locale (PLACEHOLDER "en")
    pub user_language: String,
    /// Core.debug
    pub core_debug: bool,
    /// DebugOptions.instance.translationPrefix (read only when `core_debug`)
    pub translation_prefix: bool,
    /// Translator.language
    pub language: Option<Rc<Language>>,
    available_language: Option<Vec<Rc<Language>>>,
    /// the tables of BY_NAME, same order
    maps: Vec<HashMap<String, String>>,
    /// Translator.recordedMedia_EN (never cleared)
    pub recorded_media_en: HashMap<String, String>,
    /// Translator.missing
    pub missing: HashSet<String>,
    /// Translator.brReplacements: text with `<br>` → text with line breaks (a cache, never cleared)
    pub br_replacements: HashMap<String, String>,
    /// Translator.tempMap
    temp_map: HashMap<String, String>,
}

impl Default for Translator {
    /// The initial state over `ZomboidFileSystem.base`'s default, "./".
    fn default() -> Self {
        Translator::new(Path::new("."))
    }
}

impl Translator {
    /// The class's initial static state over the game folder `base`: no table loaded, `Languages.instance` with
    /// only English, no language chosen.
    pub fn new(base: &Path) -> Self {
        Translator {
            base: base.to_path_buf(),
            languages: Languages::default(),
            option_language_name: String::new(),
            user_language: "en".into(),
            core_debug: false,
            translation_prefix: false,
            language: None,
            available_language: None,
            maps: vec![HashMap::new(); BY_NAME.len()],
            recorded_media_en: HashMap::new(),
            missing: HashSet::new(),
            br_replacements: HashMap::new(),
            temp_map: HashMap::new(),
        }
    }

    /// zombie.network.GameServer.main's start of the translations (GameWindow.init does the same):
    /// `Languages.instance.init()`, then `Translator.loadFiles()`, with Core's language option set to
    /// `option_language_name`.
    pub fn start(base: &Path, option_language_name: &str) -> Result<Self, TranslatorError> {
        let mut t = Translator::new(base);
        t.languages.init(base);
        t.option_language_name = option_language_name.to_string();
        t.load_files()?;
        Ok(t)
    }

    /// The table of a BY_NAME file name.
    pub fn map(&self, name: &str) -> Option<&HashMap<String, String>> {
        BY_NAME
            .iter()
            .position(|n| *n == name)
            .map(|i| &self.maps[i])
    }

    /// zombie.core.Translator.getUI.
    pub fn get_ui(&mut self) -> &mut HashMap<String, String> {
        &mut self.maps[UI]
    }

    /// zombie.core.Translator.radioData (the `RD_` table of `RadioData.json`), for a probe that adds texts to it.
    pub fn radio_data_mut(&mut self) -> &mut HashMap<String, String> {
        &mut self.maps[RADIO_DATA]
    }

    /// zombie.core.Translator.loadFiles: forgets the language, clears every table but `entity` (the original does
    /// not clear it, so its entries of an earlier load stay), resolves the language (`getLanguage()` in the log
    /// line), then fills each BY_NAME table from its file in every language of `forLanguageStack` (the default
    /// language first) and `recordedMedia_EN` from the default language's `Recorded_Media.json`. A file's error
    /// stops the load, as the original's RuntimeException does.
    pub fn load_files(&mut self) -> Result<(), TranslatorError> {
        self.language = None;
        self.available_language = None;
        for (i, m) in self.maps.iter_mut().enumerate() {
            if i != ENTITY {
                m.clear();
            }
        }
        self.missing.clear();
        // DebugType.Translation.println("translator: language is " + getLanguage())
        self.get_language();
        let root = self.base.to_string_lossy().into_owned();
        for (i, name) in BY_NAME.iter().enumerate() {
            for l in self.for_language_stack() {
                try_fill_map_from_file(&root, name, &mut self.maps[i], &l)?;
            }
        }
        let default = self.get_default_language();
        try_fill_map_from_file(
            &root,
            "Recorded_Media",
            &mut self.recorded_media_en,
            &default,
        )?;
        Ok(())
    }

    /// zombie.core.Translator.forLanguageStack: the language, its base, the base's base ... (until none or one seen
    /// before), then the default language if missing; returned last first (the order the consumer sees them).
    pub fn for_language_stack(&mut self) -> Vec<Rc<Language>> {
        let mut bases: Vec<Rc<Language>> = Vec::new();
        let mut language = Some(self.get_language());
        while let Some(l) = language {
            if bases.iter().any(|b| Rc::ptr_eq(b, &l)) {
                break;
            }
            bases.push(l.clone());
            language = self.languages.get_by_name(l.base.as_deref());
        }
        let default = self.get_default_language();
        if !bases.iter().any(|b| Rc::ptr_eq(b, &default)) {
            bases.push(default);
        }
        bases.reverse();
        bases
    }

    /// zombie.core.Translator.getTextInternal: the table chosen by the key's prefix (`UI_`, `Moodles_`, ... in the
    /// original's order; other keys find nothing), the key itself when missing (remembered in `missing`; `None`
    /// when `null_ok`), then `<br>`/`<BR>` turned into line breaks through the `brReplacements` cache (without the
    /// debug prefix) or the debug prefix (`*` found, `!` missing) when `Core.debug` and `translationPrefix`.
    fn get_text_internal(&mut self, desc: &str, null_ok: bool) -> Option<String> {
        // `if (ui == null) loadFiles()`: ui is a final field, never null.
        const ROUTES: [(&str, usize); 25] = [
            ("UI_", UI),
            ("Moodles_", MOODLES),
            ("SurvivalGuide_", SURVIVAL_GUIDE),
            ("Farming_", FARMING),
            ("IGUI_", IGUI),
            ("ContextMenu_", CONTEXT_MENU),
            ("GameSound_", GAME_SOUND),
            ("Sandbox_", SANDBOX),
            ("Tooltip_", TOOLTIP),
            ("Challenge_", CHALLENGE),
            ("MakeUp", MAKEUP),
            ("Stash_", STASH),
            ("RM_", RECORDED_MEDIA),
            ("SurvivorName_", SURVIVOR_NAMES),
            ("SurvivorSurname_", SURVIVOR_NAMES),
            ("Attributes_", ATTRIBUTES),
            ("Fluid_", FLUIDS),
            ("Print_Media_", PRINT_MEDIA),
            ("Print_Text_", PRINT_TEXT),
            ("EC_", ENTITY),
            ("RD_", RADIO_DATA),
            ("BODYPART_", BODY_PARTS),
            ("MapLabel_", MAP_LABEL),
            ("credits_", CREDITS),
            ("AEBS_", DYNAMIC_RADIO),
        ];
        let mut result = ROUTES
            .iter()
            .find(|(p, _)| desc.starts_with(p))
            .and_then(|(_, m)| self.maps[*m].get(desc).cloned());
        let prefix = self.core_debug && self.translation_prefix;
        let mut dbg = if prefix { Some("*") } else { None };
        if result.is_none() {
            if null_ok {
                return None;
            }
            if !self.missing.contains(desc) {
                // the original logs "ERROR: Missing translation" when Core.debug
                self.missing.insert(desc.to_string());
            }
            result = Some(desc.to_string());
            dbg = if prefix { Some("!") } else { None };
        }
        let result = result.unwrap_or_default();
        if !result.contains("<br>") && !result.contains("<BR>") {
            return Some(match dbg {
                None => result,
                Some(d) => format!("{d}{result}"),
            });
        }
        Some(
            self.br_replacements
                .entry(result.clone())
                .or_insert_with(|| result.replace("<br>", "\n").replace("<BR>", "\n"))
                .clone(),
        )
    }

    /// zombie.core.Translator.getText(String, Object...): the text, formatted with the arguments. `Err` only for a
    /// formatting this port does not print (`FormatError::NotPorted`).
    pub fn get_text(&mut self, desc: &str, args: &[FormatArg]) -> Result<String, String> {
        let text = self.get_text_internal(desc, false).unwrap_or_default();
        report_missing_arguments_from_past_abuse(desc, &text, args)
    }

    /// zombie.core.Translator.getTextOrNull(String, Object...): `None` for a missing key (not remembered as
    /// missing), else formatted as by `get_text`.
    pub fn get_text_or_null(
        &mut self,
        desc: &str,
        args: &[FormatArg],
    ) -> Result<Option<String>, String> {
        match self.get_text_internal(desc, true) {
            None => Ok(None),
            Some(text) => report_missing_arguments_from_past_abuse(desc, &text, args).map(Some),
        }
    }

    /// zombie.core.Translator.setLanguage(Language): null means the default language.
    pub fn set_language(&mut self, language: Option<Rc<Language>>) {
        self.language = Some(language.unwrap_or_else(|| self.get_default_language()));
    }

    /// zombie.core.Translator.setLanguage(int): `Languages.instance.getLanguages().get(id)`; `Err` is the
    /// IndexOutOfBoundsException.
    pub fn set_language_id(&mut self, language_id: i32) -> Result<(), String> {
        let l = self.languages.get_by_index(language_id).ok_or_else(|| {
            format!("java.lang.IndexOutOfBoundsException: Index {language_id} out of bounds")
        })?;
        self.set_language(Some(l));
        Ok(())
    }

    /// zombie.core.Translator.getLanguage: once chosen it stays; else Core's language option (when not blank), else
    /// the JVM's `user.language` upper-cased, else the default language; each by `Languages.getByName`.
    pub fn get_language(&mut self) -> Rc<Language> {
        if self.language.is_none() && !is_null_or_whitespace(Some(&self.option_language_name)) {
            self.language = self.languages.get_by_name(Some(&self.option_language_name));
        }
        if self.language.is_none() {
            let name = crate::java::to_upper_case(&self.user_language);
            self.language = self.languages.get_by_name(Some(&name));
        }
        if self.language.is_none() {
            self.language = Some(self.get_default_language());
        }
        self.language
            .clone()
            .unwrap_or_else(|| self.get_default_language())
    }

    /// zombie.core.Translator.getAvailableLanguage: `Languages.instance.getLanguages()` (the list as it is now).
    pub fn get_available_language(&mut self) -> Vec<Rc<Language>> {
        if self.available_language.is_none() {
            self.available_language = Some(self.languages.languages.clone());
        }
        self.languages.languages.clone()
    }

    /// zombie.core.Translator.getDisplayItemName: the `Items` table under the name with ' ' and '-' made '_', else
    /// the name.
    pub fn get_display_item_name(&self, trim: &str) -> String {
        // trim.replace(" ", "_").replace("-", "_")
        let key = trim.replace([' ', '-'], "_");
        self.maps[ITEMS]
            .get(&key)
            .cloned()
            .unwrap_or_else(|| trim.to_string())
    }

    /// zombie.core.Translator.getItemNameFromFullType: the `ItemName` entry, else the script item's display name
    /// (`script_display_name`: `ScriptManager.instance.getItem(fullType).getDisplayName()`, `None` when there is no
    /// such item), else the full type; the answer is cached in the table.
    pub fn get_item_name_from_full_type(
        &mut self,
        full_type: &str,
        script_display_name: Option<String>,
    ) -> Result<String, String> {
        if !full_type.contains('.') {
            return Err(FULL_TYPE_ERROR.into());
        }
        if let Some(name) = self.maps[ITEM_NAME].get(full_type) {
            return Ok(name.clone());
        }
        let name = script_display_name.unwrap_or_else(|| full_type.to_string());
        self.maps[ITEM_NAME].insert(full_type.to_string(), name.clone());
        Ok(name)
    }

    /// zombie.core.Translator.setDefaultItemEvolvedRecipeName: only while the language is the default one; adds
    /// the English name unless the table has the full type.
    pub fn set_default_item_evolved_recipe_name(
        &mut self,
        full_type: &str,
        english: &str,
    ) -> Result<(), String> {
        let (l, d) = (self.get_language(), self.get_default_language());
        if Rc::ptr_eq(&l, &d) {
            if !full_type.contains('.') {
                return Err(FULL_TYPE_ERROR.into());
            }
            self.maps[ITEM_EVOLVED_RECIPE_NAME]
                .entry(full_type.to_string())
                .or_insert_with(|| english.to_string());
        }
        Ok(())
    }

    /// zombie.core.Translator.getItemEvolvedRecipeName: as `get_item_name_from_full_type`, over the
    /// `EvolvedRecipeName` table.
    pub fn get_item_evolved_recipe_name(
        &mut self,
        full_type: &str,
        script_display_name: Option<String>,
    ) -> Result<String, String> {
        if !full_type.contains('.') {
            return Err(FULL_TYPE_ERROR.into());
        }
        if let Some(name) = self.maps[ITEM_EVOLVED_RECIPE_NAME].get(full_type) {
            return Ok(name.clone());
        }
        let name = script_display_name.unwrap_or_else(|| full_type.to_string());
        self.maps[ITEM_EVOLVED_RECIPE_NAME].insert(full_type.to_string(), name.clone());
        Ok(name)
    }

    /// zombie.core.Translator.getMoveableDisplayName(String): the `Moveables` entry of the name with ' ' and '-'
    /// made '_', `'` and the two characters `\.` removed (a literal `String.replace`), else the name.
    pub fn get_moveable_display_name(&self, name: &str) -> String {
        let prefix = self.core_debug && self.translation_prefix;
        match self.maps[MOVEABLES].get(&moveable_key(name)) {
            None if prefix => format!("!{name}"),
            None => name.to_string(),
            Some(r) if prefix => format!("*{r}"),
            Some(r) => r.clone(),
        }
    }

    /// zombie.core.Translator.getMoveableDisplayNameOrNull.
    pub fn get_moveable_display_name_or_null(&self, name: &str) -> Option<String> {
        let r = self.maps[MOVEABLES].get(&moveable_key(name))?;
        Some(if self.core_debug && self.translation_prefix {
            format!("*{r}")
        } else {
            r.clone()
        })
    }

    /// zombie.core.Translator.getRecipeName: the `Recipes` entry when not empty, else the name.
    pub fn get_recipe_name(&self, name: &str) -> String {
        match self.maps[RECIPE].get(name) {
            Some(r) if !r.is_empty() => r.clone(),
            _ => name.to_string(),
        }
    }

    /// zombie.core.Translator.getRecipeGroupName: the `RecipeGroups` entry when not empty, else the name.
    pub fn get_recipe_group_name(&self, name: &str) -> String {
        match self.maps[RECIPE_GROUPS].get(name) {
            Some(r) if !r.is_empty() => r.clone(),
            _ => name.to_string(),
        }
    }

    /// zombie.core.Translator.getDefaultLanguage.
    pub fn get_default_language(&self) -> Rc<Language> {
        self.languages.get_default_language()
    }

    /// zombie.core.Translator.getAzertyMap.
    pub fn get_azerty_map() -> Vec<String> {
        vec!["FR".to_string()]
    }

    /// zombie.core.Translator.getTextMediaEN: `RM_` keys in `recordedMedia_EN` (no formatting), else the key
    /// (remembered as missing only when `Core.debug`); only lower-case `<br>` becomes a line break, and then
    /// without the debug prefix.
    pub fn get_text_media_en(&mut self, desc: &str) -> String {
        let mut result = if desc.starts_with("RM_") {
            self.recorded_media_en.get(desc).cloned()
        } else {
            None
        };
        let prefix = self.core_debug && self.translation_prefix;
        let mut dbg = if prefix { Some("*") } else { None };
        if result.is_none() {
            if !self.missing.contains(desc) && self.core_debug {
                self.missing.insert(desc.to_string());
            }
            result = Some(desc.to_string());
            dbg = if prefix { Some("!") } else { None };
        }
        let result = result.unwrap_or_default();
        if result.contains("<br>") {
            return result.replace("<br>", "\n");
        }
        match dbg {
            None => result,
            Some(d) => format!("{d}{result}"),
        }
    }

    /// zombie.core.Translator.getAttributeText(String, boolean): the `Attributes` entry; a missing one is logged
    /// and remembered in `missing`, and gives `None` (`null_on_fail`) or the key.
    pub fn get_attribute_text(&mut self, s: &str, null_on_fail: bool) -> Option<String> {
        match self.maps[ATTRIBUTES].get(s) {
            Some(r) => Some(r.clone()),
            None => {
                if !self.missing.contains(s) {
                    self.missing.insert(s.to_string());
                }
                if null_on_fail {
                    None
                } else {
                    Some(s.to_string())
                }
            }
        }
    }

    /// zombie.core.Translator.getFluidText.
    pub fn get_fluid_text(&self, s: &str) -> String {
        self.maps[FLUIDS]
            .get(s)
            .cloned()
            .unwrap_or_else(|| s.to_string())
    }

    /// zombie.core.Translator.getEntityText.
    pub fn get_entity_text(&self, s: &str) -> String {
        self.maps[ENTITY]
            .get(s)
            .cloned()
            .unwrap_or_else(|| s.to_string())
    }

    /// zombie.core.Translator.getMapLabelText.
    pub fn get_map_label_text(&self, s: &str) -> String {
        self.maps[MAP_LABEL]
            .get(s)
            .cloned()
            .unwrap_or_else(|| s.to_string())
    }

    /// Translator.tempMap (filled by readModTranslation; readMapTranslation is not ported).
    pub fn temp_map(&self) -> &HashMap<String, String> {
        &self.temp_map
    }

    /// zombie.core.Translator.readModTranslation: `tempMap` cleared, then for each language of `forLanguageStack`
    /// the mod's `Mod.json` from its common folder, then from its version folder; the mod's name and description
    /// become the table's "name"/"description" when present. A file's error is the RuntimeException the original
    /// throws (the mod's name and description are then left as they were).
    pub fn read_mod_translation(
        &mut self,
        common_dir: &str,
        version_dir: &str,
        name: &mut String,
        desc: &mut String,
    ) -> Result<(), TranslatorError> {
        self.temp_map.clear();
        for lang in self.for_language_stack() {
            try_fill_map_from_file(common_dir, "Mod", &mut self.temp_map, &lang)?;
            try_fill_map_from_file(version_dir, "Mod", &mut self.temp_map, &lang)?;
        }
        if let Some(n) = self.temp_map.get("name") {
            *name = n.clone();
        }
        if let Some(d) = self.temp_map.get("description") {
            *desc = d.clone();
        }
        Ok(())
    }
}

const FULL_TYPE_ERROR: &str =
    "java.lang.IllegalArgumentException: fullType must contain \".\" i.e. module.type";

/// The key getMoveableDisplayName looks up: `name.replace(" ", "_").replace("-", "_").replace("'", "")
/// .replace("\\.", "")`.
fn moveable_key(name: &str) -> String {
    name.replace([' ', '-'], "_")
        .replace('\'', "")
        .replace("\\.", "")
}

/// zombie.core.Translator.tryFillMapFromFile: `<root>/media/lua/shared/Translate/<language>/<fileName>.json` when it
/// exists (a folder of that name exists too, and fails to read), else nothing.
pub fn try_fill_map_from_file(
    root: &str,
    file_name: &str,
    map: &mut HashMap<String, String>,
    language: &Language,
) -> Result<(), TranslatorError> {
    let file = format!(
        "{root}/media/lua/shared/Translate/{}/{file_name}.json",
        language.name
    );
    let path = Path::new(&file);
    if !crate::vfs::exists(path) {
        return Ok(());
    }
    let result = match crate::vfs::read(path) {
        Ok(bytes) => fill_map_from_bytes(&bytes, map),
        Err(e) => Err(FillError::Io(e.to_string())),
    };
    result.map_err(|cause| TranslatorError { file, cause })
}

/// The body of zombie.core.Translator.tryFillMapFromFile after `File.exists`: `Files.readString` (UTF-8, malformed
/// input an error), `new JSONObject(content, not strict).toMap()`, then every entry is put through `formatFixer`
/// unless the key is there already and the new value is empty. A value that is not a string fails the `(String)`
/// cast (ClassCastException), a null one `formatFixer` or `toString()` (NullPointerException). The original walks
/// `toMap()`'s HashMap order, this port the file's; the table is the same, only which of two bad values is
/// reported could differ (UNKNOWN).
pub fn fill_map_from_bytes(
    bytes: &[u8],
    map: &mut HashMap<String, String>,
) -> Result<(), FillError> {
    let content = std::str::from_utf8(bytes).map_err(|_| FillError::Malformed)?;
    let obj = JsonObject::parse_text(content).map_err(FillError::Json)?;
    for (k, v) in obj.into_entries() {
        match v {
            JsonValue::Str(s) => {
                if !map.contains_key(&k) || !s.is_empty() {
                    let fixed = format_fixer(&s);
                    map.insert(k, fixed);
                }
            }
            JsonValue::Null => return Err(FillError::NullPointer),
            _ => return Err(FillError::ClassCast),
        }
    }
    Ok(())
}

/// zombie.core.Translator.formatFixer: `FORMAT_TOKEN` (`%%|%([1-9])`) replaced left to right: `%%` stays, `%N`
/// becomes `%N$s`.
pub fn format_fixer(input: &str) -> String {
    let b = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            match b.get(i + 1) {
                Some(b'%') => {
                    out.push_str("%%");
                    i += 2;
                    continue;
                }
                Some(d @ b'1'..=b'9') => {
                    out.push('%');
                    out.push(*d as char);
                    out.push_str("$s");
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        // Copy one whole UTF-8 character.
        let len = input[i..].chars().next().map_or(1, char::len_utf8);
        out.push_str(&input[i..i + len]);
        i += len;
    }
    out
}

/// zombie.core.Translator.fixupArgs: null becomes "" and a Double its text (`Long.toString` when it is a whole
/// number in long range after the saturating cast, else `Double.toString`). The original rewrites the caller's
/// array in place; here a copy.
pub fn fixup_args(args: &[FormatArg]) -> Vec<FormatArg> {
    args.iter()
        .map(|a| match a {
            FormatArg::Null => FormatArg::Str(String::new()),
            FormatArg::Double(d) => {
                let l = *d as i64;
                FormatArg::Str(if *d == l as f64 {
                    l.to_string()
                } else {
                    crate::java::double_to_string(*d)
                })
            }
            other => other.clone(),
        })
        .collect()
}

/// zombie.core.Translator.reportMissingArgumentsFromPastAbuse: `text.formatted(fixupArgs(args))`; with too few
/// arguments the unformatted text; any other format error (outside Core.IS_DEV) retries with every `%` not followed
/// by `d`, `s` or `.` doubled and `%%<digits>` made `%<digits>$s`, and gives the unformatted text if that fails too.
fn report_missing_arguments_from_past_abuse(
    _desc: &str,
    text: &str,
    args: &[FormatArg],
) -> Result<String, String> {
    let args = fixup_args(args);
    match java_formatter::format(text, &args) {
        Ok(s) => Ok(s),
        // logged: "ERROR: Missing arguments for ..."
        Err(FormatError::Missing(_)) => Ok(text.to_string()),
        // logged: "ERROR: Formatting ..."
        Err(FormatError::Illegal(..)) => {
            let retry = number_escaped_percents(&escape_lone_percents(text));
            match java_formatter::format(&retry, &args) {
                Ok(s) => Ok(s),
                Err(FormatError::NotPorted(m)) => Err(format!("not ported: {m}")),
                Err(_) => Ok(text.to_string()),
            }
        }
        Err(FormatError::NotPorted(m)) => Err(format!("not ported: {m}")),
    }
}

/// `text.replaceAll("%(?=[^ds.]|$)", "%%")`.
fn escape_lone_percents(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        out.push(c);
        if c == '%' && !matches!(it.peek(), Some('d' | 's' | '.')) {
            out.push('%');
        }
    }
    out
}

/// `text.replaceAll("%%(\\d+)", "%$1\\$s")`: `%%` followed by ASCII digits becomes `%<digits>$s`.
fn number_escaped_percents(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len() + 8);
    let mut i = 0;
    let mut copied = 0;
    while i + 2 < b.len() {
        if b[i] == b'%' && b[i + 1] == b'%' && b[i + 2].is_ascii_digit() {
            let mut j = i + 2;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            out.push_str(&text[copied..i]);
            out.push('%');
            out.push_str(&text[i + 2..j]);
            out.push_str("$s");
            i = j;
            copied = j;
        } else {
            i += 1;
        }
    }
    out.push_str(&text[copied..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_fixer_rewrites_numbered_placeholders_only() {
        assert_eq!(
            format_fixer("%1 of %2, 100%% and %a %0"),
            "%1$s of %2$s, 100%% and %a %0"
        );
    }

    #[test]
    fn fallback_rewrites_follow_the_two_regexes() {
        assert_eq!(
            escape_lone_percents("%02dm %s %.1 5% %"),
            "%%02dm %s %.1 5%% %%"
        );
        assert_eq!(
            number_escaped_percents("%%%1 %%12x %%a"),
            "%%1$s %12$sx %%a"
        );
    }

    #[test]
    fn empty_values_do_not_override_and_bad_values_fail() {
        let mut m = HashMap::new();
        assert!(fill_map_from_bytes(br#"{"a": "x", "b": "%1"}"#, &mut m).is_ok());
        assert!(fill_map_from_bytes(br#"{"a": "", "c": ""}"#, &mut m).is_ok());
        assert_eq!(m.get("a").map(String::as_str), Some("x"));
        assert_eq!(m.get("b").map(String::as_str), Some("%1$s"));
        assert_eq!(m.get("c").map(String::as_str), Some(""));
        assert_eq!(
            fill_map_from_bytes(br#"{"a": 1}"#, &mut m),
            Err(FillError::ClassCast)
        );
    }
}
