//! zombie.scripting.entity.components.crafting.CraftRecipe: a `craftRecipe` script (B42 crafting) as `Load`,
//! `InitLoadPP`, `OnScriptsLoaded` and the ported part of `OnPostWorldDictionaryInit` leave it. Items are named by
//! their full names; io scripts (inputs, outputs and their `+`/`-` lines) live in `io_lines` and refer to each other
//! by index there.
use super::input_script::InputScript;
use super::output_script::{MapperRef, OutputScript};
use crate::java;
use crate::zombie::characters::iso_game_character::PERK_IDS;
use crate::zombie::core::math::pz_math;
use crate::zombie::core::translator::Translator;
use crate::zombie::entity::components::crafting::input_flag::InputFlag;
use crate::zombie::entity::components::crafting::item_apply_mode::ItemApplyMode;
use crate::zombie::entity::components::crafting::recipe::output_mapper::{Lookups, OutputMapper};
use crate::zombie::entity::components::crafting::recipe::overlay_mapper::OverlayMapper;
use crate::zombie::entity::components::resources::resource_type::ResourceType;
use crate::zombie::scripting::objects::{craft_recipe_group, craft_recipe_tag};
use crate::zombie::scripting::script_parser::{self, Block};
use crate::zombie::util::hash::pz_hash;
use crate::zombie::util::string_utils;

/// zombie.scripting.ScriptManager.VanillaID: the mod id of the game's own script files.
pub const VANILLA_ID: &str = "pz-vanilla";

/// zombie.scripting.entity.components.crafting.CraftRecipe.LuaCall, in ordinal order.
pub const LUA_CALLS: [&str; 5] = ["OnTest", "OnStart", "OnUpdate", "OnCreate", "OnFailed"];

/// zombie.scripting.entity.components.crafting.CraftRecipe.RequiredSkill (perk by id).
#[derive(Clone, Debug, PartialEq)]
pub struct RequiredSkill {
    pub perk: &'static str,
    pub level: i32,
}

/// zombie.scripting.entity.components.crafting.CraftRecipe.XpAward (perk by id).
#[derive(Clone, Debug, PartialEq)]
pub struct XpAward {
    pub perk: &'static str,
    pub amount: i32,
}

/// One element of `ioLines` (CraftRecipe.IOScript).
#[derive(Clone, Debug, PartialEq)]
pub enum IoScript {
    Input(InputScript),
    Output(OutputScript),
}

/// zombie.scripting.entity.components.crafting.CraftRecipe, with the BaseScriptObject fields it uses.
#[derive(Clone, Debug, PartialEq)]
pub struct CraftRecipe {
    /// BaseScriptObject.module (its name)
    pub module: String,
    /// BaseScriptObject.scriptObjectName
    pub script_object_name: String,
    pub enabled: bool,
    pub debug_only: bool,
    /// BaseScriptObject.loadedScriptBodies: (mod id, body) of every body, in load order.
    pub loaded_script_bodies: Vec<(String, String)>,
    pub script_version: i64,
    pub name: Option<String>,
    pub translation_name: Option<String>,
    pub icon_name: Option<String>,
    pub recipe_group: Option<&'static str>,
    pub need_to_be_learn: bool,
    pub skill_required: Option<Vec<RequiredSkill>>,
    pub auto_learn_any: Option<Vec<RequiredSkill>>,
    pub auto_learn_all: Option<Vec<RequiredSkill>>,
    pub xp_award: Option<Vec<XpAward>>,
    pub meta_recipe: Option<String>,
    pub has_on_tick_inputs: bool,
    pub has_on_tick_outputs: bool,
    pub time: i32,
    pub loaded_timed_action_script: Option<String>,
    /// The full type of the TimedActionScript `OnScriptsLoaded` found.
    pub timed_action_script: Option<String>,
    /// `luaCalls` by LuaCall ordinal.
    pub lua_calls: [Option<String>; 5],
    pub inputs: Vec<usize>,
    pub outputs: Vec<usize>,
    pub io_lines: Vec<IoScript>,
    pub category: Option<String>,
    pub category_tags: Vec<String>,
    pub prop1: Option<usize>,
    pub prop2: Option<usize>,
    pub animation: Option<String>,
    pub tool_left: Option<usize>,
    pub tool_right: Option<usize>,
    pub exists_as_vanilla: bool,
    pub mod_id: Option<String>,
    /// `outputMappers` (HashMap<String, OutputMapper>), in creation order.
    pub output_mappers: Vec<(String, OutputMapper)>,
    pub overlay_mapper: OverlayMapper,
    pub uses_tools: bool,
    pub tooltip: Option<String>,
    pub allow_batch_craft: bool,
    pub can_walk: bool,
    pub on_add_to_menu: Option<String>,
    pub research_skill_level: i32,
    pub research_all: Vec<&'static str>,
    pub research_any: Vec<&'static str>,
}

