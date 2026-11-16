//! zombie.characters.action.ActionStateContainer
use std::rc::Rc;

use super::action_context_events::ActionContextEvents;
use super::action_group::ActionGroup;
use super::action_state::ActionState;
use super::action_transition::ActionTransition;
use crate::zombie::core::skinnedmodel::advancedanimation::animation_variable_source::AnimationVariableSlot;
use crate::zombie::core::skinnedmodel::advancedanimation::i_animation_variable_source::IAnimationVariableSource;
use crate::zombie::util::string_utils;

/// An `ActionState` by identity: the index of the group in the `ActionContext`'s groups and of the state in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateRef {
    pub group: usize,
    pub state: usize,
}

/// What a condition reads of the `ActionContext` it is evaluated for (`context.getOwner()`, `getGroup()`,
/// `hasEventOccurred`, `peekPreviousStateName`): the context's parts, borrowed. Reading a variable goes through the
/// committed container's state variables first, as `IsoGameCharacter.getVariable` asks `getActionContext()` first.
pub struct Eval<'a> {
    pub groups: &'a [Rc<ActionGroup>],
    pub group: usize,
    pub events: &'a ActionContextEvents,
    pub history: &'a [StateRef],
    pub committed: &'a ActionStateContainer,
    pub owner: &'a dyn IAnimationVariableSource,
}

impl<'a> Eval<'a> {
    pub fn state(&self, r: StateRef) -> &'a ActionState {
        &self.groups[r.group].states[r.state]
    }

    pub fn action_group(&self) -> &'a ActionGroup {
        &self.groups[self.group]
    }

    /// ActionContext.peekPreviousStateName: the name of the state last left, else the group's default state.
    pub fn peek_previous_state_name(&self) -> String {
        match self.history.last() {
            Some(r) => self.state(*r).name.clone(),
            None => default_state_name(self.action_group()),
        }
    }

    /// ActionContext.hasEventOccurred(eventName, stateName)
    pub fn has_event_occurred(&self, event_name: &str, state_name: &str) -> bool {
        self.events
            .contains_any_layer(event_name, Some(state_name))
    }
}

/// ActionGroup.getDefaultState().getName(): the initial state's name (an empty group has none; the original would
/// fail with a null pointer there).
pub fn default_state_name(group: &ActionGroup) -> String {
    group
        .get_initial_state_index()
        .map(|i| group.states[i].name.clone())
        .unwrap_or_default()
}

impl IAnimationVariableSource for Eval<'_> {
    fn get_variable(&self, key: &str) -> Option<AnimationVariableSlot> {
        self.committed
            .get_variable(self.groups, key)
            .or_else(|| self.owner.get_variable(key))
    }

    fn get_sub_variable_source(&self, name: &str) -> Option<&dyn IAnimationVariableSource> {
        self.owner.get_sub_variable_source(name)
    }
}

/// zombie.characters.action.ActionStateContainer: the root state, the child (sub) states in order, and the
/// transition that led to the root state (only whether it was a `transitionOut` is read).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActionStateContainer {
    current_state: Option<StateRef>,
    child_states: Vec<StateRef>,
    /// `transitionUsedForThisState`: null, or the used transition's `transitionOut`.
    transition_used: Option<bool>,
}

impl ActionStateContainer {
    pub fn root_state(&self) -> Option<StateRef> {
        self.current_state
    }

    pub fn child_states(&self) -> &[StateRef] {
        &self.child_states
    }

    pub fn transition_used_for_this_state(&self) -> Option<bool> {
        self.transition_used
    }

    /// ActionStateContainer.evaluateCurrentState
    pub fn evaluate_current_state(&mut self, ctx: &Eval) {
        if self.current_state.is_some() {
            self.evaluate_current_state_transitions(ctx);
            self.evaluate_sub_state_transitions(ctx);
        }
    }

    /// ActionStateContainer.getTransitionTo
    fn get_transition_to(ctx: &Eval, transition: &ActionTransition) -> Option<String> {
        if transition.transition_out {
            Some(ctx.peek_previous_state_name())
        } else {
            transition.transition_to.clone()
        }
    }

    fn find_state(ctx: &Eval, name: &str) -> Option<StateRef> {
        ctx.action_group()
            .find_state(name)
            .map(|state| StateRef {
                group: ctx.group,
                state,
            })
    }

