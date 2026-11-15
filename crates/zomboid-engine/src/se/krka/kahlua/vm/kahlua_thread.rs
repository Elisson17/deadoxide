//! se.krka.kahlua.vm.KahluaThread: the interpreter. `luaMainloop` runs Lua frames in a loop (a Lua-to-Lua call
//! pushes a frame, a call into a host function or a metamethod nests through `call`), exceptions unwind the frames
//! the way the original's catch block does. `Core.debug` stepping, breakpoints, profiling and the error log
//! (`ExceptionLogger`, `DebugType.General`) are not ported; `errorCount` is counted.
use std::collections::HashMap;
use std::rc::Rc;

use super::coroutine::{Coroutine, FrameRef};
use super::kahlua_util::{
    bool_eval, fail, from_double, get_class_metatables, lua_assert, raw_tonumber, raw_tostring,
    to_double,
};
use super::lua_closure::{LuaClosure, Proto};
use super::object::{
    cast_table, lstr, CoroutineRef, JavaClass, JavaFunction, LuaStr, Object, TableRef, Throwable,
    ThrowableObject, R,
};
use crate::se::krka::kahlua::j2se::j2se_platform::J2SEPlatform;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;

const FIELDS_PER_FLUSH: i32 = 50;
const MAX_INDEX_RECURSION: i32 = 100;

/// se.krka.kahlua.vm.KahluaThread.meta_ops
fn meta_op_name(opcode: i32) -> &'static str {
    match opcode {
        12 => "__add",
        13 => "__sub",
        14 => "__mul",
        15 => "__div",
        16 => "__mod",
        17 => "__pow",
        23 => "__eq",
        24 => "__lt",
        25 => "__le",
        _ => "null",
    }
}

fn get_a8(op: i32) -> i32 {
    ((op as u32) >> 6 & 0xFF) as i32
}
fn get_c9(op: i32) -> i32 {
    ((op as u32) >> 14 & 511) as i32
}
fn get_b9(op: i32) -> i32 {
    ((op as u32) >> 23 & 511) as i32
}
fn get_bx(op: i32) -> i32 {
    ((op as u32) >> 14) as i32
}
fn get_sbx(op: i32) -> i32 {
    ((op as u32) >> 14) as i32 - 131071
}

fn aioobe(index: i64, length: usize) -> Throwable {
    Throwable::Java {
        class: "java.lang.ArrayIndexOutOfBoundsException",
        message: Some(format!("Index {index} out of bounds for length {length}")),
    }
}

/// An exception (not an Error): what `catch (Exception e)` catches.
fn is_exception(e: &Throwable) -> bool {
    match e {
        Throwable::Kahlua(_) => true,
        Throwable::Java { class, .. } => !class.ends_with("Error"),
    }
}

/// se.krka.kahlua.stdlib.BaseLib.luaEquals: numbers by value, everything else by reference.
pub fn lua_equals(a: &Object, b: &Object) -> bool {
    match (a, b) {
        (Object::Number(x), Object::Number(y)) => x == y,
        _ => a.same(b),
    }
}

/// The locals `luaMainloop` keeps for the running frame.
struct LoopState {
    cf: FrameRef,
    closure: Rc<LuaClosure>,
    prototype: Rc<Proto>,
    return_base: i32,
}

enum Flow {
    Next,
    Return,
}

/// se.krka.kahlua.vm.KahluaThread
pub struct KahluaThread {
    root_coroutine: CoroutineRef,
    pub current_coroutine: CoroutineRef,
    cached_metatables: HashMap<&'static str, Option<TableRef>>,
    pub platform: Rc<J2SEPlatform>,
    /// What `print` wrote (`getOut().println`), lines ended by "\n".
    pub out: String,
    pub reset: bool,
    /// se.krka.kahlua.vm.KahluaThread.errorCount (static in the original).
    pub error_count: i32,
}

impl KahluaThread {
    /// se.krka.kahlua.vm.KahluaThread.<init>(Platform, KahluaTable)
    pub fn new(platform: Rc<J2SEPlatform>, environment: TableRef) -> KahluaThread {
        let root = Coroutine::new(Some(environment), true);
        KahluaThread {
            current_coroutine: root.clone(),
            root_coroutine: root,
            cached_metatables: HashMap::new(),
            platform,
            out: String::new(),
            reset: false,
            error_count: 0,
        }
    }

    fn current_frame(&self) -> Option<FrameRef> {
        let co = self.current_coroutine.clone();
        let idx = co.borrow().current_call_frame()?;
        Some(FrameRef { coroutine: co, idx })
    }

    /// se.krka.kahlua.vm.KahluaThread.call(int): calls the function below the `n_arguments` top slots; its
    /// results replace it and the arguments. Returns the number of results.
    pub fn call(&mut self, n_arguments: i32) -> R<i32> {
        let co = self.current_coroutine.clone();
        let top = co.borrow().get_top();
        let base = top - n_arguments - 1;
        let o = co.borrow().stack_get(base);
        match o {
            Object::Nil => Err(Throwable::runtime("tried to call nil")),
            Object::Function(f) => self
                .call_java(f.clone(), base + 1, base, n_arguments)
                .map_err(|e| {
                    if is_exception(&e) {
                        Throwable::runtime(format!(
                            "{} {} in {}",
                            e.class_name(),
                            e.get_message().unwrap_or_else(|| "null".into()),
                            f.java_to_string()
                        ))
                    } else {
                        e
                    }
                }),
            Object::Closure(c) => {
                let idx = co.borrow_mut().push_new_call_frame(
                    Some(c),
                    None,
                    base + 1,
                    base,
                    n_arguments,
                    false,
                    false,
                )?;
                FrameRef {
                    coroutine: co.clone(),
                    idx,
                }
                .init()?;
                self.lua_mainloop()?;
                let n_return_values = self.current_coroutine.borrow().get_top() - base;
                self.current_coroutine.borrow_mut().stack_trace.clear();
                Ok(n_return_values)
            }
            _ => Err(Throwable::runtime("tried to call a non-function")),
        }
    }

