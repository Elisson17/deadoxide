//! zombie.characters.action.ActionContext
use std::rc::Rc;

use super::action_context_events::ActionContextEvents;
use super::action_group::ActionGroup;
use super::action_state::ActionState;
use super::action_state_container::{default_state_name, ActionStateContainer, Eval, StateRef};
use crate::zombie::core::skinnedmodel::advancedanimation::animation_variable_source::AnimationVariableSlot;
use crate::zombie::core::skinnedmodel::advancedanimation::i_animation_variable_source::IAnimationVariableSource;

/// `ActionContext.actionStateHistoryMaxSize`
const ACTION_STATE_HISTORY_MAX_SIZE: usize = 6;

/// zombie.characters.action.ActionContext: the state of a character in an action group, the previous states, and the
/// events reported since the last update. The owner is not held: what `getOwner()` serves (the character's variables)
/// is passed to each call that reads it. The callbacks (`onStateChanged`, the zombie's network AI) are the caller's
/// to run when `update` says the states changed. `GameClient.client` is false, so the state manager's enter/exit
/// calls and the profiling/recording are not here.
#[derive(Clone, Debug, Default)]
pub struct ActionContext {
    groups: Vec<Rc<ActionGroup>>,
    action_group: Option<usize>,
    action_state_history: Vec<StateRef>,
    previous_action_state_container: ActionStateContainer,
    action_state_container: ActionStateContainer,
    next_action_state_container: ActionStateContainer,
    states_changed: bool,
    occurred_anim_events: ActionContextEvents,
}

impl ActionContext {
    pub fn new() -> ActionContext {
        ActionContext::default()
    }

    fn eval<'a>(&'a self, owner: &'a dyn IAnimationVariableSource) -> Eval<'a> {
        Eval {
            groups: &self.groups,
            group: self.action_group.unwrap_or(0),
            events: &self.occurred_anim_events,
            history: &self.action_state_history,
            committed: &self.action_state_container,
            owner,
        }
    }

    /// ActionContext.update: evaluate the transitions of the current state against the owner's variables and the
    /// events, take the new states, clear the events. `true` when the states changed since the last update, which is
    /// when the original calls its `onStateChanged` callbacks (after the events were cleared).
    pub fn update(&mut self, owner: &dyn IAnimationVariableSource) -> bool {
        self.update_internal(owner);
        self.post_update_internal()
    }

    /// ActionContext.updateInternal
    fn update_internal(&mut self, owner: &dyn IAnimationVariableSource) {
        let mut next = std::mem::take(&mut self.next_action_state_container);
        next.set(&self.action_state_container);
        if self.action_group.is_some() {
            next.evaluate_current_state(&self.eval(owner));
        }
        self.next_action_state_container = next;
        self.transfer_action_state();
    }

    /// ActionContext.transferActionState(nextActionStateContainer): when the next container differs, it becomes the
    /// current one (the old one is kept as the previous), the history is updated, and the change is flagged.
    fn transfer_action_state(&mut self) {
        if !self
            .action_state_container
            .equal_to(&self.next_action_state_container)
        {
            self.previous_action_state_container
                .set(&self.action_state_container);
            self.action_state_container
                .set(&self.next_action_state_container);
            self.update_history(
                self.previous_action_state_container.root_state(),
                self.action_state_container.root_state(),
                self.action_state_container.transition_used_for_this_state(),
            );
            self.states_changed = true;
        }
    }

    /// ActionContext.updateHistory: entering a state from another pushes the other; a `transitionOut` back to the state
    /// on top of the history pops it instead (a mismatch clears the history, then pushes).
    fn update_history(
        &mut self,
        previous_state: Option<StateRef>,
        current_state: Option<StateRef>,
        transition_used: Option<bool>,
    ) {
        let Some(current_state) = current_state else {
            return;
        };
        if previous_state == Some(current_state) {
            return;
        }
        let Some(previous_state) = previous_state else {
            self.action_state_history.clear();
            return;
        };
        if !self.action_state_history.is_empty() && transition_used == Some(true) {
            let previous_state_in_history = self.peek_previous_state();
            if previous_state_in_history == Some(current_state) {
                self.pop_previous_state();
                return;
            }
            self.action_state_history.clear();
        }
        self.push_previous_state(Some(previous_state));
    }

    /// ActionContext.postUpdateInternal: the events are cleared, then the callbacks are due when the states changed.
    fn post_update_internal(&mut self) -> bool {
        self.clear_action_context_events();
        std::mem::take(&mut self.states_changed)
    }

    /// ActionContext.peekNextState
    pub fn peek_next_state(&self, owner: &dyn IAnimationVariableSource) -> Option<&ActionState> {
        let r = self
            .action_state_container
            .peek_next_state(&self.eval(owner))?;
        Some(&self.groups[r.group].states[r.state])
    }

    /// ActionContext.canTransitionToState(stateName, allowSubState)
    pub fn can_transition_to_state(
        &self,
        owner: &dyn IAnimationVariableSource,
        state_name: &str,
        allow_sub_state: bool,
    ) -> bool {
        self.action_group.is_some()
            && self.action_state_container.can_transition_to_state(
                &self.eval(owner),
                state_name,
                allow_sub_state,
            )
    }

