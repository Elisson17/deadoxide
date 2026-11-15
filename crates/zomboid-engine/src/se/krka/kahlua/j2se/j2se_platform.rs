//! se.krka.kahlua.j2se.J2SEPlatform: makes tables and the standard Lua environment (`newEnvironment`).
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use super::kahlua_table_impl::KahluaTableImpl;
use super::math_lib;
use crate::se::krka::kahlua::luaj::compiler::lua_compiler::{self, CompileFiles};
use crate::se::krka::kahlua::stdlib::{base_lib, coroutine_lib, string_lib, table_lib};
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
use crate::se::krka::kahlua::vm::kahlua_util::get_class_metatables;
use crate::se::krka::kahlua::vm::object::{
    classes, ClassObject, JavaClass, JavaFunction, Object, TableRef, Throwable, Userdata, R,
};

/// se.krka.kahlua.j2se.J2SEPlatform, with what the original keeps in statics the environment depends on: one
/// `Class` object per class (the keys of `__classmetatables`) and `FuncState.currentFile`/`currentfullFile`.
#[derive(Default)]
pub struct J2SEPlatform {
    classes: RefCell<HashMap<&'static str, Object>>,
    pub compile_files: RefCell<CompileFiles>,
}

impl J2SEPlatform {
    pub fn new() -> Rc<J2SEPlatform> {
        Rc::new(J2SEPlatform::default())
    }

    /// The `java.lang.Class` object of `c` (always the same object for the same class).
    pub fn class_object(&self, c: &'static JavaClass) -> Object {
        self.classes
            .borrow_mut()
            .entry(c.name)
            .or_insert_with(|| Object::Userdata(Rc::new(ClassObject(c))))
            .clone()
    }

    /// se.krka.kahlua.j2se.J2SEPlatform.pow (`Math.pow`)
    pub fn pow(&self, x: f64, y: f64) -> f64 {
        crate::java::math_pow(x, y)
    }

    /// se.krka.kahlua.j2se.J2SEPlatform.newTable
    pub fn new_table(&self) -> TableRef {
        KahluaTableImpl::new_ref()
    }

    /// se.krka.kahlua.j2se.J2SEPlatform.newEnvironment (`stdlib.lua` is read from `game_root`, the original's
    /// working directory).
    pub fn new_environment(self: &Rc<Self>, game_root: &Path) -> R<TableRef> {
        let env = self.new_table();
        self.setup_environment(&env, game_root)?;
        Ok(env)
    }

    /// se.krka.kahlua.j2se.J2SEPlatform.setupEnvironment
    pub fn setup_environment(self: &Rc<Self>, env: &TableRef, game_root: &Path) -> R<()> {
        env.borrow_mut().wipe();
        KahluaTableImpl::rawset(env, Object::str("_G"), Object::Table(env.clone()));
        KahluaTableImpl::rawset(
            env,
            Object::str("_VERSION"),
            Object::str("Kahlua kahlua.major.kahlua.minor.kahlua.fix for Lua lua.version (J2SE)"),
        );
        math_lib::register(env);
        base_lib::register(env);
        register_random_lib(self, env)?;
        register_userdata_array(self, env)?;
        string_lib::register(self, env)?;
        coroutine_lib::register(self, env)?;
        register_os_lib(env);
        table_lib::register(env);
        lua_compiler::register(env);
        // setupWorkerThread: the worker thread is stored under a private key object.
        KahluaTableImpl::rawset(
            env,
            Object::Userdata(Rc::new(PlainObject(&OBJECT_KEY_CLASS))),
            Object::Userdata(Rc::new(PlainObject(&WORKER_THREAD_CLASS))),
        );
        // KahluaUtil.setupLibraryText: stdlib.lua on the worker thread.
        let path = game_root.join("stdlib.lua");
        let bytes = std::fs::read(&path).map_err(|e| {
            Throwable::runtime(format!(
                "java.io.FileNotFoundException: {} ({e})",
                path.display()
            ))
        })?;
        let chars = crate::java::utf8_decode_jdk(&bytes);
        let files = self.compile_files.borrow().clone();
        let closure = lua_compiler::loadis(chars, Some("stdlib.lua"), None, env.clone(), &files)?;
        let mut worker = KahluaThread::new(self.clone(), env.clone());
        worker.call3(
            &Object::Closure(closure),
            Object::Nil,
            Object::Nil,
            Object::Nil,
        )?;
        Ok(())
    }
}

static OBJECT_KEY_CLASS: JavaClass = JavaClass {
    name: "java.lang.Object",
    superclass: None,
    interfaces: &[],
};
static WORKER_THREAD_CLASS: JavaClass = JavaClass {
    name: "se.krka.kahlua.threading.BlockingKahluaThread",
    superclass: Some(&classes::OBJECT),
    interfaces: &[],
};

