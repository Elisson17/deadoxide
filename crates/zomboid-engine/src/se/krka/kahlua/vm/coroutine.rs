//! se.krka.kahlua.vm.Coroutine, se.krka.kahlua.vm.LuaCallFrame and se.krka.kahlua.vm.UpValue: a Lua stack with
//! its call frames. The original's `LuaCallFrame` objects live in a pooled array of the coroutine and are reused
//! (their fields survive a pop); here they are values in that array and a `FrameRef` (coroutine, slot) stands for
//! the object, so identity and the reuse behave the same. Stack indices stay Java `int`s; an index outside the
//! array reads nil and a write there is dropped (the original would throw ArrayIndexOutOfBoundsException, which
//! no compiled code reaches).
use std::cell::RefCell;
use std::rc::Rc;

use super::kahlua_util::lua_assert;
use super::lua_closure::LuaClosure;
use super::object::{CoroutineRef, JavaFunction, Object, TableRef, Throwable, R};

const MAX_STACK_SIZE: i32 = 3000;
const INITIAL_STACK_SIZE: usize = 1000;
const MAX_CALL_FRAME_STACK_SIZE: i32 = 1000;
const INITIAL_CALL_FRAME_STACK_SIZE: usize = 200;

/// se.krka.kahlua.vm.UpValue
pub struct UpValue {
    coroutine: Option<CoroutineRef>,
    index: i32,
    value: Object,
}

pub type UpValueRef = Rc<RefCell<UpValue>>;

impl UpValue {
    /// se.krka.kahlua.vm.UpValue.getIndex
    pub fn get_index(&self) -> i32 {
        self.index
    }

    /// se.krka.kahlua.vm.UpValue.getValue
    pub fn get_value(&self) -> Object {
        match &self.coroutine {
            None => self.value.clone(),
            Some(c) => c.borrow().stack_get(self.index),
        }
    }

    /// se.krka.kahlua.vm.UpValue.setValue
    pub fn set_value(&mut self, o: Object) {
        match &self.coroutine {
            None => self.value = o,
            Some(c) => c.borrow_mut().stack_set(self.index, o),
        }
    }
}

/// se.krka.kahlua.vm.LuaCallFrame: the fields of one pooled frame object.
#[derive(Clone, Default)]
pub struct LuaCallFrame {
    pub closure: Option<Rc<LuaClosure>>,
    pub java_function: Option<Rc<dyn JavaFunction>>,
    pub pc: i32,
    pub local_base: i32,
    pub return_base: i32,
    pub n_arguments: i32,
    pub from_lua: bool,
    pub can_yield: bool,
    pub restore_top: bool,
}

impl LuaCallFrame {
    /// se.krka.kahlua.vm.LuaCallFrame.isLua
    pub fn is_lua(&self) -> bool {
        self.closure.is_some()
    }
}

/// se.krka.kahlua.vm.Coroutine
pub struct Coroutine {
    pub environment: Option<TableRef>,
    pub stack_trace: String,
    live_upvalues: Vec<UpValueRef>,
    pub object_stack: Vec<Object>,
    top: i32,
    call_frame_stack: Vec<LuaCallFrame>,
    call_frame_top: i32,
    parent: Option<CoroutineRef>,
    /// `thread != null`: set by the constructor that takes a thread and by `resume`, cleared by `destroy`.
    has_thread: bool,
}

impl Coroutine {
    /// se.krka.kahlua.vm.Coroutine.<init>(Platform, KahluaTable[, KahluaThread])
    pub fn new(environment: Option<TableRef>, has_thread: bool) -> CoroutineRef {
        Rc::new(RefCell::new(Coroutine {
            environment,
            stack_trace: String::new(),
            live_upvalues: Vec::new(),
            object_stack: vec![Object::Nil; INITIAL_STACK_SIZE],
            top: 0,
            call_frame_stack: vec![LuaCallFrame::default(); INITIAL_CALL_FRAME_STACK_SIZE],
            call_frame_top: 0,
            parent: None,
            has_thread,
        }))
    }

    pub fn stack_get(&self, i: i32) -> Object {
        usize::try_from(i)
            .ok()
            .and_then(|i| self.object_stack.get(i))
            .cloned()
            .unwrap_or_default()
    }

