//! se.krka.kahlua.stdlib.BaseLib: the global functions (pcall, print, select, type, tostring, tonumber,
//! getmetatable, setmetatable, error, unpack, setfenv, getfenv, rawequal, rawset, rawget, collectgarbage,
//! debugstacktrace, bytecodeloader). Kahlua has no `next`, `ipairs`/`pairs` come from `table` via stdlib.lua.
use std::rc::Rc;

use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::kahlua_thread::{lua_equals, KahluaThread};
use crate::se::krka::kahlua::vm::kahlua_util::{
    fail, from_double, get_optional_arg, get_optional_number_arg, get_optional_string_arg,
    get_string_arg, lua_assert, raw_tonumber, tonumber_radix, tostring, type_name,
};
use crate::se::krka::kahlua::vm::object::{
    cast_string, cast_table, lstr, JavaClass, JavaFunction, Object, TableRef, Throwable, R,
};

pub static CLASS: JavaClass = JavaClass {
    name: "se.krka.kahlua.stdlib.BaseLib",
    superclass: Some(&crate::se::krka::kahlua::vm::object::classes::OBJECT),
    interfaces: &[&crate::se::krka::kahlua::vm::object::classes::JAVA_FUNCTION],
};

const NAMES: [&str; 18] = [
    "pcall",
    "print",
    "select",
    "type",
    "tostring",
    "tonumber",
    "getmetatable",
    "setmetatable",
    "error",
    "unpack",
    "setfenv",
    "getfenv",
    "rawequal",
    "rawset",
    "rawget",
    "collectgarbage",
    "debugstacktrace",
    "bytecodeloader",
];

/// One BaseLib function object (`new BaseLib(index)`).
pub struct BaseLib {
    index: usize,
}

impl JavaFunction for BaseLib {
    /// se.krka.kahlua.stdlib.BaseLib.call
    fn call(&self, thread: &mut KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
        match self.index {
            0 => pcall(thread, cf, n),
            1 => print(thread, cf, n),
            2 => select(cf, n),
            3 => type_(cf, n),
            4 => tostring_(thread, cf, n),
            5 => tonumber(cf, n),
            6 => getmetatable(thread, cf, n),
            7 => setmetatable_(thread, cf, n),
            8 => error(cf, n),
            9 => unpack(cf, n),
            10 => setfenv(cf, n),
            11 => getfenv(cf, n),
            12 => rawequal(cf, n),
            13 => rawset(cf, n),
            14 => rawget(cf, n),
            15 => collectgarbage(cf, n),
            16 => debugstacktrace(cf, n),
            17 => bytecodeloader(cf, n),
            _ => Ok(0),
        }
    }
    /// se.krka.kahlua.stdlib.BaseLib.toString: the function's name.
    fn java_to_string(&self) -> String {
        NAMES[self.index].to_string()
    }
    fn class(&self) -> &'static JavaClass {
        &CLASS
    }
}

/// se.krka.kahlua.stdlib.BaseLib.register
pub fn register(env: &TableRef) {
    for (i, name) in NAMES.iter().enumerate() {
        KahluaTableImpl::rawset(
            env,
            Object::str(name),
            Object::Function(Rc::new(BaseLib { index: i })),
        );
    }
}

fn npe_rawget(var: &str) -> Throwable {
    Throwable::npe(format!(
        "Cannot invoke \"se.krka.kahlua.vm.KahluaTable.rawget(Object)\" because \"{var}\" is null"
    ))
}

/// se.krka.kahlua.stdlib.BaseLib.debugstacktrace
fn debugstacktrace(cf: &FrameRef, _n: i32) -> R<i32> {
    let co = match get_optional_arg(cf, 1) {
        Object::Nil => cf.coroutine.clone(),
        Object::Coroutine(c) => c,
        o => return Err(Throwable::class_cast(&o, "se.krka.kahlua.vm.Coroutine")),
    };
    let level = get_optional_number_arg(cf, 2)?.map_or(0, |d| d as i32);
    let count = get_optional_number_arg(cf, 3)?.map_or(i32::MAX, |d| d as i32);
    let halt_at = get_optional_number_arg(cf, 4)?.map_or(0, |d| d as i32);
    let s = co.borrow().get_current_stack_trace(level, count, halt_at);
    cf.push(Object::str(&s))
}

