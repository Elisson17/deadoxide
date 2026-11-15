//! se.krka.kahlua.vm.KahluaUtil: conversions and argument helpers of the VM and its libraries.
use std::rc::Rc;

use super::coroutine::FrameRef;
use super::kahlua_thread::KahluaThread;
use super::object::{identity_hash_code, lstr, rstr, LuaStr, Object, TableRef, Throwable, R};
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;

/// se.krka.kahlua.vm.KahluaUtil.fromDouble: `(Double) o` unboxed.
pub fn from_double(o: &Object) -> R<f64> {
    match o {
        Object::Number(d) => Ok(*d),
        Object::Nil => Err(Throwable::npe(
            "Cannot invoke \"java.lang.Double.doubleValue()\" because \"o\" is null",
        )),
        _ => Err(Throwable::class_cast(o, "java.lang.Double")),
    }
}

/// se.krka.kahlua.vm.KahluaUtil.toDouble(double), i.e. zombie.core.BoxedStaticValues.toDouble: small integral
/// values come from a cache of boxes, so -0.0 becomes +0.0; every other value is unchanged.
pub fn to_double(d: f64) -> f64 {
    if d == 0.0 {
        0.0
    } else {
        d
    }
}

/// se.krka.kahlua.vm.KahluaUtil.boolEval
pub fn bool_eval(o: &Object) -> bool {
    !matches!(o, Object::Nil | Object::Boolean(false))
}

/// se.krka.kahlua.vm.KahluaUtil.luaAssert
pub fn lua_assert(b: bool, msg: &str) -> R<()> {
    if b {
        Ok(())
    } else {
        Err(fail(msg))
    }
}

/// se.krka.kahlua.vm.KahluaUtil.fail(String): the exception it throws (`Core.debug` off).
pub fn fail(msg: &str) -> Throwable {
    Throwable::runtime(msg)
}

/// se.krka.kahlua.vm.KahluaUtil.round
pub fn round(x: f64) -> f64 {
    if x < 0.0 {
        -round(-x)
    } else {
        let x = x + 0.5;
        let x2 = x.floor();
        if x2 == x {
            x2 - ((x2 as i64 & 1) as f64)
        } else {
            x2
        }
    }
}

/// se.krka.kahlua.vm.KahluaUtil.isNegative
pub fn is_negative(v: f64) -> bool {
    (if v.is_nan() {
        0x7ff8_0000_0000_0000u64
    } else {
        v.to_bits()
    } as i64)
        < 0
}

/// se.krka.kahlua.vm.KahluaUtil.getOrCreateTable
pub fn get_or_create_table(env: &TableRef, name: &str) -> R<TableRef> {
    let key = Object::str(name);
    match KahluaTableImpl::rawget(env, &key)? {
        Object::Table(t) => Ok(t),
        _ => {
            let t = KahluaTableImpl::new_ref();
            KahluaTableImpl::rawset(env, key, Object::Table(t.clone()));
            Ok(t)
        }
    }
}

/// se.krka.kahlua.vm.KahluaUtil.getClassMetatables
pub fn get_class_metatables(env: &TableRef) -> R<TableRef> {
    get_or_create_table(env, "__classmetatables")
}

/// se.krka.kahlua.vm.KahluaUtil.numberToString
pub fn number_to_string(n: f64) -> String {
    if n.is_nan() {
        "nan".into()
    } else if n.is_infinite() {
        if is_negative(n) {
            "-inf".into()
        } else {
            "inf".into()
        }
    } else if n.floor() == n && n.abs() < 1.0E14 {
        (n as i64).to_string()
    } else {
        crate::java::double_to_string(n)
    }
}

/// se.krka.kahlua.vm.KahluaUtil.type
pub fn type_name(o: &Object) -> &'static str {
    match o {
        Object::Nil => "nil",
        Object::String(_) => "string",
        Object::Number(_) => "number",
        Object::Boolean(_) => "boolean",
        Object::Closure(_) | Object::Function(_) => "function",
        Object::Table(_) => "table",
        Object::Coroutine(_) => "coroutine",
        Object::Userdata(_) => "userdata",
    }
}