/// What `CraftRecipe.Load` asks of the rest of the game.
pub struct LoadContext<'a> {
    /// Translator.getRecipeName
    pub translator: &'a Translator,
    /// ScriptManager.getItem: the item's full name.
    pub get_item: &'a dyn Fn(&str) -> Option<String>,
}

fn nfe(s: &str) -> String {
    format!(
        "java.lang.NumberFormatException: {}",
        java::number_format_message(s)
    )
}

/// PerkFactory.Perks.FromString: the perk with that id, `None` for MAX (an unknown id).
fn perk_from_string(id: &str) -> Option<&'static str> {
    PERK_IDS.iter().copied().find(|p| *p == id)
}

/// The `Perk:level;Perk:level` lists of SkillRequired, AutoLearnAny, AutoLearnAll and xpAward: unknown perks are
/// skipped (warned about); a known perk without `:` is the ArrayIndexOutOfBoundsException; a bad level is 1.
fn parse_skills(val: &str) -> Result<Vec<(&'static str, i32)>, String> {
    let mut out = Vec::new();
    for entry in java::split_char(val, ';') {
        let split3 = java::split_char(entry, ':');
        let Some(perk) = perk_from_string(split3.first().copied().unwrap_or("")) else {
            continue;
        };
        let level = split3.get(1).ok_or_else(|| {
            format!(
                "java.lang.ArrayIndexOutOfBoundsException: Index 1 out of bounds for length {}",
                split3.len()
            )
        })?;
        out.push((perk, pz_math::try_parse_int(Some(level), 1)));
    }
    Ok(out)
}

