//! zombie.scripting.ui.XuiManager: the static XUI state ParseScripts builds from the loaded XUI scripts (the var
//! registry, the game colours, the layout and style registrations, the skins), as one value.
use super::xui_lua_style::{self, XuiVar};
use super::xui_script_type::XuiScriptType;
use super::xui_skin::{SkinContext, XuiSkin};
use super::JHashMap;
use crate::zombie::core::colors::{ColorSet, Colors};
use crate::zombie::scripting::objects::xui_colors_script::XuiColorsScript;
use crate::zombie::scripting::objects::xui_config_script::XuiConfigScript;
use crate::zombie::scripting::objects::xui_layout_script::XuiLayoutScript;
use crate::zombie::scripting::objects::xui_skin_script::XuiSkinScript;

/// zombie.scripting.ui.XuiManager.DEFAULT_SKIN_NAME
pub const DEFAULT_SKIN_NAME: &str = "default";

/// zombie.scripting.ui.XuiManager.XUI_SCRIPT_TYPES: the ScriptType tags (EnumSet order: XuiLayout, XuiStyle,
/// XuiDefaultStyle, XuiColor, XuiSkin, XuiConfig by ordinal).
pub const XUI_SCRIPT_TYPES: [&str; 6] = [
    "XuiLayout",
    "XuiStyle",
    "XuiDefaultStyle",
    "XuiColor",
    "XuiSkin",
    "XuiConfig",
];

/// zombie.scripting.ui.XuiManager's static fields, plus XuiLuaStyle.varRegisteryMap (which ParseScripts resets and
/// fills). The XuiScript lists and maps hold layout names: XuiScript is not ported, so `registerLayout` registers
/// nothing (the vanilla scripts have no layout, style or default style).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct XuiManager {
    /// name -> index into ScriptManager's xuiLayout list
    pub layout_scripts_map: JHashMap<usize>,
    /// name -> index into ScriptManager's xuiStyle list
    pub styles_scripts_map: JHashMap<usize>,
    /// name -> index into ScriptManager's xuiDefaultStyle list
    pub default_styles_scripts_map: JHashMap<usize>,
    pub combined_list: Vec<String>,
    pub xui_layouts_list: Vec<String>,
    pub xui_styles_list: Vec<String>,
    pub xui_default_styles_list: Vec<String>,
    pub xui_layouts: JHashMap<String>,
    pub xui_styles: JHashMap<String>,
    pub xui_default_styles: JHashMap<String>,
    pub xui_skins: JHashMap<XuiSkin>,
    /// XuiManager.xuiDefaultSkin, by name.
    pub xui_default_skin: Option<String>,
    pub parse_once: bool,
    pub has_parsed_once: bool,
    /// zombie.scripting.ui.XuiLuaStyle.varRegisteryMap
    pub var_registery_map: JHashMap<XuiVar>,
    /// The warnings and printed exceptions of ParseScripts.
    pub log: Vec<String>,
}

/// The ScriptManager lists XuiManager.ParseScripts reads (getAllXuiConfigScripts, getAllXuiColors,
/// getAllXuiStyles, getAllXuiDefaultStyles, getAllXuiLayouts, getAllXuiSkinScripts).
pub struct XuiScripts<'a> {
    pub configs: &'a [XuiConfigScript],
    pub colors: &'a [XuiColorsScript],
    pub styles: &'a mut [XuiLayoutScript],
    pub default_styles: &'a mut [XuiLayoutScript],
    pub layouts: &'a mut [XuiLayoutScript],
    pub skins: &'a [XuiSkinScript],
    /// ScriptManager.getXuiSkinScript: index into `skins`.
    pub get_skin_script: &'a dyn Fn(&str) -> Option<usize>,
}

