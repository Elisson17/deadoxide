//! zombie.scripting.ScriptManager: so far reading the script files into modules and loading `entity` scripts'
//! SpriteConfig components, items, models, vehicle templates, vehicles, timed actions, craft recipes, the XUI scripts
//! (with XuiManager.ParseScripts), the sound scripts (with GameSounds.ScriptsLoaded) and the `soundTimeline` and
//! `vehicleEngineRPM` scripts. Old-style recipes and the other script types are collected as raw text only.
use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::entity::components::crafting::craft_recipe::{CraftRecipe, LoadContext};
use super::item::{Item, Value};
use super::model_script::ModelScript;
use super::objects::base_script_object::ScriptObject;
use super::objects::game_sound_script::GameSoundScript;
use super::objects::sound_timeline_script::SoundTimelineScript;
use super::objects::timed_action_script::TimedActionScript;
use super::objects::vehicle_script::VehicleScript;
use super::objects::vehicle_template::{TemplateContext, VehicleTemplate};
use super::objects::xui_colors_script::XuiColorsScript;
use super::objects::xui_config_script::XuiConfigScript;
use super::objects::xui_layout_script::XuiLayoutScript;
use super::objects::xui_skin_script::XuiSkinScript;
use super::script_parser::{self, Block};
use super::sprite_config::SpriteConfigScript;
use super::ui::xui_manager::{XuiManager, XuiScripts};
use super::ui::xui_script_type::XuiScriptType;
use crate::java;
use crate::zombie::core::colors::Colors;
use crate::zombie::core::translator::Translator;
use crate::zombie::entity::components::crafting::recipe::output_mapper::Lookups;
use crate::zombie::game_sounds::GameSounds;
use crate::zombie::util::string_utils;
use crate::zombie::vehicles::vehicle_engine_rpm::VehicleEngineRPM;

/// zombie.scripting.entity.GameEntityScript, the ported part.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GameEntityScript {
    pub name: String,
    pub enabled: bool,
    pub debug_only: bool,
    pub sprite_config: Option<SpriteConfigScript>,
}

/// zombie.scripting.objects.ScriptModule with its ScriptBucket load data: script bodies by (tag, name), in file order.
#[derive(Clone, Debug, Default)]
pub struct ScriptModule {
    pub name: String,
    pub imports: Vec<String>,
    /// tag -> names in first-seen order
    pub names: HashMap<String, Vec<String>>,
    /// (tag, name) -> bodies in load order (later files add bodies to an existing script)
    pub bodies: HashMap<(String, String), Vec<String>>,
    /// (tag, name) -> index in `ScriptManager::load_file_names` of the file of the last body
    /// (`ScriptManager.currentFileName` when the bucket last ran `InitLoadPP`).
    pub last_file: HashMap<(String, String), usize>,
}

#[derive(Clone, Debug, Default)]
pub struct ScriptManager {
    /// zombie.scripting.ScriptManager.moduleList
    pub module_list: Vec<ScriptModule>,
    /// Files in load order, relative lower-case paths.
    pub load_file_names: Vec<String>,
    /// zombie.scripting.ScriptManager.getAllGameEntities
    pub entities: Vec<GameEntityScript>,
    /// Item scripts that loaded, in module then script-name order (the original's list order is not reproduced).
    pub items: Vec<super::item::Item>,
    /// Model scripts that loaded, per module (same order as `module_list`), in the order of the script names.
    pub models: Vec<Vec<ModelScript>>,
    /// zombie.core.Translator, which `Item.OnScriptsLoaded` asks for display names.
    pub translator: Translator,
    /// Items the original drops because their script failed to load: (full name, error).
    pub item_errors: Vec<(String, String)>,
    /// Timed action scripts that loaded, per module (same order as `module_list`).
    pub timed_actions: Vec<Vec<TimedActionScript>>,
    /// zombie.scripting.ScriptManager.getAllCraftRecipes: the craftRecipe scripts that loaded, in the original's list
    /// order (per module, the HashMap order of their names).
    pub craft_recipes: Vec<CraftRecipe>,
    /// craftRecipe scripts the original drops because a body failed to load (`RemoveLoadError`): (full type, error).
    pub craft_recipe_errors: Vec<(String, String)>,
    /// The exception that ended the craft recipes' OnScriptsLoaded loop (the later recipes are not processed).
    pub craft_recipe_scripts_loaded_error: Option<String>,
    /// The exception that ended the craft recipes' OnPostWorldDictionaryInit loop (the later recipes are not
    /// processed).
    pub craft_recipe_post_init_error: Option<String>,
    /// zombie.scripting.ScriptManager.getAllVehicleTemplates, in module order then first-seen order of the names
    /// (the original's HashMap order is not reproduced); their scripts as the vehicles' loading built them.
    pub vehicle_templates: Vec<VehicleTemplate>,
    /// zombie.scripting.ScriptManager.getAllVehicleScripts: per module, the HashMap order of the names kept.
    pub vehicles: Vec<VehicleScript>,
    /// Vehicle scripts the original drops: a body that threw (`RemoveLoadError`) or `Loaded` that threw (from
    /// `onScriptLoad`, before `scriptMap.put`): (full type, error).
    pub vehicle_errors: Vec<(String, String)>,
    /// The DebugLog lines and printed exceptions of the vehicles' and templates' loading.
    pub vehicle_log: Vec<String>,
    /// zombie.scripting.ScriptManager.getAllXuiConfigScripts: per module, the HashMap order of the names kept.
    pub xui_configs: Vec<XuiConfigScript>,
    /// zombie.scripting.ScriptManager.getAllXuiLayouts
    pub xui_layouts: Vec<XuiLayoutScript>,
    /// zombie.scripting.ScriptManager.getAllXuiStyles
    pub xui_styles: Vec<XuiLayoutScript>,
    /// zombie.scripting.ScriptManager.getAllXuiDefaultStyles
    pub xui_default_styles: Vec<XuiLayoutScript>,
    /// zombie.scripting.ScriptManager.getAllXuiColors
    pub xui_colors: Vec<XuiColorsScript>,
    /// zombie.scripting.ScriptManager.getAllXuiSkinScripts
    pub xui_skin_scripts: Vec<XuiSkinScript>,
    /// XUI scripts the original drops because a body failed to load (`RemoveLoadError`): (full type, error).
    pub xui_errors: Vec<(String, String)>,
    /// zombie.scripting.ui.XuiManager's static state (with XuiLuaStyle's var registry) after ParseScripts.
    pub xui_manager: XuiManager,
    /// zombie.core.Colors' static tables (ParseScripts adds the xuiGlobalColors).
    pub colors: Colors,
    /// zombie.scripting.ScriptManager.getAllGameSounds: the `sound` scripts that loaded, per module (in module
    /// order) the HashMap order of the names kept.
    pub sounds: Vec<GameSoundScript>,
    /// `sound` scripts the original drops because a body threw (`RemoveLoadError`): (full type, error).
    pub sound_errors: Vec<(String, String)>,
    /// zombie.scripting.ScriptManager.getAllSoundTimelines: per module, the HashMap order of the names kept.
    pub sound_timelines: Vec<SoundTimelineScript>,
    /// zombie.scripting.ScriptManager.getAllVehicleEngineRPMs: per module, the HashMap order of the names kept.
    pub vehicle_engine_rpms: Vec<VehicleEngineRPM>,
    /// `soundTimeline` and `vehicleEngineRPM` scripts the original drops because a body threw: (full type, error).
    pub audio_script_errors: Vec<(String, String)>,
    /// zombie.GameSounds' static state after `GameSounds.ScriptsLoaded()`, without `loadINI` (ScriptManager.Load
    /// does not know the cache directory: the caller that has one runs `game_sounds.load_ini(cache_dir)`).
    pub game_sounds: GameSounds,
    /// The exception that ended `GameSounds.ScriptsLoaded` (the later sounds are not added).
    pub game_sounds_error: Option<String>,
    /// The exception that left XuiManager.ParseScripts. The original's ScriptManager.loadScripts catches it and then
    /// skips the remaining PostLoadScripts and every OnScriptsLoaded; that is not modelled (none in vanilla).
    pub xui_parse_error: Option<String>,
}

