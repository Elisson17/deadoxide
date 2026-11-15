//! se.krka.kahlua.vm.LuaClosure: a prototype with its environment and upvalues.
use std::cell::RefCell;
use std::rc::Rc;

use super::coroutine::UpValueRef;
use super::object::{identity_hash_code, Object, TableRef};
use super::prototype::{Constant, Prototype};
use crate::java::integer_to_string_radix;

/// se.krka.kahlua.vm.Prototype as the VM runs it: the compiler's prototype with its constants as Lua values,
/// made once, so every closure of it shares the same constant objects (as the original's constant array).
pub struct Proto {
    pub code: Vec<i32>,
    pub constants: Vec<Object>,
    pub prototypes: Vec<Rc<Proto>>,
    pub num_params: i32,
    pub is_vararg: bool,
    pub name: Option<String>,
    pub lines: Vec<i32>,
    pub num_upvalues: i32,
    pub max_stacksize: i32,
    pub file: Option<String>,
    pub filename: Option<String>,
}

impl Proto {
    pub fn from_prototype(p: Prototype) -> Rc<Proto> {
        Rc::new(Proto {
            constants: p
                .constants
                .into_iter()
                .map(|k| match k {
                    Constant::Nil => Object::Nil,
                    Constant::Boolean(b) => Object::Boolean(b),
                    Constant::Number(d) => Object::Number(d),
                    Constant::String(s) => Object::str(&s),
                })
                .collect(),
            prototypes: p
                .prototypes
                .into_iter()
                .map(Proto::from_prototype)
                .collect(),
            code: p.code,
            num_params: p.num_params,
            is_vararg: p.is_vararg,
            name: p.name,
            lines: p.lines,
            num_upvalues: p.num_upvalues,
            max_stacksize: p.max_stacksize,
            file: p.file,
            filename: p.filename,
        })
    }
}

/// se.krka.kahlua.vm.LuaClosure (`LuaCompiler.rewriteEvents` rerouting, a reload feature, is not ported).
pub struct LuaClosure {
    pub prototype: Rc<Proto>,
    pub env: RefCell<TableRef>,
    pub upvalues: Vec<UpValueRef>,
    pub debug_name: RefCell<Option<String>>,
}

impl LuaClosure {
    /// se.krka.kahlua.vm.LuaClosure.<init> with its upvalues filled.
    pub fn new(prototype: Rc<Proto>, env: TableRef, upvalues: Vec<UpValueRef>) -> Rc<LuaClosure> {
        Rc::new(LuaClosure {
            prototype,
            env: RefCell::new(env),
            upvalues,
            debug_name: RefCell::new(None),
        })
    }

    /// se.krka.kahlua.vm.LuaClosure.toString
    pub fn java_to_string(c: &Rc<LuaClosure>) -> String {
        let p = &c.prototype;
        match p.lines.first() {
            Some(l) => format!("function {}:{l}", p.name.as_deref().unwrap_or("null")),
            None => format!(
                "function[{}]",
                integer_to_string_radix(identity_hash_code(c), 36)
            ),
        }
    }
}
