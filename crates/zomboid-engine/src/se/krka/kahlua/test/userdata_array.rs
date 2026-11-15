//! se.krka.kahlua.test.UserdataArray: the `array` table, a metatable that makes a java.util.Vector act as a
//! 0-based Lua array (`array.new()`, `v:push(x)`, `v[i]`, `#v`).
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::se::krka::kahlua::j2se::j2se_platform::J2SEPlatform;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
use crate::se::krka::kahlua::vm::kahlua_util::{
    from_double, get_class_metatables, lua_assert, to_double,
};
use crate::se::krka::kahlua::vm::object::{
    classes, JavaClass, JavaFunction, Object, TableRef, Throwable, Userdata, R,
};

pub static CLASS: JavaClass = JavaClass {
    name: "se.krka.kahlua.test.UserdataArray",
    superclass: Some(&classes::OBJECT),
    interfaces: &[&classes::JAVA_FUNCTION],
};
pub static VECTOR_CLASS: JavaClass = JavaClass {
    name: "java.util.Vector",
    superclass: Some(&classes::OBJECT),
    interfaces: &[],
};

/// A java.util.Vector: its elements and the length of its backing array (10, doubled when full), which the
/// message of a negative index names.
pub struct Vector {
    inner: RefCell<(Vec<Object>, usize)>,
}

impl Vector {
    /// java.util.Vector.<init>()
    pub fn new() -> Vector {
        Vector {
            inner: RefCell::new((Vec::new(), 10)),
        }
    }

    pub fn size(&self) -> usize {
        self.inner.borrow().0.len()
    }

    /// java.util.Vector.addElement (grow: the capacity doubles)
    pub fn add_element(&self, o: Object) {
        let mut inner = self.inner.borrow_mut();
        if inner.0.len() == inner.1 {
            inner.1 += inner.1.max(1);
        }
        inner.0.push(o);
    }

    fn bad_index(&self, index: i32) -> Throwable {
        let inner = self.inner.borrow();
        let message = if index >= 0 {
            format!("{index} >= {}", inner.0.len())
        } else {
            format!("Index {index} out of bounds for length {}", inner.1)
        };
        Throwable::Java {
            class: "java.lang.ArrayIndexOutOfBoundsException",
            message: Some(message),
        }
    }

    /// java.util.Vector.elementAt
    pub fn element_at(&self, index: i32) -> R<Object> {
        let inner = self.inner.borrow();
        if index >= 0 && (index as usize) < inner.0.len() {
            Ok(inner.0[index as usize].clone())
        } else {
            drop(inner);
            Err(self.bad_index(index))
        }
    }

    /// java.util.Vector.setElementAt
    pub fn set_element_at(&self, o: Object, index: i32) -> R<()> {
        let mut inner = self.inner.borrow_mut();
        if index >= 0 && (index as usize) < inner.0.len() {
            inner.0[index as usize] = o;
            Ok(())
        } else {
            drop(inner);
            Err(self.bad_index(index))
        }
    }
}

impl Default for Vector {
    fn default() -> Self {
        Vector::new()
    }
}