    /// ActionStateContainer.peekNextState
    pub fn peek_next_state(&self, ctx: &Eval) -> Option<StateRef> {
        let mut result = None;
        let current = self.current_state?;
        let current_state = ctx.state(current);
        for next_transition in &current_state.transitions {
            let transition_to = Self::get_transition_to(ctx, next_transition);
            if !string_utils::is_null_or_whitespace(transition_to.as_deref())
                && next_transition.passes(ctx, current_state)
            {
                let next_state = Self::find_state(ctx, transition_to.as_deref().unwrap_or(""));
                if let Some(next_state) = next_state {
                    if !self.has_child_state(next_state)
                        && (!next_transition.as_substate
                            || self.current_state_supports_child_state(ctx, next_state))
                    {
                        result = Some(next_state);
                        break;
                    }
                }
            }
        }
        for &sub_state_ref in &self.child_states {
            let mut next_state = None;
            let sub_state = ctx.state(sub_state_ref);
            for sub_transition in &sub_state.transitions {
                if sub_transition.passes(ctx, sub_state) {
                    if sub_transition.transition_out {
                        break;
                    }
                    let transition_to = Self::get_transition_to(ctx, sub_transition);
                    if !string_utils::is_null_or_whitespace(transition_to.as_deref()) {
                        let next_sub_state =
                            Self::find_state(ctx, transition_to.as_deref().unwrap_or(""));
                        if let Some(next_sub_state) = next_sub_state {
                            if !self.has_child_state(next_sub_state) {
                                if self.current_state_supports_child_state(ctx, next_sub_state) {
                                    break;
                                }
                                if sub_transition.force_parent {
                                    next_state = Some(next_sub_state);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            if next_state != self.current_state && next_state.is_some() {
                result = next_state;
            }
        }
        result
    }

    /// ActionStateContainer.evaluateCurrentStateTransitions: the first passing transition that is not a substate one
    /// makes its target the current state; then every substate transition of the (possibly new) current state that
    /// passes inserts its target as a child state.
    fn evaluate_current_state_transitions(&mut self, ctx: &Eval) {
        let mut i = 0;
        while let Some(cur) = self.current_state {
            let cur_state = ctx.state(cur);
            if i >= cur_state.transitions.len() {
                break;
            }
            let next_transition = &cur_state.transitions[i];
            i += 1;
            if !next_transition.as_substate {
                let transition_to = Self::get_transition_to(ctx, next_transition);
                if string_utils::is_null_or_whitespace(transition_to.as_deref()) {
                    // "Transition's target state not specified" (a warning)
                } else if next_transition.passes(ctx, cur_state) {
                    if let Some(next_state) =
                        Self::find_state(ctx, transition_to.as_deref().unwrap_or(""))
                    {
                        self.set_current_state(
                            ctx,
                            Some(next_state),
                            Some(next_transition.transition_out),
                        );
                        break;
                    }
                    // "Transition's target state not found" (a warning)
                }
            }
        }
        let mut ix = 0;
        while let Some(cur) = self.current_state {
            let cur_state = ctx.state(cur);
            if ix >= cur_state.transitions.len() {
                break;
            }
            let next_transition = &cur_state.transitions[ix];
            ix += 1;
            if next_transition.as_substate {
                let transition_to = Self::get_transition_to(ctx, next_transition);
                if string_utils::is_null_or_whitespace(transition_to.as_deref()) {
                } else if next_transition.passes(ctx, cur_state) {
                    if let Some(next_sub_state) =
                        Self::find_state(ctx, transition_to.as_deref().unwrap_or(""))
                    {
                        self.try_insert_child_state(ctx, next_sub_state);
                    }
                }
            }
        }
    }

    /// ActionStateContainer.evaluateSubStateTransitions
    fn evaluate_sub_state_transitions(&mut self, ctx: &Eval) {
        let mut sub_state_idx: isize = 0;
        while (sub_state_idx as usize) < self.child_states.len() {
            let mut next_state: Option<StateRef> = None;
            let sub_state_ref = self.child_states[sub_state_idx as usize];
            let sub_state = ctx.state(sub_state_ref);
            for sub_transition in &sub_state.transitions {
                if sub_transition.passes(ctx, sub_state) {
                    if sub_transition.transition_out {
                        self.remove_child_state_at(sub_state_idx as usize);
                        sub_state_idx -= 1;
                        break;
                    }
                    if !string_utils::is_null_or_whitespace(sub_transition.transition_to.as_deref())
                    {
                        let next_sub_state = Self::find_state(
                            ctx,
                            sub_transition.transition_to.as_deref().unwrap_or(""),
                        );
                        if let Some(next_sub_state) = next_sub_state {
                            if !self.has_child_state(next_sub_state) {
                                if self.current_state_supports_child_state(ctx, next_sub_state) {
                                    self.child_states[sub_state_idx as usize] = next_sub_state;
                                    break;
                                }
                                if sub_transition.force_parent {
                                    next_state = Some(next_sub_state);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            if next_state != self.current_state && next_state.is_some() {
                self.set_current_state(ctx, next_state, None);
            }
            sub_state_idx += 1;
        }
    }

    /// ActionStateContainer.canTransitionToState(group, stateName, allowSubState). The original fails with a null
    /// pointer without a current state; here that is `false`.
    pub fn can_transition_to_state(
        &self,
        ctx: &Eval,
        state_name: &str,
        allow_sub_state: bool,
    ) -> bool {
        let Some(next_state) = Self::find_state(ctx, state_name) else {
            return false;
        };
        let Some(current) = self.current_state else {
            return false;
        };
        for next_transition in &ctx.state(current).transitions {
            if string_utils::equals_ignore_case(
                Some(state_name),
                next_transition.transition_to.as_deref(),
            ) {
                return true;
            }
        }
        if !allow_sub_state || !self.current_state_supports_child_state(ctx, next_state) {
            return false;
        }
        for &sub_ref in &self.child_states {
            for sub_transition in &ctx.state(sub_ref).transitions {
                if !sub_transition.transition_out
                    && string_utils::equals_ignore_case(
                        Some(state_name),
                        sub_transition.transition_to.as_deref(),
                    )
                {
                    return true;
                }
            }
        }
        false
    }

    /// ActionStateContainer.currentStateSupportsChildState
    fn current_state_supports_child_state(&self, ctx: &Eval, child: StateRef) -> bool {
        match self.current_state {
            Some(cur) => can_have_sub_state(ctx, cur, child),
            None => false,
        }
    }

    /// ActionStateContainer.hasChildState
    pub fn has_child_state(&self, child: StateRef) -> bool {
        self.child_states.contains(&child)
    }

    /// ActionStateContainer.setCurrentState: false when it is the current state already; else the new root state with
    /// the transition used, and the children it cannot have dropped.
    pub fn set_current_state(
        &mut self,
        ctx: &Eval,
        next_state: Option<StateRef>,
        transition_used: Option<bool>,
    ) -> bool {
        if next_state == self.current_state {
            return false;
        }
        self.current_state = next_state;
        self.transition_used = transition_used;
        let mut i: isize = 0;
        while (i as usize) < self.child_states.len() {
            let sub_state = self.child_states[i as usize];
            let cur = self.current_state.expect("set to a state above");
            if !can_have_sub_state(ctx, cur, sub_state) {
                self.remove_child_state_at(i as usize);
                i -= 1;
            }
            i += 1;
        }
        true
    }

    /// ActionStateContainer.tryInsertChildState
    fn try_insert_child_state(&mut self, ctx: &Eval, next_state: StateRef) -> bool {
        if self.has_child_state(next_state) {
            return false;
        }
        if !self.current_state_supports_child_state(ctx, next_state) {
            return false;
        }
        let mut insert_at: Option<usize> = None;
        for (i, &lower_state) in self.child_states.iter().enumerate() {
            if can_have_sub_state(ctx, next_state, lower_state) {
                insert_at = Some(i);
                break;
            }
            if !can_have_sub_state(ctx, lower_state, next_state) {
                return false;
            }
        }
        match insert_at {
            Some(i) => self.child_states.insert(i, next_state),
            None => self.child_states.push(next_state),
        }
        true
    }

    /// ActionStateContainer.removeChildStateAt
    pub fn remove_child_state_at(&mut self, sub_state_idx: usize) {
        self.child_states.remove(sub_state_idx);
    }

    /// ActionStateContainer.getCurrentStateName
    pub fn get_current_state_name(&self, groups: &[Rc<ActionGroup>]) -> Option<String> {
        self.current_state
            .map(|r| groups[r.group].states[r.state].name.clone())
    }

    /// ActionStateContainer.getVariable: the state variables of the child states from the last, then of the root.
    pub fn get_variable(
        &self,
        groups: &[Rc<ActionGroup>],
        key: &str,
    ) -> Option<AnimationVariableSlot> {
        for r in self.child_states.iter().rev() {
            if let Some(s) = groups[r.group].states[r.state].state_variables.get_variable(key) {
                return Some(s.clone());
            }
        }
        let r = self.current_state?;
        groups[r.group].states[r.state]
            .state_variables
            .get_variable(key)
            .cloned()
    }

    /// ActionStateContainer.hasStateVariables
    pub fn has_state_variables(&self, groups: &[Rc<ActionGroup>]) -> bool {
        for r in self.child_states.iter().rev() {
            if groups[r.group].states[r.state].has_state_variables() {
                return true;
            }
        }
        match self.current_state {
            Some(r) => groups[r.group].states[r.state].has_state_variables(),
            None => false,
        }
    }

    /// ActionStateContainer.set
    pub fn set(&mut self, other: &ActionStateContainer) {
        self.current_state = other.current_state;
        self.transition_used = other.transition_used;
        self.child_states.clone_from(&other.child_states);
    }

    /// ActionStateContainer.clear
    pub fn clear(&mut self) {
        self.current_state = None;
        self.transition_used = None;
        self.child_states.clear();
    }

    /// ActionStateContainer.equalTo
    pub fn equal_to(&self, rhs: &ActionStateContainer) -> bool {
        self.current_state == rhs.current_state && self.sub_states_equal(rhs)
    }

    /// ActionStateContainer.subStatesEqual
    pub fn sub_states_equal(&self, rhs: &ActionStateContainer) -> bool {
        self.child_states == rhs.child_states
    }
}

/// ActionState.canHaveSubState(parent, child): not the same state, and the parent's child tags overlap the child's tags.
fn can_have_sub_state(ctx: &Eval, parent: StateRef, child: StateRef) -> bool {
    if parent == child {
        return false;
    }
    ActionState::can_have_sub_state(ctx.state(parent), ctx.state(child))
}