/// What ScriptBucketCollection.getScript reads for one script type: `fullTypeToScriptMap` and, per module (same order
/// as `module_list`), the names in its bucket's `scriptMap`.
pub struct BucketIndex {
    full_types: HashSet<String>,
    maps: Vec<HashSet<String>>,
}

impl BucketIndex {
    /// The index of `scripts`: (module index, script name) of every script that loaded.
    pub fn new<'a>(
        module_list: &[ScriptModule],
        scripts: impl Iterator<Item = (usize, &'a str)>,
    ) -> Self {
        let mut index = BucketIndex {
            full_types: HashSet::new(),
            maps: vec![HashSet::new(); module_list.len()],
        };
        for (m, name) in scripts {
            index
                .full_types
                .insert(format!("{}.{name}", module_list[m].name));
            index.maps[m].insert(name.to_string());
        }
        index
    }
}

/// zombie.scripting.ScriptManager.getItemName: the part after the first '.'.
pub fn get_item_name(name: &str) -> &str {
    match name.find('.') {
        Some(p) => &name[p + 1..],
        None => name,
    }
}

/// The depth at which a lookup stops: the original recurses through module imports without a guard and would end in
/// a StackOverflowError on an import cycle.
const MAX_LOOKUP_DEPTH: u32 = 64;

/// zombie.scripting.ScriptManager.searchFolders: every `.txt` below `fo`, as lower-case paths relative to the game
/// root, skipping folders whose path contains "tempNotWorking".
fn search_folders(root: &Path, fo: &Path, load_list: &mut Vec<(String, std::path::PathBuf)>) {
    if crate::vfs::is_dir(fo) {
        if fo.to_string_lossy().contains("tempNotWorking") {
            return;
        }
        let Ok(names) = crate::vfs::read_dir(fo) else {
            return;
        };
        for name in names {
            search_folders(root, &fo.join(name), load_list);
        }
    } else if fo.to_string_lossy().to_lowercase().ends_with(".txt") {
        if let Ok(rel) = fo.strip_prefix(root) {
            let rel = rel.to_string_lossy().replace('\\', "/").to_lowercase();
            load_list.push((rel, fo.to_path_buf()));
        }
    }
}

/// zombie.scripting.objects.ScriptModule.GetTokenType: the first word of a token's header.
fn get_token_type(token: &str) -> Option<&str> {
    let p = token.find('{')?;
    let header = java::trim(&token[..p]);
    let end = header.find([' ', '\t']).unwrap_or(header.len());
    Some(&header[..end])
}

/// The script tags `ScriptModule.CreateFromTokenPP` hands to a bucket; anything else is "unknown script object".
/// `template` tokens go to the template buckets and are kept under the tag "template".
fn token_name(tag: &str, token: &str) -> String {
    // token.split("[{}]")[0], minus the first occurrence of the tag text anywhere in it (String.replace replaces all).
    let head = token.split(['{', '}']).next().unwrap_or("");
    java::trim(&head.replace(tag, "")).to_string()
}

