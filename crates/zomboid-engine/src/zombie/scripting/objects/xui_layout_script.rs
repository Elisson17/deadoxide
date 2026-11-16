//! zombie.scripting.objects.XuiLayoutScript: an `xuiLayout`, `xuiStyle` or `xuiDefaultStyle` script, kept as text
//! until XuiManager.ParseScripts parses it into an XuiScript (not ported).
use super::base_script_object::{BaseScriptObject, ScriptObject};
use crate::zombie::core::translator::Translator;
use crate::zombie::scripting::ui::xui_script_type::XuiScriptType;

/// zombie.scripting.objects.XuiLayoutScript
#[derive(Clone, Debug, PartialEq)]
pub struct XuiLayoutScript {
    pub base: BaseScriptObject,
    /// XuiLayoutScript.xuiScript: XuiScript is not ported, so never built.
    pub xui_script: Option<String>,
    pub name: Option<String>,
    pub total_file: Option<String>,
    pub has_parsed: bool,
    pub script_type: XuiScriptType,
    /// XuiLayoutScript.block: whether preParse has kept a parsed block.
    pub block: bool,
}

impl XuiLayoutScript {
    /// zombie.scripting.objects.XuiLayoutScript(ScriptType, XuiScriptType)
    pub fn new(module: Option<&str>, script_type: XuiScriptType) -> XuiLayoutScript {
        XuiLayoutScript {
            base: BaseScriptObject::new(module),
            xui_script: None,
            name: None,
            total_file: None,
            has_parsed: false,
            script_type,
            block: false,
        }
    }

    /// zombie.scripting.objects.XuiLayoutScript.preParse: needs XuiScript.ReadLuaClassValue and
    /// XuiScript.CreateScriptForClass, which are not ported; `Err` says so (XuiManager.registerLayout logs it, as it
    /// logs an exception).
    pub fn pre_parse(&mut self) -> Result<(), String> {
        Err(format!(
            "XuiLayoutScript.preParse: XuiScript is not ported ({})",
            self.name.as_deref().unwrap_or("null")
        ))
    }

    /// zombie.scripting.objects.XuiLayoutScript.parseScript: once; XuiScript.Load of the kept block (`Err` is the
    /// NullPointerException of a script preParse did not build).
    pub fn parse_script(&mut self) -> Result<(), String> {
        if !self.has_parsed {
            self.has_parsed = true;
            if self.xui_script.is_none() {
                return Err("java.lang.NullPointerException: XuiLayoutScript.xuiScript".to_string());
            }
            self.block = false;
        }
        Ok(())
    }
}

impl ScriptObject for XuiLayoutScript {
    fn base(&self) -> &BaseScriptObject {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseScriptObject {
        &mut self.base
    }

    /// zombie.scripting.objects.XuiLayoutScript.Load: the text kept for ParseScripts, `enabled`/`debugOnly` read.
    fn load(&mut self, name: &str, body: &str, _translator: &Translator) -> Result<(), String> {
        self.has_parsed = false;
        self.total_file = Some(body.to_string());
        self.name = Some(name.to_string());
        self.base.load_common_block_str(body)
    }
}
