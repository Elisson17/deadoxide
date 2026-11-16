//! Package `zombie.characters.action`: action groups (`media/actiongroups`), the state machines that drive which
//! animation state a character is in. Loading is ported; the runtime (`ActionContext`, `ActionStateContainer`) is not.
pub mod action_group;
pub mod action_state;
pub mod action_transition;
pub mod conditions;
pub mod i_action_condition;
