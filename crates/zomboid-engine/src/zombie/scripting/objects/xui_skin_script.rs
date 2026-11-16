//! zombie.scripting.objects.XuiSkinScript: an `xuiSkin` script (imports, entity UI info, colours, and the style
//! blocks per Lua class), every body of a skin amending the same script.
use super::base_script_object::{first_child, BaseScriptObject, ScriptObject};
use super::xui_colors_script::XuiColorsScript;
use crate::java;
use crate::zombie::core::translator::Translator;
use crate::zombie::entity::component_type::ComponentType;
use crate::zombie::scripting::script_parser::{self, Block};
use crate::zombie::scripting::ui::JHashMap;
use crate::zombie::util::string_utils;

/// zombie.scripting.objects.XuiSkinScript.ComponentUiScript
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentUiScript {
    pub lua_panel_class: Option<String>,
    pub xui_style: Option<String>,
    pub display_name: Option<String>,
    pub icon_path: Option<String>,
    pub list_order_zset: bool,
    pub list_order_z: i32,
    pub enabled_set: bool,
    pub enabled: bool,
}

impl Default for ComponentUiScript {
    fn default() -> Self {
        ComponentUiScript {
            lua_panel_class: None,
            xui_style: None,
            display_name: None,
            icon_path: None,
            list_order_zset: false,
            list_order_z: 0,
            enabled_set: false,
            enabled: true,
        }
    }
}

/// zombie.scripting.objects.XuiSkinScript.EntityUiScript
#[derive(Clone, Debug, PartialEq, Default)]
pub struct EntityUiScript {
    pub lua_window_class: Option<String>,
    pub xui_style: Option<String>,
    pub lua_can_open_window: Option<String>,
    pub lua_open_window: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub build_description: Option<String>,
    pub icon_path: Option<String>,
    pub clear_components: bool,
    /// EntityUiScript.componentUiScriptMap, a HashMap keyed by the enum (identity hash order); in insertion order.
    pub component_ui_script_map: Vec<(ComponentType, ComponentUiScript)>,
}

impl EntityUiScript {
    /// zombie.scripting.objects.XuiSkinScript.EntityUiScript.reset
    fn reset(&mut self) {
        *self = EntityUiScript::default();
    }

    /// `componentUiScriptMap.get(type)`, made and put when missing.
    fn component(&mut self, t: ComponentType) -> &mut ComponentUiScript {
        let i = match self
            .component_ui_script_map
            .iter()
            .position(|(k, _)| *k == t)
        {
            Some(i) => i,
            None => {
                self.component_ui_script_map
                    .push((t, ComponentUiScript::default()));
                self.component_ui_script_map.len() - 1
            }
        };
        &mut self.component_ui_script_map[i].1
    }
}

/// zombie.scripting.objects.XuiSkinScript.StyleInfoScript: one Lua class's unnamed block and named blocks, key ->
/// value (null for an empty, "nil" or "null" value).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct StyleInfoScript {
    pub default_style_block: JHashMap<Option<String>>,
    pub style_blocks: JHashMap<JHashMap<Option<String>>>,
}

/// zombie.scripting.objects.XuiSkinScript
#[derive(Clone, Debug, PartialEq)]
pub struct XuiSkinScript {
    pub base: BaseScriptObject,
    pub imports: Vec<String>,
    pub default_entity_ui_script: EntityUiScript,
    pub entity_ui_script_map: JHashMap<EntityUiScript>,
    pub style_info_map: JHashMap<StyleInfoScript>,
    pub colors_script: XuiColorsScript,
}

/// zombie.util.StringUtils.trimSurroundingQuotes(s, true): trimmed, then one pair of surrounding double quotes
/// removed. `Err` is the StringIndexOutOfBoundsException of a lone `"` (substring(1, 0)).
fn trim_surrounding_quotes(s: &str) -> Result<String, String> {
    if s.is_empty() {
        return Ok(s.to_string());
    }
    let s = java::trim(s);
    if s.starts_with('"') && s.ends_with('"') {
        if s.len() < 2 {
            return Err(
                "java.lang.StringIndexOutOfBoundsException: begin 1, end 0, length 1".to_string(),
            );
        }
        Ok(s[1..s.len() - 1].to_string())
    } else {
        Ok(s.to_string())
    }
}

/// Integer.parseInt; `Err` is the NumberFormatException.
fn parse_int(s: &str) -> Result<i32, String> {
    java::parse_int(s).ok_or_else(|| {
        format!(
            "java.lang.NumberFormatException: {}",
            java::number_format_message(s)
        )
    })
}

