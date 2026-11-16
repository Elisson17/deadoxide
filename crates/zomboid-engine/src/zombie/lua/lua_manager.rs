//! zombie.Lua.LuaManager: the game's Lua state and the loading of `media/lua/<dir>` (`LoadDirBase`, `RunLua`,
//! `require`). The original keeps all of it in statics; here one value per Lua state. Mods are not loaded yet
//! (`ZomboidFileSystem.getModIDs` is empty, as in the oracle). Logging goes to `log` instead of the game's logger.
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use super::exposer::{self, JavaStatics};
use super::lua_event_manager::EventRegistry;
use crate::se::krka::kahlua::j2se::j2se_platform::J2SEPlatform;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::luaj::compiler::lua_compiler;
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
use crate::se::krka::kahlua::vm::kahlua_util::get_string_arg;
use crate::se::krka::kahlua::vm::object::{classes, JavaClass, JavaFunction, Object, TableRef, R};

/// What LuaManager keeps about loading: ZomboidFileSystem's file map for `media/lua`, the search paths, and the
/// loaded/loading sets.
pub struct Loader {
    /// ZomboidFileSystem.activeFileMap: lower-case relative path ("media/lua/shared/foo.lua") → absolute path.
    file_map: HashMap<String, String>,
    game_root: PathBuf,
    /// LuaManager.paths ("media/lua/shared/", ...).
    pub paths: Vec<String>,
    /// LuaManager.loaded, in order.
    pub loaded: Vec<String>,
    loaded_set: HashSet<String>,
    loading: HashSet<String>,
    loaded_return: HashMap<String, Object>,
    /// Warnings and errors the original logs.
    pub log: Vec<String>,
}

impl Loader {
    fn new(game_root: &Path) -> Loader {
        // ZomboidFileSystem.base is the canonical game directory: the paths the scripts are run from are absolute.
        let game_root = game_root
            .canonicalize()
            .unwrap_or_else(|_| game_root.to_path_buf());
        let game_root = game_root.as_path();
        let mut file_map = HashMap::new();
        let mut files = Vec::new();
        walk(&game_root.join("media/lua"), &mut files);
        for f in files {
            if let Ok(rel) = f.strip_prefix(game_root) {
                let rel = rel.to_string_lossy().replace('\\', "/");
                file_map.insert(rel.to_lowercase(), f.to_string_lossy().replace('\\', "/"));
            }
        }
        Loader {
            file_map,
            game_root: game_root.to_path_buf(),
            paths: Vec::new(),
            loaded: Vec::new(),
            loaded_set: HashSet::new(),
            loading: HashSet::new(),
            loaded_return: HashMap::new(),
            log: Vec::new(),
        }
    }

    /// ZomboidFileSystem.base.canonicalFile: the game directory.
    pub fn game_root(&self) -> &Path {
        &self.game_root
    }

    /// zombie.ZomboidFileSystem.getAbsolutePath
    pub fn get_absolute_path(&self, rel: &str) -> Option<String> {
        self.file_map.get(&rel.to_lowercase()).cloned()
    }

    /// zombie.ZomboidFileSystem.getMediaFile(path).getAbsolutePath(): `<game>/media/<path>` (no mods).
    pub fn media_file(&self, path: &str) -> String {
        self.game_root
            .join("media")
            .join(path)
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// `LuaManager.loaded.remove(file)`: the file runs again on the next RunLua.
    pub fn remove_loaded(&mut self, file: &str) {
        if let Some(i) = self.loaded.iter().position(|f| f == file) {
            self.loaded.remove(i);
        }
        self.loaded_set.remove(file);
        self.loaded_return.remove(file);
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else {
                out.push(p);
            }
        }
    }
}

/// String.CASE_INSENSITIVE_ORDER
fn case_insensitive_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (a, b): (Vec<u16>, Vec<u16>) = (a.encode_utf16().collect(), b.encode_utf16().collect());
    for (&c1, &c2) in a.iter().zip(b.iter()) {
        if c1 != c2 {
            let up = |c: u16| {
                char::from_u32(u32::from(c)).map_or(c, |ch| {
                    let mut u = ch.to_uppercase();
                    match (u.next(), u.next()) {
                        (Some(x), None) if (x as u32) < 0x10000 => x as u32 as u16,
                        _ => c,
                    }
                })
            };
            let low = |c: u16| {
                char::from_u32(u32::from(c)).map_or(c, |ch| {
                    let mut l = ch.to_lowercase();
                    match (l.next(), l.next()) {
                        (Some(x), None) if (x as u32) < 0x10000 => x as u32 as u16,
                        _ => c,
                    }
                })
            };
            let (u1, u2) = (up(c1), up(c2));
            if u1 != u2 {
                let (l1, l2) = (low(u1), low(u2));
                if l1 != l2 {
                    return l1.cmp(&l2);
                }
            }
        }
    }
    a.len().cmp(&b.len())
}

