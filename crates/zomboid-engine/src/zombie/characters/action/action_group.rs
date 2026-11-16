//! zombie.characters.action.ActionGroup
use std::collections::HashMap;
use std::rc::Rc;

use super::action_state::{ActionState, LoadContext};
use crate::java;
use crate::zombie::core::skinnedmodel::advancedanimation::animation_variable_source::AnimationVariableHandlePool;
use crate::zombie::util::pz_xml_util::parse_document;
use crate::zombie::zomboid_file_system::{list_files, ZomboidFileSystem};

/// zombie.characters.action.ActionGroup: the states of `media/actiongroups/<name>` in folder order, both lookups
/// (lower-case name → state index; `String.hashCode` of the name → name) and the initial state's name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActionGroup {
    pub name: String,
    pub initial_state_name: Option<String>,
    pub states: Vec<ActionState>,
    pub state_lookup: HashMap<String, usize>,
    pub state_name_lookup: HashMap<i32, String>,
}

/// The statics of the original: `ActionGroup.s_actionGroupMap`, the variable handle pool, and the log the loaders
/// write to.
#[derive(Default)]
pub struct ActionGroups {
    pub action_group_map: HashMap<String, ActionGroup>,
    pub pool: AnimationVariableHandlePool,
    pub log: Vec<String>,
    /// The groups as shared values for the `ActionContext`s that hold them (the original shares the one object).
    pub shared: HashMap<String, Rc<ActionGroup>>,
}

impl ActionGroups {
    /// `getActionGroup` as the shared object an `ActionContext` keeps: the same `Rc` for the same lower-cased name.
    pub fn get_action_group_rc(
        &mut self,
        fs: &ZomboidFileSystem,
        group_name: &str,
    ) -> Rc<ActionGroup> {
        let key = java::to_lower_case(group_name);
        if let Some(g) = self.shared.get(&key) {
            return Rc::clone(g);
        }
        let g = Rc::new(self.get_action_group(fs, group_name).clone());
        self.shared.insert(key, Rc::clone(&g));
        g
    }

    /// zombie.characters.action.ActionGroup.getActionGroup: the group of the lower-cased name, loaded and stored on
    /// first use (an error while loading is logged; the group keeps what it loaded).
    pub fn get_action_group(&mut self, fs: &ZomboidFileSystem, group_name: &str) -> &ActionGroup {
        let group_name = java::to_lower_case(group_name);
        if !self.action_group_map.contains_key(&group_name) {
            let mut grp = ActionGroup {
                name: group_name.clone(),
                ..ActionGroup::default()
            };
            let mut ctx = LoadContext {
                fs,
                pool: &mut self.pool,
                log: &mut self.log,
            };
            if let Err(e) = grp.load(&mut ctx) {
                ctx.log
                    .push(format!("Error loading action group: {group_name}: {e}"));
            }
            self.action_group_map.insert(group_name.clone(), grp);
        }
        &self.action_group_map[&group_name]
    }
}

impl ActionGroup {
    /// zombie.characters.action.ActionGroup.load: `actionGroup.xml` (`loadGroupData`) when readable, then each folder
    /// of the group (`File.listFiles` order) as a state (`getOrCreate`, then `ActionState.load`).
    fn load(&mut self, ctx: &mut LoadContext) -> Result<(), String> {
        let action_group_file = ctx
            .fs
            .get_media_file(&format!("actiongroups/{}/actionGroup.xml", self.name));
        if action_group_file.exists() {
            self.load_group_data(&action_group_file, ctx.log);
        }
        let folder = ctx
            .fs
            .get_media_file(&format!("actiongroups/{}", self.name));
        if let Some(list) = list_files(&folder) {
            for state_folder in list {
                if state_folder.is_dir() {
                    let name = state_folder
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let i = self.get_or_create(&name);
                    self.states[i].load(&state_folder, ctx)?;
                }
            }
        }
        Ok(())
    }

    /// zombie.characters.action.ActionGroup.loadGroupData: the file (`DocumentBuilder.parse`, no includes) must have an
    /// `actiongroup` root; the trimmed text of its last `initial` child is the initial state's name.
    fn load_group_data(&mut self, file: &std::path::Path, log: &mut Vec<String>) {
        let root = match std::fs::read(file)
            .map_err(|e| e.to_string())
            .and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))
            .and_then(|t| parse_document(t.strip_prefix('\u{feff}').unwrap_or(&t)))
        {
            Ok(r) => r,
            Err(e) => {
                log.push(format!("Error loading: {}: {e}", file.display()));
                return;
            }
        };
        if root.name != "actiongroup" {
            log.push(format!(
                "Error loading: {}, expected root element '<actiongroup>', received '<{}>'",
                file.display(),
                root.name
            ));
            return;
        }
        for child in root.elements() {
            if child.name == "initial" {
                self.initial_state_name = Some(java::trim(&child.text_content()).to_string());
            }
        }
    }

    /// zombie.characters.action.ActionGroup.addState: appended, with both lookups (a state already present by
    /// identity is not added again; here states are only added through `getOrCreate`).
    fn add_state(&mut self, state: ActionState) -> usize {
        self.state_lookup
            .insert(java::to_lower_case(&state.name), self.states.len());
        self.state_name_lookup
            .insert(java::string_hash_code(&state.name), state.name.clone());
        self.states.push(state);
        self.states.len() - 1
    }

    /// zombie.characters.action.ActionGroup.findState: by lower-cased name.
    pub fn find_state(&self, state_name: &str) -> Option<usize> {
        self.state_lookup
            .get(&java::to_lower_case(state_name))
            .copied()
    }

    /// zombie.characters.action.ActionGroup.getOrCreate: the state of the lower-cased name, created when missing.
    pub fn get_or_create(&mut self, state_name: &str) -> usize {
        let state_name = java::to_lower_case(state_name);
        match self.find_state(&state_name) {
            Some(i) => i,
            None => self.add_state(ActionState::new(&state_name)),
        }
    }

    /// zombie.characters.action.ActionGroup.getInitialState as an index: the state named by `initial`, else the first
    /// state.
    pub fn get_initial_state_index(&self) -> Option<usize> {
        self.initial_state_name
            .as_deref()
            .and_then(|n| self.find_state(n))
            .or(if self.states.is_empty() {
                None
            } else {
                Some(0)
            })
    }

    /// zombie.characters.action.ActionGroup.getInitialState: the state named by `initial`, else the first state.
    pub fn get_initial_state(&self) -> Option<&ActionState> {
        self.initial_state_name
            .as_deref()
            .and_then(|n| self.find_state(n))
            .or(if self.states.is_empty() {
                None
            } else {
                Some(0)
            })
            .map(|i| &self.states[i])
    }
}
