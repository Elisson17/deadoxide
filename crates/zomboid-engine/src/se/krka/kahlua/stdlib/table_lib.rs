//! se.krka.kahlua.stdlib.TableLib: `table.concat`, `insert`, `remove`, `newarray`, `pairs`, `isempty`, `wipe`,
//! `ipairs` (`pairs` returns only the table's iterator function; `ipairs` stops at the first nil).
use std::rc::Rc;

use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
use crate::se::krka::kahlua::vm::kahlua_util::{
    get_optional_arg, lua_assert, raw_tonumber, raw_tostring,
};
use crate::se::krka::kahlua::vm::object::{
    cast_table, classes, JavaClass, JavaFunction, LuaStr, Object, TableRef, Throwable, R,
};

pub static CLASS: JavaClass = JavaClass {
    name: "se.krka.kahlua.stdlib.TableLib",
    superclass: Some(&classes::OBJECT),
    interfaces: &[&classes::JAVA_FUNCTION],
};

static IPAIRS_CLASS: JavaClass = JavaClass {
    name: "se.krka.kahlua.stdlib.TableLib$$Lambda",
    superclass: Some(&classes::OBJECT),
    interfaces: &[&classes::JAVA_FUNCTION],
};

const NAMES: [&str; 8] = [
    "concat", "insert", "remove", "newarray", "pairs", "isempty", "wipe", "ipairs",
];

/// One TableLib function object.
pub struct TableLib {
    index: usize,
    ipairs_iterator: Object,
}

impl JavaFunction for TableLib {
    /// se.krka.kahlua.stdlib.TableLib.call
    fn call(&self, thread: &mut KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
        match self.index {
            0 => concat(cf, n),
            1 => insert_(thread, cf, n),
            2 => remove_(thread, cf, n),
            3 => Err(Throwable::runtime(
                "table.newarray (KahluaArray) is not ported",
            )),
            4 => pairs(cf, n),
            5 => {
                let t = get_table(cf, n)?;
                let e = t.ok_or_else(|| npe("t", "isEmpty()"))?.borrow().is_empty();
                cf.push(Object::Boolean(e))
            }
            6 => {
                get_table(cf, n)?
                    .ok_or_else(|| npe("t", "wipe()"))?
                    .borrow_mut()
                    .wipe();
                Ok(0)
            }
            7 => {
                lua_assert(n >= 1, "Not enough arguments")?;
                let o = cf.get(0);
                lua_assert(matches!(o, Object::Table(_)), "Expected a table")?;
                cf.push(self.ipairs_iterator.clone())?;
                cf.push(o)?;
                cf.push(Object::Number(0.0))?;
                Ok(3)
            }
            _ => Ok(0),
        }
    }
    /// se.krka.kahlua.stdlib.TableLib.toString
    fn java_to_string(&self) -> String {
        format!("table.{}", NAMES[self.index])
    }
    fn class(&self) -> &'static JavaClass {
        &CLASS
    }
}

fn npe(var: &str, method: &str) -> Throwable {
    Throwable::npe(format!(
        "Cannot invoke \"se.krka.kahlua.vm.KahluaTable.{method}\" because \"{var}\" is null"
    ))
}

/// The `ipairs_iterator` lambda.
struct IpairsIterator;

impl JavaFunction for IpairsIterator {
    fn call(&self, _thread: &mut KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
        lua_assert(n >= 2, "Not enough arguments")?;
        let o = cf.get(0);
        lua_assert(matches!(o, Object::Table(_)), "Expected a table")?;
        let Object::Table(t) = o else {
            return Ok(0);
        };
        let o = cf.get(1);
        lua_assert(matches!(o, Object::Number(_)), "Expected a number")?;
        let Object::Number(d) = o else {
            return Ok(0);
        };
        let next_index = d + 1.0;
        let v = KahluaTableImpl::rawget(&t, &Object::Number(next_index))?;
        if !v.is_nil() {
            cf.push2(Object::Number(next_index), v)
        } else {
            Ok(0)
        }
    }
    fn java_to_string(&self) -> String {
        "se.krka.kahlua.stdlib.TableLib$$Lambda".into()
    }
    fn class(&self) -> &'static JavaClass {
        &IPAIRS_CLASS
    }
}

/// se.krka.kahlua.stdlib.TableLib.register
pub fn register(env: &TableRef) {
    let table = KahluaTableImpl::new_ref();
    let ipairs_iterator = Object::Function(Rc::new(IpairsIterator));
    for (i, name) in NAMES.iter().enumerate() {
        let f = TableLib {
            index: i,
            ipairs_iterator: ipairs_iterator.clone(),
        };
        KahluaTableImpl::rawset(&table, Object::str(name), Object::Function(Rc::new(f)));
    }
    KahluaTableImpl::rawset(env, Object::str("table"), Object::Table(table));
}

/// se.krka.kahlua.stdlib.TableLib.pairs
fn pairs(cf: &FrameRef, n: i32) -> R<i32> {
    lua_assert(n >= 1, "Not enough arguments")?;
    let o = cf.get(0);
    lua_assert(matches!(o, Object::Table(_)), "Expected a table")?;
    let Object::Table(t) = o else {
        return Ok(0);
    };
    cf.push(KahluaTableImpl::iterator(&t))
}

