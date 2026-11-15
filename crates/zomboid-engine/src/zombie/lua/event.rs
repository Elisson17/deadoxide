//! zombie.Lua.Event: one named event (or hook) and the Lua functions added to it, in order.
use std::cell::RefCell;
use std::rc::Rc;

use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
use crate::se::krka::kahlua::vm::lua_closure::LuaClosure;
use crate::se::krka::kahlua::vm::object::{
    classes, identity_hash_code, JavaClass, JavaFunction, Object, TableRef, R,
};

static ADD_CLASS: JavaClass = JavaClass {
    name: "zombie.Lua.Event$Add",
    superclass: Some(&classes::OBJECT),
    interfaces: &[&classes::JAVA_FUNCTION],
};
static REMOVE_CLASS: JavaClass = JavaClass {
    name: "zombie.Lua.Event$Remove",
    superclass: Some(&classes::OBJECT),
    interfaces: &[&classes::JAVA_FUNCTION],
};

/// zombie.Lua.Event
pub struct Event {
    pub name: String,
    pub index: usize,
    pub callbacks: RefCell<Vec<Rc<LuaClosure>>>,
}

impl Event {
    /// zombie.Lua.Event.<init>
    pub fn new(name: &str, index: usize) -> Rc<Event> {
        Rc::new(Event {
            name: name.to_string(),
            index,
            callbacks: RefCell::new(Vec::new()),
        })
    }

    /// zombie.Lua.Event.register: `table[name] = { Add = ..., Remove = ... }`.
    pub fn register(self: &Rc<Self>, table: &TableRef) {
        let t = KahluaTableImpl::new_ref();
        KahluaTableImpl::rawset(
            &t,
            Object::str("Add"),
            Object::Function(Rc::new(Add(self.clone()))),
        );
        KahluaTableImpl::rawset(
            &t,
            Object::str("Remove"),
            Object::Function(Rc::new(Remove(self.clone()))),
        );
        KahluaTableImpl::rawset(table, Object::str(&self.name), Object::Table(t));
    }
}

/// zombie.Lua.Event.Add: appends the first argument when it is a Lua function (anything else is ignored).
/// `LuaCompiler.rewriteEvents` is never set by the loading this port does.
pub struct Add(pub Rc<Event>);

impl JavaFunction for Add {
    /// zombie.Lua.Event.Add.call
    fn call(&self, _thread: &mut KahluaThread, cf: &FrameRef, _n: i32) -> R<i32> {
        if let Object::Closure(c) = cf.get(0) {
            self.0.callbacks.borrow_mut().push(c);
        }
        Ok(0)
    }
    fn java_to_string(&self) -> String {
        format!("{}@{:x}", ADD_CLASS.name, identity_hash_code(&self.0))
    }
    fn class(&self) -> &'static JavaClass {
        &ADD_CLASS
    }
}

/// zombie.Lua.Event.Remove: removes the first occurrence of the function (`ArrayList.remove(Object)`, identity).
pub struct Remove(pub Rc<Event>);

impl JavaFunction for Remove {
    /// zombie.Lua.Event.Remove.call
    fn call(&self, _thread: &mut KahluaThread, cf: &FrameRef, _n: i32) -> R<i32> {
        if let Object::Closure(c) = cf.get(0) {
            let mut callbacks = self.0.callbacks.borrow_mut();
            if let Some(i) = callbacks.iter().position(|x| Rc::ptr_eq(x, &c)) {
                callbacks.remove(i);
            }
        }
        Ok(0)
    }
    fn java_to_string(&self) -> String {
        format!("{}@{:x}", REMOVE_CLASS.name, identity_hash_code(&self.0))
    }
    fn class(&self) -> &'static JavaClass {
        &REMOVE_CLASS
    }
}