/// An object Lua only holds (the worker thread key and value).
struct PlainObject(&'static JavaClass);

impl Userdata for PlainObject {
    fn class(&self) -> &'static JavaClass {
        self.0
    }
    fn java_to_string(&self) -> String {
        format!("{}@0", self.0.name)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// A library function the port does not have yet: registered under its name so the environment has the original's
/// shape; calling it throws (PLACEHOLDER).
struct NotPorted {
    name: &'static str,
    class: &'static JavaClass,
}

impl JavaFunction for NotPorted {
    fn call(&self, _thread: &mut KahluaThread, _cf: &FrameRef, _n: i32) -> R<i32> {
        Err(Throwable::runtime(format!("{} is not ported", self.name)))
    }
    fn java_to_string(&self) -> String {
        format!("{}@0", self.class.name)
    }
    fn class(&self) -> &'static JavaClass {
        self.class
    }
}

fn not_ported(name: &'static str, class: &'static JavaClass) -> Object {
    Object::Function(Rc::new(NotPorted { name, class }))
}

static RANDOM_LIB_CLASS: JavaClass = JavaClass {
    name: "se.krka.kahlua.stdlib.RandomLib",
    superclass: Some(&classes::OBJECT),
    interfaces: &[&classes::JAVA_FUNCTION],
};
static RANDOM_CLASS: JavaClass = JavaClass {
    name: "java.util.Random",
    superclass: Some(&classes::OBJECT),
    interfaces: &[&classes::SERIALIZABLE],
};
static USERDATA_ARRAY_CLASS: JavaClass = JavaClass {
    name: "se.krka.kahlua.test.UserdataArray",
    superclass: Some(&classes::OBJECT),
    interfaces: &[&classes::JAVA_FUNCTION],
};
static VECTOR_CLASS: JavaClass = JavaClass {
    name: "java.util.Vector",
    superclass: Some(&classes::OBJECT),
    interfaces: &[],
};
static OS_LIB_CLASS: JavaClass = JavaClass {
    name: "se.krka.kahlua.stdlib.OsLib",
    superclass: Some(&classes::OBJECT),
    interfaces: &[&classes::JAVA_FUNCTION],
};

/// se.krka.kahlua.stdlib.RandomLib.register (its functions are PLACEHOLDER).
fn register_random_lib(platform: &J2SEPlatform, env: &TableRef) -> R<()> {
    let t = KahluaTableImpl::new_ref();
    KahluaTableImpl::rawset(
        &t,
        Object::str("random"),
        not_ported("random", &RANDOM_LIB_CLASS),
    );
    KahluaTableImpl::rawset(
        &t,
        Object::str("seed"),
        not_ported("seed", &RANDOM_LIB_CLASS),
    );
    KahluaTableImpl::rawset(&t, Object::str("__index"), Object::Table(t.clone()));
    let metatables = get_class_metatables(env)?;
    KahluaTableImpl::rawset(
        &metatables,
        platform.class_object(&RANDOM_CLASS),
        Object::Table(t),
    );
    KahluaTableImpl::rawset(
        env,
        Object::str("newrandom"),
        not_ported("newrandom", &RANDOM_LIB_CLASS),
    );
    Ok(())
}

/// se.krka.kahlua.test.UserdataArray.register (its functions are PLACEHOLDER).
fn register_userdata_array(platform: &J2SEPlatform, env: &TableRef) -> R<()> {
    let m = KahluaTableImpl::new_ref();
    KahluaTableImpl::rawset(&m, Object::str("__metatable"), Object::str("restricted"));
    for name in ["__len", "__index", "__newindex", "new", "push"] {
        KahluaTableImpl::rawset(
            &m,
            Object::str(name),
            not_ported(name, &USERDATA_ARRAY_CLASS),
        );
    }
    let metatables = get_class_metatables(env)?;
    KahluaTableImpl::rawset(
        &metatables,
        platform.class_object(&VECTOR_CLASS),
        Object::Table(m.clone()),
    );
    KahluaTableImpl::rawset(env, Object::str("array"), Object::Table(m));
    Ok(())
}

/// se.krka.kahlua.stdlib.OsLib.register (its functions are PLACEHOLDER).
fn register_os_lib(env: &TableRef) {
    let os = KahluaTableImpl::new_ref();
    for name in ["date", "difftime", "time"] {
        KahluaTableImpl::rawset(&os, Object::str(name), not_ported(name, &OS_LIB_CLASS));
    }
    KahluaTableImpl::rawset(env, Object::str("os"), Object::Table(os));
}
