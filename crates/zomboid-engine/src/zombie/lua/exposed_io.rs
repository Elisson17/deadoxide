//! The Lua file API over the cache folder: zombie.Lua.LuaManager.GlobalObject's `cacheFileExists`, `getFileReader`
//! and `getFileWriter` with the `java.io.BufferedReader` and `LuaManager$GlobalObject$LuaFileWriter` objects they
//! return. Same conventions as `exposed_methods`.
use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::exposed_methods::{bool_arg, ret, ret_str, string_arg_nn, ud, want_args, with_this};
use super::exposer::{class_named, Fail, Imp, JavaStatics};
use crate::se::krka::kahlua::vm::object::{JavaClass, Object, Throwable, Userdata};

/// zombie.Lua.LuaManager.getLuaCacheDir: `<cache dir>/Lua`, made (mkdir) when missing.
pub fn lua_cache_dir(s: &Rc<JavaStatics>) -> Result<PathBuf, Fail> {
    let dir = Path::new(&s.cache_dir()?).join("Lua");
    if !dir.exists() {
        let _ = std::fs::create_dir(&dir);
    }
    Ok(dir)
}

/// zombie.Lua.LuaManager.GlobalObject.hasRelativePath: `StringUtils.containsDoubleDot(path)` (a warning is logged).
fn has_relative_path(s: &Rc<JavaStatics>, path: &str) -> bool {
    if path.contains("..") || path.contains("\u{0}.\u{0}.") {
        s.loader
            .borrow_mut()
            .log
            .push("Lua: relative paths not allowed".to_string());
        true
    } else {
        false
    }
}

/// `filename.replace("/", File.separator).replace("\\", File.separator)` (the separator is "/" on this platform).
fn to_native(filename: &str) -> String {
    let sep = std::path::MAIN_SEPARATOR.to_string();
    filename.replace(['/', '\\'], &sep)
}

/// `java.io.BufferedReader` over a UTF-8 `InputStreamReader`, as `getFileReader` makes it: the text is decoded when
/// the reader is made (malformed input becomes U+FFFD; UNKNOWN whether the JDK replaces the same ranges).
pub struct BufferedReaderObject {
    text: String,
    pos: RefCell<usize>,
    closed: RefCell<bool>,
}