impl ScriptManager {
    /// zombie.scripting.ScriptManager.Load, the ported part: the vanilla script files in the original's order
    /// (`template_*` files first, then by path), parsed into modules, then entity scripts loaded.
    pub fn load(root: &Path) -> Result<Self, String> {
        let mut game_files = Vec::new();
        search_folders(root, &root.join("media").join("scripts"), &mut game_files);
        let file_name = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
        game_files.sort_by(|a, b| {
            let (t1, t2) = (
                file_name(&a.0).starts_with("template_"),
                file_name(&b.0).starts_with("template_"),
            );
            // java.lang.String.compareTo: UTF-16 code unit order.
            t2.cmp(&t1)
                .then_with(|| a.0.encode_utf16().cmp(b.0.encode_utf16()))
        });
        game_files.dedup_by(|a, b| a.0 == b.0);
        let mut m = ScriptManager {
            translator: Translator::start(root, "EN").map_err(|e| e.to_string())?,
            ..ScriptManager::default()
        };
        for (rel, path) in game_files {
            // LoadFile: lines joined with '\n' (BufferedReader.readLine drops line terminators).
            let Ok(bytes) = crate::vfs::read(&path) else {
                continue;
            };
            let text = String::from_utf8_lossy(&bytes);
            let mut buf = String::with_capacity(text.len() + 1);
            for line in text.split('\n') {
                buf.push_str(line.strip_suffix('\r').unwrap_or(line));
                buf.push('\n');
            }
            if text.ends_with('\n') {
                buf.pop();
            }
            m.load_file_names.push(rel);
            m.parse_script(&script_parser::strip_comments(&buf));
        }
        m.load_entities();
        m.load_models();
        m.load_items();
        // ScriptManager.bucketCollectionList order: VehicleTemplate (filled while parsing) first, Item before Vehicle.
        m.load_vehicles();
        // ScriptManager.bucketCollectionList order: Item before CraftRecipe before TimedAction. CraftRecipe.Load only
        // looks items up, and the timed actions are looked up after every collection loaded.
        m.load_craft_recipes();
        m.load_timed_actions();
        // ScriptManager.loadScripts: XuiManager.setParseOnce(true), then the six XUI collections (after TimedAction)
        // load; the first one's PostLoadScripts runs XuiManager.ParseScripts, the other five find it parsed.
        m.xui_manager.set_parse_once(true);
        m.load_xui();
        m.xui_post_load_scripts();
        m.load_sounds();
        m.on_items_loaded()?;
        m.craft_recipes_on_scripts_loaded();
        // ScriptManager.Load after loadScripts: ... RecipeManager.ScriptsLoaded, then GameSounds.ScriptsLoaded.
        m.game_sounds_error = m.game_sounds.scripts_loaded(&m.sounds, None).err();
        // The end of ScriptManager.loadScripts.
        m.xui_manager.set_parse_once(false);
        // ScriptManager.Load after loadScripts: ... ModelScript.ScriptsLoaded -> Item.resolveModelScripts.
        for i in 0..m.items.len() {
            m.resolve_model_scripts(i);
        }
        // ScriptManager.PostWorldDictionaryInit, which the server runs once the world dictionary is set up.
        m.craft_recipes_on_post_world_dictionary_init();
        Ok(m)
    }

    /// zombie.scripting.ScriptManager.ParseScript / CreateFromToken: top-level `module Name { ... }` tokens.
    fn parse_script(&mut self, total_file: &str) {
        for token in script_parser::parse_tokens(total_file) {
            let token = java::trim(&token);
            if !token.starts_with("module") {
                continue;
            }
            let (Some(first_open), Some(last_close)) = (token.find('{'), token.rfind('}')) else {
                continue;
            };
            let name = java::trim(
                &token
                    .split(['{', '}'])
                    .next()
                    .unwrap_or("")
                    .replace("module", ""),
            )
            .to_string();
            if first_open + 1 > last_close {
                continue;
            }
            let actual = &token[first_open + 1..last_close];
            let idx = match self.module_list.iter().position(|w| w.name == name) {
                Some(i) => i,
                None => {
                    self.module_list.push(ScriptModule {
                        name: name.clone(),
                        ..ScriptModule::default()
                    });
                    self.module_list.len() - 1
                }
            };
            // ScriptModule.Load -> ParseScriptPP -> CreateFromTokenPP
            for t in script_parser::parse_tokens(java::trim(actual)) {
                let t = java::trim(&t).to_string();
                let Some(tag) = get_token_type(&t).map(str::to_string) else {
                    continue;
                };
                let module = &mut self.module_list[idx];
                if tag == "imports" {
                    if let Some(list) = t.split(['{', '}']).nth(1) {
                        for c in list.split(',') {
                            let c = java::trim(c);
                            if !c.is_empty() && c != module.name {
                                module.imports.push(c.to_string());
                            }
                        }
                    }
                    continue;
                }
                if tag == "template" {
                    // ScriptBucket.Template.CreateFromTokenPP of the VehicleTemplate bucket (the first bucket):
                    // `token.split("[{}]")[0]` without "template", trimmed and split on whitespace; "vehicle Name"
                    // makes a template, a later one of the same name replacing it (`scriptMap.put`).
                    let head = t
                        .split(['{', '}'])
                        .next()
                        .unwrap_or("")
                        .replace("template", "");
                    let split = java::split_whitespace_regex(java::trim(&head));
                    if split.len() == 2 && java::trim(split[0]) == "vehicle" {
                        let template = VehicleTemplate::new(&module.name, java::trim(split[1]), &t);
                        match self
                            .vehicle_templates
                            .iter_mut()
                            .find(|x| x.module == template.module && x.name == template.name)
                        {
                            Some(x) => *x = template,
                            None => self.vehicle_templates.push(template),
                        }
                    }
                }
                let name = token_name(&tag, &t);
                let names = module.names.entry(tag.clone()).or_default();
                if !names.contains(&name) {
                    names.push(name.clone());
                }
                module.last_file.insert(
                    (tag.clone(), name.clone()),
                    self.load_file_names.len().saturating_sub(1),
                );
                module.bodies.entry((tag, name)).or_default().push(t);
            }
        }
    }

    /// ScriptBucket.LoadScripts for the entity buckets, then ScriptBucketCollection.getAllScripts: per module, entities
    /// in the iteration order of the original's `HashMap` of names.
    fn load_entities(&mut self) {
        let mut all = Vec::new();
        for module in &self.module_list {
            let Some(names) = module.names.get("entity") else {
                continue;
            };
            let refs: Vec<&str> = names.iter().map(String::as_str).collect();
            let mut loaded: Vec<GameEntityScript> = Vec::new();
            for k in java::hash_map_order(&refs) {
                let name = &names[k];
                let mut script = GameEntityScript {
                    name: name.clone(),
                    enabled: true,
                    debug_only: false,
                    sprite_config: None,
                };
                let mut ok = true;
                // ScriptType.Flags.ResetExisting calls reset() before every later body, but GameEntityScript keeps
                // BaseScriptObject's empty reset: a later body (an override) amends the script.
                for body in &module.bodies[&("entity".to_string(), name.clone())] {
                    if !script.load(body) {
                        ok = false;
                        break;
                    }
                }
                // A script that failed to load is still registered by the original (no RemoveLoadError flag here).
                let _ = ok;
                if script.enabled && !script.debug_only {
                    loaded.push(script);
                }
            }
            // scriptMap.values(): the HashMap order of the names that were kept.
            let kept: Vec<&str> = loaded.iter().map(|s| s.name.as_str()).collect();
            let order = java::hash_map_order(&kept);
            all.extend(order.into_iter().map(|i| loaded[i].clone()));
        }
        self.entities = all;
    }
}