    /// se.krka.kahlua.vm.KahluaThread.callJava
    fn call_java(
        &mut self,
        f: Rc<dyn JavaFunction>,
        local_base: i32,
        return_base: i32,
        n_arguments: i32,
    ) -> R<i32> {
        let co = self.current_coroutine.clone();
        let idx = co.borrow_mut().push_new_call_frame(
            None,
            Some(f.clone()),
            local_base,
            return_base,
            n_arguments,
            false,
            false,
        )?;
        let frame = FrameRef {
            coroutine: co.clone(),
            idx,
        };
        let n_return_values = f.call(self, &frame, n_arguments)?;
        let top = frame.get_top();
        let actual_return_base = top - n_return_values;
        let diff = return_base - local_base;
        frame.stack_copy(actual_return_base, diff, n_return_values);
        frame.set_top(n_return_values + diff)?;
        co.borrow_mut().pop_call_frame()?;
        Ok(n_return_values)
    }

    /// se.krka.kahlua.vm.KahluaThread.prepareMetatableCall
    fn prepare_metatable_call(&mut self, o: &Object) -> R<Object> {
        if matches!(o, Object::Function(_) | Object::Closure(_)) {
            Ok(o.clone())
        } else {
            self.get_meta_op(o, "__call")
        }
    }

    fn load_state(cf: FrameRef) -> R<LoopState> {
        let (closure, return_base) = cf.with(|f| (f.closure.clone(), f.return_base));
        let closure = closure.ok_or_else(|| {
            Throwable::npe("Cannot read field \"prototype\" because \"closure\" is null")
        })?;
        Ok(LoopState {
            prototype: closure.prototype.clone(),
            closure,
            return_base,
            cf,
        })
    }

    /// se.krka.kahlua.vm.KahluaThread.luaMainloop
    fn lua_mainloop(&mut self) -> R<()> {
        if self.reset {
            return Ok(());
        }
        let cf = self.current_frame().ok_or_else(|| {
            Throwable::npe("Cannot read field \"closure\" because \"callFrame\" is null")
        })?;
        let mut st = Self::load_state(cf)?;
        loop {
            match self.exec_one(&mut st) {
                Ok(Flow::Next) => {}
                Ok(Flow::Return) => return Ok(()),
                Err(e) => {
                    // ExceptionLogger.logException and debugException (logs, not ported), then
                    // doStacktraceProper, which can itself throw and then replaces `e` without unwinding.
                    self.error_count += 1;
                    let cf = st.cf.clone();
                    self.do_stacktrace_proper(Some(&cf))?;
                    let mut rethrow = true;
                    let mut last: Option<FrameRef>;
                    loop {
                        let cur = self.current_coroutine.clone();
                        let Some(cf) = self.current_frame() else {
                            last = None;
                            let parent = cur.borrow().get_parent();
                            if let Some(parent) = parent {
                                let pidx = parent.borrow().current_call_frame();
                                let next = pidx.map(|idx| FrameRef {
                                    coroutine: parent.clone(),
                                    idx,
                                });
                                if let Some(next) = &next {
                                    next.push(Object::Boolean(false))?;
                                    next.push(
                                        e.get_message().map_or(Object::Nil, |m| Object::str(&m)),
                                    )?;
                                    let trace = cur.borrow().stack_trace.clone();
                                    next.push(Object::str(&trace))?;
                                }
                                cur.borrow_mut().destroy();
                                self.current_coroutine = parent;
                                let cf = self.current_frame().ok_or_else(|| {
                                    Throwable::npe("Cannot read field \"closure\" because \"callFrame\" is null")
                                })?;
                                last = Some(cf.clone());
                                st = Self::load_state(cf)?;
                                rethrow = false;
                            }
                            break;
                        };
                        cur.borrow_mut().add_stack_trace(cf.idx);
                        cur.borrow_mut().pop_call_frame()?;
                        let from_lua = cf.with(|f| f.from_lua);
                        last = Some(cf);
                        if !from_lua {
                            break;
                        }
                    }
                    if let Some(cf) = &last {
                        cf.close_upvalues(0);
                    }
                    if rethrow {
                        return Err(e);
                    }
                }
            }
            if self.reset {
                return Err(Throwable::runtime("lua was reset"));
            }
        }
    }

    fn rk(&self, st: &LoopState, index: i32) -> R<Object> {
        let cindex = index - 256;
        if cindex < 0 {
            Ok(st.cf.get(index))
        } else {
            st.prototype
                .constants
                .get(cindex as usize)
                .cloned()
                .ok_or_else(|| aioobe(i64::from(cindex), st.prototype.constants.len()))
        }
    }

    fn fetch(st: &LoopState) -> R<i32> {
        let pc = st.cf.with_mut(|f| {
            let p = f.pc;
            f.pc += 1;
            p
        });
        st.prototype
            .code
            .get(pc as usize)
            .copied()
            .ok_or_else(|| aioobe(i64::from(pc), st.prototype.code.len()))
    }

    fn func_name(st: &LoopState) -> String {
        if let Some(n) = st.closure.debug_name.borrow().as_ref() {
            return n.clone();
        }
        st.prototype
            .name
            .clone()
            .unwrap_or_else(|| "unknown".into())
    }

