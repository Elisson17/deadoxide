//! zombie.characters.action.IActionCondition
use super::conditions::character_variable_condition::CharacterVariableCondition;
use crate::zombie::util::pz_xml_util::XmlElement;

/// zombie.characters.action.IActionCondition: the condition classes the factories create.
#[derive(Clone, Debug, PartialEq)]
pub enum IActionCondition {
    CharacterVariable(CharacterVariableCondition),
    /// zombie.characters.action.conditions.EventOccurred (`eventName`)
    EventOccurred(String),
    /// zombie.characters.action.conditions.EventNotOccurred (`eventName`)
    EventNotOccurred(String),
    /// zombie.characters.action.conditions.LuaCall
    LuaCall,
}

/// zombie.characters.action.IActionCondition.createInstance with the factories `ActionContext`'s class initialiser
/// registers (by exact element name): the CharacterVariableCondition names, `eventOccurred` / `eventNotOccurred` (the
/// text content lower-cased) and `lua`. `Ok(None)` is null (no factory, or a factory that declines). `Err` is an
/// exception of a factory.
pub fn create_instance(node: &XmlElement) -> Result<Option<IActionCondition>, String> {
    Ok(match node.name.as_str() {
        "isTrue" | "isFalse" | "compare" | "gtr" | "less" | "equals" | "lessEqual" | "gtrEqual"
        | "notEquals" => {
            CharacterVariableCondition::create(node)?.map(IActionCondition::CharacterVariable)
        }
        "eventOccurred" => Some(IActionCondition::EventOccurred(
            node.text_content().to_lowercase(),
        )),
        "eventNotOccurred" => Some(IActionCondition::EventNotOccurred(
            node.text_content().to_lowercase(),
        )),
        "lua" => Some(IActionCondition::LuaCall),
        _ => None,
    })
}
