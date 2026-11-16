//! zombie.scripting.objects.BaseScriptObject: the state every script object shares (module, name, enabled,
//! debugOnly, the loaded bodies and the script version), for the script types ported on top of it.
use crate::java;
use crate::zombie::scripting::entity::components::crafting::craft_recipe::VANILLA_ID;
use crate::zombie::scripting::script_parser::{self, Block};
use crate::zombie::util::hash::pz_hash;

/// zombie.scripting.objects.BaseScriptObject, the ported part.
#[derive(Clone, Debug, PartialEq)]
pub struct BaseScriptObject {
    /// BaseScriptObject.module, by name (`None` for the original's null).
    pub module: Option<String>,
    pub script_object_name: Option<String>,
    /// BaseScriptObject.scriptVersion
    pub script_version: i64,
    pub enabled: bool,
    pub debug_only: bool,
    /// BaseScriptObject.loadedScriptBodies as (mod id, body) pairs.
    pub loaded_script_bodies: Vec<(String, String)>,
}

impl BaseScriptObject {
    /// zombie.scripting.objects.BaseScriptObject(ScriptType): no module or name, enabled, script version -1.
    pub fn new(module: Option<&str>) -> Self {
        BaseScriptObject {
            module: module.map(str::to_string),
            script_object_name: None,
            script_version: -1,
            enabled: true,
            debug_only: false,
            loaded_script_bodies: Vec::new(),
        }
    }

    /// zombie.scripting.objects.BaseScriptObject.InitLoadPP, with ScriptBucket.CreateFromTokenPP's
    /// `addLoadedScriptBody(modId, token)` (vanilla files: "pz-vanilla").
    pub fn init_load_pp(&mut self, name: &str, body: &str) {
        self.script_object_name = Some(name.to_string());
        self.loaded_script_bodies
            .push((VANILLA_ID.to_string(), body.to_string()));
    }

    /// zombie.scripting.objects.BaseScriptObject.getScriptObjectFullType: "module.name". The original throws a
    /// RuntimeException when either is missing; every script a bucket loads has both, and "null" stands in here.
    pub fn get_script_object_full_type(&self) -> String {
        format!(
            "{}.{}",
            self.module.as_deref().unwrap_or("null"),
            self.script_object_name.as_deref().unwrap_or("null")
        )
    }

    /// zombie.scripting.objects.BaseScriptObject.getLoadedScriptBodyCount: mod id and body count as two entries each.
    pub fn get_loaded_script_body_count(&self) -> usize {
        self.loaded_script_bodies.len() * 2
    }

    /// zombie.scripting.objects.BaseScriptObject.calculateScriptVersion: murmur hash 64 of the full type followed by
    /// every (mod id, body) entry.
    pub fn calculate_script_version(&mut self) {
        let mut sb = self.get_script_object_full_type();
        for (m, b) in &self.loaded_script_bodies {
            sb.push_str(m);
            sb.push_str(b);
        }
        self.script_version = pz_hash::murmur_64_str(&sb);
    }

    /// zombie.scripting.objects.BaseScriptObject.LoadCommonBlock (String): the first block of the parsed body. `Err`
    /// is the original's IndexOutOfBoundsException for a body without a block.
    pub fn load_common_block_str(&mut self, body: &str) -> Result<(), String> {
        let parsed = script_parser::parse(body);
        let block = first_child(&parsed)?;
        self.load_common_block(block);
        Ok(())
    }

    /// zombie.scripting.objects.BaseScriptObject.LoadCommonBlock (Block): `enabled` and `debugOnly`, keys in any case.
    pub fn load_common_block(&mut self, block: &Block) {
        for value in &block.values {
            let key = java::trim(value.get_key());
            let val = java::trim(value.get_value());
            if !key.is_empty() && !val.is_empty() {
                if java::equals_ignore_case(key, "enabled") {
                    self.enabled = java::equals_ignore_case(val, "true");
                } else if java::equals_ignore_case(key, "debugOnly") {
                    self.debug_only = java::equals_ignore_case(val, "true");
                }
            }
        }
    }
}

/// `block.children.get(0)`: `Err` is the IndexOutOfBoundsException of an empty list.
pub fn first_child(block: &Block) -> Result<&Block, String> {
    block.children.first().ok_or_else(|| {
        "java.lang.IndexOutOfBoundsException: Index 0 out of bounds for length 0".to_string()
    })
}

/// What ScriptBucket.LoadScripts calls on a script object of a ported type.
pub trait ScriptObject {
    fn base(&self) -> &BaseScriptObject;
    fn base_mut(&mut self) -> &mut BaseScriptObject;
    /// BaseScriptObject.reset: empty unless the type overrides it.
    fn reset(&mut self) {}
    /// BaseScriptObject.Load (String, String): `Err` where the original throws.
    fn load(
        &mut self,
        name: &str,
        body: &str,
        translator: &crate::zombie::core::translator::Translator,
    ) -> Result<(), String>;
}