    /// One instruction of `luaMainloop`'s switch.
    fn exec_one(&mut self, st: &mut LoopState) -> R<Flow> {
        let op = Self::fetch(st)?;
        let opcode = op & 63;
        let cf = st.cf.clone();
        match opcode {
            0 => {
                let (a, b) = (get_a8(op), get_b9(op));
                cf.set(a, cf.get(b));
            }
            1 => {
                let (a, b) = (get_a8(op), get_bx(op));
                let k = st.prototype.constants.get(b as usize).cloned();
                cf.set(
                    a,
                    k.ok_or_else(|| aioobe(i64::from(b), st.prototype.constants.len()))?,
                );
            }
            2 => {
                let (a, b, c) = (get_a8(op), get_b9(op), get_c9(op));
                cf.set(a, Object::Boolean(b != 0));
                if c != 0 {
                    cf.with_mut(|f| f.pc += 1);
                }
            }
            3 => {
                let (a, b) = (get_a8(op), get_b9(op));
                cf.stack_clear(a, b);
            }
            4 => {
                let (a, b) = (get_a8(op), get_b9(op));
                let uv = st.closure.upvalues.get(b as usize).cloned();
                let uv = uv.ok_or_else(|| aioobe(i64::from(b), st.closure.upvalues.len()))?;
                let v = uv.borrow().get_value();
                cf.set(a, v);
            }
            5 => {
                let (a, b) = (get_a8(op), get_bx(op));
                let env = Object::Table(st.closure.env.borrow().clone());
                let key = st
                    .prototype
                    .constants
                    .get(b as usize)
                    .cloned()
                    .unwrap_or_default();
                let res = self.tableget(&env, &key)?;
                cf.set(a, res);
            }
            6 => {
                let (a, b, c) = (get_a8(op), get_b9(op), get_c9(op));
                let b_obj = cf.get(b);
                let key = self.rk(st, c)?;
                let res = self.tableget(&b_obj, &key)?;
                cf.set(a, res);
            }
            7 => {
                let (a, b) = (get_a8(op), get_bx(op));
                let value = cf.get(a);
                let key = st
                    .prototype
                    .constants
                    .get(b as usize)
                    .cloned()
                    .unwrap_or_default();
                if let (Object::Closure(c), Object::String(k)) = (&value, &key) {
                    *c.debug_name.borrow_mut() = Some(super::object::rstr(k));
                }
                let env = Object::Table(st.closure.env.borrow().clone());
                self.table_set(&env, &key, &value)?;
            }
            8 => {
                let (a, b) = (get_a8(op), get_b9(op));
                let uv = st.closure.upvalues.get(b as usize).cloned();
                let uv = uv.ok_or_else(|| aioobe(i64::from(b), st.closure.upvalues.len()))?;
                let v = cf.get(a);
                uv.borrow_mut().set_value(v);
            }
            9 => {
                let (a, b, c) = (get_a8(op), get_b9(op), get_c9(op));
                let a_obj = cf.get(a);
                let key = self.rk(st, b)?;
                let value = self.rk(st, c)?;
                self.table_set(&a_obj, &key, &value)?;
            }
            10 => {
                let a = get_a8(op);
                cf.set(a, Object::Table(KahluaTableImpl::new_ref()));
            }
            11 => {
                let (a, b, c) = (get_a8(op), get_b9(op), get_c9(op));
                let key = self.rk(st, c)?;
                let b_obj = cf.get(b);
                let fun = self.tableget(&b_obj, &key)?;
                cf.set(a, fun);
                cf.set(a + 1, b_obj);
            }
            12..=17 => {
                let (a, b, c) = (get_a8(op), get_b9(op), get_c9(op));
                let bo = self.rk(st, b)?;
                let co = self.rk(st, c)?;
                let bd = raw_tonumber(&bo)?;
                let cd = if bd.is_some() {
                    raw_tonumber(&co)?
                } else {
                    None
                };
                let res = match (bd, cd) {
                    (Some(x), Some(y)) => {
                        Object::Number(to_double(self.primitive_math(x, y, opcode)))
                    }
                    _ => {
                        let meta_op = meta_op_name(opcode);
                        let metafun = self.get_bin_meta_op(&bo, &co, meta_op)?;
                        if metafun.is_nil() {
                            self.do_stacktrace_proper(Some(&cf))?;
                            return Err(fail(&format!(
                                "{meta_op} not defined for operands in {}",
                                Self::func_name(st)
                            )));
                        }
                        self.call3(&metafun, bo, co, Object::Nil)?
                    }
                };
                cf.set(a, res);
            }
            18 => {
                let (a, b) = (get_a8(op), get_b9(op));
                let a_obj = cf.get(b);
                let res = match raw_tonumber(&a_obj)? {
                    Some(d) => Object::Number(to_double(-d)),
                    None => {
                        let metafun = self.get_meta_op(&a_obj, "__unm")?;
                        self.call3(&metafun, a_obj, Object::Nil, Object::Nil)?
                    }
                };
                cf.set(a, res);
            }
            19 => {
                let (a, b) = (get_a8(op), get_b9(op));
                let a_obj = cf.get(b);
                cf.set(a, Object::Boolean(!bool_eval(&a_obj)));
            }
            20 => {
                let (a, b) = (get_a8(op), get_b9(op));
                let o = cf.get(b);
                let res = match &o {
                    Object::Table(t) => Object::Number(f64::from(KahluaTableImpl::len(t)?)),
                    Object::String(s) => Object::Number(s.len() as f64),
                    _ => {
                        let f = self.get_meta_op(&o, "__len")?;
                        if f.is_nil() {
                            self.do_stacktrace_proper(Some(&cf))?;
                        }
                        lua_assert(!f.is_nil(), "__len not defined for operand")?;
                        self.call3(&f, o.clone(), Object::Nil, Object::Nil)?
                    }
                };
                cf.set(a, res);
            }
            21 => {
                let (a, b, c) = (get_a8(op), get_b9(op), get_c9(op));
                let first = b;
                let mut resx = cf.get(c);
                let mut last = c - 1;
                while first <= last {
                    if let Some(res_str) = raw_tostring(&resx) {
                        let mut n_strings = 0;
                        let mut pos = last;
                        while first <= pos {
                            let o = cf.get(pos);
                            pos -= 1;
                            if raw_tostring(&o).is_none() {
                                break;
                            }
                            n_strings += 1;
                        }
                        if n_strings > 0 {
                            let mut buf: Vec<u16> = Vec::new();
                            for i in (last - n_strings + 1)..=last {
                                if let Some(s) = raw_tostring(&cf.get(i)) {
                                    buf.extend_from_slice(&s);
                                }
                            }
                            buf.extend_from_slice(&res_str);
                            resx = Object::String(buf.into());
                            last -= n_strings;
                        }
                    }
                    if first <= last {
                        // The decompiled `(String)` cast here is Vineflower's: the bytecode keeps an Object.
                        let left_obj = cf.get(last);
                        let metafunx = self.get_bin_meta_op(&left_obj, &resx, "__concat")?;
                        if metafunx.is_nil() {
                            return Err(fail(&format!(
                                "__concat not defined for operands: {} and {}",
                                left_obj.java_to_string(),
                                resx.java_to_string()
                            )));
                        }
                        resx = self.call3(&metafunx, left_obj, resx, Object::Nil)?;
                        last -= 1;
                    }
                }
                cf.set(a, resx);
            }
            22 => {
                let sbx = get_sbx(op);
                cf.with_mut(|f| f.pc += sbx);
            }
            23..=25 => {
                let (a, b, c) = (get_a8(op), get_b9(op), get_c9(op));
                let mut box_ = self.rk(st, b)?;
                let mut cox = self.rk(st, c)?;
                let skip = match (&box_, &cox) {
                    (Object::Number(x), Object::Number(y)) => {
                        let r = match opcode {
                            23 => x == y,
                            24 => x < y,
                            _ => x <= y,
                        };
                        r == (a == 0)
                    }
                    (Object::String(x), Object::String(y)) => {
                        let r = if opcode == 23 {
                            x[..] == y[..]
                        } else {
                            let cmp = x[..].cmp(&y[..]);
                            if opcode == 24 {
                                cmp.is_lt()
                            } else {
                                cmp.is_le()
                            }
                        };
                        r == (a == 0)
                    }
                    _ => {
                        let res_bool = if box_.same(&cox) && opcode == 23 {
                            true
                        } else {
                            let mut invert = false;
                            let meta_op = meta_op_name(opcode);
                            let mut metafun = self.get_comp_meta_op(&box_, &cox, meta_op)?;
                            if metafun.is_nil() && opcode == 25 {
                                metafun = self.get_comp_meta_op(&box_, &cox, "__lt")?;
                                std::mem::swap(&mut box_, &mut cox);
                                invert = true;
                            }
                            let r = if metafun.is_nil() && opcode == 23 {
                                lua_equals(&box_, &cox)
                            } else {
                                if metafun.is_nil() {
                                    self.do_stacktrace_proper(Some(&cf))?;
                                    return Err(fail(&format!(
                                        "{meta_op} not defined for operand"
                                    )));
                                }
                                let res =
                                    self.call3(&metafun, box_.clone(), cox.clone(), Object::Nil)?;
                                bool_eval(&res)
                            };
                            if invert {
                                !r
                            } else {
                                r
                            }
                        };
                        res_bool == (a == 0)
                    }
                };
                if skip {
                    cf.with_mut(|f| f.pc += 1);
                }
            }
            26 => {
                let (a, c) = (get_a8(op), get_c9(op));
                let value = cf.get(a);
                if bool_eval(&value) == (c == 0) {
                    cf.with_mut(|f| f.pc += 1);
                }
            }
            27 => {
                let (a, b, c) = (get_a8(op), get_b9(op), get_c9(op));
                let value = cf.get(b);
                if bool_eval(&value) != (c == 0) {
                    cf.set(a, value);
                } else {
                    cf.with_mut(|f| f.pc += 1);
                }
            }
            28 => return self.op_call(st, op),
            29 => return self.op_tailcall(st, op),
            30 => return self.op_return(st, op),
            31 => {
                let a = get_a8(op);
                let mut iter = from_double(&cf.get(a))?;
                let end = from_double(&cf.get(a + 1))?;
                let step = from_double(&cf.get(a + 2))?;
                iter += step;
                let iter_double = to_double(iter);
                cf.set(a, Object::Number(iter_double));
                // `!(a <= b)` as in the original: a NaN bound or step ends the loop.
                #[allow(clippy::neg_cmp_op_on_partial_ord)]
                let done = if step > 0.0 {
                    !(iter <= end)
                } else {
                    !(iter >= end)
                };
                if done {
                    cf.clear_from_index(a)?;
                } else {
                    let bx = get_sbx(op);
                    cf.with_mut(|f| f.pc += bx);
                    cf.set(a + 3, Object::Number(iter_double));
                }
            }
            32 => {
                let (a, b) = (get_a8(op), get_sbx(op));
                let iter = from_double(&cf.get(a))?;
                let step = from_double(&cf.get(a + 2))?;
                cf.set(a, Object::Number(to_double(iter - step)));
                cf.with_mut(|f| f.pc += b);
            }
            33 => {
                let (a, c) = (get_a8(op), get_c9(op));
                cf.set_top(a + 6)?;
                cf.stack_copy(a, a + 3, 3);
                self.call(2)?;
                cf.clear_from_index(a + 3 + c)?;
                cf.set_prototype_stacksize()?;
                let a_obj3 = cf.get(a + 3);
                if !a_obj3.is_nil() {
                    cf.set(a + 2, a_obj3);
                } else {
                    cf.with_mut(|f| f.pc += 1);
                }
            }
            34 => {
                let (a, mut b, mut c) = (get_a8(op), get_b9(op), get_c9(op));
                if b == 0 {
                    b = cf.get_top() - a - 1;
                }
                if c == 0 {
                    c = Self::fetch(st)?;
                }
                let offset = (c - 1) * FIELDS_PER_FLUSH;
                let t = cast_table(&cf.get(a))?.ok_or_else(|| {
                    Throwable::npe(
                        "Cannot invoke \"se.krka.kahlua.vm.KahluaTable.rawset(Object, Object)\" because \"t\" is null",
                    )
                })?;
                for i in 1..=b {
                    KahluaTableImpl::rawset(
                        &t,
                        Object::Number(f64::from(offset + i)),
                        cf.get(a + i),
                    );
                }
            }
            35 => {
                let a = get_a8(op);
                cf.close_upvalues(a);
            }
            36 => {
                let (a, b) = (get_a8(op), get_bx(op));
                let new_prototype = st
                    .prototype
                    .prototypes
                    .get(b as usize)
                    .cloned()
                    .ok_or_else(|| aioobe(i64::from(b), st.prototype.prototypes.len()))?;
                let mut upvalues = Vec::with_capacity(new_prototype.num_upvalues.max(0) as usize);
                for _ in 0..new_prototype.num_upvalues {
                    let op2 = Self::fetch(st)?;
                    let b2 = get_b9(op2);
                    match op2 & 63 {
                        0 => upvalues.push(cf.find_upvalue(b2)),
                        4 => {
                            let uv = st.closure.upvalues.get(b2 as usize).cloned();
                            upvalues.push(
                                uv.ok_or_else(|| aioobe(i64::from(b2), st.closure.upvalues.len()))?,
                            );
                        }
                        _ => {}
                    }
                }
                let env = st.closure.env.borrow().clone();
                cf.set(
                    a,
                    Object::Closure(LuaClosure::new(new_prototype, env, upvalues)),
                );
            }
            37 => {
                let (a, b) = (get_a8(op), get_b9(op) - 1);
                cf.push_varargs(a, b)?;
            }
            _ => {}
        }
        Ok(Flow::Next)
    }