/// se.krka.kahlua.stdlib.BaseLib.rawget
fn rawget(cf: &FrameRef, n: i32) -> R<i32> {
    lua_assert(n >= 2, "Not enough arguments")?;
    let t = cast_table(&cf.get(0))?.ok_or_else(|| npe_rawget("t"))?;
    let key = cf.get(1);
    let v = KahluaTableImpl::rawget(&t, &key)?;
    cf.push(v)?;
    Ok(1)
}

/// se.krka.kahlua.stdlib.BaseLib.rawset
fn rawset(cf: &FrameRef, n: i32) -> R<i32> {
    lua_assert(n >= 3, "Not enough arguments")?;
    let t = cast_table(&cf.get(0))?.ok_or_else(|| {
        Throwable::npe("Cannot invoke \"se.krka.kahlua.vm.KahluaTable.rawset(Object, Object)\" because \"t\" is null")
    })?;
    KahluaTableImpl::rawset(&t, cf.get(1), cf.get(2));
    cf.set_top(1)?;
    Ok(1)
}

/// se.krka.kahlua.stdlib.BaseLib.rawequal
fn rawequal(cf: &FrameRef, n: i32) -> R<i32> {
    lua_assert(n >= 2, "Not enough arguments")?;
    let eq = lua_equals(&cf.get(0), &cf.get(1));
    cf.push(Object::Boolean(eq))?;
    Ok(1)
}

/// se.krka.kahlua.stdlib.BaseLib.setfenv
fn setfenv(cf: &FrameRef, n: i32) -> R<i32> {
    lua_assert(n >= 2, "Not enough arguments")?;
    let new_env = cast_table(&cf.get(1))?;
    lua_assert(new_env.is_some(), "expected a table")?;
    let Some(new_env) = new_env else {
        return Ok(0);
    };
    let o = cf.get(0);
    let closure = match o {
        Object::Closure(c) => c,
        o => {
            let d = raw_tonumber(&o)?;
            lua_assert(d.is_some(), "expected a lua function or a number")?;
            let level = d.unwrap_or(0.0) as i32;
            if level == 0 {
                cf.coroutine.borrow_mut().environment = Some(new_env);
                return Ok(0);
            }
            let idx = cf.coroutine.borrow().get_parent_frame(level)?;
            let parent = FrameRef {
                coroutine: cf.coroutine.clone(),
                idx,
            };
            match parent.closure() {
                Some(c) => c,
                None => return Err(fail(&format!("No closure found at this level: {level}"))),
            }
        }
    };
    *closure.env.borrow_mut() = new_env;
    cf.set_top(1)?;
    Ok(1)
}

/// se.krka.kahlua.stdlib.BaseLib.getfenv
fn getfenv(cf: &FrameRef, n: i32) -> R<i32> {
    let o = if n >= 1 {
        cf.get(0)
    } else {
        Object::Number(1.0)
    };
    let res = match &o {
        Object::Nil | Object::Function(_) => cf.coroutine.borrow().environment.clone(),
        Object::Closure(c) => Some(c.env.borrow().clone()),
        _ => {
            let d = raw_tonumber(&o)?;
            lua_assert(d.is_some(), "Expected number")?;
            let level = d.unwrap_or(0.0) as i32;
            lua_assert(level >= 0, "level must be non-negative")?;
            let idx = cf.coroutine.borrow().get_parent_frame(level)?;
            FrameRef {
                coroutine: cf.coroutine.clone(),
                idx,
            }
            .get_environment()
        }
    };
    cf.push(res.map_or(Object::Nil, Object::Table))?;
    Ok(1)
}

