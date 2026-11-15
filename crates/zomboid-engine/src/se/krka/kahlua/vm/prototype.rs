//! se.krka.kahlua.vm.Prototype: one compiled Lua function (its bytecode, constants and nested functions).

/// A constant of a prototype's pool: `null`, a `Boolean`, a `Double` or a `String`.
#[derive(Clone, Debug, PartialEq)]
pub enum Constant {
    Nil,
    Boolean(bool),
    Number(f64),
    String(String),
}

/// se.krka.kahlua.vm.Prototype (the fields the compiler fills; `locvarlines` only under `Core.debug`, which is not
/// ported).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prototype {
    pub code: Vec<i32>,
    pub constants: Vec<Constant>,
    pub prototypes: Vec<Prototype>,
    pub num_params: i32,
    pub is_vararg: bool,
    pub name: Option<String>,
    pub lines: Vec<i32>,
    pub num_upvalues: i32,
    pub max_stacksize: i32,
    pub file: Option<String>,
    pub filename: Option<String>,
    pub locvars: Vec<String>,
}