    pub fn stack_set(&mut self, i: i32, o: Object) {
        if let Some(slot) = usize::try_from(i)
            .ok()
            .and_then(|i| self.object_stack.get_mut(i))
        {
            *slot = o;
        }
    }

    /// se.krka.kahlua.vm.Coroutine.getParent
    pub fn get_parent(&self) -> Option<CoroutineRef> {
        self.parent.clone()
    }

    pub fn has_thread(&self) -> bool {
        self.has_thread
    }

    /// se.krka.kahlua.vm.Coroutine.pushNewCallFrame: the slot of the frame.
    #[allow(clippy::too_many_arguments)]
    pub fn push_new_call_frame(
        &mut self,
        closure: Option<Rc<LuaClosure>>,
        java_function: Option<Rc<dyn JavaFunction>>,
        local_base: i32,
        return_base: i32,
        n_arguments: i32,
        from_lua: bool,
        inside_coroutine: bool,
    ) -> R<usize> {
        self.set_call_frame_stack_top(self.call_frame_top + 1)?;
        let idx = self
            .current_call_frame()
            .ok_or_else(|| Throwable::runtime("Stack underflow"))?;
        // LuaCallFrame.setup (Core.debug off).
        let f = self.frame_mut(idx);
        f.local_base = local_base;
        f.return_base = return_base;
        f.n_arguments = n_arguments;
        f.from_lua = from_lua;
        f.can_yield = inside_coroutine;
        f.closure = closure;
        f.java_function = java_function;
        Ok(idx)
    }

    /// se.krka.kahlua.vm.Coroutine.popCallFrame
    pub fn pop_call_frame(&mut self) -> R<()> {
        if self.is_dead() {
            return Err(Throwable::runtime("Stack underflow"));
        }
        self.set_call_frame_stack_top(self.call_frame_top - 1)
    }

    /// se.krka.kahlua.vm.Coroutine.ensureCallFrameStackSize
    fn ensure_call_frame_stack_size(&mut self, index: i32) -> R<()> {
        if index > MAX_CALL_FRAME_STACK_SIZE {
            return Err(Throwable::runtime("Stack overflow"));
        }
        let old_size = self.call_frame_stack.len();
        let mut new_size = old_size;
        while new_size <= index as usize {
            new_size *= 2;
        }
        if new_size > old_size {
            self.call_frame_stack
                .resize(new_size, LuaCallFrame::default());
        }
        Ok(())
    }

    /// se.krka.kahlua.vm.Coroutine.setCallFrameStackTop
    pub fn set_call_frame_stack_top(&mut self, new_top: i32) -> R<()> {
        if new_top > self.call_frame_top {
            self.ensure_call_frame_stack_size(new_top)?;
        } else {
            self.call_frame_stack_clear(new_top, self.call_frame_top - 1);
        }
        self.call_frame_top = new_top;
        Ok(())
    }

    /// se.krka.kahlua.vm.Coroutine.callFrameStackClear
    fn call_frame_stack_clear(&mut self, start: i32, end: i32) {
        for i in start.max(0)..=end {
            if let Some(f) = self.call_frame_stack.get_mut(i as usize) {
                f.closure = None;
                f.java_function = None;
            }
        }
    }

    /// se.krka.kahlua.vm.Coroutine.ensureStacksize
    fn ensure_stacksize(&mut self, index: i32) -> R<()> {
        if index > MAX_STACK_SIZE {
            return Err(Throwable::runtime("Stack overflow"));
        }
        let old_size = self.object_stack.len();
        let mut new_size = old_size;
        while new_size <= index as usize {
            new_size *= 2;
        }
        if new_size > old_size {
            self.object_stack.resize(new_size, Object::Nil);
        }
        Ok(())
    }

    /// se.krka.kahlua.vm.Coroutine.setTop
    pub fn set_top(&mut self, new_top: i32) -> R<()> {
        if self.top < new_top {
            self.ensure_stacksize(new_top)?;
        } else {
            self.stack_clear(new_top, self.top - 1);
        }
        self.top = new_top;
        Ok(())
    }

    /// se.krka.kahlua.vm.Coroutine.stackCopy (the `Core.debug` local-name bookkeeping is off in the game).
    pub fn stack_copy(&mut self, start: i32, dest: i32, len: i32) {
        if len > 0 && start != dest {
            let tmp: Vec<Object> = (0..len).map(|i| self.stack_get(start + i)).collect();
            for (i, o) in tmp.into_iter().enumerate() {
                self.stack_set(dest + i as i32, o);
            }
        }
    }