    fn op_call(&mut self, st: &mut LoopState, op: i32) -> R<Flow> {
        let cf = st.cf.clone();
        let (a, b, c) = (get_a8(op), get_b9(op), get_c9(op));
        let mut n_arguments = b - 1;
        if n_arguments != -1 {
            cf.set_top(a + n_arguments + 1)?;
        } else {
            n_arguments = cf.get_top() - a - 1;
        }
        cf.with_mut(|f| f.restore_top = c != 0);
        let base = cf.local_base();
        let mut local_base2 = base + a + 1;
        let return_base2 = base + a;
        let fun_object = cf.get(a);
        if fun_object.is_nil() {
            self.do_stacktrace_proper(Some(&cf))?;
            return Err(fail(&format!(
                "Object tried to call nil in {}",
                Self::func_name(st)
            )));
        }
        let fun = self.prepare_metatable_call(&fun_object)?;
        if fun.is_nil() {
            return Err(fail(&format!(
                "Object {} did not have __call metatable set",
                fun_object.java_to_string()
            )));
        }
        if !fun.same(&fun_object) {
            local_base2 = return_base2;
            n_arguments += 1;
        }
        match fun {
            Object::Closure(lc) => {
                let can_yield = cf.with(|f| f.can_yield);
                let co = self.current_coroutine.clone();
                let idx = co.borrow_mut().push_new_call_frame(
                    Some(lc),
                    None,
                    local_base2,
                    return_base2,
                    n_arguments,
                    true,
                    can_yield,
                )?;
                let new_cf = FrameRef { coroutine: co, idx };
                new_cf.init()?;
                *st = Self::load_state(new_cf)?;
                Ok(Flow::Next)
            }
            Object::Function(jf) => {
                self.call_java(jf, local_base2, return_base2, n_arguments)?;
                match self.current_frame() {
                    Some(cf2) if cf2.is_lua() => {
                        *st = Self::load_state(cf2)?;
                        if st.cf.with(|f| f.restore_top) {
                            st.cf.set_top(st.prototype.max_stacksize)?;
                        }
                        Ok(Flow::Next)
                    }
                    _ => Ok(Flow::Return),
                }
            }
            other => Err(Throwable::runtime(format!(
                "Tried to call a non-function: {}",
                other.java_to_string()
            ))),
        }
    }

