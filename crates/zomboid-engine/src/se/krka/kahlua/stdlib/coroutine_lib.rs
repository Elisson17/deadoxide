//! se.krka.kahlua.stdlib.CoroutineLib: `coroutine.create`, `resume`, `yield`, `status`, `running` (`wrap` is
//! stdlib.lua). The `coroutine` table is also the class metatable of coroutines.
use std::rc::Rc;

use crate::se::krka::kahlua::j2se::j2se_platform::J2SEPlatform;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::coroutine::{Coroutine, FrameRef};
use crate::se::krka::kahlua::vm::kahlua_thread::{yield_helper, KahluaThread};
use crate::se::krka::kahlua::vm::kahlua_util::{fail, get_arg, get_class_metatables, lua_assert};
use crate::se::krka::kahlua::vm::object::{
    classes, CoroutineRef, JavaClass, JavaFunction, Object, TableRef, R,
};

pub static CLASS: JavaClass = JavaClass {
    name: "se.krka.kahlua.stdlib.CoroutineLib",
    superclass: Some(&classes::OBJECT),
    interfaces: &[&classes::JAVA_FUNCTION],
};

const NAMES: [&str; 5] = ["create", "resume", "yield", "status", "running"];

pub struct CoroutineLib {
    index: usize,
}

impl JavaFunction for CoroutineLib {
    /// se.krka.kahlua.stdlib.CoroutineLib.call
    fn call(&self, thread: &mut KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
        match self.index {
            0 => create(cf),
            1 => resume(thread, cf, n),
            2 => yield_int(thread, cf, n),
            3 => {
                let t = get_coroutine(cf, "status")?;
                if Rc::ptr_eq(&cf.coroutine, &t) {
                    cf.push(Object::str("running"))
                } else {
                    let s = t.borrow().get_status();
                    cf.push(Object::str(s))
                }
            }
            4 => {
                let t = cf.coroutine.clone();
                let status = t.borrow().get_status();
                cf.push(if status != "normal" {
                    Object::Nil
                } else {
                    Object::Coroutine(t)
                })
            }
            _ => Ok(0),
        }
    }
    /// se.krka.kahlua.stdlib.CoroutineLib.toString
    fn java_to_string(&self) -> String {
        format!("coroutine.{}", NAMES[self.index])
    }
    fn class(&self) -> &'static JavaClass {
        &CLASS
    }
}

/// se.krka.kahlua.stdlib.CoroutineLib.register
pub fn register(platform: &J2SEPlatform, env: &TableRef) -> R<()> {
    let coroutine = KahluaTableImpl::new_ref();
    for (i, name) in NAMES.iter().enumerate() {
        KahluaTableImpl::rawset(
            &coroutine,
            Object::str(name),
            Object::Function(Rc::new(CoroutineLib { index: i })),
        );
    }
    KahluaTableImpl::rawset(
        &coroutine,
        Object::str("__index"),
        Object::Table(coroutine.clone()),
    );
    let metatables = get_class_metatables(env)?;
    KahluaTableImpl::rawset(
        &metatables,
        platform.class_object(&classes::COROUTINE),
        Object::Table(coroutine.clone()),
    );
    KahluaTableImpl::rawset(env, Object::str("coroutine"), Object::Table(coroutine));
    Ok(())
}

/// se.krka.kahlua.stdlib.CoroutineLib.resume
fn resume(thread: &mut KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
    let t = get_coroutine(cf, "resume")?;
    let status = t.borrow().get_status();
    if status != "suspended" {
        return Err(fail(&format!(
            "Can not resume coroutine that is in status: {status}"
        )));
    }
    let parent = cf.coroutine.clone();
    t.borrow_mut().resume(parent);
    let idx = t.borrow().current_call_frame();
    let Some(idx) = idx else {
        return Ok(0);
    };
    let next = FrameRef {
        coroutine: t.clone(),
        idx,
    };
    if next.n_arguments() == -1 {
        next.set_top(0)?;
    }
    for i in 1..n {
        next.push(cf.get(i))?;
    }
    if next.n_arguments() == -1 {
        next.with_mut(|f| f.n_arguments = n - 1);
        next.init()?;
    }
    thread.current_coroutine = t;
    Ok(0)
}

/// se.krka.kahlua.stdlib.CoroutineLib.yield_int
fn yield_int(thread: &mut KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
    let t = cf.coroutine.clone();
    let parent = t.borrow().get_parent();
    lua_assert(parent.is_some(), "Can not yield outside of a coroutine")?;
    let real = t.borrow().get_call_frame(-2);
    let real_call_frame = FrameRef {
        coroutine: t,
        idx: real,
    };
    yield_helper(thread, &real_call_frame, cf, n)?;
    Ok(0)
}

/// se.krka.kahlua.stdlib.CoroutineLib.create
fn create(cf: &FrameRef) -> R<i32> {
    let o = get_arg(cf, 1, "create")?;
    lua_assert(
        matches!(o, Object::Closure(_)),
        "argument must be a lua function",
    )?;
    let Object::Closure(c) = o else {
        return Ok(0);
    };
    let co = Coroutine::new(cf.get_environment(), false);
    co.borrow_mut()
        .push_new_call_frame(Some(c), None, 0, 0, -1, true, true)?;
    cf.push(Object::Coroutine(co))?;
    Ok(1)
}

/// se.krka.kahlua.stdlib.CoroutineLib.getCoroutine
fn get_coroutine(cf: &FrameRef, name: &str) -> R<CoroutineRef> {
    let o = get_arg(cf, 1, name)?;
    lua_assert(
        matches!(o, Object::Coroutine(_)),
        "argument must be a coroutine",
    )?;
    match o {
        Object::Coroutine(c) => Ok(c),
        _ => Err(fail("argument must be a coroutine")),
    }
}