/// The game's Lua state: LuaManager.platform, env, thread, the event registries and the loader.
pub struct LuaManager {
    pub platform: Rc<J2SEPlatform>,
    pub env: TableRef,
    pub thread: KahluaThread,
    pub events: Rc<EventRegistry>,
    pub hooks: Rc<EventRegistry>,
    pub loader: Rc<RefCell<Loader>>,
    /// The Java side the exposed API reaches.
    pub statics: Rc<JavaStatics>,
}

impl LuaManager {
    /// zombie.Lua.LuaManager.init: a new platform and environment, `Events`, `Hook`, then the exposed Java API
    /// (`exposer`; reading SandboxOptions.instance runs `defines.lua` and `Sandbox/Apocalypse.lua`).
    pub fn init(game_root: &Path) -> R<LuaManager> {
        Self::init_after(game_root, None)
    }

    /// zombie.Lua.LuaManager.init in a client whose startup steps ran before it (GameWindow.init): the exposed
    /// Java API reads what they left (`startup`).
    pub fn init_after(
        game_root: &Path,
        startup: Option<Rc<crate::zombie::game_window::ClientStartup>>,
    ) -> R<LuaManager> {
        let platform = J2SEPlatform::new();
        let env = platform.new_environment(game_root)?;
        let mut thread = KahluaThread::new(platform.clone(), env.clone());
        let events = EventRegistry::register("Events", &env)?;
        let hooks = EventRegistry::register("Hook", &env)?;
        let loader = Rc::new(RefCell::new(Loader::new(game_root)));
        KahluaTableImpl::rawset(
            &env,
            Object::str("require"),
            Object::Function(Rc::new(Require(loader.clone(), Rc::downgrade(&env)))),
        );
        let statics = JavaStatics::new(platform.clone(), loader.clone(), &env, events.clone());
        if let Some(startup) = startup {
            if let Some(tm) = startup.text_manager.borrow_mut().take() {
                statics.set_singleton(
                    "zombie.ui.TextManager",
                    Object::Userdata(Rc::new(super::exposed_client::TextManagerObject(
                        RefCell::new(tm),
                    ))),
                );
            }
            *statics.state.client.borrow_mut() = Some(startup);
        }
        exposer::expose_all(&statics, &mut thread)?;
        Ok(LuaManager {
            platform,
            env,
            thread,
            events,
            hooks,
            loader,
            statics,
        })
    }

    /// zombie.Lua.LuaManager.LoadDirBase(String): every `.lua` file under `media/lua/<sub>` (lower-case relative
    /// paths in case-insensitive order), each run once.
    pub fn load_dir_base(&mut self, sub: &str) -> R<()> {
        let rel_path = format!("media/lua/{sub}/");
        let game_files = {
            let mut l = self.loader.borrow_mut();
            if !l.paths.contains(&rel_path) {
                l.paths.push(rel_path);
            }
            let mut files = Vec::new();
            walk(&l.game_root.join("media/lua").join(sub), &mut files);
            let mut list: Vec<String> = files
                .iter()
                .filter(|f| f.to_string_lossy().to_lowercase().ends_with(".lua"))
                .filter_map(|f| f.strip_prefix(&l.game_root).ok())
                .map(|r| r.to_string_lossy().replace('\\', "/").to_lowercase())
                .collect();
            list.sort_by(|a, b| case_insensitive_cmp(a, b));
            list
        };
        let mut done = HashSet::new();
        for s in game_files {
            if done.insert(s.clone()) {
                let abs = self.loader.borrow().get_absolute_path(&s);
                if let Some(abs) = abs {
                    run_lua(
                        &self.loader,
                        &self.platform,
                        &self.env,
                        &mut self.thread,
                        &abs,
                    )?;
                }
            }
        }
        Ok(())
    }
}

/// zombie.Lua.LuaManager.RunLua(String, false)
pub fn run_lua(
    loader: &Rc<RefCell<Loader>>,
    platform: &Rc<J2SEPlatform>,
    env: &TableRef,
    thread: &mut KahluaThread,
    filename: &str,
) -> R<Object> {
    let filename1 = filename.replace('\\', "/");
    if loader.borrow().loading.contains(&filename1) {
        loader
            .borrow_mut()
            .log
            .push(format!("recursive require(): {filename1}"));
        return Ok(Object::Nil);
    }
    loader.borrow_mut().loading.insert(filename1.clone());
    let r = run_lua_internal(loader, platform, env, thread, filename);
    loader.borrow_mut().loading.remove(&filename1);
    r
}