    fn op_tailcall(&mut self, st: &mut LoopState, op: i32) -> R<Flow> {
        let cf = st.cf.clone();
        let base = cf.local_base();
        self.current_coroutine.borrow_mut().close_upvalues(base);
        let (a, b) = (get_a8(op), get_b9(op));
        let mut n_arguments2 = b - 1;
        if n_arguments2 == -1 {
            n_arguments2 = cf.get_top() - a - 1;
        }
        cf.with_mut(|f| f.restore_top = false);
        let fun_object = cf.get(a);
        if fun_object.is_nil() {
            self.error_count += 1;
            self.do_stacktrace_proper(Some(&cf))?;
            return Err(fail(""));
        }
        let fun = self.prepare_metatable_call(&fun_object)?;
        if fun.is_nil() {
            return Err(fail("Object did not have __call metatable set"));
        }
        let return_base = st.return_base;
        let mut local_base2 = return_base + 1;
        if !fun.same(&fun_object) {
            local_base2 = return_base;
            n_arguments2 += 1;
        }
        {
            let mut co = self.current_coroutine.borrow_mut();
            co.stack_copy(base + a, return_base, n_arguments2 + 1);
            co.set_top(return_base + n_arguments2 + 1)?;
        }
        let next_cf = match fun {
            Object::Closure(lc) => {
                cf.with_mut(|f| {
                    f.local_base = local_base2;
                    f.n_arguments = n_arguments2;
                    f.closure = Some(lc);
                });
                cf.init()?;
                cf
            }
            other => {
                let Object::Function(jf) = other else {
                    return Err(fail(&format!(
                        "Tried to call a non-function: {}",
                        other.java_to_string()
                    )));
                };
                let old_coroutine = self.current_coroutine.clone();
                self.call_java(jf, local_base2, return_base, n_arguments2)?;
                let popped = self.current_frame();
                old_coroutine.borrow_mut().pop_call_frame()?;
                if !Rc::ptr_eq(&old_coroutine, &self.current_coroutine) {
                    let old_dead = old_coroutine.borrow().is_dead();
                    let cur_parent = self.current_coroutine.borrow().get_parent();
                    if old_dead
                        && !Rc::ptr_eq(&old_coroutine, &self.root_coroutine)
                        && cur_parent
                            .as_ref()
                            .is_some_and(|p| Rc::ptr_eq(p, &old_coroutine))
                    {
                        let old_parent = old_coroutine.borrow().get_parent();
                        if let Some(op) = old_parent {
                            self.current_coroutine.borrow_mut().resume(op);
                        }
                        old_coroutine.borrow_mut().destroy();
                        let p = self.current_coroutine.borrow().get_parent();
                        if let Some(p) = p {
                            let pidx = p.borrow().current_call_frame();
                            if let Some(idx) = pidx {
                                FrameRef { coroutine: p, idx }.push(Object::Boolean(true))?;
                            }
                        }
                    }
                    let cf2 = self.current_frame().ok_or_else(|| {
                        Throwable::npe(
                            "Cannot invoke \"se.krka.kahlua.vm.LuaCallFrame.isJava()\" because \"callFrame\" is null",
                        )
                    })?;
                    if !cf2.is_lua() {
                        return Ok(Flow::Return);
                    }
                    cf2
                } else {
                    let from_lua = popped.as_ref().is_some_and(|p| p.with(|f| f.from_lua));
                    if !from_lua {
                        return Ok(Flow::Return);
                    }
                    let cf2 = self.current_frame().ok_or_else(|| {
                        Throwable::npe(
                            "Cannot read field \"restoreTop\" because \"callFrame\" is null",
                        )
                    })?;
                    if cf2.with(|f| f.restore_top) {
                        let ms = cf2.closure().map_or(0, |c| c.prototype.max_stacksize);
                        cf2.set_top(ms)?;
                    }
                    cf2
                }
            }
        };
        *st = Self::load_state(next_cf)?;
        Ok(Flow::Next)
    }