impl Userdata for BufferedReaderObject {
    fn class(&self) -> &'static JavaClass {
        class_named("java.io.BufferedReader")
    }
    fn java_to_string(&self) -> String {
        format!(
            "java.io.BufferedReader@{:x}",
            super::exposed_methods::hash(self)
        )
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl BufferedReaderObject {
    /// java.io.BufferedReader.readLine: the text up to a `\n`, `\r` or `\r\n` (not included); null at the end of the
    /// stream; an IOException ("Stream closed") once closed.
    fn read_line(&self) -> Result<Option<String>, Fail> {
        if *self.closed.borrow() {
            return Err(Fail::Java(Throwable::Java {
                class: "java.io.IOException",
                message: Some("Stream closed".to_string()),
            }));
        }
        let mut pos = self.pos.borrow_mut();
        let rest = &self.text[*pos..];
        if rest.is_empty() {
            return Ok(None);
        }
        let (line, skip) = match rest.find(['\n', '\r']) {
            Some(i) => {
                let skip = if rest[i..].starts_with("\r\n") { 2 } else { 1 };
                (&rest[..i], i + skip)
            }
            None => (rest, rest.len()),
        };
        *pos += skip;
        Ok(Some(line.to_string()))
    }
}

/// `LuaManager.GlobalObject.LuaFileWriter` over `PrintWriter(OutputStreamWriter(FileOutputStream(file, append),
/// UTF-8))`. The file is opened (truncated unless appending) when the writer is made; the text written goes to the
/// file when the writer is closed (UNKNOWN: the original's encoder buffer is flushed earlier when it fills, and a
/// writer that is never closed loses its last buffer).
pub struct LuaFileWriterObject {
    file: RefCell<Option<std::fs::File>>,
    buffer: RefCell<Vec<u8>>,
}

impl Userdata for LuaFileWriterObject {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.Lua.LuaManager$GlobalObject$LuaFileWriter")
    }
    fn java_to_string(&self) -> String {
        format!(
            "zombie.Lua.LuaManager$GlobalObject$LuaFileWriter@{:x}",
            super::exposed_methods::hash(self)
        )
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl LuaFileWriterObject {
    /// PrintWriter.write(String): a closed writer ignores the write (the error is only flagged).
    fn write(&self, text: &str) {
        if self.file.borrow().is_some() {
            self.buffer.borrow_mut().extend_from_slice(text.as_bytes());
        }
    }

    /// PrintWriter.close.
    fn close(&self) {
        if let Some(mut f) = self.file.borrow_mut().take() {
            let _ = f.write_all(&self.buffer.borrow());
            self.buffer.borrow_mut().clear();
        }
    }
}

/// The Lua-exposed file globals.
pub fn global(name: &str) -> Option<Imp> {
    Some(match name {
        // LuaManager.GlobalObject.cacheFileExists: `<cache>/Lua/<filename>` exists
        "cacheFileExists" => |s, _, cf, n| {
            want_args(n, false, 1)?;
            let filename = string_arg_nn(cf, 0)?;
            let path = lua_cache_dir(s)?.join(to_native(&filename));
            super::exposed_methods::ret_bool(cf, path.exists())
        },
        // LuaManager.GlobalObject.getFileReader(String, boolean)
        "getFileReader" => |s, _, cf, n| {
            want_args(n, false, 2)?;
            let filename = string_arg_nn(cf, 0)?;
            let create = bool_arg(cf, 1)?;
            if has_relative_path(s, &filename) {
                return ret(cf, Object::Nil);
            }
            let path = lua_cache_dir(s)?.join(to_native(&filename));
            if !path.exists() && create {
                std::fs::File::create(&path).map_err(|e| {
                    Fail::Java(Throwable::Java {
                        class: "java.io.IOException",
                        message: Some(e.to_string()),
                    })
                })?;
            }
            if !path.exists() {
                return ret(cf, Object::Nil);
            }
            let text = match std::fs::read(&path) {
                Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                // FileInputStream failing is logged and a null reader returned
                Err(_) => return ret(cf, Object::Nil),
            };
            ret(
                cf,
                ud(Rc::new(BufferedReaderObject {
                    text,
                    pos: RefCell::new(0),
                    closed: RefCell::new(false),
                })),
            )
        },
        // LuaManager.GlobalObject.getFileWriter(String, boolean createIfNull, boolean append): only the extensions
        // of ALLOWED_FILE_EXTENSIONS ("ini", "cfg", "txt", "log", "json"); the folders are made
        "getFileWriter" => |s, _, cf, n| {
            want_args(n, false, 3)?;
            let filename = string_arg_nn(cf, 0)?;
            let create = bool_arg(cf, 1)?;
            let append = bool_arg(cf, 2)?;
            // ZomboidFileSystem.getFileExtension: the text after the last '.' of the file name
            let name = filename.rsplit(['/', '\\']).next().unwrap_or("");
            let extension = name.rfind('.').map_or("", |i| &name[i + 1..]);
            if has_relative_path(s, &filename)
                || !["ini", "cfg", "txt", "log", "json"].contains(&extension)
            {
                return ret(cf, Object::Nil);
            }
            let path = lua_cache_dir(s)?.join(to_native(&filename));
            if let Some(dir) = path.parent() {
                if !dir.exists() {
                    let _ = std::fs::create_dir_all(dir);
                }
            }
            if !path.exists() && create {
                let _ = std::fs::File::create(&path);
            }
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .append(append)
                .truncate(!append)
                .open(&path);
            match file {
                Ok(f) => ret(
                    cf,
                    ud(Rc::new(LuaFileWriterObject {
                        file: RefCell::new(Some(f)),
                        buffer: RefCell::new(Vec::new()),
                    })),
                ),
                // the FileNotFoundException is logged and null returned
                Err(_) => ret(cf, Object::Nil),
            }
        },
        _ => return None,
    })
}

/// The methods of the reader and writer objects.
pub fn instance_method(class: &str, name: &str) -> Option<Imp> {
    match class {
        "java.io.BufferedReader" => Some(match name {
            // BufferedReader.readLine
            "readLine" => |_, _, cf, n| {
                want_args(n, true, 0)?;
                with_this!(cf, BufferedReaderObject, r, {
                    let line = r.read_line()?;
                    ret_str(cf, line.as_deref())
                })
            },
            // BufferedReader.close
            "close" => |_, _, cf, n| {
                want_args(n, true, 0)?;
                with_this!(cf, BufferedReaderObject, r, {
                    *r.closed.borrow_mut() = true;
                    Ok(0)
                })
            },
            _ => return None,
        }),
        "zombie.Lua.LuaManager$GlobalObject$LuaFileWriter" => Some(match name {
            // LuaFileWriter.write(String)
            "write" => |_, _, cf, n| {
                want_args(n, true, 1)?;
                with_this!(cf, LuaFileWriterObject, w, {
                    // PrintWriter.write(null String) is a NullPointerException
                    let text = string_arg_nn(cf, 1)?;
                    w.write(&text);
                    Ok(0)
                })
            },
            // LuaFileWriter.writeln(String): the text, then System.lineSeparator()
            "writeln" => |_, _, cf, n| {
                want_args(n, true, 1)?;
                with_this!(cf, LuaFileWriterObject, w, {
                    let text = string_arg_nn(cf, 1)?;
                    w.write(&text);
                    w.write(if cfg!(windows) { "\r\n" } else { "\n" });
                    Ok(0)
                })
            },
            // LuaFileWriter.close
            "close" => |_, _, cf, n| {
                want_args(n, true, 0)?;
                with_this!(cf, LuaFileWriterObject, w, {
                    w.close();
                    Ok(0)
                })
            },
            _ => return None,
        }),
        _ => None,
    }
}
