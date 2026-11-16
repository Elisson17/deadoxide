//! zombie.audio.MusicIntensityConfig: the events that raise the music intensity (`media/lua/client/Music/
//! MusicIntensityConfig.lua` hands its table to `initEvents`).
use std::collections::HashMap;
use std::rc::Rc;

use crate::java;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::object::{Object, TableRef, Throwable, R};
use crate::zombie::util::string_utils::discard_null_or_whitespace;

/// zombie.audio.MusicIntensityConfig.Event
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub id: String,
    pub intensity: f32,
    pub duration: i64,
    pub multiple: bool,
}

/// zombie.audio.MusicIntensityConfig (one instance: getInstance()).
#[derive(Debug, Default)]
pub struct MusicIntensityConfig {
    pub events: Vec<Rc<Event>>,
    /// A HashMap in the original: its order is not observable through the class.
    pub event_by_id: HashMap<String, Rc<Event>>,
}

/// `(KahluaTableImpl) value` of a table iterator's value (never nil): a non-table is a ClassCastException.
pub(crate) fn cast_table_impl(o: &Object) -> R<TableRef> {
    match o {
        Object::Table(t) => Ok(t.clone()),
        _ => Err(Throwable::class_cast(
            o,
            "se.krka.kahlua.j2se.KahluaTableImpl",
        )),
    }
}

impl MusicIntensityConfig {
    /// zombie.audio.MusicIntensityConfig.initEvents: the list and the map cleared, then every value whose key is
    /// not "VERSION" (ignoring case) is an event table: `id` (blank is none), `intensity` (rawgetFloat),
    /// `duration` (rawgetInt), `multiple` when it is a boolean (true otherwise). An event without id is dropped;
    /// one whose id is known replaces the earlier (removed from the list, added at the end).
    pub fn init_events(&mut self, events_table: &TableRef) -> R<()> {
        self.events.clear();
        self.event_by_id.clear();
        for (key, value) in KahluaTableImpl::iterate_snapshot(events_table) {
            if java::equals_ignore_case("VERSION", &key.java_to_string()) {
                continue;
            }
            let t = cast_table_impl(&value)?;
            let id = discard_null_or_whitespace(KahluaTableImpl::rawget_str(&t, "id")?);
            let intensity = KahluaTableImpl::rawget_float(&t, "intensity")?;
            let duration = i64::from(KahluaTableImpl::rawget_int_of(&t, "duration")?);
            let multiple = match KahluaTableImpl::rawget(&t, &Object::str("multiple"))? {
                Object::Boolean(b) => b,
                _ => true,
            };
            let Some(id) = id else { continue };
            let event = Rc::new(Event {
                id: id.clone(),
                intensity,
                duration,
                multiple,
            });
            if let Some(old) = self.event_by_id.get(&id) {
                // ArrayList.remove(Object): the first element equal to it (Event has identity equals)
                if let Some(i) = self.events.iter().position(|e| Rc::ptr_eq(e, old)) {
                    self.events.remove(i);
                }
            }
            self.events.push(event.clone());
            self.event_by_id.insert(id, event);
        }
        Ok(())
    }
}