    /// se.krka.kahlua.vm.Coroutine.stackClear
    pub fn stack_clear(&mut self, start: i32, end: i32) {
        for i in start..=end {
            self.stack_set(i, Object::Nil);
        }
    }

    /// se.krka.kahlua.vm.Coroutine.closeUpvalues
    pub fn close_upvalues(&mut self, close_index: i32) {
        while let Some(uv) = self.live_upvalues.last().cloned() {
            let index = uv.borrow().index;
            if index < close_index {
                return;
            }
            let v = self.stack_get(index);
            {
                let mut u = uv.borrow_mut();
                u.value = v;
                u.coroutine = None;
            }
            self.live_upvalues.pop();
        }
    }

    /// se.krka.kahlua.vm.Coroutine.findUpvalue (`me` is this coroutine's own reference).
    pub fn find_upvalue(&mut self, scan_index: i32, me: &CoroutineRef) -> UpValueRef {
        let mut loop_index = self.live_upvalues.len() as i32;
        loop {
            loop_index -= 1;
            if loop_index < 0 {
                break;
            }
            let uv = &self.live_upvalues[loop_index as usize];
            let index = uv.borrow().index;
            if index == scan_index {
                return uv.clone();
            }
            if index < scan_index {
                break;
            }
        }
        let uv = Rc::new(RefCell::new(UpValue {
            coroutine: Some(me.clone()),
            index: scan_index,
            value: Object::Nil,
        }));
        self.live_upvalues
            .insert((loop_index + 1) as usize, uv.clone());
        uv
    }

    /// se.krka.kahlua.vm.Coroutine.currentCallFrame: the slot (the pooled frame objects all exist here; the
    /// original creates each on first use, with the same initial fields).
    pub fn current_call_frame(&self) -> Option<usize> {
        if self.is_dead() {
            None
        } else {
            Some((self.call_frame_top - 1) as usize)
        }
    }

    pub fn frame(&self, idx: usize) -> &LuaCallFrame {
        &self.call_frame_stack[idx]
    }

    pub fn frame_mut(&mut self, idx: usize) -> &mut LuaCallFrame {
        &mut self.call_frame_stack[idx]
    }

    /// se.krka.kahlua.vm.Coroutine.getTop
    pub fn get_top(&self) -> i32 {
        self.top
    }

    /// se.krka.kahlua.vm.Coroutine.getParent(int): the slot.
    pub fn get_parent_frame(&self, level: i32) -> R<usize> {
        lua_assert(level >= 0, "Level must be non-negative")?;
        let index = self.call_frame_top - level - 1;
        lua_assert(index >= 0, "Level too high")?;
        Ok(index as usize)
    }

    /// se.krka.kahlua.vm.Coroutine.getCurrentStackTrace
    pub fn get_current_stack_trace(&self, level: i32, count: i32, halt_at: i32) -> String {
        let level = level.max(0);
        let mut count = count.max(0);
        let mut buffer = String::new();
        let mut i = self.call_frame_top - 1 - level;
        while i >= halt_at && {
            let c = count;
            count -= 1;
            c > 0
        } {
            buffer.push_str(&self.get_stack_trace(i as usize));
            i -= 1;
        }
        buffer
    }

    /// se.krka.kahlua.vm.Coroutine.cleanCallFrames (frames are the same object when they are the same slot).
    pub fn clean_call_frames(&mut self, caller_frame: Option<usize>) -> R<()> {
        loop {
            let frame = self.current_call_frame();
            if frame.is_none() || frame == caller_frame {
                return Ok(());
            }
            if let Some(f) = frame {
                self.add_stack_trace(f);
            }
            self.pop_call_frame()?;
        }
    }

    /// se.krka.kahlua.vm.Coroutine.addStackTrace
    pub fn add_stack_trace(&mut self, frame: usize) {
        let s = self.get_stack_trace(frame);
        self.stack_trace.push_str(&s);
    }

