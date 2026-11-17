//! zombie.characters.action.ActionState
use std::path::Path;

use super::action_transition::ActionTransition;
use crate::java;
use crate::java_collator;
use crate::zombie::core::skinnedmodel::advancedanimation::animation_variable_source::{
    AnimationVariableHandlePool, AnimationVariableSource,
};
use crate::zombie::util::pz_xml_util::{parse_xml_resolved, XmlElement};
use crate::zombie::util::string_utils;
use crate::zombie::zomboid_file_system::{list_files, ZomboidFileSystem};

/// zombie.characters.action.ActionState: a state of an action group with its transitions, tags, child tags and
/// read-only state variables. `parentActionGroup` is the group that holds it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActionState {
    pub name: String,
    pub transitions: Vec<ActionTransition>,
    pub tags: Option<Vec<String>>,
    pub child_tags: Option<Vec<String>>,
    pub is_grappler_state: bool,
    pub state_variables: AnimationVariableSource,
}

/// zombie.util.list.PZArrayUtil.concat: null when both are empty, else the non-empty one, else both joined.
fn concat(a: Option<Vec<String>>, b: Vec<String>) -> Option<Vec<String>> {
    let a_empty = a.as_ref().is_none_or(Vec::is_empty);
    if a_empty && b.is_empty() {
        None
    } else if a_empty {
        Some(b)
    } else if b.is_empty() {
        a
    } else {
        let mut a = a.unwrap_or_default();
        a.extend(b);
        Some(a)
    }
}

/// What `ActionState.parse` needs besides the state: the file system, the handle pool and the log.
pub struct LoadContext<'a> {
    pub fs: &'a ZomboidFileSystem,
    pub pool: &'a mut AnimationVariableHandlePool,
    pub log: &'a mut Vec<String>,
}

impl ActionState {
    /// zombie.characters.action.ActionState(String)
    pub fn new(name: &str) -> ActionState {
        ActionState {
            name: name.to_string(),
            ..ActionState::default()
        }
    }

    /// zombie.characters.action.ActionState.load: the entries of the folder whose lower-cased name ends in ".xml",
    /// sorted by `Collator.getInstance(Locale.ENGLISH)`, parsed in that order, then the transitions sorted. `Err`
    /// where a name is outside the collator's measured range (not ported).
    pub fn load(&mut self, state_folder_path: &Path, ctx: &mut LoadContext) -> Result<(), String> {
        let Some(list) = list_files(state_folder_path) else {
            return Ok(());
        };
        let mut files: Vec<(String, std::path::PathBuf)> = list
            .into_iter()
            .filter_map(|p| {
                let name = p.file_name()?.to_string_lossy().into_owned();
                java::to_lower_case(&name)
                    .ends_with(".xml")
                    .then_some((name, p))
            })
            .collect();
        for (name, _) in &files {
            java_collator::compare_english(name, name)
                .ok_or_else(|| format!("Collator order of {name:?} is not ported"))?;
        }
        files.sort_by(|a, b| {
            java_collator::compare_english(&a.0, &b.0).unwrap_or(std::cmp::Ordering::Equal)
        });
        for (_, file) in files {
            self.parse(&file, ctx);
        }
        self.sort_transitions();
        Ok(())
    }

    /// zombie.characters.action.ActionState.parse: the file (`PZXmlUtil.parseXml`); an `ActionState` root sets the
    /// grappler flag and variables; a transition file adds its transitions (but those back to this state, which it
    /// warns about); a `tags` / `childTags` file appends its tags. Errors are logged.
    pub fn parse(&mut self, file: &Path, ctx: &mut LoadContext) {
        let file_path = file.to_string_lossy().replace('\\', "/");
        let root = match parse_xml_resolved(ctx.fs, &file_path) {
            Ok(r) => r,
            Err(e) => {
                ctx.log.push(format!("Error loading: {file_path}: {e}"));
                return;
            }
        };
        if root.name == "ActionState" {
            self.parse_action_state(&root, ctx);
            return;
        }
        let mut loaded = Vec::new();
        match ActionTransition::parse(&root, &mut loaded, ctx.log) {
            Ok(true) => {
                loaded.retain(|t| {
                    let back = t.transition_to.as_deref() == Some(self.name.as_str());
                    if back {
                        ctx.log.push(format!(
                            "Canceled loading wrong transition from {0} to {0} in file {file_path}",
                            self.name
                        ));
                    }
                    !back
                });
                self.transitions.extend(loaded);
                return;
            }
            Ok(false) => {}
            Err(e) => {
                ctx.log.push(format!("Error loading: {file_path}: {e}"));
                return;
            }
        }
        if let Some((tags, child_tags)) = parse_tags(&root) {
            self.tags = concat(self.tags.take(), tags);
            self.child_tags = concat(self.child_tags.take(), child_tags);
            return;
        }
        ctx.log.push(format!(
            "Unrecognized xml file. It does not appear to be a transition nor a tag(s). {file_path}"
        ));
    }