/// se.krka.kahlua.vm.KahluaUtil.tostring: a string object is returned itself.
pub fn tostring(o: &Object, thread: Option<&mut KahluaThread>) -> R<LuaStr> {
    Ok(match o {
        Object::Nil => lstr("nil"),
        Object::String(s) => s.clone(),
        Object::Number(d) => lstr(&number_to_string(*d)),
        Object::Boolean(b) => lstr(if *b { "true" } else { "false" }),
        Object::Closure(c) => lstr(&format!("closure 0x{}", identity_hash_code(c))),
        Object::Function(f) => lstr(&format!("function 0x{}", identity_hash_code(f))),
        _ => {
            if let Some(thread) = thread {
                if let Object::Table(_) = o {
                    let to_string_member = thread.tableget(o, &Object::str("tostring"))?;
                    if type_name(&to_string_member) == "function" {
                        let r =
                            thread.call3(&to_string_member, o.clone(), Object::Nil, Object::Nil)?;
                        return Ok(value_of(&r));
                    }
                }
                let tostring_fun = thread.get_meta_op(o, "__tostring")?;
                if !tostring_fun.is_nil() {
                    let r = thread.call3(&tostring_fun, o.clone(), Object::Nil, Object::Nil)?;
                    return Ok(value_of(&r));
                }
                if let Object::Table(table) = o {
                    let mut sb = table_to_string(o, thread)?;
                    sb.push_str("{ ");
                    let mut any_found = false;
                    for (key, value) in KahluaTableImpl::iterate_snapshot(table) {
                        if any_found {
                            sb.push_str(", ");
                        } else {
                            any_found = true;
                        }
                        let k = if matches!(key, Object::Table(_)) {
                            table_to_string(&key, thread)?
                        } else {
                            rstr(&tostring(&key, Some(thread))?)
                        };
                        sb.push_str(&k);
                        sb.push('=');
                        let v = if matches!(value, Object::Table(_)) {
                            table_to_string(&value, thread)?
                        } else {
                            rstr(&tostring(&value, Some(thread))?)
                        };
                        sb.push_str(&v);
                    }
                    sb.push_str(" }");
                    return Ok(lstr(&sb));
                }
            }
            lstr(&o.java_to_string())
        }
    })
}

/// `String.valueOf(o)` as a string object (a string is itself).
pub fn value_of(o: &Object) -> LuaStr {
    match o {
        Object::String(s) => s.clone(),
        _ => lstr(&o.java_to_string()),
    }
}

/// se.krka.kahlua.vm.KahluaUtil.tableToString
fn table_to_string(o: &Object, thread: &mut KahluaThread) -> R<String> {
    let hash = match o {
        Object::Table(t) => identity_hash_code(t),
        _ => 0,
    };
    Ok(match thread.tableget(o, &Object::str("Type"))? {
        Object::String(t) => format!("{} 0x{hash}", rstr(&t)),
        _ => format!("{} 0x{hash}", type_name(o)),
    })
}

/// se.krka.kahlua.vm.KahluaUtil.tonumber(String)
pub fn tonumber(s: &[u16]) -> R<Option<f64>> {
    tonumber_radix(s, 10)
}

/// se.krka.kahlua.vm.KahluaUtil.tonumber(String, int): `Double.parseDouble` in base 10, `Integer.parseInt`
/// otherwise; a string the parser rejects is still NaN when it ends in "nan" and infinite when it ends in "inf".
pub fn tonumber_radix(s: &[u16], radix: i32) -> R<Option<f64>> {
    if !(2..=36).contains(&radix) {
        return Err(Throwable::runtime("base out of range"));
    }
    let text = rstr(s);
    let parsed = if radix == 10 {
        crate::java::parse_double(&text)
    } else {
        crate::java::parse_long(&text, radix as u32)
            .filter(|v| i32::try_from(*v).is_ok())
            .map(|v| v as f64)
    };
    if parsed.is_some() {
        return Ok(parsed.map(to_double));
    }
    let lower = crate::java::to_lower_case(&text);
    Ok(if lower.ends_with("nan") {
        Some(f64::NAN)
    } else if lower.ends_with("inf") {
        Some(if lower.starts_with('-') {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        })
    } else {
        None
    })
}

/// se.krka.kahlua.vm.KahluaUtil.rawTostring: a string is itself, a number its text, anything else null.
pub fn raw_tostring(o: &Object) -> Option<LuaStr> {
    match o {
        Object::String(s) => Some(s.clone()),
        Object::Number(d) => Some(lstr(&number_to_string(*d))),
        _ => None,
    }
}

