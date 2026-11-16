//! zombie.scripting.ui.XuiSkin: a loaded skin (its imports, colours, entity and component UI styles, and the Lua
//! styles per Lua class), built from its xuiSkin script and the scripts of the skins it imports.
use super::xui_lua_style::{ColorLookup, XuiLuaStyle, XuiVar};
use super::JHashMap;
use crate::zombie::core::color::Color;
use crate::zombie::core::colors::Colors;
use crate::zombie::entity::component_type::ComponentType;
use crate::zombie::scripting::objects::xui_skin_script::{
    EntityUiScript, StyleInfoScript, XuiSkinScript,
};

/// zombie.core.textures.Texture.trygetTexture on the dedicated server: `GameServer.server && !ServerGUI.isCreated()`
/// returns null before any lookup.
fn try_get_texture(_name: &str) -> Option<String> {
    None
}

/// zombie.core.textures.Texture.getSharedTexture on the dedicated server: null, as `trygetTexture`.
fn get_shared_texture(_name: &str) -> Option<String> {
    None
}

/// zombie.scripting.ui.XuiSkin.ComponentUiStyle
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentUiStyle {
    pub lua_panel_class: Option<String>,
    pub xui_style: Option<String>,
    pub display_name: Option<String>,
    /// ComponentUiStyle.icon, a Texture by name (always null on the server).
    pub icon: Option<String>,
    pub list_order_z: i32,
    pub enabled: bool,
}

impl Default for ComponentUiStyle {
    fn default() -> Self {
        ComponentUiStyle {
            lua_panel_class: None,
            xui_style: None,
            display_name: None,
            icon: None,
            list_order_z: 0,
            enabled: true,
        }
    }
}

/// zombie.scripting.ui.XuiSkin.EntityUiStyle
#[derive(Clone, Debug, PartialEq, Default)]
pub struct EntityUiStyle {
    pub lua_window_class: Option<String>,
    pub xui_style: Option<String>,
    pub lua_can_open_window: Option<String>,
    pub lua_open_window: Option<String>,
    /// EntityUiStyle.componentUiStyleMap, a HashMap keyed by the enum (identity hash order); in insertion order.
    pub component_ui_style_map: Vec<(ComponentType, ComponentUiStyle)>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub build_description: Option<String>,
    /// EntityUiStyle.icon, a Texture by name (always null on the server).
    pub icon: Option<String>,
}

impl EntityUiStyle {
    /// zombie.scripting.ui.XuiSkin.EntityUiStyle.getComponentUiStyle
    pub fn get_component_ui_style(&self, t: ComponentType) -> Option<&ComponentUiStyle> {
        self.component_ui_style_map
            .iter()
            .find(|(k, _)| *k == t)
            .map(|(_, v)| v)
    }

    /// zombie.scripting.ui.XuiSkin.EntityUiStyle.isComponentEnabled
    pub fn is_component_enabled(&self, t: ComponentType) -> bool {
        self.get_component_ui_style(t).is_some_and(|i| i.enabled)
    }

    /// zombie.scripting.ui.XuiSkin.EntityUiStyle.Load: every field the script sets; an icon the texture lookup does
    /// not find falls back to "media/inventory/Question_On.png" when there is none yet (null on the server too).
    fn load(&mut self, script: &EntityUiScript, log: &mut Vec<String>) {
        if script.lua_window_class.is_some() {
            self.lua_window_class = script.lua_window_class.clone();
        }
        if script.xui_style.is_some() {
            self.xui_style = script.xui_style.clone();
        }
        if script.lua_can_open_window.is_some() {
            self.lua_can_open_window = script.lua_can_open_window.clone();
        }
        if script.lua_open_window.is_some() {
            self.lua_open_window = script.lua_open_window.clone();
        }
        if script.display_name.is_some() {
            self.display_name = script.display_name.clone();
        }
        if script.description.is_some() {
            self.description = script.description.clone();
        }
        if script.build_description.is_some() {
            self.build_description = script.build_description.clone();
        }
        if let Some(path) = &script.icon_path {
            match try_get_texture(path) {
                Some(tex) => self.icon = Some(tex),
                None => {
                    log.push(format!(
                        "Could not find icon: {path}, script = {}",
                        script.display_name.as_deref().unwrap_or("null")
                    ));
                    if self.icon.is_none() {
                        self.icon = get_shared_texture("media/inventory/Question_On.png");
                    }
                }
            }
        }
        if script.clear_components {
            self.component_ui_style_map.clear();
        }
        self.load_component_info(script, log);
    }