    fn op_return(&mut self, st: &mut LoopState, op: i32) -> R<Flow> {
        let cf = st.cf.clone();
        let a = get_a8(op);
        let mut b = get_b9(op) - 1;
        let base = cf.local_base();
        self.current_coroutine.borrow_mut().close_upvalues(base);
        if b == -1 {
            b = cf.get_top() - a;
        }
        {
            let mut co = self.current_coroutine.borrow_mut();
            co.stack_copy(base + a, st.return_base, b);
            co.set_top(st.return_base + b)?;
        }
        let (from_lua, can_yield) = cf.with(|f| (f.from_lua, f.can_yield));
        if !from_lua {
            self.current_coroutine.borrow_mut().pop_call_frame()?;
            return Ok(Flow::Return);
        }
        if can_yield && self.current_coroutine.borrow().at_bottom() {
            cf.with_mut(|f| f.local_base = f.return_base);
            let coroutine = self.current_coroutine.clone();
            yield_helper(self, &cf, &cf, b)?;
            coroutine.borrow_mut().pop_call_frame()?;
            match self.current_frame() {
                Some(c) if c.is_lua() => {}
                _ => return Ok(Flow::Return),
            }
        } else {
            self.current_coroutine.borrow_mut().pop_call_frame()?;
        }
        let cf2 = self.current_frame().ok_or_else(|| {
            Throwable::npe("Cannot read field \"closure\" because \"callFrame\" is null")
        })?;
        *st = Self::load_state(cf2)?;
        if st.cf.with(|f| f.restore_top) {
            st.cf.set_top(st.prototype.max_stacksize)?;
        }
        Ok(Flow::Next)
    }

    /// se.krka.kahlua.vm.KahluaThread.doStacktraceProper: the log line itself is not ported, but building it turns
    /// every Lua frame of the coroutine into a `StackTraceElement`, and for a built-in function (no file name,
    /// as `loadstring` chunks have while `FuncState.currentfullFile` is unset) without a name the JDK throws
    /// NullPointerException("Method name is null"), which then replaces the error being reported.
    fn do_stacktrace_proper(&mut self, frame: Option<&FrameRef>) -> R<()> {
        let Some(frame) = frame else {
            return Ok(());
        };
        self.error_count += 1;
        let co = frame.coroutine.borrow();
        for i in (0..co.get_callframe_top()).rev() {
            if let Some(c) = &co.frame(i as usize).closure {
                let p = &c.prototype;
                if !p.lines.is_empty() && p.filename.is_none() && p.name.is_none() {
                    return Err(Throwable::npe("Method name is null"));
                }
            }
        }
        Ok(())
    }

    /// se.krka.kahlua.vm.KahluaThread.getMetaOp
    pub fn get_meta_op(&mut self, o: &Object, meta_op: &str) -> R<Object> {
        match self.getmetatable(o, true)? {
            Object::Table(meta) => KahluaTableImpl::rawget(&meta, &Object::str(meta_op)),
            _ => Ok(Object::Nil),
        }
    }

    /// se.krka.kahlua.vm.KahluaThread.getCompMetaOp
    fn get_comp_meta_op(&mut self, a: &Object, b: &Object, meta_op: &str) -> R<Object> {
        let meta1 = self.getmetatable(a, true)?;
        let meta2 = self.getmetatable(b, true)?;
        match (meta1, meta2) {
            (Object::Table(m1), Object::Table(m2)) => {
                let op1 = KahluaTableImpl::rawget(&m1, &Object::str(meta_op))?;
                let op2 = KahluaTableImpl::rawget(&m2, &Object::str(meta_op))?;
                Ok(if op1.same(&op2) && !op1.is_nil() {
                    op1
                } else {
                    Object::Nil
                })
            }
            _ => Ok(Object::Nil),
        }
    }

    /// se.krka.kahlua.vm.KahluaThread.getBinMetaOp
    fn get_bin_meta_op(&mut self, a: &Object, b: &Object, meta_op: &str) -> R<Object> {
        let op = self.get_meta_op(a, meta_op)?;
        if !op.is_nil() {
            Ok(op)
        } else {
            self.get_meta_op(b, meta_op)
        }
    }

    /// se.krka.kahlua.vm.KahluaThread.primitiveMath
    fn primitive_math(&self, v1: f64, v2: f64, opcode: i32) -> f64 {
        match opcode {
            12 => v1 + v2,
            13 => v1 - v2,
            14 => v1 * v2,
            15 => v1 / v2,
            16 => {
                if v2 == 0.0 {
                    f64::NAN
                } else {
                    let ipart = (v1 / v2) as i32;
                    v1 - f64::from(ipart) * v2
                }
            }
            17 => self.platform.pow(v1, v2),
            _ => 0.0,
        }
    }