impl ScriptManager {
    /// ScriptBucket.LoadScripts for the item buckets: every body of an item in file order, the script reset before
    /// each later body (`ScriptType.Flags.ResetExisting`: a later file replaces the item rather than amending it).
    /// A body that fails removes the item (`RemoveLoadError`); obsolete, disabled and debug-only items are left out.
    fn load_items(&mut self) {
        for module in &self.module_list {
            let Some(names) = module.names.get("item") else {
                continue;
            };
            let mut sorted: Vec<&String> = names.iter().collect();
            sorted.sort();
            for name in sorted {
                let mut item = Item::new(&module.name, name);
                let mut error = None;
                // ResetExisting calls Item.reset(), which is BaseScriptObject's empty one: later bodies amend.
                for body in &module.bodies[&("item".to_string(), name.clone())] {
                    if let Err(e) = item.load(body) {
                        error = Some(e);
                        break;
                    }
                }
                if let Some(e) = error {
                    self.item_errors.push((item.full_name(), e));
                    continue;
                }
                let obsolete = matches!(item.get("obsolete"), Some(Value::Bool(true)));
                if !obsolete && item.enabled && !item.debug_only {
                    self.items.push(item);
                }
            }
        }
    }
}

impl ScriptManager {
    /// ScriptBucket.LoadScripts for the model buckets: every body in file order, `ModelScript.reset` before each later
    /// one; a body that throws removes the script; disabled and debug-only models are left out.
    fn load_models(&mut self) {
        let mut all = Vec::with_capacity(self.module_list.len());
        for module in &self.module_list {
            let mut loaded = Vec::new();
            for name in module.names.get("model").into_iter().flatten() {
                let mut script = ModelScript::new(&module.name, name);
                let mut ok = true;
                for (index, body) in module.bodies[&("model".to_string(), name.clone())]
                    .iter()
                    .enumerate()
                {
                    if index >= 1 {
                        script.reset();
                    }
                    if script.load(body).is_err() {
                        ok = false;
                        break;
                    }
                }
                if ok && script.enabled && !script.debug_only {
                    loaded.push(script);
                }
            }
            all.push(loaded);
        }
        self.models = all;
    }

    /// zombie.scripting.objects.ScriptModule.getModelScript -> ScriptBucket.get: the module's own model, else one
    /// from its imports (each import searched the same way; `seen` stops an import cycle, where the original would
    /// overflow its stack).
    fn module_model_script(
        &self,
        module: usize,
        name: &str,
        seen: &mut Vec<usize>,
    ) -> Option<&ModelScript> {
        if seen.contains(&module) {
            return None;
        }
        seen.push(module);
        if let Some(s) = self.models[module]
            .iter()
            .find(|s| s.script_object_name == name)
        {
            return Some(s);
        }
        for import in &self.module_list[module].imports {
            // ScriptManager.getModule(name): the first module with that name.
            let Some(m) = self.module_list.iter().position(|x| &x.name == import) else {
                continue;
            };
            if let Some(s) = self.module_model_script(m, name, seen) {
                return Some(s);
            }
        }
        None
    }

    /// zombie.scripting.ScriptManager.resolveModelScript: `None` for a blank name, a dotted name unchanged, else the
    /// full type of the model found in the item's module, then in any other module, else `???.name`.
    pub fn resolve_model_script(
        &self,
        module: usize,
        model_script_name: Option<&str>,
    ) -> Option<String> {
        let name = model_script_name?;
        if string_utils::is_null_or_whitespace(Some(name)) {
            return None;
        }
        if name.contains('.') {
            return Some(name.to_string());
        }
        if let Some(s) = self.module_model_script(module, name, &mut Vec::new()) {
            return Some(s.get_full_type());
        }
        for other in 0..self.module_list.len() {
            if other != module {
                if let Some(s) = self.module_model_script(other, name, &mut Vec::new()) {
                    return Some(s.get_full_type());
                }
            }
        }
        Some(format!("???.{name}"))
    }

    /// zombie.scripting.objects.Item.resolveModelScripts for `items[index]`.
    fn resolve_model_scripts(&mut self, index: usize) {
        let item = &self.items[index];
        let Some(module) = self.module_list.iter().position(|m| m.name == item.module) else {
            return;
        };
        let text = |f: &str| match item.get(f) {
            Some(Value::Str(s)) => s.clone(),
            _ => None,
        };
        let (s, w) = (text("staticModel"), text("worldStaticModel"));
        let s = self.resolve_model_script(module, s.as_deref());
        let w = self.resolve_model_script(module, w.as_deref());
        let item = &mut self.items[index];
        item.set("staticModel", Value::Str(s));
        item.set("worldStaticModel", Value::Str(w));
    }

    /// zombie.scripting.objects.Item.OnScriptsLoaded, the ported part: the display name becomes
    /// `Translator.getItemNameFromFullType(fullName)`.
    fn on_items_loaded(&mut self) -> Result<(), String> {
        for item in &mut self.items {
            let full = item.full_name();
            let own = match item.get("displayName") {
                Some(Value::Str(Some(s))) if !s.is_empty() => s.clone(),
                _ => full.clone(),
            };
            // getItem(fullType) finds this item: its getDisplayName() (the full name when it has none).
            let name = self
                .translator
                .get_item_name_from_full_type(&full, Some(own))?;
            item.set("displayName", Value::Str(Some(name)));
        }
        Ok(())
    }
}

impl ScriptManager {
    /// zombie.scripting.ScriptManager.getModule(name, true): "Base" (and any `Base.` name) is the Base module, a module
    /// name is that module (modules are never disabled and have no aliases here), a dotted name the module of its
    /// prefix; anything else falls back to Base.
    pub fn get_module(&self, name: &str) -> Option<usize> {
        let base = || self.module_list.iter().position(|m| m.name == "Base");
        if java::trim(name) == "Base" || name.starts_with("Base.") {
            return base();
        }
        if let Some(i) = self.module_list.iter().position(|m| m.name == name) {
            return Some(i);
        }
        if let Some(idx) = name.find('.') {
            if let Some(m) = self.get_module(&name[..idx]) {
                return Some(m);
            }
        }
        base()
    }