    /// zombie.characters.action.ActionState.parseActionState: `isGrapplerState` (`StringUtils.tryParseBoolean`) and
    /// `variables` children, by name ignoring case.
    fn parse_action_state(&mut self, root: &XmlElement, ctx: &mut LoadContext) {
        for child in root.elements() {
            if java::equals_ignore_case("isGrapplerState", &child.name) {
                self.is_grappler_state =
                    string_utils::try_parse_boolean(Some(&child.text_content()));
            }
            if java::equals_ignore_case("variables", &child.name) {
                self.parse_state_variables(child, ctx);
            }
        }
    }

    /// zombie.characters.action.ActionState.parseStateVariables: each `var` child (name ignoring case) with a `key`
    /// attribute sets a variable from its text by its `type` attribute: blank or "string", "bool"
    /// (`StringUtils.tryParseBoolean`), "float" (`StringUtils.tryParseFloat`); then every slot becomes read-only.
    /// "Enum" (`AnimationVariableEnumParser`) is not ported (logged; no game file uses it).
    fn parse_state_variables(&mut self, root: &XmlElement, ctx: &mut LoadContext) {
        for child in root.elements() {
            if !java::equals_ignore_case("var", &child.name) {
                continue;
            }
            let var_key = child.attribute("key");
            if string_utils::is_null_or_whitespace(Some(var_key)) {
                ctx.log
                    .push("Could not parse ActionState variable: missing 'key'".into());
                continue;
            }
            let var_type = child.attribute("type");
            let var_value = child.text_content();
            let vars = &mut self.state_variables;
            if string_utils::is_null_or_whitespace(Some(var_type))
                || java::equals_ignore_case("string", var_type)
            {
                vars.set_variable_string(ctx.pool, var_key, &var_value);
            } else if java::equals_ignore_case("bool", var_type) {
                vars.set_variable_bool(
                    ctx.pool,
                    var_key,
                    string_utils::try_parse_boolean(Some(&var_value)),
                );
            } else if java::equals_ignore_case("float", var_type) {
                vars.set_variable_float(
                    ctx.pool,
                    var_key,
                    string_utils::try_parse_float(Some(&var_value)),
                );
            } else if java::equals_ignore_case("Enum", var_type) {
                ctx.log
                    .push(format!("ActionState Enum variable {var_key} is not ported"));
            }
        }
        for slot in self.state_variables.get_game_variables_mut() {
            slot.read_only = true;
        }
    }

    /// zombie.characters.action.ActionState.canHaveSubState(ActionState parent, ActionState child) for two states that
    /// are not the same one (`parent == child`, by identity, is the caller's check): the parent's child tags overlap the
    /// child's tags.
    pub fn can_have_sub_state(parent: &ActionState, child: &ActionState) -> bool {
        tags_overlap(parent.child_tags.as_deref(), child.tags.as_deref())
    }

    /// zombie.characters.action.ActionState.hasStateVariables
    pub fn has_state_variables(&self) -> bool {
        !self.state_variables.is_empty()
    }

    /// zombie.characters.action.ActionState.sortTransitions: stable, higher condition priority first, then more
    /// conditions first.
    pub fn sort_transitions(&mut self) {
        self.transitions.sort_by(|lhs, rhs| {
            let d = if rhs.condition_priority != lhs.condition_priority {
                rhs.condition_priority.wrapping_sub(lhs.condition_priority)
            } else {
                (rhs.conditions.len() as i32).wrapping_sub(lhs.conditions.len() as i32)
            };
            d.cmp(&0)
        });
    }
}

/// zombie.characters.action.ActionState.tagsOverlap: both lists non-empty and a tag of one equals one of the other
/// (`StringUtils.equalsIgnoreCase`).
pub fn tags_overlap(parent_tags: Option<&[String]>, child_tags: Option<&[String]>) -> bool {
    let (Some(parent_tags), Some(child_tags)) = (parent_tags, child_tags) else {
        return false;
    };
    parent_tags.iter().any(|p| {
        child_tags
            .iter()
            .any(|c| string_utils::equals_ignore_case(Some(p), Some(c)))
    })
}

/// zombie.characters.action.ActionState.parseTags: a `tags` root gives tags, a `childTags` root child tags (the text
/// of each `tag` child); `None` for another root.
fn parse_tags(root: &XmlElement) -> Option<(Vec<String>, Vec<String>)> {
    let tag_texts = || -> Vec<String> {
        root.elements()
            .filter(|c| c.name == "tag")
            .map(XmlElement::text_content)
            .collect()
    };
    match root.name.as_str() {
        "tags" => Some((tag_texts(), Vec::new())),
        "childTags" => Some((Vec::new(), tag_texts())),
        _ => None,
    }
}