impl XuiSkinScript {
    /// zombie.scripting.objects.XuiSkinScript()
    pub fn new(module: Option<&str>) -> XuiSkinScript {
        XuiSkinScript {
            base: BaseScriptObject::new(module),
            imports: Vec::new(),
            default_entity_ui_script: EntityUiScript::default(),
            entity_ui_script_map: JHashMap::new(),
            style_info_map: JHashMap::new(),
            colors_script: XuiColorsScript::new(None),
        }
    }

    /// zombie.scripting.objects.XuiSkinScript.LoadStyleBlock: `<LuaClass> [styleName] { key = value, ... }` into the
    /// class's unnamed block (no name) or named block; quotes around a value are removed, and an empty, "nil" or
    /// "null" value is stored as null. `Err` for the name "default" or a name with a dot.
    fn load_style_block(&mut self, block: &Block) -> Result<(), String> {
        if !self.style_info_map.contains_key(&block.r#type) {
            self.style_info_map
                .put(&block.r#type, StyleInfoScript::default());
        }
        let Some(info) = self.style_info_map.get_mut(&block.r#type) else {
            return Ok(());
        };
        let map = match block.id.as_deref() {
            id if string_utils::is_null_or_whitespace(id) => &mut info.default_style_block,
            Some(id) => {
                if java::equals_ignore_case(id, "default") {
                    return Err(
                        "java.lang.Exception: Default is protected and cannot be used as style name."
                            .to_string(),
                    );
                }
                if id.contains('.') {
                    return Err(
                        "java.lang.Exception: Style name may not contain '.' (dot).".to_string()
                    );
                }
                if !info.style_blocks.contains_key(id) {
                    info.style_blocks.put(id, JHashMap::new());
                }
                match info.style_blocks.get_mut(id) {
                    Some(m) => m,
                    None => return Ok(()),
                }
            }
            None => &mut info.default_style_block,
        };
        for value in &block.values {
            let key = java::trim(value.get_key());
            let val = java::trim(value.get_value());
            if !key.is_empty() {
                let val = trim_surrounding_quotes(val)?;
                let stored = if !val.is_empty()
                    && !java::equals_ignore_case(&val, "nil")
                    && !java::equals_ignore_case(&val, "null")
                {
                    Some(val)
                } else {
                    None
                };
                map.put(key, stored);
            }
        }
        Ok(())
    }

    /// zombie.scripting.objects.XuiSkinScript.LoadImports: every non-blank entry (a self-import only warns, as
    /// Core.debug is false).
    fn load_imports(&mut self, block: &Block) {
        for value in &block.values {
            let s = java::trim(&value.string);
            if !string_utils::is_null_or_whitespace(Some(s)) {
                self.imports.push(s.to_string());
            }
        }
    }

    /// zombie.scripting.objects.XuiSkinScript.LoadEntityBlock: `entity [alias] { ... }` into the default entity
    /// script (no alias) or the alias's; `displayName` is looked up as a recipe name (spaces removed) and replaced by
    /// the translation when there is one. `Err` for the alias "default" and from the components.
    fn load_entity_block(&mut self, block: &Block, translator: &Translator) -> Result<(), String> {
        let ui = match block.id.as_deref() {
            id if string_utils::is_null_or_whitespace(id) => &mut self.default_entity_ui_script,
            Some(id) => {
                if java::equals_ignore_case(id, "default") {
                    return Err(
                        "java.lang.Exception: Default is protected and cannot be used as style name."
                            .to_string(),
                    );
                }
                if !self.entity_ui_script_map.contains_key(id) {
                    self.entity_ui_script_map.put(id, EntityUiScript::default());
                }
                match self.entity_ui_script_map.get_mut(id) {
                    Some(u) => u,
                    None => return Ok(()),
                }
            }
            None => &mut self.default_entity_ui_script,
        };
        for value in &block.values {
            let key = java::trim(value.get_key());
            let val = java::trim(value.get_value());
            if !key.is_empty() && !val.is_empty() {
                let v = Some(val.to_string());
                if java::equals_ignore_case(key, "luaWindowClass") {
                    ui.lua_window_class = v;
                } else if java::equals_ignore_case(key, "xuiStyle") {
                    ui.xui_style = v;
                } else if java::equals_ignore_case(key, "luaCanOpenWindow") {
                    ui.lua_can_open_window = v;
                } else if java::equals_ignore_case(key, "luaOpenWindow") {
                    ui.lua_open_window = v;
                } else if java::equals_ignore_case(key, "displayName") {
                    let new_val = val.replace(' ', "");
                    let translated = translator.get_recipe_name(&new_val);
                    ui.display_name = if !java::equals_ignore_case(&translated, &new_val) {
                        Some(translated)
                    } else {
                        v
                    };
                } else if java::equals_ignore_case(key, "description") {
                    ui.description = v;
                } else if java::equals_ignore_case(key, "buildDescription") {
                    ui.build_description = v;
                } else if java::equals_ignore_case(key, "icon") {
                    ui.icon_path = v;
                } else if java::equals_ignore_case(key, "clearComponents") {
                    ui.clear_components = java::equals_ignore_case(val, "true");
                }
            }
        }
        for child in &block.children {
            if java::equals_ignore_case("components", &child.r#type) {
                load_components(child, ui)?;
            }
        }
        Ok(())
    }
}

/// zombie.scripting.objects.XuiSkinScript.LoadComponents: `Type = LuaClass[:style]` entries (ComponentType.valueOf),
/// then a block per component. `Err` for an unknown component type, a "Class:" value without a style
/// (ArrayIndexOutOfBoundsException) and from the blocks.
fn load_components(block: &Block, ui: &mut EntityUiScript) -> Result<(), String> {
    for value in &block.values {
        let key = java::trim(value.get_key());
        let val = java::trim(value.get_value());
        if !key.is_empty() && !val.is_empty() {
            let t = ComponentType::value_of(key)?;
            let mut lua_class = val.to_string();
            let mut style = None;
            if val.contains(':') {
                let s = java::split_char(val, ':');
                lua_class = s
                    .first()
                    .map(|x| x.to_string())
                    .ok_or_else(|| "java.lang.ArrayIndexOutOfBoundsException: Index 0 out of bounds for length 0".to_string())?;
                style = Some(
                    s.get(1)
                        .map(|x| x.to_string())
                        .ok_or_else(|| {
                            format!(
                                "java.lang.ArrayIndexOutOfBoundsException: Index 1 out of bounds for length {}",
                                s.len()
                            )
                        })?,
                );
            }
            let info = ui.component(t);
            info.lua_panel_class = Some(lua_class);
            if style.is_some() {
                info.xui_style = style;
            }
        }
    }
    for child in &block.children {
        load_component_block(child, ui)?;
    }
    Ok(())
}

/// zombie.scripting.objects.XuiSkinScript.LoadComponentBlock: `<ComponentType> { ... }`. `Err` for an unknown
/// component type or a `listOrderZ` that is not an int.
fn load_component_block(block: &Block, ui: &mut EntityUiScript) -> Result<(), String> {
    let t = ComponentType::value_of(&block.r#type)?;
    let info = ui.component(t);
    for value in &block.values {
        let key = java::trim(value.get_key());
        let val = java::trim(value.get_value());
        if !key.is_empty() && !val.is_empty() {
            let v = Some(val.to_string());
            if java::equals_ignore_case(key, "luaPanelClass") {
                info.lua_panel_class = v;
            } else if java::equals_ignore_case(key, "xuiStyle") {
                info.xui_style = v;
            } else if java::equals_ignore_case(key, "displayName") {
                info.display_name = v;
            } else if java::equals_ignore_case(key, "icon") {
                info.icon_path = v;
            } else if java::equals_ignore_case(key, "listOrderZ") {
                info.list_order_z = parse_int(val)?;
                info.list_order_zset = true;
            } else if java::equals_ignore_case(key, "enabled") {
                info.enabled = java::equals_ignore_case(val, "true");
                info.enabled_set = true;
            }
        }
    }
    Ok(())
}

impl ScriptObject for XuiSkinScript {
    fn base(&self) -> &BaseScriptObject {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseScriptObject {
        &mut self.base
    }

    /// zombie.scripting.objects.XuiSkinScript.reset
    fn reset(&mut self) {
        self.imports.clear();
        self.default_entity_ui_script.reset();
        self.entity_ui_script_map.clear();
        self.style_info_map.clear();
        self.colors_script.color_map.clear();
    }

    /// zombie.scripting.objects.XuiSkinScript.Load: the child blocks `imports`, `entity`, `colors` (any case), any
    /// other a style block of the Lua class it names.
    fn load(&mut self, _name: &str, body: &str, translator: &Translator) -> Result<(), String> {
        let parsed = script_parser::parse(body);
        let block = first_child(&parsed)?;
        self.base.load_common_block(block);
        for child in &block.children {
            if java::equals_ignore_case("imports", &child.r#type) {
                self.load_imports(child);
            } else if java::equals_ignore_case("entity", &child.r#type) {
                self.load_entity_block(child, translator)?;
            } else if java::equals_ignore_case("colors", &child.r#type) {
                self.colors_script.load_colors_block(child)?;
            } else {
                self.load_style_block(child)?;
            }
        }
        Ok(())
    }
}
