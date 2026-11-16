//! zombie.scripting.objects.XuiConfigScript: which style keys are of which var type (`xuiConfig` scripts).
use super::base_script_object::{first_child, BaseScriptObject, ScriptObject};
use crate::java;
use crate::zombie::core::translator::Translator;
use crate::zombie::scripting::script_parser::{self, Block};
use crate::zombie::scripting::ui::xui_lua_style::{self, ALLOWED_VAR_TYPES};
use crate::zombie::scripting::ui::xui_var_type::XuiVarType;
use crate::zombie::util::string_utils;

/// zombie.scripting.objects.XuiConfigScript
#[derive(Clone, Debug, PartialEq)]
pub struct XuiConfigScript {
    pub base: BaseScriptObject,
    /// XuiConfigScript.varConfigs: a HashMap keyed by the enum (identity hash order, UNKNOWN), one list per allowed
    /// type; here in ALLOWED_VAR_TYPES order.
    pub var_configs: Vec<(XuiVarType, Vec<String>)>,
}

impl XuiConfigScript {
    /// zombie.scripting.objects.XuiConfigScript(): an empty key list per allowed var type.
    pub fn new(module: Option<&str>) -> XuiConfigScript {
        XuiConfigScript {
            base: BaseScriptObject::new(module),
            var_configs: ALLOWED_VAR_TYPES.iter().map(|t| (*t, Vec::new())).collect(),
        }
    }

    /// XuiConfigScript.getVarConfigs().get(type)
    pub fn keys(&self, t: XuiVarType) -> Option<&[String]> {
        self.var_configs
            .iter()
            .find(|(k, _)| *k == t)
            .map(|(_, v)| v.as_slice())
    }

    /// zombie.scripting.objects.XuiConfigScript.LoadVarTypeBlock: every non-blank entry as a key of the type, once;
    /// `Err` for a key another type has.
    fn load_var_type_block(&mut self, var_type: XuiVarType, block: &Block) -> Result<(), String> {
        for value in &block.values {
            if !string_utils::is_null_or_whitespace(Some(&value.string)) {
                let key = java::trim(&value.string).to_string();
                if self.other_types_contains_key(&key, var_type) {
                    return Err(format!(
                        "java.lang.Exception: Key '{key}' duplicate from another value block. this var type = {}",
                        var_type.name()
                    ));
                }
                if let Some((_, list)) = self.var_configs.iter_mut().find(|(k, _)| *k == var_type) {
                    if !list.contains(&key) {
                        list.push(key);
                    }
                }
            }
        }
        Ok(())
    }

    /// zombie.scripting.objects.XuiConfigScript.otherTypesContainsKey
    fn other_types_contains_key(&self, key: &str, ignore: XuiVarType) -> bool {
        self.var_configs
            .iter()
            .any(|(t, list)| *t != ignore && list.iter().any(|k| k == key))
    }
}

impl ScriptObject for XuiConfigScript {
    fn base(&self) -> &BaseScriptObject {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseScriptObject {
        &mut self.base
    }

    /// zombie.scripting.objects.XuiConfigScript.reset: every key list emptied.
    fn reset(&mut self) {
        for (_, list) in &mut self.var_configs {
            list.clear();
        }
    }

    /// zombie.scripting.objects.XuiConfigScript.Load: each child block names a var type (XuiVarType.valueOf) and
    /// lists its keys. `Err` for an unknown or disallowed type or a key of two types.
    fn load(&mut self, _name: &str, body: &str, _translator: &Translator) -> Result<(), String> {
        let parsed = script_parser::parse(body);
        let block = first_child(&parsed)?;
        self.base.load_common_block(block);
        for child in &block.children {
            let var_type = XuiVarType::value_of(&child.r#type)?;
            if !xui_lua_style::is_allowed(var_type) {
                return Err(format!(
                    "java.lang.Exception: VarType not allowed: {}",
                    var_type.name()
                ));
            }
            self.load_var_type_block(var_type, child)?;
        }
        Ok(())
    }
}
