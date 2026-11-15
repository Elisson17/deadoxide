//! What Kahlua keeps in its stacks and tables: a `java.lang.Object` as the VM sees it. `null` is nil, `Boolean`,
//! `Double` and `String` are the Lua scalars, `KahluaTable`, `LuaClosure`, `JavaFunction` and `Coroutine` keep Java
//! object identity (an `Rc` pointer), anything else is userdata. Strings are UTF-16 like `java.lang.String`
//! (`#s`, `string.sub` and `string.byte` count UTF-16 units), shared through `Rc` so that the places where the
//! original compares strings by reference (`rawequal`) can too.
use std::any::Any;
use std::cell::RefCell;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use super::coroutine::{Coroutine, FrameRef};
use super::kahlua_thread::KahluaThread;
use super::lua_closure::LuaClosure;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;

/// A `java.lang.String`: its UTF-16 units.
pub type LuaStr = Rc<[u16]>;
/// A `KahluaTableImpl` by reference.
pub type TableRef = Rc<RefCell<KahluaTableImpl>>;
/// A `Coroutine` by reference.
pub type CoroutineRef = Rc<RefCell<Coroutine>>;
/// What a ported Java method throws.
pub type R<T> = Result<T, Throwable>;

/// The UTF-16 units of a Rust string, as a new `String` object.
pub fn lstr(s: &str) -> LuaStr {
    s.encode_utf16().collect::<Vec<u16>>().into()
}

/// A Java string as Rust text (a lone surrogate becomes U+FFFD).
pub fn rstr(s: &[u16]) -> String {
    String::from_utf16_lossy(s)
}

/// se.krka.kahlua.vm.JavaFunction: a function implemented by the host. `java_to_string` is the object's
/// `toString()` (for `BaseLib` the function's name; for anonymous classes the JDK's `Class@hash`).
pub trait JavaFunction {
    /// se.krka.kahlua.vm.JavaFunction.call: the arguments are the frame's slots 0..n, the return values are the
    /// top `n` slots; returns `n`.
    fn call(&self, thread: &mut KahluaThread, frame: &FrameRef, n_arguments: i32) -> R<i32>;
    fn java_to_string(&self) -> String;
    /// The Java class, for the class metatables (`KahluaThread.getClassMetatable`).
    fn class(&self) -> &'static JavaClass;
}

/// A host object Lua only passes around (type "userdata").
pub trait Userdata {
    fn class(&self) -> &'static JavaClass;
    fn java_to_string(&self) -> String;
    fn as_any(&self) -> &dyn Any;
}

/// A `java.lang.Class` as far as the VM uses it: its name, superclass and interfaces (the breadth-first search of
/// `KahluaThread.getClassMetatable`).
#[derive(Debug)]
pub struct JavaClass {
    pub name: &'static str,
    pub superclass: Option<&'static JavaClass>,
    pub interfaces: &'static [&'static JavaClass],
}