    /// zombie.scripting.ui.XuiSkin.EntityUiStyle.LoadComponentInfo
    fn load_component_info(&mut self, script: &EntityUiScript, log: &mut Vec<String>) {
        for (t, s) in &script.component_ui_script_map {
            let i = match self.component_ui_style_map.iter().position(|(k, _)| k == t) {
                Some(i) => i,
                None => {
                    self.component_ui_style_map
                        .push((*t, ComponentUiStyle::default()));
                    self.component_ui_style_map.len() - 1
                }
            };
            let info = &mut self.component_ui_style_map[i].1;
            if s.lua_panel_class.is_some() {
                info.lua_panel_class = s.lua_panel_class.clone();
            }
            if s.xui_style.is_some() {
                info.xui_style = s.xui_style.clone();
            }
            if s.display_name.is_some() {
                info.display_name = s.display_name.clone();
            }
            if let Some(path) = &s.icon_path {
                match try_get_texture(path) {
                    Some(tex) => info.icon = Some(tex),
                    None => {
                        log.push(format!("Could not find icon: {path}"));
                        if info.icon.is_none() {
                            info.icon = get_shared_texture("media/inventory/Question_On.png");
                        }
                    }
                }
            }
            if s.list_order_zset {
                info.list_order_z = s.list_order_z;
            }
            if s.enabled_set {
                info.enabled = s.enabled;
            }
        }
    }
}

/// zombie.scripting.ui.XuiSkin.StyleInfo: the default style of a Lua class and its named styles.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct StyleInfo {
    pub default_style: Option<XuiLuaStyle>,
    pub styles: JHashMap<XuiLuaStyle>,
}

impl StyleInfo {
    /// zombie.scripting.ui.XuiSkin.StyleInfo.getStyle
    pub fn get_style(&self, name: &str) -> Option<&XuiLuaStyle> {
        self.styles.get(name)
    }
}

/// zombie.scripting.ui.XuiSkin
#[derive(Clone, Debug, PartialEq)]
pub struct XuiSkin {
    /// XuiSkin.imports, by skin name.
    pub imports: Vec<String>,
    pub color_map: JHashMap<Color>,
    pub default_entity_ui_style: EntityUiStyle,
    pub entity_ui_style_map: JHashMap<EntityUiStyle>,
    pub styles: JHashMap<StyleInfo>,
    pub name: String,
    /// XuiSkin.script: index into the xuiSkin scripts ParseScripts was given.
    pub script: usize,
    pub has_loaded: bool,
    pub invalidated: bool,
}

/// zombie.scripting.ui.XuiSkin.colorInternal over a skin's colour map: the skin's colour, else (with the system
/// fallback) Colors.GetColorByName, else null or white.
pub fn color_internal(
    color_map: &JHashMap<Color>,
    colors: &Colors,
    alias: Option<&str>,
    do_system_colors_fallback: bool,
    allow_null: bool,
) -> Option<Color> {
    let alias = alias?;
    if let Some(c) = color_map.get(alias) {
        Some(*c)
    } else if do_system_colors_fallback {
        colors.get_color_by_name(alias)
    } else if !allow_null {
        // Colors.White
        Some(Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        })
    } else {
        None
    }
}

/// What XuiSkin.Load reads: the xuiSkin scripts, ScriptManager.getXuiSkinScript, the var registry and the colours.
pub struct SkinContext<'a> {
    pub scripts: &'a [XuiSkinScript],
    /// ScriptManager.getXuiSkinScript: index into `scripts`.
    pub get_skin_script: &'a dyn Fn(&str) -> Option<usize>,
    pub registry: &'a JHashMap<XuiVar>,
    pub colors: &'a Colors,
}

impl XuiSkin {
    /// zombie.scripting.ui.XuiSkin(String, XuiSkinScript)
    pub fn new(name: &str, script: usize) -> XuiSkin {
        XuiSkin {
            imports: Vec::new(),
            color_map: JHashMap::new(),
            default_entity_ui_style: EntityUiStyle::default(),
            entity_ui_style_map: JHashMap::new(),
            styles: JHashMap::new(),
            name: name.to_string(),
            script,
            has_loaded: false,
            invalidated: false,
        }
    }

    /// zombie.scripting.ui.XuiSkin.color
    pub fn color(&self, colors: &Colors, alias: Option<&str>) -> Option<Color> {
        color_internal(&self.color_map, colors, alias, true, true)
    }