    /// zombie.scripting.ScriptBucketCollection.getScript for a type with the CacheFullType and SeekImports flags (item,
    /// craftRecipe, timedAction): the full type of the script found, `None` for the original's null.
    pub fn get_script(&self, index: &BucketIndex, name: &str) -> Option<String> {
        self.get_script_depth(index, name, 0)
    }

    fn get_script_depth(&self, index: &BucketIndex, name: &str, depth: u32) -> Option<String> {
        if depth > MAX_LOOKUP_DEPTH {
            return None;
        }
        if name.contains('.') && index.full_types.contains(name) {
            return Some(name.to_string());
        }
        let module = if !name.contains('.') {
            self.get_module("Base")
        } else {
            self.get_module(name)
        }?;
        self.bucket_get(index, module, get_item_name(name), depth + 1)
    }

    /// zombie.scripting.ScriptBucket.get: a dotted name that is not a script name of the module (`dotInName`) goes
    /// back to the manager (`getFromManager`); else the module's own script, else the first found in an import
    /// (`getFromModule`, which is the import's own `ScriptBucket.get`).
    fn bucket_get(
        &self,
        index: &BucketIndex,
        module: usize,
        name: &str,
        depth: u32,
    ) -> Option<String> {
        if depth > MAX_LOOKUP_DEPTH {
            return None;
        }
        let map = index.maps.get(module)?;
        if name.contains('.') && !map.contains(name) {
            return self.get_script_depth(index, name, depth + 1);
        }
        if map.contains(name) {
            return Some(format!("{}.{name}", self.module_list[module].name));
        }
        for import in &self.module_list[module].imports {
            let m = self.get_module(import)?;
            if let Some(s) = self.bucket_get(index, m, name, depth + 1) {
                return Some(s);
            }
        }
        None
    }

    /// zombie.scripting.ScriptManager.FindItem(name, true): a known full type, else the item of the module's bucket
    /// (`getModule(name)`, then `ScriptModule.getItem(getItemName(name))`; the original's retry over every module
    /// asks the same module again). The full type of the item found.
    pub fn find_item(&self, index: &BucketIndex, name: &str) -> Option<String> {
        if name.contains('.') && index.full_types.contains(name) {
            return Some(name.to_string());
        }
        let module = self.get_module(name)?;
        self.bucket_get(index, module, get_item_name(name), 1)
    }

    /// The scripts of one bucket type as ScriptBucket.LoadScripts visits them: modules in `moduleList` order, in each
    /// the names in the order of its `loadData` HashMap, each body of a name in file order. (name, body) pairs.
    pub fn bucket_scripts(&self, tag: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for module in &self.module_list {
            let Some(names) = module.names.get(tag) else {
                continue;
            };
            let keys: Vec<&str> = names.iter().map(String::as_str).collect();
            for i in java::hash_map_order(&keys) {
                let name = &names[i];
                for body in &module.bodies[&(tag.to_string(), name.clone())] {
                    out.push((name.clone(), body.clone()));
                }
            }
        }
        out
    }

    /// ScriptBucketCollection.getAllScripts of the items (ScriptManager.getAllItems): per module of `moduleList`, its
    /// bucket's `scriptMap.values()`, the HashMap the kept items were put into in `loadData` order. Indices into
    /// `items`.
    pub fn all_items(&self) -> Vec<usize> {
        let index: HashMap<String, usize> = (self.items.iter().enumerate())
            .map(|(i, item)| (item.full_name(), i))
            .collect();
        let mut out = Vec::with_capacity(self.items.len());
        for module in &self.module_list {
            let Some(names) = module.names.get("item") else {
                continue;
            };
            let all: Vec<&str> = names.iter().map(String::as_str).collect();
            let kept: Vec<(&str, usize)> = java::hash_map_order(&all)
                .into_iter()
                .filter_map(|i| Some((all[i], *index.get(&format!("{}.{}", module.name, all[i]))?)))
                .collect();
            let keys: Vec<&str> = kept.iter().map(|k| k.0).collect();
            out.extend(java::hash_map_order(&keys).into_iter().map(|i| kept[i].1));
        }
        out
    }

    /// Item.InitLoadPP's statics after ScriptManager.Load: `netIdToItem` gets an id per item token (each body of an
    /// item, in every module), `netItemToId` an entry per full type. (sizes)
    pub fn item_net_id_counts(&self) -> (usize, usize) {
        let mut ids = 0;
        let mut types = 0;
        for module in &self.module_list {
            for name in module.names.get("item").into_iter().flatten() {
                ids += module.bodies[&("item".to_string(), name.clone())].len();
                types += 1;
            }
        }
        (ids, types)
    }

    /// The lookup data of the item collection (ScriptManager.getItem).
    pub fn item_index(&self) -> BucketIndex {
        let module_of = |name: &str| self.module_list.iter().position(|m| m.name == name);
        BucketIndex::new(
            &self.module_list,
            self.items
                .iter()
                .filter_map(|i| Some((module_of(&i.module)?, i.name.as_str()))),
        )
    }

    /// The lookup data of the craftRecipe collection (ScriptManager.getCraftRecipe without its fallback to the
    /// CraftRecipe components of entities, which are not ported).
    pub fn craft_recipe_index(&self) -> BucketIndex {
        let module_of = |name: &str| self.module_list.iter().position(|m| m.name == name);
        BucketIndex::new(
            &self.module_list,
            self.craft_recipes
                .iter()
                .filter_map(|r| Some((module_of(&r.module)?, r.script_object_name.as_str()))),
        )
    }

    /// The lookup data of the timedAction collection (ScriptManager.getTimedActionScript).
    pub fn timed_action_index(&self) -> BucketIndex {
        BucketIndex::new(
            &self.module_list,
            self.timed_actions
                .iter()
                .enumerate()
                .flat_map(|(m, list)| list.iter().map(move |s| (m, s.name.as_str()))),
        )
    }