impl CraftRecipe {
    /// zombie.scripting.entity.components.crafting.CraftRecipe(): the field initialisers.
    pub fn new(module: &str) -> Self {
        CraftRecipe {
            module: module.to_string(),
            script_object_name: String::new(),
            enabled: true,
            debug_only: false,
            loaded_script_bodies: Vec::new(),
            script_version: -1,
            name: None,
            translation_name: None,
            icon_name: None,
            recipe_group: None,
            need_to_be_learn: false,
            skill_required: None,
            auto_learn_any: None,
            auto_learn_all: None,
            xp_award: None,
            meta_recipe: None,
            has_on_tick_inputs: false,
            has_on_tick_outputs: false,
            time: 50,
            loaded_timed_action_script: None,
            timed_action_script: None,
            lua_calls: Default::default(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            io_lines: Vec::new(),
            category: None,
            category_tags: Vec::new(),
            prop1: None,
            prop2: None,
            animation: None,
            tool_left: None,
            tool_right: None,
            exists_as_vanilla: false,
            mod_id: None,
            output_mappers: Vec::new(),
            overlay_mapper: OverlayMapper::default(),
            uses_tools: false,
            tooltip: None,
            allow_batch_craft: true,
            can_walk: false,
            on_add_to_menu: None,
            research_skill_level: -1,
            research_all: Vec::new(),
            research_any: Vec::new(),
        }
    }

    /// BaseScriptObject.getScriptObjectFullType: `module.name`.
    pub fn get_script_object_full_type(&self) -> String {
        format!("{}.{}", self.module, self.script_object_name)
    }

    /// zombie.scripting.entity.components.crafting.CraftRecipe.InitLoadPP with BaseScriptObject.InitLoadPP and the
    /// bucket's `addLoadedScriptBody`: run for every body found in a file. Only the game's own files are read, so the
    /// mod id is always `pz-vanilla` (ChooseGameInfo.getModDetails for a mod is not ported).
    pub fn init_load_pp(&mut self, name: &str, body: &str) {
        self.script_object_name = name.to_string();
        self.mod_id = Some(VANILLA_ID.to_string());
        self.exists_as_vanilla = true;
        self.loaded_script_bodies
            .push((VANILLA_ID.to_string(), body.to_string()));
    }

    /// BaseScriptObject.calculateScriptVersion: murmur_64 of the full type followed by every (mod id, body).
    pub fn calculate_script_version(&mut self) {
        let mut sb = self.get_script_object_full_type();
        for (m, b) in &self.loaded_script_bodies {
            sb.push_str(m);
            sb.push_str(b);
        }
        self.script_version = pz_hash::murmur_64_str(&sb);
    }

    /// zombie.scripting.entity.components.crafting.CraftRecipe.getOutputMapper
    pub fn get_output_mapper(&self, name: &str) -> Option<&OutputMapper> {
        self.output_mappers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, m)| m)
    }

    /// zombie.scripting.entity.components.crafting.CraftRecipe.getOrCreateOutputMapper
    pub fn get_or_create_output_mapper(&mut self, name: &str) -> &mut OutputMapper {
        let i = match self.output_mappers.iter().position(|(n, _)| n == name) {
            Some(i) => i,
            None => {
                self.output_mappers
                    .push((name.to_string(), OutputMapper::new(name)));
                self.output_mappers.len() - 1
            }
        };
        &mut self.output_mappers[i].1
    }

    /// zombie.scripting.entity.components.crafting.CraftRecipe.Load(String, String): `Err` is the exception that
    /// makes the bucket drop the recipe (`RemoveLoadError`). Core.debug is false, so unknown keys and blocks are only
    /// logged.
    pub fn load(&mut self, name: &str, body: &str, ctx: &LoadContext) -> Result<(), String> {
        let parsed = script_parser::parse(body);
        let block = parsed
            .children
            .first()
            .ok_or("java.lang.IndexOutOfBoundsException: Index 0 out of bounds for length 0")?;
        self.load_block(name, block, ctx)
    }

    /// zombie.scripting.entity.components.crafting.CraftRecipe.Load(String, ScriptParser.Block)
    fn load_block(&mut self, name: &str, block: &Block, ctx: &LoadContext) -> Result<(), String> {
        self.name = Some(name.to_string());
        self.translation_name = Some(ctx.translator.get_recipe_name(name));
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
        let eq = java::equals_ignore_case;
        for value in &block.values {
            let (key, val) = (java::trim(value.get_key()), java::trim(value.get_value()));
            if key.is_empty() || val.is_empty() || eq(key, "consumeOnFinish") {
                continue;
            }
            let lua_call = LUA_CALLS.iter().position(|c| eq(key, c));
            if eq(key, "recipeGroup") {
                self.recipe_group = craft_recipe_group::from_string(val);
            } else if eq(key, "icon") {
                self.icon_name = Some(val.to_string());
            } else if eq(key, "time") {
                self.time = java::parse_int(val).ok_or_else(|| nfe(val))?;
            } else if eq(key, "ResearchSkillLevel") {
                self.research_skill_level = java::parse_int(val).ok_or_else(|| nfe(val))?;
            } else if eq(key, "OnTest") || eq(key, "OnStart") || eq(key, "OnUpdate") {
                self.lua_calls[lua_call.unwrap_or(0)] = Some(val.to_string());
            } else if eq(key, "OnAddToMenu") {
                self.on_add_to_menu = Some(val.to_string());
            } else if eq(key, "OnCreate") || eq(key, "OnFailed") {
                self.lua_calls[lua_call.unwrap_or(0)] = Some(val.to_string());
            } else if eq(key, "requiresPlayer") {
            } else if eq(key, "needToBeLearn") {
                self.need_to_be_learn = eq(val, "true");
            } else if eq(key, "SkillRequired") {
                let skills = parse_skills(val)?;
                self.skill_required = Some(
                    skills
                        .into_iter()
                        .filter(|(_, level)| *level > 0)
                        .map(|(perk, level)| RequiredSkill { perk, level })
                        .collect(),
                );
            } else if eq(key, "AutoLearnAny") || eq(key, "AutoLearnAll") {
                let list = parse_skills(val)?
                    .into_iter()
                    .map(|(perk, level)| RequiredSkill { perk, level })
                    .collect();
                if eq(key, "AutoLearnAny") {
                    self.auto_learn_any = Some(list);
                } else {
                    self.auto_learn_all = Some(list);
                }
            } else if eq(key, "ResearchAll") || eq(key, "ResearchAny") {
                let perks = java::split_char(val, ';')
                    .into_iter()
                    .filter_map(perk_from_string);
                if eq(key, "ResearchAll") {
                    self.research_all.extend(perks);
                } else {
                    self.research_any.extend(perks);
                }
            } else if eq(key, "xpAward") {
                self.xp_award = Some(
                    parse_skills(val)?
                        .into_iter()
                        .map(|(perk, amount)| XpAward { perk, amount })
                        .collect(),
                );
            } else if eq(key, "MetaRecipe") {
                self.meta_recipe = Some(java::trim(val).to_string());
            } else if eq(key, "Animation") {
                self.animation = Some(java::trim(val).to_string());
            } else if eq(key, "timedAction") {
                self.loaded_timed_action_script = Some(val.to_string());
            } else if eq(key, "category") {
                self.category = Some(val.to_string());
            } else if eq(key, "Tags") {
                for t in java::split_char(val, ';') {
                    self.category_tags.push(java::trim(t).to_string());
                }
            } else if eq(key, "Tooltip") {
                self.tooltip =
                    (!string_utils::is_null_or_whitespace(Some(val))).then(|| val.to_string());
            } else if eq(key, "AllowBatchCraft") {
                self.allow_batch_craft = eq(val, "true");
            } else if eq(key, "CanWalk") {
                self.can_walk = eq(val, "true");
            } else if eq(key, "overlayStyle") {
                self.overlay_mapper.set_default_overlay_style(val);
            }
            // else: "Unknown key" is only logged (Core.debug is false)
        }
        for child in &block.children {
            let t = child.r#type.as_str();
            if eq(t, "inputs") || eq(t, "outputs") {
                self.load_io(child, eq(t, "inputs"))?;
            } else if eq(t, "itemMapper") {
                self.load_output_mapper(child)?;
            } else if eq(t, "fluidMapper") || eq(t, "energyMapper") {
                return Err("java.lang.Exception: Not yet implemented.".into());
            } else if eq(t, "overlayMapper") {
                self.load_overlay_mapper(child, ctx)?;
            }
            // else: "Unknown block" is only logged (Core.debug is false)
        }
        Ok(())
    }

    /// zombie.scripting.entity.components.crafting.CraftRecipe.LoadIO: every line of an `inputs`/`outputs` block;
    /// `+` lines become the previous line's createToItemScript, `-` lines (inputs only) its consumeFromItemScript.
    fn load_io(&mut self, block: &Block, is_input: bool) -> Result<(), String> {
        let mut last_input: Option<usize> = None;
        let mut last_output: Option<usize> = None;
        let name = self.name.clone().unwrap_or_else(|| "null".into());
        for value in &block.values {
            let s = java::trim(&value.string);
            if string_utils::is_null_or_whitespace(Some(s)) {
                continue;
            }
            // "Cannot load" for a line without whitespace is only warned about
            let index = self.io_lines.len();
            if is_input {
                if java::to_lower_case(s).starts_with("replace") {
                    // "Replace inputs have been deprecated" is only logged
                } else if let Some(rest) = s.strip_prefix('+') {
                    let last = last_input.ok_or_else(|| {
                        format!("java.io.IOException: Previous input is null [{name}] line: {s}")
                    })?;
                    if self.input(last).create_to_item_script.is_some() {
                        return Err(format!("java.io.IOException: Previous input already has '+' output [{name}] line: {s}"));
                    }
                    let script = OutputScript::load(self, rest, true)?;
                    self.input_mut(last).create_to_item_script = script.as_ref().map(|_| index);
                    self.push_io(script.map(IoScript::Output));
                } else if let Some(rest) = s.strip_prefix('-') {
                    let last = last_input.ok_or_else(|| {
                        format!("java.io.IOException: Previous input is null [{name}] line: {s}")
                    })?;
                    if self.input(last).consume_from_item_script.is_some() {
                        return Err(format!("java.io.IOException: Previous input already has '-' input [{name}] line: {s}"));
                    }
                    let mut script = InputScript::load(self, rest, true, index)?;
                    self.input_mut(last).consume_from_item_script = script.as_ref().map(|_| index);
                    if let Some(sc) = &mut script {
                        sc.parent_script = Some(last);
                    }
                    self.push_io(script.map(IoScript::Input));
                } else {
                    let input = InputScript::load(self, &value.string, false, index)?;
                    self.inputs.push(index);
                    self.push_io(input.map(IoScript::Input));
                    last_input = Some(index);
                }
            } else if let Some(rest) = s.strip_prefix('+') {
                let last = last_output.ok_or_else(|| {
                    format!("java.io.IOException: Previous input is null [{name}] line: {s}")
                })?;
                let script = OutputScript::load(self, rest, true)?;
                if let IoScript::Output(o) = &mut self.io_lines[last] {
                    o.create_to_item_script = script.as_ref().map(|_| index);
                }
                self.push_io(script.map(IoScript::Output));
                last_output = None;
            } else {
                if s.starts_with('-') {
                    return Err(format!(
                        "java.io.IOException: Cannot add '-' line to output, [{name}] line: {s}"
                    ));
                }
                let output = OutputScript::load(self, &value.string, false)?;
                self.outputs.push(index);
                self.push_io(output.map(IoScript::Output));
                last_output = Some(index);
            }
        }
        Ok(())
    }

    /// `ioLines.add(script)`. A script is null only for a blank line, which LoadIO skips before loading, so `None`
    /// does not occur.
    fn push_io(&mut self, script: Option<IoScript>) {
        if let Some(s) = script {
            self.io_lines.push(s);
        }
    }

    /// The input script at `ioLines[i]`.
    pub fn input(&self, i: usize) -> &InputScript {
        match &self.io_lines[i] {
            IoScript::Input(s) => s,
            IoScript::Output(_) => unreachable!("io line {i} is an input"),
        }
    }

    fn input_mut(&mut self, i: usize) -> &mut InputScript {
        match &mut self.io_lines[i] {
            IoScript::Input(s) => s,
            IoScript::Output(_) => unreachable!("io line {i} is an input"),
        }
    }

    /// zombie.scripting.entity.components.crafting.CraftRecipe.LoadOutputMapper: an `itemMapper <name>` block,
    /// `result = item;item` entries and `default = item`; `fluid$`/`energy$` prefixes throw, other `x$` prefixes are
    /// cut off. A mapper left empty throws.
    fn load_output_mapper(&mut self, block: &Block) -> Result<(), String> {
        let id = block
            .id
            .as_deref()
            .ok_or("java.lang.NullPointerException: block.id")?;
        let mapper_name = java::trim(id).to_string();
        self.get_or_create_output_mapper(&mapper_name);
        for value in &block.values {
            let (key, val) = (java::trim(value.get_key()), java::trim(value.get_value()));
            if key.is_empty() || val.is_empty() {
                continue;
            }
            let mapper = self.get_or_create_output_mapper(&mapper_name);
            if java::equals_ignore_case(key, "default") {
                mapper.set_default_output_entree(val);
            } else {
                let mut split: Vec<String> = java::split_char(val, ';')
                    .into_iter()
                    .map(str::to_string)
                    .collect();
                for s in &mut split {
                    if s.contains('$') {
                        let d = java::string_index_of(s, "$");
                        let prefix = java::to_lower_case(&java::string_substring(s, 0, d)?);
                        if prefix == "fluid" || prefix == "energy" {
                            return Err("java.lang.Exception: Not yet implemented".into());
                        }
                        *s = java::string_substring_from(s, d + 1)?;
                    }
                }
                mapper.add_output_entree(key, &split);
            }
        }
        if self.get_or_create_output_mapper(&mapper_name).is_empty() {
            return Err(format!(
                "java.lang.Exception: Failed to load contents for output mapper: {id}"
            ));
        }
        Ok(())
    }

    /// zombie.scripting.entity.components.crafting.CraftRecipe.LoadOverlayMapper: `default = style;style` and
    /// `item;item = style` entries (items through ScriptManager.getItem); an empty mapper throws.
    fn load_overlay_mapper(&mut self, block: &Block, ctx: &LoadContext) -> Result<(), String> {
        for value in &block.values {
            let (key, val) = (java::trim(value.get_key()), java::trim(value.get_value()));
            if key.is_empty() || val.is_empty() {
                continue;
            }
            if java::equals_ignore_case(key, "default") {
                self.overlay_mapper.set_default_overlay_style(val);
            } else {
                let split = java::split_char(key, ';');
                self.overlay_mapper
                    .add_overlay_style_entry(val, &split, ctx.get_item)?;
            }
        }
        if self.overlay_mapper.is_empty() {
            return Err("java.lang.Exception: Failed to load contents for overlay mapper.".into());
        }
        Ok(())
    }

    /// zombie.scripting.entity.components.crafting.CraftRecipe.OnScriptsLoaded: the icon name gets its `Item_`
    /// prefix (`Texture.trygetTexture` is not ported: a dedicated server has no textures), the tools and props are
    /// taken from the input flags (a second one throws), the on-tick flags set, and the timed action looked up
    /// through `get_timed_action` (ScriptManager.getTimedActionScript, its full type; missing throws).
    pub fn on_scripts_loaded(
        &mut self,
        get_timed_action: &dyn Fn(&str) -> Option<String>,
    ) -> Result<(), String> {
        let name = self.name.clone().unwrap_or_else(|| "null".into());
        if let Some(icon) = &self.icon_name {
            if !icon.starts_with("Item_") {
                self.icon_name = Some(format!("Item_{icon}"));
            }
        }
        for k in 0..self.inputs.len() {
            let i = self.inputs[k];
            let input = self.input(i);
            let (on_tick, tool, left, right, p1, p2) = (
                input.apply_on_tick,
                input.is_tool(),
                input.flags.contains(InputFlag::TOOL_LEFT),
                input.flags.contains(InputFlag::TOOL_RIGHT),
                input.flags.contains(InputFlag::PROP1),
                input.flags.contains(InputFlag::PROP2),
            );
            if on_tick {
                self.has_on_tick_inputs = true;
            }
            if tool {
                self.uses_tools = true;
                if left {
                    if self.tool_left.is_some() {
                        return Err(format!(
                            "java.lang.Exception: Duplicate tool left in recipe {name}"
                        ));
                    }
                    self.tool_left = Some(i);
                }
                if right {
                    if self.tool_right.is_some() {
                        return Err(format!(
                            "java.lang.Exception: Duplicate tool left in recipe {name}"
                        ));
                    }
                    self.tool_right = Some(i);
                }
            }
            if p1 {
                if self.prop1.is_some() {
                    return Err(format!(
                        "java.io.IOException: Duplicate Prop1 in recipe {name}"
                    ));
                }
                self.prop1 = Some(i);
            }
            if p2 {
                if self.prop2.is_some() {
                    return Err(format!(
                        "java.io.IOException: Duplicate Prop2 in recipe {name}"
                    ));
                }
                self.prop2 = Some(i);
            }
        }
        for &o in &self.outputs {
            if let IoScript::Output(out) = &self.io_lines[o] {
                if out.apply_on_tick {
                    self.has_on_tick_outputs = true;
                }
            }
        }
        if let Some(t) = &self.loaded_timed_action_script {
            self.timed_action_script = get_timed_action(t);
            if self.timed_action_script.is_none() {
                return Err(format!("java.lang.Exception: TimedActionScript '{t}' could not be found in recipe {name}"));
            }
        }
        Ok(())
    }

    /// zombie.scripting.entity.components.crafting.CraftRecipe.OnPostWorldDictionaryInit, the ported part: every
    /// input, then every output (icon textures are not ported). `research` collects the recipe names
    /// `OutputMapper.getItem` runs `canBeResearched()` on.
    pub fn on_post_world_dictionary_init(
        &mut self,
        lookups: &Lookups,
        research: &mut Vec<String>,
    ) -> Result<(), String> {
        for k in 0..self.inputs.len() {
            self.input_on_post_world_dictionary_init(self.inputs[k], lookups, research)?;
        }
        for k in 0..self.outputs.len() {
            self.output_on_post_world_dictionary_init(self.outputs[k], lookups, research)?;
        }
        Ok(())
    }

    /// zombie.scripting.entity.components.crafting.InputScript.OnPostWorldDictionaryInit for `ioLines[i]`, the
    /// ported part: the checks of lines that carry a `+` or `-` line and the apply mode they force. Not ported: the
    /// resolution of items (ScriptManager.getItem, `ItemTags.getItemsForTag`, which throws for a tag without items),
    /// fluids (`Fluid.Get`, FluidCategory) and energies (`Energy.Get`), each of which throws when not found, and the
    /// `getUsedInRecipes` bookkeeping on items.
    fn input_on_post_world_dictionary_init(
        &mut self,
        i: usize,
        lookups: &Lookups,
        research: &mut Vec<String>,
    ) -> Result<(), String> {
        let (create_to, consume_from) = {
            let s = self.input(i);
            (s.create_to_item_script, s.consume_from_item_script)
        };
        let line = self.input(i).original_line.clone();
        if let (Some(c), Some(f)) = (create_to, consume_from) {
            if self.io_type(c) == self.io_type(f) {
                return Err(format!("java.lang.Exception: Input line cannot have a '-' and '+' line of the same ResourceType. line: {line}"));
            }
        }
        if let Some(c) = create_to {
            self.output_on_post_world_dictionary_init(c, lookups, research)?;
            self.check_carrier(i, "+")?;
            self.input_mut(i).item_apply_mode = ItemApplyMode::Keep;
        }
        if let Some(f) = consume_from {
            self.input_on_post_world_dictionary_init(f, lookups, research)?;
            self.check_carrier(i, "-")?;
            let input = self.input_mut(i);
            if input.item_apply_mode != ItemApplyMode::Destroy {
                input.item_apply_mode = ItemApplyMode::Keep;
            }
        }
        Ok(())
    }

    /// The three checks InputScript.OnPostWorldDictionaryInit makes on a line that carries a `+`/`-` line.
    fn check_carrier(&self, i: usize, sign: &str) -> Result<(), String> {
        let s = self.input(i);
        let line = &s.original_line;
        if s.get_int_amount() != 1 || s.is_variable_amount() {
            return Err(format!("java.lang.Exception: Lines prior to a '{sign}' line should have 1 item amount. line: {line}"));
        }
        if s.r#type != ResourceType::Item {
            return Err(format!("java.lang.Exception: Lines prior to a '{sign}' line should be of resource type Item. line: {line}"));
        }
        if s.apply_on_tick {
            return Err(format!("java.lang.Exception: Lines prior to a '{sign}' line should not be apply on tick. line: {line}"));
        }
        Ok(())
    }

    fn io_type(&self, i: usize) -> ResourceType {
        match &self.io_lines[i] {
            IoScript::Input(s) => s.r#type,
            IoScript::Output(s) => s.r#type,
        }
    }

    /// zombie.scripting.entity.components.crafting.OutputScript.OnPostWorldDictionaryInit for `ioLines[o]`: a `+` line
    /// first, then an item output's mapper resolved with the recipe's name, then `isValid` for items. Not ported: the
    /// fluid (`Fluid.Get`) and energy (`Energy.Get`) lookups, which throw when not found, and `isValid` for them.
    fn output_on_post_world_dictionary_init(
        &mut self,
        o: usize,
        lookups: &Lookups,
        research: &mut Vec<String>,
    ) -> Result<(), String> {
        let IoScript::Output(out) = &self.io_lines[o] else {
            return Ok(());
        };
        let (create_to, r#type, line) = (
            out.create_to_item_script,
            out.r#type,
            out.original_line.clone(),
        );
        if let Some(c) = create_to {
            self.output_on_post_world_dictionary_init(c, lookups, research)?;
            let IoScript::Output(out) = &mut self.io_lines[o] else {
                return Ok(());
            };
            if out.get_int_amount() != 1 || out.is_variable_amount() {
                return Err(format!("java.lang.Exception: Lines prior to a '+' line should have 1 item amount. line: {line}"));
            }
            if out.r#type != ResourceType::Item {
                return Err(format!("java.lang.Exception: Lines prior to a '+' line should be of resource type Item. line: {line}"));
            }
            if out.apply_on_tick {
                return Err(format!("java.lang.Exception: Lines prior to a '+' line should not be apply on tick. line: {line}"));
            }
            out.item_apply_mode = ItemApplyMode::Normal;
        }
        let name = self.name.clone();
        let valid = match r#type {
            ResourceType::Item => {
                let mapper = match &mut self.io_lines[o] {
                    IoScript::Output(out) => out.output_mapper.clone(),
                    IoScript::Input(_) => None,
                };
                let results = match mapper {
                    None => {
                        return Err(format!(
                            "java.lang.Exception: No outputMapper set. line: {line}"
                        ))
                    }
                    Some(MapperRef::Recipe(n)) => {
                        let m = self.get_or_create_output_mapper(&n);
                        m.on_post_world_dictionary_init(name.as_deref(), lookups, research)?;
                        m.result_items.len()
                    }
                    Some(MapperRef::Own(_)) => {
                        let IoScript::Output(out) = &mut self.io_lines[o] else {
                            return Ok(());
                        };
                        let Some(MapperRef::Own(m)) = &mut out.output_mapper else {
                            return Ok(());
                        };
                        m.on_post_world_dictionary_init(name.as_deref(), lookups, research)?;
                        m.result_items.len()
                    }
                };
                results > 0
            }
            ResourceType::Fluid | ResourceType::Energy => true,
            ResourceType::Any => false,
        };
        if !valid {
            return Err(format!("java.lang.Exception: Invalid output. line: {line}"));
        }
        Ok(())
    }

    /// zombie.scripting.entity.components.crafting.CraftRecipe.hasTag: `getTags().contains(tag.toString())`.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.category_tags.iter().any(|t| t == tag)
    }

    /// zombie.scripting.entity.components.crafting.CraftRecipe.cannotBeResearched
    pub fn cannot_be_researched(&self) -> bool {
        self.has_tag(craft_recipe_tag::CANNOT_BE_RESEARCHED)
    }

    /// zombie.scripting.entity.components.crafting.CraftRecipe.canAlwaysBeResearched (evaluates
    /// `getResearchSkillLevel()`, which settles the research level, only when the recipe needs to be learnt and has no
    /// CanAlwaysBeResearched tag).
    pub fn can_always_be_researched(&mut self) -> bool {
        if self.cannot_be_researched() {
            return false;
        }
        self.need_to_be_learn
            && (self.has_tag(craft_recipe_tag::CAN_ALWAYS_BE_RESEARCHED)
                || self.get_research_skill_level() == 0)
    }

    /// zombie.scripting.entity.components.crafting.CraftRecipe.canBeResearched
    pub fn can_be_researched(&mut self) -> bool {
        if self.cannot_be_researched() {
            return false;
        }
        self.can_always_be_researched() || self.need_to_be_learn && !self.cannot_be_researched()
    }

    /// zombie.scripting.entity.components.crafting.CraftRecipe.getRequiredSkills().size(): one entry per required
    /// skill (every perk is known, so none is skipped).
    fn required_skill_count(&self) -> usize {
        self.skill_required.as_ref().map_or(0, Vec::len)
    }

    /// zombie.scripting.entity.components.crafting.CraftRecipe.getHighestSkillRequirement(boolean). The AutoLearnAll
    /// levels count only when AutoLearnAny is set (the original tests `getAutoLearnAnySkills() != null` there too).
    pub fn get_highest_skill_requirement(&self, include_auto_learn: bool) -> i32 {
        let mut highest = 0;
        for i in 0..self.required_skill_count() {
            let level = self.skill_required.as_ref().map_or(0, |s| s[i].level);
            if level > highest {
                highest = level;
            }
        }
        let any = self.auto_learn_any.as_ref();
        if include_auto_learn && any.is_some_and(|a| !a.is_empty()) {
            for s in any.into_iter().flatten() {
                if s.level > highest {
                    highest = s.level;
                }
            }
        }
        if include_auto_learn
            && any.is_some()
            && self.auto_learn_all.as_ref().is_some_and(|a| !a.is_empty())
        {
            for s in self.auto_learn_all.iter().flatten() {
                if s.level > highest {
                    highest = s.level;
                }
            }
        }
        highest
    }

    /// zombie.scripting.entity.components.crafting.CraftRecipe.normalizeSkillLevel: clamped to 0..10.
    fn normalize_skill_level(level: i32) -> i32 {
        level.max(0).min(10)
    }

    /// zombie.scripting.entity.components.crafting.CraftRecipe.getResearchSkillLevel(null): an unset level (-1)
    /// becomes two thirds of the highest required level (with auto-learn levels when there is none), rounded, at
    /// least 1; the field is then normalised in place (so an unset level without requirements becomes 0).
    pub fn get_research_skill_level(&mut self) -> i32 {
        for include in [false, true] {
            let highest = self.get_highest_skill_requirement(include);
            if self.research_skill_level == -1 && highest > 0 {
                let research_level = highest as f32 * 2.0 / 3.0;
                let research_level = java::math_round_f32(research_level) as f32;
                let level = 1.0f32.max(research_level) as i32;
                self.research_skill_level = Self::normalize_skill_level(level);
                break;
            }
        }
        self.research_skill_level = Self::normalize_skill_level(self.research_skill_level);
        self.research_skill_level
    }
}