    /// se.krka.kahlua.vm.Coroutine.getStackTrace
    fn get_stack_trace(&self, frame: usize) -> String {
        let f = self.frame(frame);
        match &f.closure {
            Some(c) => {
                let lines = &c.prototype.lines;
                let pc = f.pc - 1;
                if pc >= 0 && (pc as usize) < lines.len() {
                    format!(
                        "at {}:{}\n",
                        c.prototype.name.as_deref().unwrap_or("null"),
                        lines[pc as usize]
                    )
                } else {
                    String::new()
                }
            }
            None => format!(
                "at {}\n",
                f.java_function
                    .as_ref()
                    .map_or("null".into(), |j| j.java_to_string())
            ),
        }
    }

    /// se.krka.kahlua.vm.Coroutine.isDead
    pub fn is_dead(&self) -> bool {
        self.call_frame_top == 0
    }

    /// se.krka.kahlua.vm.Coroutine.getStatus
    pub fn get_status(&self) -> &'static str {
        if self.parent.is_none() {
            if self.is_dead() {
                "dead"
            } else {
                "suspended"
            }
        } else {
            "normal"
        }
    }

    /// se.krka.kahlua.vm.Coroutine.atBottom
    pub fn at_bottom(&self) -> bool {
        self.call_frame_top == 1
    }

    /// se.krka.kahlua.vm.Coroutine.getCallframeTop
    pub fn get_callframe_top(&self) -> i32 {
        self.call_frame_top
    }

    /// se.krka.kahlua.vm.Coroutine.getCallFrame: the slot (a negative index counts from the top).
    pub fn get_call_frame(&self, index: i32) -> usize {
        let index = if index < 0 {
            index + self.call_frame_top
        } else {
            index
        };
        index.max(0) as usize
    }

    /// se.krka.kahlua.vm.Coroutine.resume
    pub fn resume(&mut self, parent: CoroutineRef) {
        self.parent = Some(parent);
        self.has_thread = true;
    }

    /// se.krka.kahlua.vm.Coroutine.destroy
    pub fn destroy(&mut self) {
        self.parent = None;
        self.has_thread = false;
    }
}

/// A `LuaCallFrame` object: a frame slot of a coroutine.
#[derive(Clone)]
pub struct FrameRef {
    pub coroutine: CoroutineRef,
    pub idx: usize,
}

impl PartialEq for FrameRef {
    fn eq(&self, o: &FrameRef) -> bool {
        Rc::ptr_eq(&self.coroutine, &o.coroutine) && self.idx == o.idx
    }
}

impl FrameRef {
    pub fn with<T>(&self, f: impl FnOnce(&LuaCallFrame) -> T) -> T {
        f(self.coroutine.borrow().frame(self.idx))
    }

    pub fn with_mut<T>(&self, f: impl FnOnce(&mut LuaCallFrame) -> T) -> T {
        f(self.coroutine.borrow_mut().frame_mut(self.idx))
    }

    pub fn local_base(&self) -> i32 {
        self.with(|f| f.local_base)
    }

    pub fn closure(&self) -> Option<Rc<LuaClosure>> {
        self.with(|f| f.closure.clone())
    }

    pub fn n_arguments(&self) -> i32 {
        self.with(|f| f.n_arguments)
    }

    /// se.krka.kahlua.vm.LuaCallFrame.get
    pub fn get(&self, index: i32) -> Object {
        let c = self.coroutine.borrow();
        let lb = c.frame(self.idx).local_base;
        c.stack_get(lb + index)
    }

    /// se.krka.kahlua.vm.LuaCallFrame.set
    pub fn set(&self, index: i32, o: Object) {
        let mut c = self.coroutine.borrow_mut();
        let lb = c.frame(self.idx).local_base;
        c.stack_set(lb + index, o);
    }

    /// se.krka.kahlua.vm.LuaCallFrame.push(Object)
    pub fn push(&self, x: Object) -> R<i32> {
        let top = self.get_top();
        self.set_top(top + 1)?;
        self.set(top, x);
        Ok(1)
    }

    /// se.krka.kahlua.vm.LuaCallFrame.push(Object, Object)
    pub fn push2(&self, x: Object, y: Object) -> R<i32> {
        let top = self.get_top();
        self.set_top(top + 2)?;
        self.set(top, x);
        self.set(top + 1, y);
        Ok(2)
    }

    /// se.krka.kahlua.vm.LuaCallFrame.pushNil
    pub fn push_nil(&self) -> R<i32> {
        self.push(Object::Nil)
    }