    /// zombie.scripting.ui.XuiSkin.getEntityUiStyle: the default for a null alias or one not found.
    pub fn get_entity_ui_style(&self, alias: Option<&str>) -> &EntityUiStyle {
        match alias {
            None => &self.default_entity_ui_style,
            Some(a) => self
                .entity_ui_style_map
                .get(a)
                .unwrap_or(&self.default_entity_ui_style),
        }
    }

    /// zombie.scripting.ui.XuiSkin.getComponentUiStyle
    pub fn get_component_ui_style(
        &self,
        entity_alias: Option<&str>,
        t: ComponentType,
    ) -> Option<&ComponentUiStyle> {
        self.get_entity_ui_style(entity_alias)
            .get_component_ui_style(t)
    }

    /// zombie.scripting.ui.XuiSkin.get: the default style of the Lua class for a null alias, else its named style.
    pub fn get(&self, lua_class: &str, alias: Option<&str>) -> Option<&XuiLuaStyle> {
        let info = self.styles.get(lua_class)?;
        match alias {
            None => info.default_style.as_ref(),
            Some(a) => info.get_style(a),
        }
    }

    /// zombie.scripting.ui.XuiSkin.getDefault
    pub fn get_default(&self, lua_class: &str) -> Option<&XuiLuaStyle> {
        self.get(lua_class, None)
    }

    /// zombie.scripting.ui.XuiSkin.Load for the skin at position `at` of `skins` (insertion order): once; each import
    /// is loaded first and gives its script's colours, then the default entity style and default Lua styles of the
    /// imports' scripts and this skin's script, then their entity styles and named Lua styles. `Err` is the exception
    /// that ends XuiManager.ParseScripts (an unknown import, an unregistered var key).
    pub fn load(
        skins: &mut JHashMap<XuiSkin>,
        at: usize,
        ctx: &SkinContext,
        log: &mut Vec<String>,
    ) -> Result<(), String> {
        if skins.entry_at(at).1.has_loaded {
            return Ok(());
        }
        skins.value_at_mut(at).has_loaded = true;
        let script_index = skins.entry_at(at).1.script;
        let script = &ctx.scripts[script_index];
        let import_script = |name: &str| -> Result<&XuiSkinScript, String> {
            (ctx.get_skin_script)(name)
                .map(|i| &ctx.scripts[i])
                .ok_or_else(|| "java.lang.NullPointerException".to_string())
        };
        for import_name in &script.imports {
            // XuiManager.GetSkin: xuiSkins.get, else xuiDefaultSkin, which ParseScripts sets only after loading.
            let Some(j) = skins.index_of(import_name) else {
                log.push(format!("Skin not found: {import_name}"));
                return Err(format!(
                    "java.lang.Exception: Import skin '{import_name}' not found for skin: {}",
                    skins.entry_at(at).1.name
                ));
            };
            let found = (ctx.get_skin_script)(import_name);
            XuiSkin::load(skins, j, ctx, log)?;
            let imported = skins.entry_at(j).1.name.clone();
            let me = skins.value_at_mut(at);
            me.imports.push(imported);
            let import = found
                .map(|i| &ctx.scripts[i])
                .ok_or_else(|| "java.lang.NullPointerException".to_string())?;
            me.load_colors(import);
        }
        let me = skins.value_at_mut(at);
        me.load_colors(script);
        for import_name in &script.imports {
            let import = import_script(import_name)?;
            me.load_default_entity_ui_info(import, log);
            me.load_all_default_styles(import, ctx, log)?;
        }
        me.load_default_entity_ui_info(script, log);
        me.load_all_default_styles(script, ctx, log)?;
        for import_name in &script.imports {
            let import = import_script(import_name)?;
            me.load_entity_ui_info(import, log);
            me.load_all_styles(import, ctx, log)?;
        }
        me.load_entity_ui_info(script, log);
        me.load_all_styles(script, ctx, log)
    }

    /// zombie.scripting.ui.XuiSkin.LoadDefaultEntityUiInfo
    fn load_default_entity_ui_info(&mut self, script: &XuiSkinScript, log: &mut Vec<String>) {
        self.default_entity_ui_style
            .load(&script.default_entity_ui_script, log);
    }