    /// se.krka.kahlua.vm.KahluaThread.call(Object, Object, Object, Object): the first result or nil.
    pub fn call3(&mut self, fun: &Object, arg1: Object, arg2: Object, arg3: Object) -> R<Object> {
        self.call_args(fun, &[arg1, arg2, arg3])
    }

    /// se.krka.kahlua.vm.KahluaThread.call(Object, Object[])
    pub fn call_args(&mut self, fun: &Object, args: &[Object]) -> R<Object> {
        let old_top = self.current_coroutine.borrow().get_top();
        {
            let mut co = self.current_coroutine.borrow_mut();
            co.set_top(old_top + 1 + args.len() as i32)?;
            co.stack_set(old_top, fun.clone());
            for (i, a) in args.iter().enumerate() {
                co.stack_set(old_top + 1 + i as i32, a.clone());
            }
        }
        let n_return_values = self.call(args.len() as i32)?;
        let mut co = self.current_coroutine.borrow_mut();
        let ret = if n_return_values >= 1 {
            co.stack_get(old_top)
        } else {
            Object::Nil
        };
        co.set_top(old_top)?;
        Ok(ret)
    }

    /// se.krka.kahlua.vm.KahluaThread.tableget: raw lookup, then `__index` (a function is called with the original
    /// table; a table is looked into), at most 100 levels.
    pub fn tableget(&mut self, table: &Object, key: &Object) -> R<Object> {
        let mut cur = table.clone();
        for _ in 0..MAX_INDEX_RECURSION {
            let is_table = matches!(cur, Object::Table(_));
            if let Object::Table(t) = &cur {
                let res = KahluaTableImpl::rawget(t, key)?;
                if !res.is_nil() {
                    return Ok(res);
                }
            }
            let meta_op = self.get_meta_op(&cur, "__index")?;
            if meta_op.is_nil() {
                if is_table {
                    return Ok(Object::Nil);
                }
                self.error_count += 1;
                let cf = self.current_frame();
                self.do_stacktrace_proper(cf.as_ref())?;
                return Err(Throwable::runtime(format!(
                    "attempted index: {} of non-table: {}",
                    key.java_to_string(),
                    cur.java_to_string()
                )));
            }
            if matches!(meta_op, Object::Function(_) | Object::Closure(_)) {
                return self.call3(&meta_op, table.clone(), key.clone(), Object::Nil);
            }
            cur = meta_op;
        }
        Err(Throwable::runtime("loop in gettable"))
    }

    /// se.krka.kahlua.vm.KahluaThread.tableSet
    pub fn table_set(&mut self, table: &Object, key: &Object, value: &Object) -> R<()> {
        let mut cur = table.clone();
        for _ in 0..MAX_INDEX_RECURSION {
            let meta_op = if let Object::Table(t) = &cur {
                if !KahluaTableImpl::rawget(t, key)?.is_nil() {
                    KahluaTableImpl::rawset(t, key.clone(), value.clone());
                    return Ok(());
                }
                let m = self.get_meta_op(&cur, "__newindex")?;
                if m.is_nil() {
                    KahluaTableImpl::rawset(t, key.clone(), value.clone());
                    return Ok(());
                }
                m
            } else {
                let m = self.get_meta_op(&cur, "__newindex")?;
                if m.is_nil() {
                    let cf = self.current_frame();
                    self.do_stacktrace_proper(cf.as_ref())?;
                }
                lua_assert(!m.is_nil(), "attempted index of non-table")?;
                m
            };
            if matches!(meta_op, Object::Function(_) | Object::Closure(_)) {
                self.call3(&meta_op, table.clone(), key.clone(), value.clone())?;
                return Ok(());
            }
            cur = meta_op;
        }
        Err(Throwable::runtime("loop in settable"))
    }

    /// se.krka.kahlua.vm.KahluaThread.setmetatable
    pub fn setmetatable(&mut self, o: &Object, metatable: Option<TableRef>) -> R<()> {
        lua_assert(!o.is_nil(), "Can't set metatable for nil")?;
        match o {
            Object::Table(t) => {
                t.borrow_mut().metatable = metatable;
                Ok(())
            }
            _ => Err(fail("Could not set metatable for object")),
        }
    }

    /// se.krka.kahlua.vm.KahluaThread.getmetatable
    pub fn getmetatable(&mut self, o: &Object, raw: bool) -> R<Object> {
        let metatable = match o {
            Object::Nil => return Ok(Object::Nil),
            Object::Table(t) => t.borrow().metatable.clone(),
            _ => match o.class() {
                Some(c) => self.get_class_metatable(c)?,
                None => None,
            },
        };
        if let (false, Some(m)) = (raw, &metatable) {
            let meta2 = KahluaTableImpl::rawget(m, &Object::str("__metatable"))?;
            if !meta2.is_nil() {
                return Ok(meta2);
            }
        }
        Ok(metatable.map_or(Object::Nil, Object::Table))
    }

    /// se.krka.kahlua.vm.KahluaThread.getClassMetatable: breadth first over the class, its superclass and
    /// interfaces; the answer (also "none") is cached per thread.
    fn get_class_metatable(&mut self, c: &'static JavaClass) -> R<Option<TableRef>> {
        if let Some(m) = self.cached_metatables.get(c.name) {
            return Ok(m.clone());
        }
        let env = self.get_environment();
        let metatables = Object::Table(get_class_metatables(&env)?);
        let mut queue: std::collections::VecDeque<&'static JavaClass> =
            std::collections::VecDeque::new();
        queue.push_back(c);
        let mut metatable = None;
        while let Some(clazz) = queue.pop_front() {
            let key = self.platform.class_object(clazz);
            metatable = cast_table(&self.tableget(&metatables, &key)?)?;
            if metatable.is_some() {
                break;
            }
            if let Some(s) = clazz.superclass {
                queue.push_back(s);
            }
            queue.extend(clazz.interfaces.iter().copied());
        }
        self.cached_metatables.insert(c.name, metatable.clone());
        Ok(metatable)
    }

