//! se.krka.kahlua.stdlib.StringLib: the `string` table, also the class metatable of strings (`("x"):sub(1)`).
//! Ported: sub, char, byte, lower, upper, reverse, trim, sort, contains. Not yet (they throw a "not ported"
//! RuntimeException, PLACEHOLDER): format, find, match, gsub, split (Lua patterns, Java's formatter and regex).
use std::rc::Rc;

use crate::se::krka::kahlua::j2se::j2se_platform::J2SEPlatform;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
use crate::se::krka::kahlua::vm::kahlua_util::{
    get_class_metatables, get_number_arg, get_optional_number_arg, get_string_arg, lua_assert,
};
use crate::se::krka::kahlua::vm::object::{
    classes, rstr, JavaClass, JavaFunction, LuaStr, Object, TableRef, Throwable, R,
};

pub static CLASS: JavaClass = JavaClass {
    name: "se.krka.kahlua.stdlib.StringLib",
    superclass: Some(&classes::OBJECT),
    interfaces: &[&classes::JAVA_FUNCTION],
};

const NAMES: [&str; 14] = [
    "sub", "char", "byte", "lower", "upper", "reverse", "format", "find", "match", "gsub", "trim",
    "split", "sort", "contains",
];

pub struct StringLib {
    method_id: usize,
}

/// se.krka.kahlua.stdlib.StringLib.register
pub fn register(platform: &J2SEPlatform, env: &TableRef) -> R<()> {
    let string = KahluaTableImpl::new_ref();
    for (i, name) in NAMES.iter().enumerate() {
        KahluaTableImpl::rawset(
            &string,
            Object::str(name),
            Object::Function(Rc::new(StringLib { method_id: i })),
        );
    }
    KahluaTableImpl::rawset(
        &string,
        Object::str("__index"),
        Object::Table(string.clone()),
    );
    let metatables = get_class_metatables(env)?;
    KahluaTableImpl::rawset(
        &metatables,
        platform.class_object(&classes::STRING),
        Object::Table(string.clone()),
    );
    KahluaTableImpl::rawset(env, Object::str("string"), Object::Table(string));
    Ok(())
}

impl JavaFunction for StringLib {
    /// se.krka.kahlua.stdlib.StringLib.call
    fn call(&self, _thread: &mut KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
        match self.method_id {
            0 => sub(cf, n),
            1 => string_char(cf, n),
            2 => string_byte(cf, n),
            3 => {
                lua_assert(n >= 1, "not enough arguments")?;
                let s = get_string_arg(cf, 1, NAMES[3])?;
                cf.push(same_or_new(
                    s.clone(),
                    &crate::java::to_lower_case(&rstr(&s)),
                ))?;
                Ok(1)
            }
            4 => {
                lua_assert(n >= 1, "not enough arguments")?;
                let s = get_string_arg(cf, 1, NAMES[4])?;
                cf.push(same_or_new(
                    s.clone(),
                    &crate::java::to_upper_case(&rstr(&s)),
                ))?;
                Ok(1)
            }
            5 => {
                lua_assert(n >= 1, "not enough arguments")?;
                let s = get_string_arg(cf, 1, NAMES[5])?;
                cf.push(Object::String(string_builder_reverse(&s)))?;
                Ok(1)
            }
            10 => {
                let s = get_string_arg(cf, 1, NAMES[10])?;
                cf.push(Object::String(java_trim(&s)))
            }
            12 => {
                let a = get_string_arg(cf, 1, NAMES[12])?;
                let b = get_string_arg(cf, 2, NAMES[12])?;
                cf.push(Object::Boolean(a[..].cmp(&b[..]).is_gt()))
            }
            13 => {
                lua_assert(n >= 2, "not enough arguments")?;
                let s = get_string_arg(cf, 1, NAMES[13])?;
                let s2 = get_string_arg(cf, 2, NAMES[13])?;
                let found = s2.is_empty() || s.windows(s2.len()).any(|w| w == &s2[..]);
                cf.push(Object::Boolean(found))?;
                Ok(1)
            }
            i => Err(Throwable::runtime(format!(
                "string.{} is not ported",
                NAMES[i]
            ))),
        }
    }
    /// se.krka.kahlua.stdlib.StringLib.toString
    fn java_to_string(&self) -> String {
        NAMES[self.method_id].to_string()
    }
    fn class(&self) -> &'static JavaClass {
        &CLASS
    }
}