pub mod classes {
    //! The JDK and Kahlua classes the VM looks up class metatables for.
    use super::JavaClass;
    pub static OBJECT: JavaClass = JavaClass {
        name: "java.lang.Object",
        superclass: None,
        interfaces: &[],
    };
    pub static SERIALIZABLE: JavaClass = JavaClass {
        name: "java.io.Serializable",
        superclass: None,
        interfaces: &[],
    };
    pub static COMPARABLE: JavaClass = JavaClass {
        name: "java.lang.Comparable",
        superclass: None,
        interfaces: &[],
    };
    pub static CHAR_SEQUENCE: JavaClass = JavaClass {
        name: "java.lang.CharSequence",
        superclass: None,
        interfaces: &[],
    };
    pub static CONSTABLE: JavaClass = JavaClass {
        name: "java.lang.constant.Constable",
        superclass: None,
        interfaces: &[],
    };
    pub static CONSTANT_DESC: JavaClass = JavaClass {
        name: "java.lang.constant.ConstantDesc",
        superclass: None,
        interfaces: &[],
    };
    pub static NUMBER: JavaClass = JavaClass {
        name: "java.lang.Number",
        superclass: Some(&OBJECT),
        interfaces: &[&SERIALIZABLE],
    };
    pub static STRING: JavaClass = JavaClass {
        name: "java.lang.String",
        superclass: Some(&OBJECT),
        interfaces: &[
            &SERIALIZABLE,
            &COMPARABLE,
            &CHAR_SEQUENCE,
            &CONSTABLE,
            &CONSTANT_DESC,
        ],
    };
    pub static DOUBLE: JavaClass = JavaClass {
        name: "java.lang.Double",
        superclass: Some(&NUMBER),
        interfaces: &[&COMPARABLE, &CONSTABLE, &CONSTANT_DESC],
    };
    pub static BOOLEAN: JavaClass = JavaClass {
        name: "java.lang.Boolean",
        superclass: Some(&OBJECT),
        interfaces: &[&SERIALIZABLE, &COMPARABLE, &CONSTABLE],
    };
    pub static JAVA_FUNCTION: JavaClass = JavaClass {
        name: "se.krka.kahlua.vm.JavaFunction",
        superclass: None,
        interfaces: &[],
    };
    pub static LUA_CLOSURE: JavaClass = JavaClass {
        name: "se.krka.kahlua.vm.LuaClosure",
        superclass: Some(&OBJECT),
        interfaces: &[],
    };
    pub static COROUTINE: JavaClass = JavaClass {
        name: "se.krka.kahlua.vm.Coroutine",
        superclass: Some(&OBJECT),
        interfaces: &[],
    };
    pub static CLASS: JavaClass = JavaClass {
        name: "java.lang.Class",
        superclass: Some(&OBJECT),
        interfaces: &[&SERIALIZABLE],
    };
    pub static THROWABLE: JavaClass = JavaClass {
        name: "java.lang.Throwable",
        superclass: Some(&OBJECT),
        interfaces: &[&SERIALIZABLE],
    };
    pub static KAHLUA_TABLE_ITERATOR: JavaClass = JavaClass {
        name: "se.krka.kahlua.vm.KahluaTableIterator",
        superclass: None,
        interfaces: &[&JAVA_FUNCTION],
    };
    pub static TABLE_ITERATOR: JavaClass = JavaClass {
        name: "se.krka.kahlua.j2se.KahluaTableImpl$1",
        superclass: Some(&OBJECT),
        interfaces: &[&KAHLUA_TABLE_ITERATOR],
    };
}