impl XuiManager {
    /// zombie.scripting.ui.XuiManager.getDefaultSkinName
    pub fn get_default_skin_name() -> &'static str {
        DEFAULT_SKIN_NAME
    }

    /// zombie.scripting.ui.XuiManager.GetLayoutScript: index into ScriptManager's xuiLayout list.
    pub fn get_layout_script(&self, name: Option<&str>) -> Option<usize> {
        self.layout_scripts_map.get(name?).copied()
    }

    /// zombie.scripting.ui.XuiManager.GetStyleScript: index into ScriptManager's xuiStyle list.
    pub fn get_style_script(&self, name: Option<&str>) -> Option<usize> {
        self.styles_scripts_map.get(name?).copied()
    }

    /// zombie.scripting.ui.XuiManager.GetDefaultStyleScript: index into ScriptManager's xuiDefaultStyle list.
    pub fn get_default_style_script(&self, name: Option<&str>) -> Option<usize> {
        self.default_styles_scripts_map.get(name?).copied()
    }

    /// zombie.scripting.ui.XuiManager.GetDefaultSkin
    pub fn get_default_skin(&self) -> Option<&XuiSkin> {
        self.xui_skins.get(self.xui_default_skin.as_deref()?)
    }

    /// zombie.scripting.ui.XuiManager.GetSkin: the skin of that name, else (with a warning for a name) the default
    /// skin.
    pub fn get_skin(&self, name: Option<&str>) -> Option<&XuiSkin> {
        match name.and_then(|n| self.xui_skins.get(n)) {
            Some(s) => Some(s),
            None => self.get_default_skin(),
        }
    }

    /// zombie.scripting.ui.XuiManager.reset, with XuiLuaStyle.Reset. The original marks the old skins invalidated
    /// for the Lua objects that still hold them; here they are dropped.
    fn reset(&mut self) {
        self.layout_scripts_map.clear();
        self.styles_scripts_map.clear();
        self.default_styles_scripts_map.clear();
        self.combined_list.clear();
        self.xui_layouts_list.clear();
        self.xui_styles_list.clear();
        self.xui_default_styles_list.clear();
        self.xui_layouts.clear();
        self.xui_styles.clear();
        self.xui_default_styles.clear();
        self.xui_skins.clear();
        self.xui_default_skin = None;
        self.var_registery_map.clear();
    }

    /// zombie.scripting.ui.XuiManager.setParseOnce
    pub fn set_parse_once(&mut self, b: bool) {
        self.parse_once = b;
        self.has_parsed_once = false;
    }

    /// zombie.scripting.ui.XuiManager.ParseScripts: once per load while `parseOnce` is set. The var registry from
    /// the configs; the xuiGlobalColors into zombie.core.Colors (a game colour takes the values, an unknown name
    /// becomes a game colour); every default style, style and layout script registered and parsed; a skin per
    /// xuiSkin script of module Base, each loaded in the HashMap order of their names; the default skin. `Err` is the
    /// exception that leaves ParseScripts (ScriptManager.loadScripts catches it).
    pub fn parse_scripts(&mut self, s: XuiScripts, colors: &mut Colors) -> Result<(), String> {
        if self.parse_once && self.has_parsed_once {
            return Ok(());
        }
        self.has_parsed_once = true;
        self.reset();
        xui_lua_style::read_configs(&mut self.var_registery_map, s.configs)?;
        for colors_script in s.colors {
            for (key, value) in colors_script.color_map.iter() {
                if let Some(nfo) = colors.get_color_info(key) {
                    if nfo.color_set == ColorSet::Game {
                        let i = nfo.color;
                        colors.set_color(i, *value);
                        continue;
                    }
                }
                if colors.get_color_by_name(key).is_none() {
                    colors.add_game_color(key, *value);
                } else {
                    self.log
                        .push(format!("Color '{key}' is already defined in Colors.java"));
                }
            }
        }
        for (i, script) in s.default_styles.iter_mut().enumerate() {
            if let Some(name) = &script.name {
                self.default_styles_scripts_map.put(name, i);
            }
            self.register_layout(script);
        }
        for (i, script) in s.styles.iter_mut().enumerate() {
            if let Some(name) = &script.name {
                self.styles_scripts_map.put(name, i);
            }
            self.register_layout(script);
        }
        for (i, script) in s.layouts.iter_mut().enumerate() {
            if let Some(name) = &script.name {
                self.layout_scripts_map.put(name, i);
            }
            self.register_layout(script);
        }
        for script in s
            .default_styles
            .iter_mut()
            .chain(s.styles.iter_mut())
            .chain(s.layouts.iter_mut())
        {
            self.parse_layout(script);
        }
        self.combined_list
            .extend(self.xui_layouts_list.iter().cloned());
        self.combined_list
            .extend(self.xui_styles_list.iter().cloned());
        self.combined_list
            .extend(self.xui_default_styles_list.iter().cloned());
        for (i, script) in s.skins.iter().enumerate() {
            let name = script.base.script_object_name.clone().unwrap_or_default();
            if script.base.module.as_deref() != Some("Base") {
                self.log.push(format!(
                    "XuiSkin '{}' ignored, skin needs to be module Base.",
                    script.base.get_script_object_full_type()
                ));
            } else {
                self.xui_skins.put(&name, XuiSkin::new(&name, i));
            }
        }
        let XuiManager {
            xui_skins,
            var_registery_map,
            log,
            ..
        } = self;
        let ctx = SkinContext {
            scripts: s.skins,
            get_skin_script: s.get_skin_script,
            registry: var_registery_map,
            colors,
        };
        for i in xui_skins.java_order() {
            XuiSkin::load(xui_skins, i, &ctx, log)?;
        }
        // GetSkin("default"): xuiSkins.get, else xuiDefaultSkin (still null here).
        self.xui_default_skin = self
            .xui_skins
            .get(DEFAULT_SKIN_NAME)
            .map(|s| s.name.clone());
        Ok(())
    }

    /// zombie.scripting.ui.XuiManager.registerLayout: XuiLayoutScript.preParse, then the XuiScript into the list and
    /// map of its type. XuiScript is not ported: preParse fails, and the exception is logged as the original logs
    /// one.
    fn register_layout(&mut self, layout: &mut XuiLayoutScript) {
        match layout.pre_parse() {
            Ok(()) => {
                // A parsed XuiScript of the layout's type would be listed here.
                let name = layout.name.clone().unwrap_or_default();
                match layout.script_type {
                    XuiScriptType::Layout => {
                        self.xui_layouts_list.push(name.clone());
                        self.xui_layouts.put(&name, name.clone());
                    }
                    XuiScriptType::Style => {
                        self.xui_styles_list.push(name.clone());
                        self.xui_styles.put(&name, name.clone());
                    }
                    XuiScriptType::DefaultStyle => {
                        self.xui_default_styles_list.push(name.clone());
                        self.xui_default_styles.put(&name, name.clone());
                    }
                    XuiScriptType::Reference => {}
                }
            }
            Err(e) => self.log.push(e),
        }
    }

    /// zombie.scripting.ui.XuiManager.parseLayout: XuiLayoutScript.parseScript, its exception logged.
    fn parse_layout(&mut self, layout: &mut XuiLayoutScript) {
        if let Err(e) = layout.parse_script() {
            self.log.push(e);
        }
    }
}