/// se.krka.kahlua.stdlib.BaseLib.unpack
fn unpack(cf: &FrameRef, n: i32) -> R<i32> {
    lua_assert(n >= 1, "Not enough arguments")?;
    let t = cast_table(&cf.get(0))?;
    let di = if n >= 2 { cf.get(1) } else { Object::Nil };
    let dj = if n >= 3 { cf.get(2) } else { Object::Nil };
    let i = if !di.is_nil() {
        from_double(&di)? as i32
    } else {
        1
    };
    let j = if !dj.is_nil() {
        from_double(&dj)? as i32
    } else {
        let t = t.as_ref().ok_or_else(|| {
            Throwable::npe(
                "Cannot invoke \"se.krka.kahlua.vm.KahluaTable.len()\" because \"t\" is null",
            )
        })?;
        KahluaTableImpl::len(t)?
    };
    let n_return_values = 1 + j - i;
    if n_return_values <= 0 {
        cf.set_top(0)?;
        return Ok(0);
    }
    cf.set_top(n_return_values)?;
    let t = t.ok_or_else(|| npe_rawget("t"))?;
    for b in 0..n_return_values {
        let v = KahluaTableImpl::rawget(&t, &Object::Number(f64::from(i + b)))?;
        cf.set(b, v);
    }
    Ok(n_return_values)
}

/// se.krka.kahlua.stdlib.BaseLib.error: throws KahluaException with the first argument (no position is added);
/// the second, if a string, becomes the coroutine's stack trace. No arguments: returns nothing.
fn error(cf: &FrameRef, n: i32) -> R<i32> {
    if n >= 1 {
        let stacktrace = get_optional_string_arg(cf, 2);
        cf.coroutine.borrow_mut().stack_trace =
            stacktrace.map_or(String::new(), |s| String::from_utf16_lossy(&s));
        Err(Throwable::Kahlua(cf.get(0)))
    } else {
        Ok(0)
    }
}

/// se.krka.kahlua.stdlib.BaseLib.pcall
fn pcall(thread: &mut KahluaThread, _cf: &FrameRef, n: i32) -> R<i32> {
    thread.pcall(n - 1)
}

/// se.krka.kahlua.stdlib.BaseLib.print: each argument through the global `tostring`, tab-separated.
fn print(thread: &mut KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
    let env = Object::Table(thread.get_environment());
    let to_string_fun = thread.tableget(&env, &Object::str("tostring"))?;
    let mut sb: Vec<u16> = Vec::new();
    for i in 0..n {
        if i > 0 {
            sb.push(u16::from(b'\t'));
        }
        let res = thread.call3(&to_string_fun, cf.get(i), Object::Nil, Object::Nil)?;
        match res {
            Object::String(s) => sb.extend_from_slice(&s),
            other => sb.extend(other.java_to_string().encode_utf16()),
        }
    }
    thread.println(&sb);
    Ok(0)
}

/// se.krka.kahlua.stdlib.BaseLib.select
fn select(cf: &FrameRef, n: i32) -> R<i32> {
    lua_assert(n >= 1, "Not enough arguments")?;
    let arg1 = cf.get(0);
    if let Object::String(s) = &arg1 {
        if s.first() == Some(&u16::from(b'#')) {
            cf.push(Object::Number(f64::from(n - 1)))?;
            return Ok(1);
        }
    }
    let d = raw_tonumber(&arg1)?;
    let d_index = from_double(&d.map_or(Object::Nil, Object::Number))?;
    let index = d_index as i32;
    Ok(if index >= 1 && index < n {
        n - index
    } else {
        0
    })
}

/// se.krka.kahlua.stdlib.BaseLib.getmetatable
fn getmetatable(thread: &mut KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
    lua_assert(n >= 1, "Not enough arguments")?;
    let m = thread.getmetatable(&cf.get(0), false)?;
    cf.push(m)?;
    Ok(1)
}

/// se.krka.kahlua.stdlib.BaseLib.setmetatable(LuaCallFrame, int)
fn setmetatable_(thread: &mut KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
    lua_assert(n >= 2, "Not enough arguments")?;
    let o = cf.get(0);
    let new_meta = cast_table(&cf.get(1))?;
    setmetatable(thread, &o, new_meta, false)?;
    cf.set_top(1)?;
    Ok(1)
}

