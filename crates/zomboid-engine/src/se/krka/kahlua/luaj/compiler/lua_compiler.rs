//! se.krka.kahlua.luaj.compiler.LuaCompiler: Lua source to a closure, and the global `loadstring`/`loadstream`.
use std::rc::Rc;

use crate::org::luaj::kahluafork::compiler::lex_state::compile;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
use crate::se::krka::kahlua::vm::kahlua_util::lua_assert;
use crate::se::krka::kahlua::vm::lua_closure::{LuaClosure, Proto};
use crate::se::krka::kahlua::vm::object::{
    cast_string, classes, rstr, JavaClass, JavaFunction, Object, TableRef, Throwable, R,
};

pub static CLASS: JavaClass = JavaClass {
    name: "se.krka.kahlua.luaj.compiler.LuaCompiler",
    superclass: Some(&classes::OBJECT),
    interfaces: &[&classes::JAVA_FUNCTION],
};

const NAMES: [&str; 2] = ["loadstring", "loadstream"];

/// The file statics of `org.luaj.kahluafork.compiler.FuncState` the compiler stamps on prototypes
/// (`currentFile`, `currentfullFile`); `LuaManager` sets them before it loads a file.
#[derive(Clone, Debug, Default)]
pub struct CompileFiles {
    pub current_file: Option<String>,
    pub currentfull_file: Option<String>,
}

pub struct LuaCompiler {
    index: usize,
}

/// se.krka.kahlua.luaj.compiler.LuaCompiler.register
pub fn register(env: &TableRef) {
    for (i, name) in NAMES.iter().enumerate() {
        KahluaTableImpl::rawset(
            env,
            Object::str(name),
            Object::Function(Rc::new(LuaCompiler { index: i })),
        );
    }
}

/// The compiler's error string as the exception it stands for.
fn compile_error(e: String) -> Throwable {
    if let Some(m) = e.strip_prefix("java.lang.ArrayIndexOutOfBoundsException: ") {
        return Throwable::Java {
            class: "java.lang.ArrayIndexOutOfBoundsException",
            message: Some(m.to_string()),
        };
    }
    if let Some(m) = e.strip_prefix("java.lang.NullPointerException: ") {
        return Throwable::npe(m.to_string());
    }
    Throwable::Kahlua(Object::str(&e))
}

/// se.krka.kahlua.luaj.compiler.LuaCompiler.loadis(Reader, String, String, KahluaTable): `chars` is what the
/// reader yields.
pub fn loadis(
    chars: Vec<u16>,
    name: Option<&str>,
    source: Option<&str>,
    environment: TableRef,
    files: &CompileFiles,
) -> R<Rc<LuaClosure>> {
    let p = compile(
        chars,
        name,
        source,
        files.current_file.as_deref(),
        files.currentfull_file.as_deref(),
    )
    .map_err(compile_error)?;
    Ok(LuaClosure::new(
        Proto::from_prototype(p),
        environment,
        Vec::new(),
    ))
}

/// se.krka.kahlua.luaj.compiler.LuaCompiler.loadstring(String, String, KahluaTable): the source goes through
/// UTF-8 bytes and the default (UTF-8) reader.
pub fn loadstring(
    source: &[u16],
    name: Option<&str>,
    environment: TableRef,
    files: &CompileFiles,
) -> R<Rc<LuaClosure>> {
    let text = rstr(source);
    loadis(
        text.encode_utf16().collect(),
        name,
        Some(&text),
        environment,
        files,
    )
}

impl JavaFunction for LuaCompiler {
    /// se.krka.kahlua.luaj.compiler.LuaCompiler.call: a failure answers nil and the message.
    fn call(&self, thread: &mut KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
        let attempt = (|| -> R<i32> {
            if self.index == 1 {
                lua_assert(n >= 2, "not enough arguments")?;
                let input = cf.get(0);
                lua_assert(!input.is_nil(), "No input given")?;
                let _name = cast_string(&cf.get(1))?;
                return Err(Throwable::runtime(format!(
                    "Invalid type to loadstream: class {}",
                    input.class().map_or("null", |c| c.name)
                )));
            }
            lua_assert(n >= 1, "not enough arguments")?;
            let source = cast_string(&cf.get(0))?;
            lua_assert(source.is_some(), "No source given")?;
            let name = if n >= 2 {
                cast_string(&cf.get(1))?
            } else {
                None
            };
            let env = cf
                .get_environment()
                .unwrap_or_else(KahluaTableImpl::new_ref);
            let name = name.map(|s| rstr(&s));
            let files = thread.platform.compile_files.borrow().clone();
            let closure = loadstring(
                &source.unwrap_or_else(|| Rc::from([])),
                name.as_deref(),
                env,
                &files,
            )?;
            cf.push(Object::Closure(closure))
        })();
        match attempt {
            Ok(n) => Ok(n),
            Err(e @ Throwable::Java { class, .. }) if class.ends_with("Error") => Err(e),
            Err(e) => cf.push2(
                Object::Nil,
                e.get_message().map_or(Object::Nil, |m| Object::str(&m)),
            ),
        }
    }
    /// Object.toString (LuaCompiler has none of its own).
    fn java_to_string(&self) -> String {
        format!("se.krka.kahlua.luaj.compiler.LuaCompiler@{:x}", self.index)
    }
    fn class(&self) -> &'static JavaClass {
        &CLASS
    }
}