/// A `java.lang.Class` object as a Lua value (the keys of `__classmetatables`). One per class and platform
/// (`J2SEPlatform::class_object`), so identity works as for the original's `Class` objects.
pub struct ClassObject(pub &'static JavaClass);

impl Userdata for ClassObject {
    fn class(&self) -> &'static JavaClass {
        &classes::CLASS
    }
    /// Class.toString
    fn java_to_string(&self) -> String {
        format!("class {}", self.0.name)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A caught exception as a Lua value (the fourth result of a failed `pcall`).
pub struct ThrowableObject(pub Throwable);

impl Userdata for ThrowableObject {
    fn class(&self) -> &'static JavaClass {
        &classes::THROWABLE
    }
    /// Throwable.toString: the class name, then ": " and the message when there is one.
    fn java_to_string(&self) -> String {
        match self.0.get_message() {
            Some(m) => format!("{}: {m}", self.0.class_name()),
            None => self.0.class_name().to_string(),
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A Lua value.
#[derive(Clone, Default)]
pub enum Object {
    #[default]
    Nil,
    Boolean(bool),
    Number(f64),
    String(LuaStr),
    Table(TableRef),
    Closure(Rc<LuaClosure>),
    Function(Rc<dyn JavaFunction>),
    Coroutine(CoroutineRef),
    Userdata(Rc<dyn Userdata>),
}

impl fmt::Debug for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Object::Nil => write!(f, "nil"),
            Object::Boolean(b) => write!(f, "{b}"),
            Object::Number(d) => write!(f, "{d:?}"),
            Object::String(s) => write!(f, "{:?}", rstr(s)),
            Object::Table(t) => write!(f, "table@{:p}", Rc::as_ptr(t)),
            Object::Closure(c) => write!(f, "closure@{:p}", Rc::as_ptr(c)),
            Object::Function(j) => write!(f, "function {}", j.java_to_string()),
            Object::Coroutine(c) => write!(f, "coroutine@{:p}", Rc::as_ptr(c)),
            Object::Userdata(u) => write!(f, "userdata {}", u.java_to_string()),
        }
    }
}

/// A stand-in for `System.identityHashCode`: stable for the object's life, not the JVM's number (which differs
/// between runs of the original too).
pub fn identity_hash_code<T: ?Sized>(rc: &Rc<T>) -> i32 {
    ((Rc::as_ptr(rc) as *const u8 as usize >> 4) & 0x7fff_ffff) as i32
}

impl Object {
    pub fn str(s: &str) -> Object {
        Object::String(lstr(s))
    }

    pub fn is_nil(&self) -> bool {
        matches!(self, Object::Nil)
    }

    /// Reference identity (`==` on two Java references); `Double` and `Boolean` boxes compare by value, as the
    /// original's cached boxes (`BoxedStaticValues`, `Boolean.TRUE`) make every comparison the VM does on them.
    pub fn same(&self, other: &Object) -> bool {
        match (self, other) {
            (Object::Nil, Object::Nil) => true,
            (Object::Boolean(a), Object::Boolean(b)) => a == b,
            (Object::Number(a), Object::Number(b)) => a.to_bits() == b.to_bits(),
            (Object::String(a), Object::String(b)) => Rc::ptr_eq(a, b),
            (Object::Table(a), Object::Table(b)) => Rc::ptr_eq(a, b),
            (Object::Closure(a), Object::Closure(b)) => Rc::ptr_eq(a, b),
            (Object::Function(a), Object::Function(b)) => Rc::ptr_eq(a, b),
            (Object::Coroutine(a), Object::Coroutine(b)) => Rc::ptr_eq(a, b),
            (Object::Userdata(a), Object::Userdata(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }

    /// The Java class of the value (`o.getClass()`); `None` for nil.
    pub fn class(&self) -> Option<&'static JavaClass> {
        Some(match self {
            Object::Nil => return None,
            Object::Boolean(_) => &classes::BOOLEAN,
            Object::Number(_) => &classes::DOUBLE,
            Object::String(_) => &classes::STRING,
            Object::Table(_) => return None,
            Object::Closure(_) => &classes::LUA_CLOSURE,
            Object::Function(f) => f.class(),
            Object::Coroutine(_) => &classes::COROUTINE,
            Object::Userdata(u) => u.class(),
        })
    }

    /// `String.valueOf(o)`: the object's `toString()`, "null" for nil.
    pub fn java_to_string(&self) -> String {
        match self {
            Object::Nil => "null".into(),
            Object::Boolean(b) => b.to_string(),
            Object::Number(d) => crate::java::double_to_string(*d),
            Object::String(s) => rstr(s),
            Object::Table(t) => KahluaTableImpl::java_to_string(t),
            Object::Closure(c) => LuaClosure::java_to_string(c),
            Object::Function(f) => f.java_to_string(),
            Object::Coroutine(c) => {
                format!("se.krka.kahlua.vm.Coroutine@{:x}", identity_hash_code(c))
            }
            Object::Userdata(u) => u.java_to_string(),
        }
    }
}

/// A table key with `Object.equals`/`hashCode` semantics: `Double` by `doubleToLongBits` (so 0.0 and -0.0 are two
/// keys and every NaN is one), `String` and `Boolean` by value, everything else by identity.
#[derive(Clone, Debug)]
pub struct Key(pub Object);

fn double_key_bits(d: f64) -> u64 {
    if d.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        d.to_bits()
    }
}

impl PartialEq for Key {
    fn eq(&self, other: &Key) -> bool {
        match (&self.0, &other.0) {
            (Object::Number(a), Object::Number(b)) => double_key_bits(*a) == double_key_bits(*b),
            (Object::String(a), Object::String(b)) => a[..] == b[..],
            (a, b) => a.same(b),
        }
    }
}

impl Eq for Key {}

impl Hash for Key {
    fn hash<H: Hasher>(&self, h: &mut H) {
        match &self.0 {
            Object::Nil => 0u8.hash(h),
            Object::Boolean(b) => b.hash(h),
            Object::Number(d) => double_key_bits(*d).hash(h),
            Object::String(s) => s[..].hash(h),
            Object::Table(t) => (Rc::as_ptr(t) as *const u8 as usize).hash(h),
            Object::Closure(c) => (Rc::as_ptr(c) as *const u8 as usize).hash(h),
            Object::Function(f) => (Rc::as_ptr(f) as *const u8 as usize).hash(h),
            Object::Coroutine(c) => (Rc::as_ptr(c) as *const u8 as usize).hash(h),
            Object::Userdata(u) => (Rc::as_ptr(u) as *const u8 as usize).hash(h),
        }
    }
}

/// A Java exception the port raises or catches.
#[derive(Clone)]
pub enum Throwable {
    /// se.krka.kahlua.vm.KahluaException with its `errorMessage` (what `error(x)` throws).
    Kahlua(Object),
    /// Any other exception: its class name and `getMessage()` (`None` for null).
    Java {
        class: &'static str,
        message: Option<String>,
    },
}

impl fmt::Debug for Throwable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?}", self.class_name(), self.get_message())
    }
}

