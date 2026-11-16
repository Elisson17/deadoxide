//! zombie.scripting.objects.XuiColorsScript: named colours (`xuiGlobalColors` scripts, and the `colors` block of a
//! skin script).
use super::base_script_object::{first_child, BaseScriptObject, ScriptObject};
use crate::java;
use crate::zombie::core::color::Color;
use crate::zombie::core::translator::Translator;
use crate::zombie::scripting::script_parser::{self, Block};
use crate::zombie::scripting::ui::JHashMap;

/// zombie.scripting.objects.XuiColorsScript
#[derive(Clone, Debug, PartialEq)]
pub struct XuiColorsScript {
    pub base: BaseScriptObject,
    pub name: Option<String>,
    pub color_map: JHashMap<Color>,
}

/// Float.parseFloat; `Err` is the NumberFormatException.
fn parse_float(s: &str) -> Result<f32, String> {
    java::parse_float(s).ok_or_else(|| {
        format!(
            "java.lang.NumberFormatException: {}",
            java::number_format_message(s)
        )
    })
}

impl XuiColorsScript {
    /// zombie.scripting.objects.XuiColorsScript()
    pub fn new(module: Option<&str>) -> XuiColorsScript {
        XuiColorsScript {
            base: BaseScriptObject::new(module),
            name: None,
            color_map: JHashMap::new(),
        }
    }

    /// zombie.scripting.objects.XuiColorsScript.LoadColorsBlock: `name = r:g:b[:a]` (0..1) or `rgb:r:g:b[:a]`
    /// (0..255) on a Color() (black, alpha 1); a later key replaces an earlier one. `Err` is the
    /// NumberFormatException of a channel.
    pub fn load_colors_block(&mut self, block: &Block) -> Result<(), String> {
        for value in &block.values {
            let key = java::trim(value.get_key());
            let val = java::trim(value.get_value());
            if !key.is_empty() && !val.is_empty() {
                let mut color = Color::default();
                let split = java::split_char(val, ':');
                if split.len() > 1 && java::equals_ignore_case(java::trim(split[0]), "rgb") {
                    for (i, s) in split.iter().enumerate().skip(1) {
                        match i {
                            1 => color.r = parse_float(java::trim(s))? / 255.0,
                            2 => color.g = parse_float(java::trim(s))? / 255.0,
                            3 => color.b = parse_float(java::trim(s))? / 255.0,
                            4 => color.a = parse_float(java::trim(s))? / 255.0,
                            _ => {}
                        }
                    }
                } else {
                    for (i, s) in split.iter().enumerate() {
                        match i {
                            0 => color.r = parse_float(java::trim(s))?,
                            1 => color.g = parse_float(java::trim(s))?,
                            2 => color.b = parse_float(java::trim(s))?,
                            3 => color.a = parse_float(java::trim(s))?,
                            _ => {}
                        }
                    }
                }
                self.color_map.put(key, color);
            }
        }
        Ok(())
    }
}

impl ScriptObject for XuiColorsScript {
    fn base(&self) -> &BaseScriptObject {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseScriptObject {
        &mut self.base
    }

    /// zombie.scripting.objects.XuiColorsScript.Load
    fn load(&mut self, name: &str, body: &str, _translator: &Translator) -> Result<(), String> {
        self.name = Some(name.to_string());
        let parsed = script_parser::parse(body);
        let block = first_child(&parsed)?;
        self.base.load_common_block(block);
        self.load_colors_block(block)
    }
}
