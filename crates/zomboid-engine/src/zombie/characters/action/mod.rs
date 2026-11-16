//! Package `zombie.characters.action`: action groups (`media/actiongroups`), the state machines that drive which
//! animation state a character is in: loading (E45) and the runtime (`ActionContext`, `ActionStateContainer`, the
//! conditions).
pub mod action_context;
pub mod action_context_events;
pub mod action_group;
pub mod action_state;
pub mod action_state_container;
pub mod action_transition;
pub mod conditions;
pub mod i_action_condition;