    /// zombie.scripting.ui.XuiSkin.LoadEntityUiInfo: a new entity style starts as a copy of the default one.
    fn load_entity_ui_info(&mut self, script: &XuiSkinScript, log: &mut Vec<String>) {
        for (alias, s) in script.entity_ui_script_map.iter() {
            if !self.entity_ui_style_map.contains_key(alias) {
                let copy = self.default_entity_ui_style.clone();
                self.entity_ui_style_map.put(alias, copy);
            }
            if let Some(info) = self.entity_ui_style_map.get_mut(alias) {
                info.load(s, log);
            }
        }
    }

    /// zombie.scripting.ui.XuiSkin.LoadColors: `colorMap.putAll`.
    fn load_colors(&mut self, script: &XuiSkinScript) {
        for (k, v) in script.colors_script.color_map.iter() {
            self.color_map.put(k, *v);
        }
    }

    /// zombie.scripting.ui.XuiSkin.LoadAllDefaultStyles
    fn load_all_default_styles(
        &mut self,
        script: &XuiSkinScript,
        ctx: &SkinContext,
        log: &mut Vec<String>,
    ) -> Result<(), String> {
        for (lua_class, s) in script.style_info_map.iter() {
            self.load_default_style(lua_class, s, ctx, log)?;
        }
        Ok(())
    }

    /// zombie.scripting.ui.XuiSkin.LoadDefaultStyle: the class's default style ("default"), made on first use, takes
    /// every key of the script's unnamed block.
    fn load_default_style(
        &mut self,
        lua_class: &str,
        script: &StyleInfoScript,
        ctx: &SkinContext,
        log: &mut Vec<String>,
    ) -> Result<(), String> {
        let XuiSkin {
            color_map,
            styles,
            name,
            ..
        } = self;
        if !styles.contains_key(lua_class) {
            styles.put(lua_class, StyleInfo::default());
        }
        let Some(info) = styles.get_mut(lua_class) else {
            return Ok(());
        };
        let style = info.default_style.get_or_insert_with(|| {
            let mut s = XuiLuaStyle::new(lua_class, "default");
            s.xui_skin = Some(name.clone());
            s
        });
        let lookup = ColorLookup {
            skin_colors: Some(color_map),
            colors: ctx.colors,
        };
        for (k, v) in script.default_style_block.iter() {
            if !style.load_var(k, v.as_deref(), ctx.registry, &lookup, log)? {
                log.push(format!(
                    "Cannot load key = {k}, val = {}, for class = {lua_class}",
                    v.as_deref().unwrap_or("null")
                ));
            }
        }
        Ok(())
    }

    /// zombie.scripting.ui.XuiSkin.LoadAllStyles
    fn load_all_styles(
        &mut self,
        script: &XuiSkinScript,
        ctx: &SkinContext,
        log: &mut Vec<String>,
    ) -> Result<(), String> {
        for (lua_class, s) in script.style_info_map.iter() {
            self.load_style(lua_class, s, ctx, log)?;
        }
        Ok(())
    }

    /// zombie.scripting.ui.XuiSkin.LoadStyle: each named style, made on first use as a copy of the class's default
    /// style's vars, takes the keys of its block.
    fn load_style(
        &mut self,
        lua_class: &str,
        script: &StyleInfoScript,
        ctx: &SkinContext,
        log: &mut Vec<String>,
    ) -> Result<(), String> {
        let XuiSkin {
            color_map,
            styles,
            name,
            ..
        } = self;
        if !styles.contains_key(lua_class) {
            styles.put(lua_class, StyleInfo::default());
        }
        let Some(info) = styles.get_mut(lua_class) else {
            return Ok(());
        };
        let lookup = ColorLookup {
            skin_colors: Some(color_map),
            colors: ctx.colors,
        };
        for (alias, map) in script.style_blocks.iter() {
            if info.get_style(alias).is_none() {
                let mut s = XuiLuaStyle::new(lua_class, alias);
                s.xui_skin = Some(name.clone());
                if let Some(d) = &info.default_style {
                    s.copy_vars_from(d)?;
                }
                info.styles.put(alias, s);
            }
            let Some(style) = info.styles.get_mut(alias) else {
                continue;
            };
            for (k, v) in map.iter() {
                if !style.load_var(k, v.as_deref(), ctx.registry, &lookup, log)? {
                    log.push(format!(
                        "Cannot load key = {k}, val = {}, for class = {lua_class}",
                        v.as_deref().unwrap_or("null")
                    ));
                }
            }
        }
        Ok(())
    }
}
