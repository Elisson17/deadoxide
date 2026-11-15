//! se.krka.kahlua.j2se.J2SEPlatform: makes tables and the standard Lua environment (`newEnvironment`).
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use super::kahlua_table_impl::KahluaTableImpl;
use super::math_lib;
use crate::se::krka::kahlua::luaj::compiler::lua_compiler::{self, CompileFiles};
use crate::se::krka::kahlua::stdlib::{
    base_lib, coroutine_lib, os_lib, random_lib, string_lib, table_lib,
};
use crate::se::krka::kahlua::test::userdata_array;
use crate::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
use crate::se::krka::kahlua::vm::object::{
    classes, ClassObject, JavaClass, Object, TableRef, Throwable, Userdata, R,
};

/// se.krka.kahlua.j2se.J2SEPlatform, with what the original keeps in statics the environment depends on: one
/// `Class` object per class (the keys of `__classmetatables`) and `FuncState.currentFile`/`currentfullFile`.
#[derive(Default)]
pub struct J2SEPlatform {
    classes: RefCell<HashMap<&'static str, Object>>,
    pub compile_files: RefCell<CompileFiles>,
    /// `System.currentTimeMillis()` for `os.time`/`os.date`, set by the host (the original reads the wall
    /// clock); 0 until set (PLACEHOLDER).
    pub current_time_millis: Cell<i64>,
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
        random_lib::register(self, env)?;
        userdata_array::register(self, env)?;
        string_lib::register(self, env)?;
        coroutine_lib::register(self, env)?;
        os_lib::register(env);
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