    /// ScriptBucket.LoadScripts for the timedAction buckets: every body in file order (TimedActionScript keeps
    /// BaseScriptObject's empty `reset`, so later bodies amend); a body that throws removes the script; disabled and
    /// debug-only scripts are left out.
    fn load_timed_actions(&mut self) {
        let tag = "timedAction".to_string();
        let mut all = Vec::with_capacity(self.module_list.len());
        for module in &self.module_list {
            let mut loaded = Vec::new();
            for name in module.names.get(&tag).into_iter().flatten() {
                let mut script = TimedActionScript::new(&module.name, name);
                let mut ok = true;
                for body in module
                    .bodies
                    .get(&(tag.clone(), name.clone()))
                    .into_iter()
                    .flatten()
                {
                    if script.load(body).is_err() {
                        ok = false;
                        break;
                    }
                }
                if ok && script.enabled && !script.debug_only {
                    loaded.push(script);
                }
            }
            all.push(loaded);
        }
        self.timed_actions = all;
    }

    /// ScriptBucket.CreateFromTokenPP and ScriptBucket.LoadScripts for the craftRecipe buckets, then
    /// ScriptBucketCollection.getAllScripts. Every body found gets `InitLoadPP` and `addLoadedScriptBody`; then, in
    /// the HashMap order of the names (`loadData.values()`), every body is loaded in file order (CraftRecipe keeps
    /// BaseScriptObject's empty `reset`, so later bodies amend). A body that throws removes the recipe
    /// (`RemoveLoadError`); disabled and debug-only recipes are left out (Core.debug is false); the others get their
    /// script version. The list is, per module, the HashMap order of the names kept (`scriptMap.values()`).
    fn load_craft_recipes(&mut self) {
        let tag = "craftRecipe".to_string();
        let items = self.item_index();
        let get_item = |name: &str| self.get_script(&items, name);
        let ctx = LoadContext {
            translator: &self.translator,
            get_item: &get_item,
        };
        let mut all = Vec::new();
        let mut errors = Vec::new();
        for module in &self.module_list {
            let Some(names) = module.names.get(&tag) else {
                continue;
            };
            let refs: Vec<&str> = names.iter().map(String::as_str).collect();
            let mut kept: Vec<CraftRecipe> = Vec::new();
            for k in java::hash_map_order(&refs) {
                let name = &names[k];
                let bodies = module
                    .bodies
                    .get(&(tag.clone(), name.clone()))
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                let mut recipe = CraftRecipe::new(&module.name);
                for body in bodies {
                    recipe.init_load_pp(name, body);
                }
                let mut error = None;
                for body in bodies {
                    if let Err(e) = recipe.load(name, body, &ctx) {
                        error = Some(e);
                        break;
                    }
                }
                if let Some(e) = error {
                    errors.push((recipe.get_script_object_full_type(), e));
                    continue;
                }
                if !recipe.enabled || recipe.debug_only {
                    continue;
                }
                recipe.calculate_script_version();
                kept.push(recipe);
            }
            let kept_names: Vec<&str> =
                kept.iter().map(|r| r.script_object_name.as_str()).collect();
            let order = java::hash_map_order(&kept_names);
            let mut slots: Vec<Option<CraftRecipe>> = kept.into_iter().map(Some).collect();
            all.extend(order.into_iter().filter_map(|i| slots[i].take()));
        }
        self.craft_recipes = all;
        self.craft_recipe_errors = errors;
    }

    /// ScriptBucketCollection.OnScriptsLoaded for the craft recipes: CraftRecipe.OnScriptsLoaded on every recipe in
    /// list order; the first exception ends the loop (ScriptManager.loadScripts catches it).
    fn craft_recipes_on_scripts_loaded(&mut self) {
        let timed = self.timed_action_index();
        let mut recipes = std::mem::take(&mut self.craft_recipes);
        let get_timed_action = |name: &str| self.get_script(&timed, name);
        let mut error = None;
        for r in &mut recipes {
            if let Err(e) = r.on_scripts_loaded(&get_timed_action) {
                error = Some(e);
                break;
            }
        }
        self.craft_recipe_scripts_loaded_error = error;
        self.craft_recipes = recipes;
    }

    /// ScriptBucketCollection.OnPostWorldDictionaryInit for the craft recipes: CraftRecipe.OnPostWorldDictionaryInit
    /// on every recipe in list order; the first exception ends the loop (ScriptManager.PostWorldDictionaryInit catches
    /// it per collection). Every recipe `OutputMapper.getItem` names (ScriptManager.getCraftRecipe) has
    /// `canBeResearched()` evaluated right away, as in the original.
    fn craft_recipes_on_post_world_dictionary_init(&mut self) {
        let items = self.item_index();
        let recipes_index = self.craft_recipe_index();
        let mut recipes = std::mem::take(&mut self.craft_recipes);
        let by_full: HashMap<String, usize> = recipes
            .iter()
            .enumerate()
            .map(|(i, r)| (r.get_script_object_full_type(), i))
            .collect();
        let get_item = |name: &str| self.get_script(&items, name);
        let find_recipe = |name: &str| self.get_script(&recipes_index, name);
        let has_craft_recipe = |name: &str| find_recipe(name).is_some();
        let lookups = Lookups {
            get_item: &get_item,
            has_craft_recipe: &has_craft_recipe,
        };
        let mut error = None;
        for i in 0..recipes.len() {
            let mut research = Vec::new();
            let result = recipes[i].on_post_world_dictionary_init(&lookups, &mut research);
            for name in research {
                if let Some(&j) = find_recipe(&name).and_then(|f| by_full.get(&f)) {
                    recipes[j].can_be_researched();
                }
            }
            if let Err(e) = result {
                error = Some(e);
                break;
            }
        }
        self.craft_recipe_post_init_error = error;
        self.craft_recipes = recipes;
    }
}

impl ScriptManager {
    /// zombie.scripting.ScriptManager.resolveItemType: a dotted type as it is, else the item of that name in
    /// `module` (with its imports), else in the first module that has it, else "???." + the type; null for a null or
    /// whitespace type.
    pub fn resolve_item_type(
        &self,
        items: &BucketIndex,
        module: usize,
        item_type: &str,
    ) -> Option<String> {
        if string_utils::is_null_or_whitespace(Some(item_type)) {
            return None;
        }
        if item_type.contains('.') {
            return Some(item_type.to_string());
        }
        if let Some(full) = self.bucket_get(items, module, item_type, 0) {
            return Some(full);
        }
        // Modules are never disabled here.
        for m in 0..self.module_list.len() {
            if let Some(full) = self.bucket_get(items, m, item_type, 0) {
                return Some(full);
            }
        }
        Some(format!("???.{item_type}"))
    }