impl Throwable {
    /// `new RuntimeException(msg)`.
    pub fn runtime(msg: impl Into<String>) -> Throwable {
        Throwable::Java {
            class: "java.lang.RuntimeException",
            message: Some(msg.into()),
        }
    }

    /// A `NullPointerException` with the JDK's helpful message.
    pub fn npe(msg: impl Into<String>) -> Throwable {
        Throwable::Java {
            class: "java.lang.NullPointerException",
            message: Some(msg.into()),
        }
    }

    /// `StackOverflowError` (no message).
    pub fn stack_overflow() -> Throwable {
        Throwable::Java {
            class: "java.lang.StackOverflowError",
            message: None,
        }
    }

    /// The `ClassCastException` of casting `o` to `target` (the JDK's message for two classes of the
    /// game's class path).
    pub fn class_cast(o: &Object, target: &str) -> Throwable {
        let from = o.class().map_or("null", |c| c.name);
        let from_where = if from.starts_with("java.") {
            "in module java.base of loader 'bootstrap'"
        } else {
            "in unnamed module of loader 'app'"
        };
        let to_where = if target.starts_with("java.") {
            "in module java.base of loader 'bootstrap'"
        } else {
            "in unnamed module of loader 'app'"
        };
        let tail = if from_where == to_where {
            format!("{from} and {target} are {from_where}")
        } else {
            format!("{from} is {from_where}; {target} is {to_where}")
        };
        Throwable::Java {
            class: "java.lang.ClassCastException",
            message: Some(format!(
                "class {from} cannot be cast to class {target} ({tail})"
            )),
        }
    }

    pub fn class_name(&self) -> &str {
        match self {
            Throwable::Kahlua(_) => "se.krka.kahlua.vm.KahluaException",
            Throwable::Java { class, .. } => class,
        }
    }

    /// Throwable.getMessage (KahluaException: "nil" for a nil error object, else its `toString()`).
    pub fn get_message(&self) -> Option<String> {
        match self {
            Throwable::Kahlua(Object::Nil) => Some("nil".into()),
            Throwable::Kahlua(o) => Some(o.java_to_string()),
            Throwable::Java { message, .. } => message.clone(),
        }
    }
}

/// `(KahluaTable) o`: nil passes as null, a non-table throws ClassCastException.
pub fn cast_table(o: &Object) -> R<Option<TableRef>> {
    match o {
        Object::Nil => Ok(None),
        Object::Table(t) => Ok(Some(t.clone())),
        _ => Err(Throwable::class_cast(o, "se.krka.kahlua.vm.KahluaTable")),
    }
}

/// `(String) o`.
pub fn cast_string(o: &Object) -> R<Option<LuaStr>> {
    match o {
        Object::Nil => Ok(None),
        Object::String(s) => Ok(Some(s.clone())),
        _ => Err(Throwable::class_cast(o, "java.lang.String")),
    }
}

/// `(Double) o`.
pub fn cast_double(o: &Object) -> R<Option<f64>> {
    match o {
        Object::Nil => Ok(None),
        Object::Number(d) => Ok(Some(*d)),
        _ => Err(Throwable::class_cast(o, "java.lang.Double")),
    }
}