    /// ActionContext.setCurrentState: the container is cleared and holds just that state.
    pub fn set_current_state(&mut self, next_state: Option<StateRef>) {
        self.next_action_state_container.clear();
        if let Some(next_state) = next_state {
            // `ActionStateContainer.setCurrentState` with no children to drop: just the state
            self.next_action_state_container = {
                let mut c = ActionStateContainer::default();
                let nobody = NoVariables;
                c.set_current_state(&self.eval(&nobody), Some(next_state), None);
                c
            };
        }
        self.transfer_action_state();
    }

    /// ActionContext.setGroup: the group's initial state becomes the current state.
    pub fn set_group(&mut self, group: Rc<ActionGroup>) {
        let idx = match self.groups.iter().position(|g| Rc::ptr_eq(g, &group)) {
            Some(i) => i,
            None => {
                self.groups.push(Rc::clone(&group));
                self.groups.len() - 1
            }
        };
        self.action_group = Some(idx);
        let initial = group
            .get_initial_state_index()
            .map(|state| StateRef { group: idx, state });
        self.set_current_state(initial);
    }

    /// `getGroup().findState(name)` as a state of this context.
    pub fn find_state_ref(&self, name: &str) -> Option<StateRef> {
        let group = self.action_group?;
        self.groups[group]
            .find_state(name)
            .map(|state| StateRef { group, state })
    }

    /// ActionContext.getGroup
    pub fn get_group(&self) -> Option<&ActionGroup> {
        self.action_group.map(|i| &*self.groups[i])
    }

    /// ActionContext.getCurrentState
    pub fn get_current_state(&self) -> Option<&ActionState> {
        self.action_state_container
            .root_state()
            .map(|r| &self.groups[r.group].states[r.state])
    }

    /// ActionContext.getCurrentStateName: the root state's name, else the group's default state's.
    pub fn get_current_state_name(&self) -> String {
        match self
            .action_state_container
            .get_current_state_name(&self.groups)
        {
            Some(n) => n,
            None => self.get_group().map(default_state_name).unwrap_or_default(),
        }
    }

    /// ActionContext.getChildStates: the names of the child states, in order.
    pub fn get_child_state_names(&self) -> Vec<String> {
        self.action_state_container
            .child_states()
            .iter()
            .map(|r| self.groups[r.group].states[r.state].name.clone())
            .collect()
    }

    /// ActionContext.getChildStateAt
    pub fn get_child_state_at(&self, idx: usize) -> Option<&ActionState> {
        self.action_state_container
            .child_states()
            .get(idx)
            .map(|r| &self.groups[r.group].states[r.state])
    }

    /// ActionContext.peekPreviousStateName
    pub fn peek_previous_state_name(&self) -> String {
        match self.peek_previous_state() {
            Some(r) => self.groups[r.group].states[r.state].name.clone(),
            None => self.get_group().map(default_state_name).unwrap_or_default(),
        }
    }

    /// ActionContext.popPreviousState
    fn pop_previous_state(&mut self) -> Option<StateRef> {
        self.action_state_history.pop()
    }

    /// ActionContext.peekPreviousState
    fn peek_previous_state(&self) -> Option<StateRef> {
        self.action_state_history.last().copied()
    }

    /// ActionContext.pushPreviousState: pushed unless it is on top already; the history keeps at most 5 (the oldest
    /// drop while it holds 6).
    fn push_previous_state(&mut self, current_state: Option<StateRef>) {
        if let Some(c) = current_state {
            if self.peek_previous_state() != Some(c) {
                self.action_state_history.push(c);
            }
        }
        while self.action_state_history.len() >= ACTION_STATE_HISTORY_MAX_SIZE {
            self.action_state_history.remove(0);
        }
    }

    /// ActionContext.reportEvent(event)
    pub fn report_event(&mut self, event: &str) {
        self.report_event_for_state(None, event);
    }

    /// ActionContext.reportEvent(state, event)
    pub fn report_event_for_state(&mut self, state: Option<&str>, event: &str) {
        self.occurred_anim_events.add(event, state);
    }

    /// ActionContext.hasEventOccurred(eventName)
    pub fn has_event_occurred(&self, event_name: &str) -> bool {
        self.occurred_anim_events
            .contains_any_layer(event_name, None)
    }

    /// ActionContext.hasEventOccurred(eventName, stateName)
    pub fn has_event_occurred_for_state(&self, event_name: &str, state_name: Option<&str>) -> bool {
        self.occurred_anim_events
            .contains_any_layer(event_name, state_name)
    }

    /// ActionContext.clearEvent
    pub fn clear_event(&mut self, event_name: &str) {
        self.occurred_anim_events.clear_event(event_name);
    }

    /// ActionContext.clearActionContextEvents
    pub fn clear_action_context_events(&mut self) {
        self.occurred_anim_events.clear();
    }

    /// ActionContext.getEvents: the names of the events reported for no state.
    pub fn get_events(&self) -> Vec<String> {
        self.occurred_anim_events.get()
    }

    /// ActionContext.getVariable: the state variable of the current states, if any.
    pub fn get_variable(&self, key: &str) -> Option<AnimationVariableSlot> {
        self.action_state_container.get_variable(&self.groups, key)
    }

    /// ActionContext.hasStateVariables
    pub fn has_state_variables(&self) -> bool {
        self.action_state_container
            .has_state_variables(&self.groups)
    }
}

/// An owner with no variables (for the calls that read none).
struct NoVariables;

impl IAnimationVariableSource for NoVariables {
    fn get_variable(&self, _key: &str) -> Option<AnimationVariableSlot> {
        None
    }
}