    /// The lookup data of the vehicleTemplate collection (ScriptManager.getVehicleTemplate).
    pub fn vehicle_template_index(&self) -> BucketIndex {
        let module_of = |name: &str| self.module_list.iter().position(|m| m.name == name);
        BucketIndex::new(
            &self.module_list,
            self.vehicle_templates
                .iter()
                .filter_map(|t| Some((module_of(&t.module)?, t.name.as_str()))),
        )
    }

    /// ScriptBucket.LoadScripts for the vehicle buckets, then ScriptBucketCollection.getAllScripts. Every body found
    /// gets `InitLoadPP` and `addLoadedScriptBody`; then, in the HashMap order of the names, every body is loaded in
    /// file order into the same script (the Vehicle type has no ResetExisting flag). A body that throws removes the
    /// vehicle (`RemoveLoadError`); disabled and debug-only vehicles are left out (Core.debug is false); the others
    /// get their script version, then `onScriptLoad` runs `VehicleScript.Loaded`, and one that throws there is not
    /// put in the map either. Templates (`template = ...`, `template! = ...`) are looked up with
    /// ScriptManager.getVehicleTemplate and built on first use.
    fn load_vehicles(&mut self) {
        let tag = "vehicle".to_string();
        let items = self.item_index();
        let template_index = self.vehicle_template_index();
        let mut templates = std::mem::take(&mut self.vehicle_templates);
        let mut all = Vec::new();
        let mut errors = Vec::new();
        let mut log = Vec::new();
        {
            let full_types: Vec<String> = templates
                .iter()
                .map(VehicleTemplate::get_script_object_full_type)
                .collect();
            let get_vehicle_template = |name: &str| {
                let full = self.get_script(&template_index, name)?;
                full_types.iter().position(|f| *f == full)
            };
            let mut ctx = TemplateContext {
                get_vehicle_template: &get_vehicle_template,
                templates: &mut templates,
                building: Vec::new(),
                log: Vec::new(),
            };
            for (mi, module) in self.module_list.iter().enumerate() {
                let Some(names) = module.names.get(&tag) else {
                    continue;
                };
                let refs: Vec<&str> = names.iter().map(String::as_str).collect();
                let mut kept: Vec<VehicleScript> = Vec::new();
                for k in java::hash_map_order(&refs) {
                    let name = &names[k];
                    let key = (tag.clone(), name.clone());
                    let bodies = module
                        .bodies
                        .get(&key)
                        .map(Vec::as_slice)
                        .unwrap_or_default();
                    let file = module
                        .last_file
                        .get(&key)
                        .and_then(|i| self.load_file_names.get(*i))
                        .map_or("", String::as_str);
                    let mut script = VehicleScript::new(&module.name);
                    for body in bodies {
                        script.init_load_pp(name, file, body);
                    }
                    let mut error = None;
                    for body in bodies {
                        if let Err(e) = script.load(name, body, &mut ctx, None, 0) {
                            error = Some(e);
                            break;
                        }
                    }
                    if let Some(e) = error {
                        errors.push((script.get_script_object_full_type(), e));
                        continue;
                    }
                    if !script.enabled || script.debug_only {
                        continue;
                    }
                    script.calculate_script_version();
                    let resolve = |t: &str| self.resolve_item_type(&items, mi, t);
                    if let Err(e) = script.loaded(&resolve) {
                        errors.push((script.get_script_object_full_type(), e));
                        continue;
                    }
                    kept.push(script);
                }
                let kept_names: Vec<String> = kept
                    .iter()
                    .map(|v| v.script_object_name.clone().unwrap_or_default())
                    .collect();
                let refs: Vec<&str> = kept_names.iter().map(String::as_str).collect();
                let order = java::hash_map_order(&refs);
                let mut slots: Vec<Option<VehicleScript>> = kept.into_iter().map(Some).collect();
                all.extend(order.into_iter().filter_map(|i| slots[i].take()));
            }
            log.append(&mut ctx.log);
        }
        self.vehicle_templates = templates;
        self.vehicles = all;
        self.vehicle_errors = errors;
        self.vehicle_log = log;
    }
}

impl GameEntityScript {
    /// zombie.scripting.entity.GameEntityScript.Load (String, String): `false` where the original throws.
    /// Entity templates (`entitytemplate = ...`) are not applied; they cannot carry a SpriteConfig in the original.
    fn load(&mut self, body: &str) -> bool {
        let parsed = script_parser::parse(body);
        let Some(block) = parsed.children.first() else {
            return false;
        };
        // BaseScriptObject.LoadCommonBlock
        for value in &block.values {
            let (key, val) = (java::trim(value.get_key()), java::trim(value.get_value()));
            if !key.is_empty() && !val.is_empty() {
                if java::equals_ignore_case(key, "enabled") {
                    self.enabled = java::equals_ignore_case(val, "true");
                } else if java::equals_ignore_case(key, "debugOnly") {
                    self.debug_only = java::equals_ignore_case(val, "true");
                }
            }
        }
        for child in &block.children {
            self.load_component_block(child);
        }
        true
    }

