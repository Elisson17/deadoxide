//! se.krka.kahlua.j2se.KahluaTableImpl: the live table of the VM and the save form of a Lua table (object and
//! square `modData`).
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use crate::java_nio::{read_string, read_string_stream, ByteBuffer};
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
use crate::se::krka::kahlua::vm::object::{
    cast_string, classes, identity_hash_code, rstr, JavaClass, JavaFunction, Key, Object, TableRef,
    Throwable, R,
};

/// A Lua value as a saved table can hold it.
#[derive(Clone, Debug, PartialEq)]
pub enum LuaValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Table(KahluaTable),
}

/// se.krka.kahlua.j2se.KahluaTableImpl: entries in first-insertion order (`rawset` of an existing key replaces the
/// value in place). That is the original's order too: `J2SEPlatform.newTable` backs every table with a
/// `LinkedHashMap` (an earlier note here said "hash order", which was wrong).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KahluaTable {
    pub entries: Vec<(LuaValue, LuaValue)>,
}

/// Raw key equality: strings and booleans by value, numbers as `Double.equals` (bit pattern), tables by identity
/// (two loaded tables are never the same key).
fn same_key(a: &LuaValue, b: &LuaValue) -> bool {
    match (a, b) {
        (LuaValue::String(x), LuaValue::String(y)) => x == y,
        (LuaValue::Number(x), LuaValue::Number(y)) => x.to_bits() == y.to_bits(),
        (LuaValue::Boolean(x), LuaValue::Boolean(y)) => x == y,
        _ => false,
    }
}

impl KahluaTable {
    /// se.krka.kahlua.j2se.KahluaTableImpl.rawset for a non-nil value.
    pub fn rawset(&mut self, key: LuaValue, value: LuaValue) {
        match self.entries.iter_mut().find(|(k, _)| same_key(k, &key)) {
            Some(e) => e.1 = value,
            None => self.entries.push((key, value)),
        }
    }

    /// se.krka.kahlua.j2se.KahluaTableImpl.load (ByteBuffer, int): a count, then typed keys and values (world
    /// version 25 and later; older saves stored string keys only).
    pub fn load(b: &mut ByteBuffer, world_version: i32) -> Result<Self, String> {
        let count = b.get_int()?;
        let mut t = KahluaTable::default();
        for _ in 0..count {
            if world_version >= 25 {
                let key_byte = b.get()?;
                let key = Self::load_value(b, world_version, key_byte)?;
                let value_byte = b.get()?;
                let value = Self::load_value(b, world_version, value_byte)?;
                t.rawset(key, value);
            } else {
                let value_byte = b.get()?;
                let key = LuaValue::String(read_string(b)?);
                let value = Self::load_value(b, world_version, value_byte)?;
                t.rawset(key, value);
            }
        }
        Ok(t)
    }

    /// se.krka.kahlua.j2se.KahluaTableImpl.load(DataInputStream, int): as the ByteBuffer form, but into this table
    /// without wiping it (entries already there stay unless a loaded key replaces them), with strings as
    /// `GameWindow.ReadString(DataInputStream)`, and a boolean true only for byte 1.
    pub fn load_stream(&mut self, b: &mut ByteBuffer, world_version: i32) -> Result<(), String> {
        let count = b.get_int()?;
        for _ in 0..count {
            if world_version >= 25 {
                let key_byte = b.get()?;
                let key = Self::load_value_stream(b, world_version, key_byte)?;
                let value_byte = b.get()?;
                let value = Self::load_value_stream(b, world_version, value_byte)?;
                self.rawset(key, value);
            } else {
                let value_byte = b.get()?;
                let key = LuaValue::String(read_string_stream(b)?);
                let value = Self::load_value_stream(b, world_version, value_byte)?;
                self.rawset(key, value);
            }
        }
        Ok(())
    }

    /// se.krka.kahlua.j2se.KahluaTableImpl.load(DataInputStream, int, byte): a nested table is a new table.
    fn load_value_stream(
        b: &mut ByteBuffer,
        world_version: i32,
        sbyt: i8,
    ) -> Result<LuaValue, String> {
        Ok(match sbyt {
            0 => LuaValue::String(read_string_stream(b)?),
            1 => LuaValue::Number(b.get_double()?),
            3 => LuaValue::Boolean(b.get()? == 1),
            2 => {
                let mut t = KahluaTable::default();
                t.load_stream(b, world_version)?;
                LuaValue::Table(t)
            }
            _ => {
                return Err(format!(
                    "java.lang.RuntimeException: invalid lua table type {sbyt}"
                ))
            }
        })
    }