/// se.krka.kahlua.vm.KahluaUtil.rawTonumber
pub fn raw_tonumber(o: &Object) -> R<Option<f64>> {
    match o {
        Object::Number(d) => Ok(Some(*d)),
        Object::String(s) => tonumber(s),
        _ => Ok(None),
    }
}

/// se.krka.kahlua.vm.KahluaUtil.fail(int, String, String, String)
fn fail_arg(n: i32, function: &str, wanted_type: &str, gotten: &str) -> Throwable {
    Throwable::runtime(format!(
        "bad argument #{n} to '{function}' ({wanted_type} expected, got {gotten})"
    ))
}

/// se.krka.kahlua.vm.KahluaUtil.getStringArg (the "got" of its message is the type of the failed conversion's
/// null, always "nil").
pub fn get_string_arg(frame: &FrameRef, n: i32, function: &str) -> R<LuaStr> {
    let o = get_arg(frame, n, function)?;
    raw_tostring(&o).ok_or_else(|| fail_arg(n, function, "string", "nil"))
}

/// se.krka.kahlua.vm.KahluaUtil.getOptionalStringArg
pub fn get_optional_string_arg(frame: &FrameRef, n: i32) -> Option<LuaStr> {
    raw_tostring(&get_optional_arg(frame, n))
}

/// se.krka.kahlua.vm.KahluaUtil.getNumberArg
pub fn get_number_arg(frame: &FrameRef, n: i32, function: &str) -> R<f64> {
    let o = get_arg(frame, n, function)?;
    raw_tonumber(&o)?.ok_or_else(|| fail_arg(n, function, "double", "nil"))
}

/// se.krka.kahlua.vm.KahluaUtil.getOptionalNumberArg
pub fn get_optional_number_arg(frame: &FrameRef, n: i32) -> R<Option<f64>> {
    raw_tonumber(&get_optional_arg(frame, n))
}

/// se.krka.kahlua.vm.KahluaUtil.getOptionalArg
pub fn get_optional_arg(frame: &FrameRef, n: i32) -> Object {
    let top = frame.get_top();
    let index = n - 1;
    if index >= top {
        Object::Nil
    } else {
        frame.get(n - 1)
    }
}

/// se.krka.kahlua.vm.KahluaUtil.getArg
pub fn get_arg(frame: &FrameRef, n: i32, function: &str) -> R<Object> {
    let res = get_optional_arg(frame, n);
    if res.is_nil() {
        Err(Throwable::runtime(format!(
            "missing argument #{n} to '{function}'"
        )))
    } else {
        Ok(res)
    }
}

/// se.krka.kahlua.vm.KahluaUtil.len: binary search for a border between `low` and `high`, then a walk up.
pub fn len(table: &TableRef, low: i32, high: i32) -> R<i32> {
    let (mut low, mut high) = (low, high);
    while low < high {
        let middle = (high.wrapping_add(low).wrapping_add(1)) >> 1;
        if KahluaTableImpl::rawget(table, &Object::Number(f64::from(middle)))?.is_nil() {
            high = middle - 1;
        } else {
            low = middle;
        }
    }
    while !KahluaTableImpl::rawget(table, &Object::Number(f64::from(low + 1)))?.is_nil() {
        low += 1;
    }
    Ok(low)
}

/// se.krka.kahlua.vm.KahluaUtil.getDoubleArg
pub fn get_double_arg(frame: &FrameRef, i: i32, name: &str) -> R<f64> {
    get_number_arg(frame, i, name)
}

/// A host function object made from a Rust function (for the libraries' function tables).
pub struct NamedFunction {
    pub name: String,
    pub class: &'static super::object::JavaClass,
    pub f: fn(&mut KahluaThread, &FrameRef, i32) -> R<i32>,
}

impl super::object::JavaFunction for NamedFunction {
    fn call(&self, thread: &mut KahluaThread, frame: &FrameRef, n_arguments: i32) -> R<i32> {
        (self.f)(thread, frame, n_arguments)
    }
    fn java_to_string(&self) -> String {
        self.name.clone()
    }
    fn class(&self) -> &'static super::object::JavaClass {
        self.class
    }
}

/// A library function as a Lua value.
pub fn function(
    name: &str,
    class: &'static super::object::JavaClass,
    f: fn(&mut KahluaThread, &FrameRef, i32) -> R<i32>,
) -> Object {
    Object::Function(Rc::new(NamedFunction {
        name: name.to_string(),
        class,
        f,
    }))
}