    /// zombie.scripting.entity.GameEntityScript.LoadComponentBlock, for the one component type ported so far.
    fn load_component_block(&mut self, block: &Block) {
        if java::equals_ignore_case(&block.r#type, "component")
            && block.id.as_deref() == Some("SpriteConfig")
        {
            self.sprite_config
                .get_or_insert_with(SpriteConfigScript::default)
                .load(block);
        }
    }
}

impl ScriptManager {
    /// ScriptBucket.CreateFromTokenPP and ScriptBucket.LoadScripts for the buckets of one script tag, then
    /// ScriptBucketCollection.getAllScripts. Every body found gets `InitLoadPP` and `addLoadedScriptBody`; then, in the
    /// HashMap order of the names, every body is loaded in file order (`reset()` before each later body with the
    /// ResetExisting flag). A body that throws removes the script (`RemoveLoadError`); disabled and debug-only scripts
    /// are left out (Core.debug is false); the others get their script version. The list is, per module, the HashMap
    /// order of the names kept (`scriptMap.values()`).
    fn load_bucket<S: ScriptObject>(
        &self,
        tag: &str,
        reset_existing: bool,
        new: impl Fn(&str) -> S,
        errors: &mut Vec<(String, String)>,
    ) -> Vec<S> {
        let mut all = Vec::new();
        for module in &self.module_list {
            let Some(names) = module.names.get(tag) else {
                continue;
            };
            let refs: Vec<&str> = names.iter().map(String::as_str).collect();
            let mut kept: Vec<S> = Vec::new();
            for k in java::hash_map_order(&refs) {
                let name = &names[k];
                let bodies = module
                    .bodies
                    .get(&(tag.to_string(), name.clone()))
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                let mut script = new(&module.name);
                for body in bodies {
                    script.base_mut().init_load_pp(name, body);
                }
                let mut error = None;
                for (index, body) in bodies.iter().enumerate() {
                    if reset_existing && index >= 1 {
                        script.reset();
                    }
                    if let Err(e) = script.load(name, body, &self.translator) {
                        error = Some(e);
                        break;
                    }
                }
                if let Some(e) = error {
                    errors.push((script.base().get_script_object_full_type(), e));
                    continue;
                }
                if !script.base().enabled || script.base().debug_only {
                    continue;
                }
                script.base_mut().calculate_script_version();
                kept.push(script);
            }
            let kept_names: Vec<String> = kept
                .iter()
                .map(|s| s.base().script_object_name.clone().unwrap_or_default())
                .collect();
            let refs: Vec<&str> = kept_names.iter().map(String::as_str).collect();
            let order = java::hash_map_order(&refs);
            let mut slots: Vec<Option<S>> = kept.into_iter().map(Some).collect();
            all.extend(order.into_iter().filter_map(|i| slots[i].take()));
        }
        all
    }

    /// ScriptBucketCollection.LoadScripts of the XUI collections, in bucketCollectionList order: xuiConfig,
    /// xuiLayout, xuiStyle, xuiDefaultStyle, xuiGlobalColors (the default flags, with ResetExisting) and xuiSkin
    /// (no ResetExisting: every body amends the skin).
    fn load_xui(&mut self) {
        let mut errors = Vec::new();
        let layout = |t: XuiScriptType| move |m: &str| XuiLayoutScript::new(Some(m), t);
        self.xui_configs = self.load_bucket(
            "xuiConfig",
            true,
            |m| XuiConfigScript::new(Some(m)),
            &mut errors,
        );
        self.xui_layouts = self.load_bucket(
            "xuiLayout",
            true,
            layout(XuiScriptType::Layout),
            &mut errors,
        );
        self.xui_styles =
            self.load_bucket("xuiStyle", true, layout(XuiScriptType::Style), &mut errors);
        self.xui_default_styles = self.load_bucket(
            "xuiDefaultStyle",
            true,
            layout(XuiScriptType::DefaultStyle),
            &mut errors,
        );
        self.xui_colors = self.load_bucket(
            "xuiGlobalColors",
            true,
            |m| XuiColorsScript::new(Some(m)),
            &mut errors,
        );
        self.xui_skin_scripts = self.load_bucket(
            "xuiSkin",
            false,
            |m| XuiSkinScript::new(Some(m)),
            &mut errors,
        );
        self.xui_errors = errors;
    }

    /// ScriptBucketCollection.LoadScripts of the `sound`, `soundTimeline` and `vehicleEngineRPM` collections (the default
    /// flags: ResetExisting, RemoveLoadError): Load of every body, the script reset before each later one.
    fn load_sounds(&mut self) {
        let mut errors = Vec::new();
        self.sounds = self.load_bucket(
            "sound",
            true,
            |m| GameSoundScript::new(Some(m)),
            &mut errors,
        );
        self.sound_errors = errors;
        let mut errors = Vec::new();
        self.sound_timelines = self.load_bucket(
            "soundTimeline",
            true,
            |m| SoundTimelineScript::new(Some(m)),
            &mut errors,
        );
        self.vehicle_engine_rpms = self.load_bucket(
            "vehicleEngineRPM",
            true,
            |m| VehicleEngineRPM::new(Some(m)),
            &mut errors,
        );
        self.audio_script_errors = errors;
    }

    /// The lookup data of the xuiSkin collection (ScriptManager.getXuiSkinScript).
    pub fn xui_skin_index(&self) -> BucketIndex {
        let module_of = |name: &str| self.module_list.iter().position(|m| m.name == name);
        BucketIndex::new(
            &self.module_list,
            self.xui_skin_scripts.iter().filter_map(|s| {
                Some((
                    module_of(s.base.module.as_deref()?)?,
                    s.base.script_object_name.as_deref()?,
                ))
            }),
        )
    }

    /// The PostLoadScripts of the XUI collections: XuiManager.ParseScripts (once while `parseOnce` is set).
    fn xui_post_load_scripts(&mut self) {
        let index = self.xui_skin_index();
        let full_types: Vec<String> = self
            .xui_skin_scripts
            .iter()
            .map(|s| s.base.get_script_object_full_type())
            .collect();
        let mut manager = std::mem::take(&mut self.xui_manager);
        let mut colors = std::mem::take(&mut self.colors);
        let mut styles = std::mem::take(&mut self.xui_styles);
        let mut default_styles = std::mem::take(&mut self.xui_default_styles);
        let mut layouts = std::mem::take(&mut self.xui_layouts);
        let result = {
            let get_skin_script = |name: &str| {
                let full = self.get_script(&index, name)?;
                full_types.iter().position(|f| *f == full)
            };
            manager.parse_scripts(
                XuiScripts {
                    configs: &self.xui_configs,
                    colors: &self.xui_colors,
                    styles: &mut styles,
                    default_styles: &mut default_styles,
                    layouts: &mut layouts,
                    skins: &self.xui_skin_scripts,
                    get_skin_script: &get_skin_script,
                },
                &mut colors,
            )
        };
        self.xui_parse_error = result.err();
        self.xui_manager = manager;
        self.colors = colors;
        self.xui_styles = styles;
        self.xui_default_styles = default_styles;
        self.xui_layouts = layouts;
    }
}