/// se.krka.kahlua.stdlib.TableLib.concat (a value that is not a string or number appends "null").
fn concat(cf: &FrameRef, n: i32) -> R<i32> {
    let table = get_table(cf, n)?;
    let separator: Option<LuaStr> = if n >= 2 {
        raw_tostring(&cf.get(1))
    } else {
        Some(Rc::from([]))
    };
    let first = if n >= 3 {
        number_int(&cf.get(2), "firstDouble")?
    } else {
        1
    };
    let last = if n >= 4 {
        number_int(&cf.get(3), "lastDouble")?
    } else {
        KahluaTableImpl::len(table.as_ref().ok_or_else(|| npe("table", "len()"))?)?
    };
    let mut buffer: Vec<u16> = Vec::new();
    let null: Vec<u16> = "null".encode_utf16().collect();
    let mut i = first;
    while i <= last {
        if i > first {
            match &separator {
                Some(s) => buffer.extend_from_slice(s),
                None => buffer.extend_from_slice(&null),
            }
        }
        let t = table
            .as_ref()
            .ok_or_else(|| npe("table", "rawget(Object)"))?;
        let value = KahluaTableImpl::rawget(t, &Object::Number(f64::from(i)))?;
        match raw_tostring(&value) {
            Some(s) => buffer.extend_from_slice(&s),
            None => buffer.extend_from_slice(&null),
        }
        i += 1;
    }
    cf.push(Object::String(buffer.into()))
}

/// `KahluaUtil.rawTonumber(o).intValue()`.
fn number_int(o: &Object, var: &str) -> R<i32> {
    match raw_tonumber(o)? {
        Some(d) => Ok(d as i32),
        None => Err(Throwable::npe(format!(
            "Cannot invoke \"java.lang.Double.intValue()\" because \"{var}\" is null"
        ))),
    }
}

/// se.krka.kahlua.stdlib.TableLib.insert(KahluaThread, KahluaTable, int, Object)
pub fn insert(
    thread: &mut KahluaThread,
    table: &TableRef,
    position: i32,
    element: Object,
) -> R<()> {
    let len = KahluaTableImpl::len(table)?;
    let t = Object::Table(table.clone());
    let mut i = len;
    while i >= position {
        let v = thread.tableget(&t, &Object::Number(f64::from(i)))?;
        thread.table_set(&t, &Object::Number(f64::from(i + 1)), &v)?;
        i -= 1;
    }
    thread.table_set(&t, &Object::Number(f64::from(position)), &element)
}

/// se.krka.kahlua.stdlib.TableLib.insert(LuaCallFrame, int)
fn insert_(thread: &mut KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
    lua_assert(n >= 2, "Not enough arguments")?;
    let t = cast_table(&cf.get(0))?.ok_or_else(|| npe("t", "len()"))?;
    let mut pos = KahluaTableImpl::len(&t)? + 1;
    let elem = if n > 2 {
        pos = match raw_tonumber(&cf.get(1))? {
            Some(d) => d as i32,
            None => {
                return Err(Throwable::npe(
                    "Cannot invoke \"java.lang.Double.intValue()\" because the return value of \"se.krka.kahlua.vm.KahluaUtil.rawTonumber(Object)\" is null",
                ))
            }
        };
        cf.get(2)
    } else {
        cf.get(1)
    };
    insert(thread, &t, pos, elem)?;
    Ok(0)
}

/// se.krka.kahlua.stdlib.TableLib.remove(KahluaThread, KahluaTable, int)
pub fn remove(thread: &mut KahluaThread, table: &TableRef, position: i32) -> R<Object> {
    let t = Object::Table(table.clone());
    let ret = thread.tableget(&t, &Object::Number(f64::from(position)))?;
    let len = KahluaTableImpl::len(table)?;
    for i in position..len {
        let v = thread.tableget(&t, &Object::Number(f64::from(i + 1)))?;
        thread.table_set(&t, &Object::Number(f64::from(i)), &v)?;
    }
    thread.table_set(&t, &Object::Number(f64::from(len)), &Object::Nil)?;
    Ok(ret)
}

/// se.krka.kahlua.stdlib.TableLib.remove(LuaCallFrame, int)
fn remove_(thread: &mut KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
    let t = get_table(cf, n)?.ok_or_else(|| npe("t", "len()"))?;
    let mut pos = KahluaTableImpl::len(&t)?;
    if n > 1 {
        pos = match raw_tonumber(&cf.get(1))? {
            Some(d) => d as i32,
            None => {
                return Err(Throwable::npe(
                    "Cannot invoke \"java.lang.Double.intValue()\" because the return value of \"se.krka.kahlua.vm.KahluaUtil.rawTonumber(Object)\" is null",
                ))
            }
        };
    }
    let r = remove(thread, &t, pos)?;
    cf.push(r)?;
    Ok(1)
}

/// se.krka.kahlua.stdlib.TableLib.getTable
fn get_table(cf: &FrameRef, n: i32) -> R<Option<TableRef>> {
    lua_assert(n >= 1, "expected table, got no arguments")?;
    cast_table(&cf.get(0))
}

/// The optional first argument as `KahluaUtil.getOptionalArg` gives it (for host callers).
pub fn first_arg(cf: &FrameRef) -> Object {
    get_optional_arg(cf, 1)
}