    /// se.krka.kahlua.vm.KahluaThread.pcall(Object, Object[]): the results, `true` or `false` first.
    pub fn pcall_args(&mut self, fun: &Object, args: &[Object]) -> R<Vec<Object>> {
        let coroutine = self.current_coroutine.clone();
        let old_top = coroutine.borrow().get_top();
        {
            let mut co = coroutine.borrow_mut();
            co.set_top(old_top + 1 + args.len() as i32)?;
            co.stack_set(old_top, fun.clone());
            for (i, a) in args.iter().enumerate() {
                co.stack_set(old_top + 1 + i as i32, a.clone());
            }
        }
        let n_ret = self.pcall(args.len() as i32)?;
        lua_assert(
            Rc::ptr_eq(&coroutine, &self.current_coroutine),
            "Internal Kahlua error - coroutine changed in pcall",
        )?;
        let mut co = coroutine.borrow_mut();
        let ret = (0..n_ret).map(|i| co.stack_get(old_top + i)).collect();
        co.set_top(old_top)?;
        Ok(ret)
    }

    /// se.krka.kahlua.vm.KahluaThread.pcall(int): `true` and the results, or `false`, the error message, the stack
    /// trace and the exception.
    pub fn pcall(&mut self, n_arguments: i32) -> R<i32> {
        let coroutine = self.current_coroutine.clone();
        let current_call_frame = coroutine.borrow().current_call_frame();
        coroutine.borrow_mut().stack_trace.clear();
        let old_base = coroutine.borrow().get_top() - n_arguments - 1;
        let attempt = (|| -> R<i32> {
            let old_callframe_top = coroutine.borrow().get_callframe_top();
            let n_values = self.call(n_arguments)?;
            let new_callframe_top = coroutine.borrow().get_callframe_top();
            lua_assert(
                old_callframe_top == new_callframe_top,
                "error - call stack depth changed.",
            )?;
            let mut co = coroutine.borrow_mut();
            co.set_top(old_base + n_values + 1)?;
            co.stack_copy(old_base, old_base + 1, n_values);
            co.stack_set(old_base, Object::Boolean(true));
            Ok(1 + n_values)
        })();
        let exception = match attempt {
            Ok(n) => return Ok(n),
            Err(e) => e,
        };
        let error_message = match &exception {
            Throwable::Kahlua(o) => o.clone(),
            Throwable::Java { class, message } => {
                Object::str(&format!("{} {class}", message.as_deref().unwrap_or("null")))
            }
        };
        lua_assert(
            Rc::ptr_eq(&coroutine, &self.current_coroutine),
            "Internal Kahlua error - coroutine changed in pcall",
        )?;
        if let Some(idx) = current_call_frame {
            FrameRef {
                coroutine: coroutine.clone(),
                idx,
            }
            .close_upvalues(0);
        }
        coroutine
            .borrow_mut()
            .clean_call_frames(current_call_frame)?;
        let mut co = coroutine.borrow_mut();
        co.set_top(old_base + 4)?;
        co.stack_set(old_base, Object::Boolean(false));
        co.stack_set(old_base + 1, error_message);
        let trace = std::mem::take(&mut co.stack_trace);
        co.stack_set(old_base + 2, Object::str(&trace));
        co.stack_set(
            old_base + 3,
            Object::Userdata(Rc::new(ThrowableObject(exception))),
        );
        Ok(4)
    }

    /// se.krka.kahlua.vm.KahluaThread.getEnvironment
    pub fn get_environment(&self) -> TableRef {
        match &self.current_coroutine.borrow().environment {
            Some(e) => e.clone(),
            None => KahluaTableImpl::new_ref(),
        }
    }

    /// `getOut().println(s)`.
    pub fn println(&mut self, s: &[u16]) {
        self.out.push_str(&super::object::rstr(s));
        self.out.push('\n');
    }
}

/// se.krka.kahlua.vm.Coroutine.yieldHelper
pub fn yield_helper(
    thread: &mut KahluaThread,
    call_frame: &FrameRef,
    args_call_frame: &FrameRef,
    n_arguments: i32,
) -> R<()> {
    lua_assert(
        call_frame.with(|f| f.can_yield),
        "Can not yield outside of a coroutine",
    )?;
    let coroutine = call_frame.coroutine.clone();
    let parent = coroutine.borrow().get_parent();
    lua_assert(
        parent.is_some(),
        "Internal error, coroutine must be running",
    )?;
    lua_assert(
        Rc::ptr_eq(&coroutine, &thread.current_coroutine),
        "Internal error, must yield current thread",
    )?;
    let Some(parent) = parent else {
        return Ok(());
    };
    coroutine.borrow_mut().destroy();
    let next = parent.borrow().current_call_frame();
    match next {
        None => {
            let values: Vec<Object> = (0..n_arguments).map(|i| args_call_frame.get(i)).collect();
            let mut p = parent.borrow_mut();
            p.set_top(n_arguments + 1)?;
            p.stack_set(0, Object::Boolean(true));
            for (i, v) in values.into_iter().enumerate() {
                p.stack_set(i as i32 + 1, v);
            }
        }
        Some(idx) => {
            let next = FrameRef {
                coroutine: parent.clone(),
                idx,
            };
            next.push(Object::Boolean(true))?;
            for i in 0..n_arguments {
                let value = args_call_frame.get(i);
                next.push(value)?;
            }
        }
    }
    thread.current_coroutine = parent;
    Ok(())
}

/// A string argument as Lua sees it (helper for the libraries).
pub fn lstr_obj(s: &str) -> Object {
    Object::String(lstr(s))
}

/// The string of a value if it is one.
pub fn as_str(o: &Object) -> Option<LuaStr> {
    match o {
        Object::String(s) => Some(s.clone()),
        _ => None,
    }
}
