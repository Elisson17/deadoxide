//! zombie.characters.action.ActionTransition
use super::i_action_condition::{create_instance, IActionCondition};
use crate::java;
use crate::zombie::core::math::pz_math;
use crate::zombie::util::pz_xml_util::XmlElement;
use crate::zombie::util::string_utils;

/// zombie.characters.action.ActionTransition: a transition of an action state to another (`transitionTo`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActionTransition {
    pub transition_to: Option<String>,
    pub as_substate: bool,
    pub transition_out: bool,
    pub force_parent: bool,
    pub condition_priority: i32,
    pub conditions: Vec<IActionCondition>,
}

impl ActionTransition {
    /// zombie.characters.action.ActionTransition.parse: a `transitions` root (each `transition` child; others are
    /// warned about) or a single `transition` root replace `transitions`; `Ok(false)` for another root. `Err` is the
    /// exception `PZXmlUtil.forEachElement` lets out of `transitions`.
    pub fn parse(
        root: &XmlElement,
        transitions: &mut Vec<ActionTransition>,
        log: &mut Vec<String>,
    ) -> Result<bool, String> {
        if root.name == "transitions" {
            transitions.clear();
            for child in root.elements() {
                if child.name != "transition" {
                    log.push(format!("Warning: Unrecognised element '{}'", child.name));
                } else {
                    transitions.push(ActionTransition::load(child, log));
                }
            }
            Ok(true)
        } else if root.name == "transition" {
            transitions.clear();
            transitions.push(ActionTransition::load(root, log));
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// zombie.characters.action.ActionTransition.load: each child element by name ignoring case: `transitionTo` (the
    /// text content), `transitionOut`, `forceParent`, `asSubstate` (`StringUtils.tryParseBoolean`),
    /// `conditionPriority` (`PZMath.tryParseInt`, 0), `conditions` (each child through `IActionCondition.createInstance`,
    /// nulls skipped). An exception while reading a child is logged and ends that child (the original's catch).
    fn load(e: &XmlElement, log: &mut Vec<String>) -> ActionTransition {
        let mut t = ActionTransition::default();
        for child in e.elements() {
            let s = child.name.as_str();
            let is = |n: &str| java::equals_ignore_case(n, s);
            if is("transitionTo") {
                t.transition_to = Some(child.text_content());
            } else if is("transitionOut") {
                t.transition_out = string_utils::try_parse_boolean(Some(&child.text_content()));
            } else if is("forceParent") {
                t.force_parent = string_utils::try_parse_boolean(Some(&child.text_content()));
            } else if is("asSubstate") {
                t.as_substate = string_utils::try_parse_boolean(Some(&child.text_content()));
            } else if is("conditionPriority") {
                t.condition_priority = pz_math::try_parse_int(Some(&child.text_content()), 0);
            } else if is("conditions") {
                for condition_node in child.elements() {
                    match create_instance(condition_node) {
                        Ok(Some(c)) => t.conditions.push(c),
                        Ok(None) => {}
                        Err(err) => {
                            log.push(format!("Error while parsing xml element: {s}: {err}"));
                            break;
                        }
                    }
                }
            }
        }
        t
    }
}
