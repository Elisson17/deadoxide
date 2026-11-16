//! zombie.Lua.LuaEventManager and zombie.Lua.LuaHookManager: the `Events` and `Hook` tables. The originals keep
//! their event lists in statics; here one registry per Lua state.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::event::Event;
use super::lua_events_table::{EVENTS, HOOKS};
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
use crate::se::krka::kahlua::vm::lua_closure::LuaClosure;
use crate::se::krka::kahlua::vm::object::{Object, TableRef, R};

/// The event list of LuaEventManager (global `Events`) or LuaHookManager (global `Hook`).
pub struct EventRegistry {
    /// The global the events are registered in.
    pub global: &'static str,
    pub event_list: RefCell<Vec<Rc<Event>>>,
    event_map: RefCell<HashMap<String, Rc<Event>>>,
}

impl EventRegistry {
    fn new(global: &'static str) -> Rc<EventRegistry> {
        Rc::new(EventRegistry {
            global,
            event_list: RefCell::new(Vec::new()),
            event_map: RefCell::new(HashMap::new()),
        })
    }

    /// zombie.Lua.LuaEventManager.register / LuaHookManager.register: a new table under `global`, then AddEvents.
    pub fn register(global: &'static str, env: &TableRef) -> R<Rc<EventRegistry>> {
        let r = EventRegistry::new(global);
        KahluaTableImpl::rawset(
            env,
            Object::str(global),
            Object::Table(KahluaTableImpl::new_ref()),
        );
        let names: &[&str] = if global == "Hook" { &HOOKS } else { &EVENTS };
        for name in names {
            r.add_event(name, env)?;
        }
        Ok(r)
    }

    /// zombie.Lua.LuaEventManager.AddEvent: the existing event, or a new one registered in the global table
    /// (when that global is still a table).
    pub fn add_event(&self, name: &str, env: &TableRef) -> R<Rc<Event>> {
        if let Some(e) = self.event_map.borrow().get(name) {
            return Ok(e.clone());
        }
        let event = Event::new(name, self.event_list.borrow().len());
        self.event_list.borrow_mut().push(event.clone());
        self.event_map
            .borrow_mut()
            .insert(name.to_string(), event.clone());
        if let Object::Table(t) = KahluaTableImpl::rawget(env, &Object::str(self.global))? {
            event.register(&t);
        }
        Ok(event)
    }

    /// The event called `name`, if registered.
    pub fn get(&self, name: &str) -> Option<Rc<Event>> {
        self.event_map.borrow().get(name).cloned()
    }

    /// zombie.Lua.LuaEventManager.checkEvent: the event, added when unknown (the original logs that on
    /// DebugType.Lua); `None` when it has no callbacks.
    pub fn check_event(&self, name: &str, env: &TableRef) -> R<Option<Rc<Event>>> {
        let e = match self.get(name) {
            Some(e) => e,
            None => self.add_event(name, env)?,
        };
        Ok(if e.callbacks.borrow().is_empty() {
            None
        } else {
            Some(e)
        })
    }

    /// zombie.Lua.LuaEventManager.triggerEvent (on the main thread, LuaManager.env set): checkEvent, then
    /// Event.trigger with `params`. Returns each callback's protected-call results (see Event.trigger), empty when
    /// nothing was called.
    pub fn trigger_event(
        &self,
        name: &str,
        env: &TableRef,
        thread: &mut KahluaThread,
        params: &[Object],
    ) -> R<Vec<(Rc<LuaClosure>, Vec<Object>)>> {
        match self.check_event(name, env)? {
            Some(e) => e.trigger(thread, params),
            None => Ok(Vec::new()),
        }
    }
}