/// se.krka.kahlua.stdlib.BaseLib.setmetatable(KahluaThread, Object, KahluaTable, boolean)
pub fn setmetatable(
    thread: &mut KahluaThread,
    o: &Object,
    new_meta: Option<TableRef>,
    raw: bool,
) -> R<()> {
    lua_assert(!o.is_nil(), "Expected table, got nil")?;
    let old_meta = thread.getmetatable(o, true)?;
    if !raw
        && !old_meta.is_nil()
        && !thread
            .tableget(&old_meta, &Object::str("__metatable"))?
            .is_nil()
    {
        return Err(Throwable::runtime("cannot change a protected metatable"));
    }
    thread.setmetatable(o, new_meta)
}

/// se.krka.kahlua.stdlib.BaseLib.type
fn type_(cf: &FrameRef, n: i32) -> R<i32> {
    lua_assert(n >= 1, "Not enough arguments")?;
    let t = type_name(&cf.get(0));
    cf.push(Object::str(t))?;
    Ok(1)
}

/// se.krka.kahlua.stdlib.BaseLib.tostring
fn tostring_(thread: &mut KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
    lua_assert(n >= 1, "Not enough arguments")?;
    let s = tostring(&cf.get(0), Some(thread))?;
    cf.push(Object::String(s))?;
    Ok(1)
}

/// se.krka.kahlua.stdlib.BaseLib.tonumber
fn tonumber(cf: &FrameRef, n: i32) -> R<i32> {
    lua_assert(n >= 1, "Not enough arguments")?;
    let o = cf.get(0);
    if n == 1 {
        let v = raw_tonumber(&o)?;
        cf.push(v.map_or(Object::Nil, Object::Number))?;
        return Ok(1);
    }
    let s = cast_string(&o)?;
    let radix_double = raw_tonumber(&cf.get(1))?;
    let Some(dradix) = radix_double else {
        cf.push(Object::Nil)?;
        return Ok(1);
    };
    let radix = dradix as i32;
    if f64::from(radix) != dradix {
        cf.push(Object::Nil)?;
        return Ok(1);
    }
    let s = s.ok_or_else(|| {
        Throwable::npe("Cannot invoke \"String.toLowerCase()\" because \"s\" is null")
    })?;
    let res = tonumber_radix(&s, radix)?;
    cf.push(res.map_or(Object::Nil, Object::Number))?;
    Ok(1)
}

/// se.krka.kahlua.stdlib.BaseLib.collectgarbage. "count" answers the JVM's memory figures in the original; the
/// port has no such heap and answers zeros (UNKNOWN to match).
fn collectgarbage(cf: &FrameRef, n: i32) -> R<i32> {
    let option = if n > 0 { cf.get(0) } else { Object::Nil };
    let is = |s: &str| matches!(&option, Object::String(o) if o[..] == lstr(s)[..]);
    if option.is_nil() || is("step") || is("collect") {
        return Ok(0);
    }
    if is("count") {
        cf.set_top(3)?;
        cf.set(0, Object::Number(0.0));
        cf.set(1, Object::Number(0.0));
        cf.set(2, Object::Number(0.0));
        return Ok(3);
    }
    Err(Throwable::runtime(format!(
        "invalid option: {}",
        option.java_to_string()
    )))
}

/// se.krka.kahlua.stdlib.BaseLib.bytecodeloader: precompiled chunks from the class path are not supported by
/// the port; with a `package.classpath` it answers that nothing was found.
fn bytecodeloader(cf: &FrameRef, _n: i32) -> R<i32> {
    let modname = get_string_arg(cf, 1, "loader")?;
    let env = cf
        .get_environment()
        .ok_or_else(|| npe_rawget("environment"))?;
    let package = cast_table(&KahluaTableImpl::rawget(&env, &Object::str("package"))?)?
        .ok_or_else(|| npe_rawget("packageTable"))?;
    let _classpath = cast_string(&KahluaTableImpl::rawget(
        &package,
        &Object::str("classpath"),
    )?)?;
    cf.push(Object::str(&format!(
        "Could not find the bytecode for '{}' in classpath",
        String::from_utf16_lossy(&modname)
    )))
}