/// `toLowerCase`/`toUpperCase` return the string itself when nothing changes.
fn same_or_new(orig: LuaStr, changed: &str) -> Object {
    let units: Vec<u16> = changed.encode_utf16().collect();
    if units[..] == orig[..] {
        Object::String(orig)
    } else {
        Object::String(units.into())
    }
}

/// `String.trim`: drops leading and trailing units up to U+0020.
pub fn java_trim(s: &LuaStr) -> LuaStr {
    let start = s.iter().position(|&c| c > 0x20).unwrap_or(s.len());
    let end = s.iter().rposition(|&c| c > 0x20).map_or(start, |e| e + 1);
    if start == 0 && end == s.len() {
        s.clone()
    } else {
        s[start..end].into()
    }
}

/// `new StringBuilder(s).reverse()`: units reversed, valid surrogate pairs kept in order.
pub fn string_builder_reverse(s: &[u16]) -> LuaStr {
    let mut v: Vec<u16> = s.iter().rev().copied().collect();
    let mut i = 0;
    while i + 1 < v.len() {
        if (0xDC00..0xE000).contains(&v[i]) && (0xD800..0xDC00).contains(&v[i + 1]) {
            v.swap(i, i + 1);
            i += 2;
        } else {
            i += 1;
        }
    }
    v.into()
}

/// se.krka.kahlua.stdlib.StringLib.stringByte
fn string_byte(cf: &FrameRef, n: i32) -> R<i32> {
    lua_assert(n >= 1, "not enough arguments")?;
    let s = get_string_arg(cf, 1, NAMES[2])?;
    let mut i = get_optional_number_arg(cf, 2)?.map_or(1, |d| d as i32);
    let mut j = get_optional_number_arg(cf, 3)?.map_or(i, |d| d as i32);
    let len = s.len() as i32;
    if i < 0 {
        i += len + 1;
    }
    if i <= 0 {
        i = 1;
    }
    if j < 0 {
        j += len + 1;
    } else if j > len {
        j = len;
    }
    let n_returns = 1 + j - i;
    if n_returns <= 0 {
        return Ok(0);
    }
    cf.set_top(n_returns)?;
    let offset = i - 1;
    for i2 in 0..n_returns {
        let c = s
            .get((offset + i2) as usize)
            .copied()
            .ok_or_else(|| Throwable::Java {
                class: "java.lang.StringIndexOutOfBoundsException",
                message: Some(format!(
                    "Index {} out of bounds for length {len}",
                    offset + i2
                )),
            })?;
        cf.set(i2, Object::Number(f64::from(c)));
    }
    Ok(n_returns)
}

/// se.krka.kahlua.stdlib.StringLib.stringChar: each number cast to a UTF-16 unit.
fn string_char(cf: &FrameRef, n: i32) -> R<i32> {
    let mut sb: Vec<u16> = Vec::with_capacity(n.max(0) as usize);
    for i in 0..n {
        let num = get_number_arg(cf, i + 1, NAMES[1])? as i32;
        sb.push(num as u16);
    }
    cf.push(Object::String(sb.into()))
}

/// se.krka.kahlua.stdlib.StringLib.sub
fn sub(cf: &FrameRef, n: i32) -> R<i32> {
    let s = get_string_arg(cf, 1, NAMES[0])?;
    let start = get_number_arg(cf, 2, NAMES[0])?;
    let end = if n >= 3 {
        get_number_arg(cf, 3, NAMES[0])?
    } else {
        -1.0
    };
    let mut istart = start as i32;
    let mut iend = end as i32;
    let len = s.len() as i32;
    if istart < 0 {
        istart = (len + istart + 1).max(1);
    } else if istart == 0 {
        istart = 1;
    }
    if iend < 0 {
        iend = (iend + len + 1).max(0);
    } else if iend > len {
        iend = len;
    }
    if istart > iend {
        return cf.push(Object::str(""));
    }
    // String.substring returns the string itself for the whole range.
    if istart == 1 && iend == len {
        return cf.push(Object::String(s));
    }
    let res: LuaStr = s[(istart - 1) as usize..iend as usize].into();
    cf.push(Object::String(res))
}
