//! zombie.characters.action.ActionContextEvents
use crate::java;
use crate::zombie::util::string_utils;

#[derive(Clone, Debug, PartialEq)]
struct Event {
    name: String,
    state: Option<String>,
}

/// zombie.characters.action.ActionContextEvents: the events reported since the last clear, each for a state or for
/// every state (`state` null). The original keeps them in a linked list with a free pool; only membership is
/// observable, so a vector does.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActionContextEvents {
    events: Vec<Event>,
}

impl ActionContextEvents {
    /// ActionContextEvents.add: not when the same name and state are in already (`contains(name, state, false)`).
    pub fn add(&mut self, name: &str, state: Option<&str>) {
        if !self.contains(name, state, false) {
            self.events.push(Event {
                name: name.to_string(),
                state: state.map(str::to_string),
            });
        }
    }

    /// ActionContextEvents.contains(name, state): with `bAgnosticLayer` true.
    pub fn contains_any_layer(&self, name: &str, state: Option<&str>) -> bool {
        self.contains(name, state, true)
    }

    /// ActionContextEvents.contains(name, state, bAgnosticLayer): an event of the name (ignoring case) when `state` is
    /// null, or of that state (`StringUtils.equalsIgnoreCase`), or, agnostic of layer, of no state.
    pub fn contains(&self, name: &str, state: Option<&str>, b_agnostic_layer: bool) -> bool {
        for event in &self.events {
            if java::equals_ignore_case(&event.name, name) {
                let Some(state) = state else {
                    return true;
                };
                if string_utils::equals_ignore_case(event.state.as_deref(), Some(state)) {
                    return true;
                }
                if b_agnostic_layer && event.state.is_none() {
                    return true;
                }
            }
        }
        false
    }

    /// ActionContextEvents.clear
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// ActionContextEvents.clearEvent: every event of the name (ignoring case).
    pub fn clear_event(&mut self, name: &str) {
        self.events
            .retain(|e| !java::equals_ignore_case(&e.name, name));
    }

    /// ActionContextEvents.get: the names of the events with no state (`HashMap.put(name, null)`, so once each).
    pub fn get(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for e in self.events.iter().rev() {
            if e.state.is_none() && !names.contains(&e.name) {
                names.push(e.name.clone());
            }
        }
        names
    }
}