    /// se.krka.kahlua.j2se.KahluaTableImpl.load (ByteBuffer, int, byte)
    fn load_value(b: &mut ByteBuffer, world_version: i32, sbyt: i8) -> Result<LuaValue, String> {
        Ok(match sbyt {
            0 => LuaValue::String(read_string(b)?),
            1 => LuaValue::Number(b.get_double()?),
            3 => LuaValue::Boolean(b.get()? != 0),
            2 => LuaValue::Table(Self::load(b, world_version)?),
            _ => {
                return Err(format!(
                    "java.lang.RuntimeException: invalid lua table type {sbyt}"
                ))
            }
        })
    }
}

/// se.krka.kahlua.j2se.KahluaTableImpl as the VM uses it: a `LinkedHashMap` delegate (insertion order; putting an
/// existing key keeps its place, removing and putting again moves it to the end) and a metatable. `rawget` falls
/// through to the metatable's `rawget` for a missing key, as the original does. `reloadReplace` (a reload feature)
/// and the `Core.debug` data breakpoints are not ported.
#[derive(Default)]
pub struct KahluaTableImpl {
    entries: Vec<Option<(Object, Object)>>,
    index: HashMap<Key, usize>,
    pub metatable: Option<TableRef>,
}

/// Metatable hops before `rawget` gives up with the StackOverflowError a cycle of metatables throws in the original.
const RAWGET_CHAIN_LIMIT: usize = 10_000;

impl KahluaTableImpl {
    /// se.krka.kahlua.j2se.J2SEPlatform.newTable
    pub fn new_ref() -> TableRef {
        Rc::new(RefCell::new(KahluaTableImpl::default()))
    }

    fn delegate_get(&self, key: &Object) -> Option<&Object> {
        self.index
            .get(&Key(key.clone()))
            .and_then(|&i| self.entries[i].as_ref())
            .map(|(_, v)| v)
    }

    fn delegate_put(&mut self, key: Object, value: Object) {
        match self.index.get(&Key(key.clone())) {
            Some(&i) => {
                if let Some(e) = self.entries[i].as_mut() {
                    e.1 = value;
                }
            }
            None => {
                self.index.insert(Key(key.clone()), self.entries.len());
                self.entries.push(Some((key, value)));
            }
        }
    }

    fn delegate_remove(&mut self, key: &Object) {
        if let Some(i) = self.index.remove(&Key(key.clone())) {
            self.entries[i] = None;
            if self.entries.len() > 32 && self.index.len() * 2 < self.entries.len() {
                self.entries.retain(Option::is_some);
                self.index.clear();
                for (i, e) in self.entries.iter().enumerate() {
                    if let Some((k, _)) = e {
                        self.index.insert(Key(k.clone()), i);
                    }
                }
            }
        }
    }

    /// se.krka.kahlua.j2se.KahluaTableImpl.size
    pub fn size(&self) -> i32 {
        self.index.len() as i32
    }