/// zombie.Lua.LuaManager.RunLuaInternal: compile (the chunk is named after the file), run protected, remember.
fn run_lua_internal(
    loader: &Rc<RefCell<Loader>>,
    platform: &Rc<J2SEPlatform>,
    env: &TableRef,
    thread: &mut KahluaThread,
    filename: &str,
) -> R<Object> {
    let filename = filename.replace('\\', "/");
    if loader.borrow().loaded_set.contains(&filename) {
        return Ok(loader
            .borrow()
            .loaded_return
            .get(&filename)
            .cloned()
            .unwrap_or_default());
    }
    let base = filename[filename.rfind('/').map_or(0, |i| i + 1)..].to_string();
    {
        let mut files = platform.compile_files.borrow_mut();
        files.current_file = Some(base.clone());
        files.currentfull_file = Some(filename.clone());
    }
    let bytes = match std::fs::read(&filename) {
        Ok(b) => b,
        Err(e) => {
            loader
                .borrow_mut()
                .log
                .push(format!("java.io.FileNotFoundException: {filename} ({e})"));
            return Ok(Object::Nil);
        }
    };
    let chars = crate::java::utf8_decode_jdk(&bytes);
    let files = platform.compile_files.borrow().clone();
    let closure = match lua_compiler::loadis(chars, Some(&base), None, env.clone(), &files) {
        Ok(c) => c,
        Err(e) => {
            loader.borrow_mut().log.push(format!(
                "Error found in LUA file: {filename}: {:?}",
                e.get_message()
            ));
            return Ok(Object::Nil);
        }
    };
    let ret = thread.pcall_args(&Object::Closure(closure), &[])?;
    let success = matches!(ret.first(), Some(Object::Boolean(true)));
    if !success {
        let msg = ret.get(1).map(Object::java_to_string).unwrap_or_default();
        loader.borrow_mut().log.push(format!("{base}: {msg}"));
    }
    let ret_val = if success {
        ret.get(1).cloned().unwrap_or_default()
    } else {
        Object::Nil
    };
    let mut l = loader.borrow_mut();
    l.loaded.push(filename.clone());
    l.loaded_set.insert(filename.clone());
    if ret_val.is_nil() {
        l.loaded_return.remove(&filename);
    } else {
        l.loaded_return.insert(filename, ret_val.clone());
    }
    Ok(ret_val)
}

static GLOBAL_OBJECT_CLASS: JavaClass = JavaClass {
    name: "se.krka.kahlua.integration.expose.LuaJavaInvoker",
    superclass: Some(&classes::OBJECT),
    interfaces: &[&classes::JAVA_FUNCTION],
};

/// zombie.Lua.LuaManager.GlobalObject.require: `<path>.lua` in each search path, run once (in LuaManager.env:
/// weak, as the environment holds this function).
pub struct Require(Rc<RefCell<Loader>>, Weak<RefCell<KahluaTableImpl>>);

impl JavaFunction for Require {
    /// zombie.Lua.LuaManager.GlobalObject.require
    fn call(&self, thread: &mut KahluaThread, cf: &FrameRef, _n: i32) -> R<i32> {
        let f = crate::se::krka::kahlua::vm::object::rstr(&get_string_arg(cf, 1, "require")?);
        let f_copy = if f.ends_with(".lua") {
            f.clone()
        } else {
            format!("{f}.lua")
        };
        let paths = self.0.borrow().paths.clone();
        for rel in paths {
            let abs = self.0.borrow().get_absolute_path(&format!("{rel}{f_copy}"));
            if let Some(abs) = abs {
                let platform = thread.platform.clone();
                let Some(env) = self.1.upgrade() else {
                    break;
                };
                let r = run_lua(&self.0, &platform, &env, thread, &abs)?;
                cf.push(r)?;
                return Ok(1);
            }
        }
        self.0
            .borrow_mut()
            .log
            .push(format!("require(\"{f}\") failed"));
        cf.push(Object::Nil)?;
        Ok(1)
    }
    fn java_to_string(&self) -> String {
        "require".into()
    }
    fn class(&self) -> &'static JavaClass {
        &GLOBAL_OBJECT_CLASS
    }
}