impl Userdata for Vector {
    fn class(&self) -> &'static JavaClass {
        &VECTOR_CLASS
    }
    /// AbstractCollection.toString: "[a, b]", "(this Collection)" for the vector itself.
    fn java_to_string(&self) -> String {
        let inner = self.inner.borrow();
        let parts: Vec<String> = inner
            .0
            .iter()
            .map(|o| match o {
                Object::Userdata(u)
                    if std::ptr::addr_eq(Rc::as_ptr(u), std::ptr::from_ref(self)) =>
                {
                    "(this Collection)".to_string()
                }
                _ => o.java_to_string(),
            })
            .collect();
        format!("[{}]", parts.join(", "))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub struct UserdataArray {
    index: usize,
    /// The `array` table (`index` reads it for a key that is not a number); weak, as the table holds this function.
    metatable: Option<Weak<RefCell<KahluaTableImpl>>>,
}

/// se.krka.kahlua.test.UserdataArray.register (the original builds the metatable once per JVM; here once per
/// environment).
pub fn register(platform: &J2SEPlatform, env: &TableRef) -> R<()> {
    let m = KahluaTableImpl::new_ref();
    KahluaTableImpl::rawset(&m, Object::str("__metatable"), Object::str("restricted"));
    for (i, name) in ["__len", "__index", "__newindex", "new", "push"]
        .iter()
        .enumerate()
    {
        let f = UserdataArray {
            index: i,
            metatable: (i == 1).then(|| Rc::downgrade(&m)),
        };
        KahluaTableImpl::rawset(&m, Object::str(name), Object::Function(Rc::new(f)));
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

/// `(Vector) o`: a Vector, null for nil, else ClassCastException.
fn cast_vector(o: &Object) -> R<Option<Rc<dyn Userdata>>> {
    match o {
        Object::Nil => Ok(None),
        Object::Userdata(u) if u.as_any().is::<Vector>() => Ok(Some(u.clone())),
        _ => Err(Throwable::class_cast(o, VECTOR_CLASS.name)),
    }
}

fn vector(u: &Rc<dyn Userdata>) -> &Vector {
    // cast_vector only lets Vectors through.
    u.as_any()
        .downcast_ref::<Vector>()
        .unwrap_or_else(|| unreachable!())
}

fn null_vector(method: &str) -> Throwable {
    Throwable::npe(format!(
        "Cannot invoke \"java.util.Vector.{method}\" because \"v\" is null"
    ))
}

impl JavaFunction for UserdataArray {
    /// se.krka.kahlua.test.UserdataArray.call
    fn call(&self, _thread: &mut KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
        match self.index {
            0 => {
                // length
                lua_assert(n >= 1, "not enough parameters")?;
                let v = cast_vector(&cf.get(0))?.ok_or_else(|| null_vector("size()"))?;
                cf.push(Object::Number(to_double(vector(&v).size() as f64)))?;
                Ok(1)
            }
            1 => {
                // index
                lua_assert(n >= 2, "not enough parameters")?;
                let obj = cf.get(0);
                let Object::Userdata(u) = &obj else {
                    return Ok(0);
                };
                let Some(v) = u.as_any().downcast_ref::<Vector>() else {
                    return Ok(0);
                };
                let key = cf.get(1);
                let res = match &key {
                    Object::Number(d) => v.element_at(*d as i32)?,
                    _ => match self.metatable.as_ref().and_then(Weak::upgrade) {
                        Some(m) => KahluaTableImpl::rawget(&m, &key)?,
                        None => Object::Nil,
                    },
                };
                cf.push(res)?;
                Ok(1)
            }
            2 => {
                // newindex
                lua_assert(n >= 3, "not enough parameters")?;
                let v = cast_vector(&cf.get(0))?;
                let key = cf.get(1);
                let value = cf.get(2);
                let index = from_double(&key)? as i32;
                let v = v.ok_or_else(|| null_vector("setElementAt(Object, int)"))?;
                vector(&v).set_element_at(value, index)?;
                Ok(0)
            }
            3 => {
                cf.push(Object::Userdata(Rc::new(Vector::new())))?;
                Ok(1)
            }
            _ => {
                // push
                lua_assert(n >= 2, "not enough parameters")?;
                let v = cast_vector(&cf.get(0))?;
                let value = cf.get(1);
                let v = v.ok_or_else(|| null_vector("addElement(Object)"))?;
                vector(&v).add_element(value);
                cf.push(Object::Userdata(v))?;
                Ok(1)
            }
        }
    }
    fn java_to_string(&self) -> String {
        format!("{}@0", CLASS.name)
    }
    fn class(&self) -> &'static JavaClass {
        &CLASS
    }
}