    /// se.krka.kahlua.j2se.KahluaTableImpl.isEmpty
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// se.krka.kahlua.j2se.KahluaTableImpl.wipe
    pub fn wipe(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    /// `delegate.keySet().toArray()`: the keys in iteration order.
    pub fn keys(&self) -> Vec<Object> {
        self.entries
            .iter()
            .flatten()
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// `delegate.get(key)` without the metatable (what the iterator reads).
    pub fn get_direct(&self, key: &Object) -> Object {
        self.delegate_get(key).cloned().unwrap_or_default()
    }

    /// se.krka.kahlua.j2se.KahluaTableImpl.rawget(Object)
    pub fn rawget(t: &TableRef, key: &Object) -> R<Object> {
        if key.is_nil() {
            return Ok(Object::Nil);
        }
        let mut cur = t.clone();
        for _ in 0..RAWGET_CHAIN_LIMIT {
            let next = {
                let b = cur.borrow();
                if let Some(v) = b.delegate_get(key) {
                    return Ok(v.clone());
                }
                match &b.metatable {
                    None => return Ok(Object::Nil),
                    Some(m) => m.clone(),
                }
            };
            cur = next;
        }
        Err(Throwable::stack_overflow())
    }

    /// se.krka.kahlua.j2se.KahluaTableImpl.rawset(Object, Object): nil removes the key; a nil key is stored like
    /// any other (`LinkedHashMap` takes a null key).
    pub fn rawset(t: &TableRef, key: Object, value: Object) {
        let mut b = t.borrow_mut();
        if value.is_nil() {
            b.delegate_remove(&key);
        } else {
            b.delegate_put(key, value);
        }
    }

    /// se.krka.kahlua.j2se.KahluaTableImpl.rawget(int)
    pub fn rawget_int(t: &TableRef, key: i32) -> R<Object> {
        Self::rawget(t, &Object::Number(f64::from(key)))
    }

    /// se.krka.kahlua.j2se.KahluaTableImpl.rawgetStr(Object): `(String) rawget(key)` (nil is null, any other
    /// non-string a ClassCastException).
    pub fn rawget_str(t: &TableRef, key: &str) -> R<Option<String>> {
        Ok(cast_string(&Self::rawget(t, &Object::str(key))?)?.map(|s| rstr(&s)))
    }

    /// se.krka.kahlua.j2se.KahluaTableImpl.rawgetInt(Object): a number's `intValue()`, else -1.
    pub fn rawget_int_of(t: &TableRef, key: &str) -> R<i32> {
        Ok(match Self::rawget(t, &Object::str(key))? {
            Object::Number(d) => d as i32,
            _ => -1,
        })
    }

    /// se.krka.kahlua.j2se.KahluaTableImpl.rawgetBool(Object): a boolean, else false.
    pub fn rawget_bool(t: &TableRef, key: &str) -> R<bool> {
        Ok(matches!(
            Self::rawget(t, &Object::str(key))?,
            Object::Boolean(true)
        ))
    }

    /// `(KahluaTableImpl) table.rawget(key)`: nil is null, any other non-table a ClassCastException.
    pub fn rawget_table(t: &TableRef, key: &str) -> R<Option<TableRef>> {
        match Self::rawget(t, &Object::str(key))? {
            Object::Nil => Ok(None),
            Object::Table(t) => Ok(Some(t)),
            o => Err(Throwable::class_cast(
                &o,
                "se.krka.kahlua.j2se.KahluaTableImpl",
            )),
        }
    }

    /// se.krka.kahlua.j2se.KahluaTableImpl.rawset(int, Object)
    pub fn rawset_int(t: &TableRef, key: i32, value: Object) {
        Self::rawset(t, Object::Number(f64::from(key)), value)
    }

    /// se.krka.kahlua.j2se.KahluaTableImpl.len
    pub fn len(t: &TableRef) -> R<i32> {
        let size = t.borrow().size();
        crate::se::krka::kahlua::vm::kahlua_util::len(t, 0, 2 * size)
    }

    /// The pairs of `iterator()` run to the end: the keys taken when it starts, each value read when reached.
    pub fn iterate_snapshot(t: &TableRef) -> Vec<(Object, Object)> {
        let keys = t.borrow().keys();
        keys.into_iter()
            .map(|k| {
                let v = t.borrow().get_direct(&k);
                (k, v)
            })
            .collect()
    }

    /// se.krka.kahlua.j2se.KahluaTableImpl.iterator: a function object over the keys present now.
    pub fn iterator(t: &TableRef) -> Object {
        let keys = t.borrow().keys();
        Object::Function(Rc::new(TableIterator {
            table: t.clone(),
            keys,
            key_index: Cell::new(0),
        }))
    }

    /// se.krka.kahlua.j2se.KahluaTableImpl.toString
    pub fn java_to_string(t: &TableRef) -> String {
        let hash = identity_hash_code(t);
        match Self::rawget(t, &Object::str("Type")) {
            Ok(Object::String(ty)) => format!("{} 0x{hash}", rstr(&ty)),
            _ => format!("table 0x{hash}"),
        }
    }
}

/// The anonymous `KahluaTableIterator` of `KahluaTableImpl.iterator()`.
pub struct TableIterator {
    table: TableRef,
    keys: Vec<Object>,
    key_index: Cell<usize>,
}

impl TableIterator {
    /// advance + getKey/getValue: the next pair, or none at the end.
    pub fn advance(&self) -> Option<(Object, Object)> {
        let i = self.key_index.get();
        let k = self.keys.get(i)?.clone();
        let v = self.table.borrow().get_direct(&k);
        self.key_index.set(i + 1);
        Some((k, v))
    }
}

impl JavaFunction for TableIterator {
    /// KahluaTableImpl$1.call
    fn call(&self, _thread: &mut KahluaThread, frame: &FrameRef, _n_arguments: i32) -> R<i32> {
        match self.advance() {
            Some((k, v)) => frame.push2(k, v),
            None => Ok(0),
        }
    }
    fn java_to_string(&self) -> String {
        format!(
            "se.krka.kahlua.j2se.KahluaTableImpl$1@{:x}",
            identity_hash_code(&self.table)
        )
    }
    fn class(&self) -> &'static JavaClass {
        &classes::TABLE_ITERATOR
    }
}