    /// se.krka.kahlua.vm.LuaCallFrame.stackCopy
    pub fn stack_copy(&self, start: i32, dest: i32, len: i32) {
        let mut c = self.coroutine.borrow_mut();
        let lb = c.frame(self.idx).local_base;
        c.stack_copy(lb + start, lb + dest, len);
    }

    /// se.krka.kahlua.vm.LuaCallFrame.stackClear
    pub fn stack_clear(&self, start: i32, end: i32) {
        let mut c = self.coroutine.borrow_mut();
        let lb = c.frame(self.idx).local_base;
        for i in start..=end {
            c.stack_set(lb + i, Object::Nil);
        }
    }

    /// se.krka.kahlua.vm.LuaCallFrame.clearFromIndex
    pub fn clear_from_index(&self, index: i32) -> R<()> {
        if self.get_top() < index {
            self.set_top(index)?;
        }
        self.stack_clear(index, self.get_top() - 1);
        Ok(())
    }

    /// se.krka.kahlua.vm.LuaCallFrame.setTop
    pub fn set_top(&self, index: i32) -> R<()> {
        let mut c = self.coroutine.borrow_mut();
        let lb = c.frame(self.idx).local_base;
        c.set_top(lb + index)
    }

    /// se.krka.kahlua.vm.LuaCallFrame.closeUpvalues
    pub fn close_upvalues(&self, a: i32) {
        let mut c = self.coroutine.borrow_mut();
        let lb = c.frame(self.idx).local_base;
        c.close_upvalues(lb + a);
    }

    /// se.krka.kahlua.vm.LuaCallFrame.findUpvalue
    pub fn find_upvalue(&self, b: i32) -> UpValueRef {
        let me = self.coroutine.clone();
        let mut c = self.coroutine.borrow_mut();
        let lb = c.frame(self.idx).local_base;
        c.find_upvalue(lb + b, &me)
    }

    /// se.krka.kahlua.vm.LuaCallFrame.getTop
    pub fn get_top(&self) -> i32 {
        let c = self.coroutine.borrow();
        c.get_top() - c.frame(self.idx).local_base
    }

    /// se.krka.kahlua.vm.LuaCallFrame.init
    pub fn init(&self) -> R<()> {
        let Some(closure) = self.closure() else {
            return Ok(());
        };
        let p = &closure.prototype;
        self.with_mut(|f| f.pc = 0);
        let n_arguments = self.n_arguments();
        if p.is_vararg {
            self.with_mut(|f| f.local_base += n_arguments);
            self.set_top(p.max_stacksize)?;
            let to_copy = n_arguments.min(p.num_params);
            self.stack_copy(-n_arguments, 0, to_copy);
        } else {
            self.set_top(p.max_stacksize)?;
            self.stack_clear(p.num_params, n_arguments);
        }
        Ok(())
    }

    /// se.krka.kahlua.vm.LuaCallFrame.setPrototypeStacksize
    pub fn set_prototype_stacksize(&self) -> R<()> {
        if let Some(c) = self.closure() {
            self.set_top(c.prototype.max_stacksize)?;
        }
        Ok(())
    }

    /// se.krka.kahlua.vm.LuaCallFrame.pushVarargs
    pub fn push_varargs(&self, index: i32, n: i32) -> R<()> {
        let n_params = self.closure().map_or(0, |c| c.prototype.num_params);
        let mut n_varargs = (self.n_arguments() - n_params).max(0);
        let mut n = n;
        if n == -1 {
            n = n_varargs;
            self.set_top(index + n_varargs)?;
        }
        if n_varargs > n {
            n_varargs = n;
        }
        self.stack_copy(-self.n_arguments() + n_params, index, n_varargs);
        let num_nils = n - n_varargs;
        if num_nils > 0 {
            self.stack_clear(index + n_varargs, index + n - 1);
        }
        Ok(())
    }

    /// se.krka.kahlua.vm.LuaCallFrame.getEnvironment
    pub fn get_environment(&self) -> Option<TableRef> {
        match self.closure() {
            Some(c) => Some(c.env.borrow().clone()),
            None => self.coroutine.borrow().environment.clone(),
        }
    }

    /// se.krka.kahlua.vm.LuaCallFrame.isLua
    pub fn is_lua(&self) -> bool {
        self.with(|f| f.is_lua())
    }
}
