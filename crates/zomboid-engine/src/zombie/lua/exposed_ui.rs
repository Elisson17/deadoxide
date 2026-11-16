//! The exposed zombie.ui.UIElement and zombie.ui.UIManager: the Java side of the Lua UI (ISUIElement's
//! `javaObject`), over `zombie::ui::ui_element` and `zombie::ui::ui_manager`. Drawing (Draw*, stencils, outlines)
//! and input only run from UIManager.render/update, which no headless state runs: not ported.
use std::cell::RefCell;
use std::rc::Rc;

use super::exposed_client::static_args;
use super::exposed_methods::{
    npe, ret, ret_bool, ret_int, ret_str, string_arg, this, type_error, ud, want_args,
};
use super::exposer::{class_named, Fail, Imp, JavaStatics, Ret};
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
use crate::se::krka::kahlua::vm::object::{JavaClass, Object, TableRef, Throwable, Userdata};
use crate::zombie::ui::ui_element::{self as ue, ui_element, UIElement, UIElementObject};
use crate::zombie::ui::ui_transition::UITransition;

/// zombie.ui.UITransition as a Lua value.
#[derive(Default)]
pub struct UITransitionObject(pub RefCell<UITransition>);

impl Userdata for UITransitionObject {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.ui.UITransition")
    }
    fn java_to_string(&self) -> String {
        format!(
            "zombie.ui.UITransition@{:x}",
            super::exposed_methods::hash(self)
        )
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Runs `f` on a UITransition receiver.
fn with_fade<T>(cf: &FrameRef, f: impl FnOnce(&mut UITransition) -> T) -> Result<T, Fail> {
    let u = this::<UITransitionObject>(cf)?;
    let t = u
        .as_any()
        .downcast_ref::<UITransitionObject>()
        .ok_or_else(npe)?;
    let r = f(&mut t.0.borrow_mut());
    Ok(r)
}

/// UITransition's instance methods.
fn transition_method(name: &str) -> Option<Imp> {
    Some(match name {
        "init" => |_, _, cf, n| {
            want_args(n, true, 2)?;
            let d = super::exposed_methods::float_arg(cf, 1)?;
            let out = boolean_arg(cf, 2)?;
            with_fade(cf, |t| t.init(d, out))?;
            Ok(0)
        },
        "update" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_fade(cf, UITransition::update)?;
            Ok(0)
        },
        "fraction" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with_fade(cf, |t| t.fraction())?;
            ret(cf, Object::Number(f64::from(v)))
        },
        "setFadeIn" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with_fade(cf, |t| t.set_fade_in(v))?;
            Ok(0)
        },
        "reset" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_fade(cf, |t| t.elapsed = 0.0)?;
            Ok(0)
        },
        "setIgnoreUpdateTime" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with_fade(cf, |t| t.ignore_update_time = v)?;
            Ok(0)
        },
        "getElapsed" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with_fade(cf, |t| t.elapsed)?;
            ret(cf, Object::Number(f64::from(v)))
        },
        "setElapsed" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = super::exposed_methods::float_arg(cf, 1)?;
            with_fade(cf, |t| t.elapsed = v)?;
            Ok(0)
        },
        _ => return None,
    })
}

impl Userdata for UIElementObject {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.ui.UIElement")
    }
    /// UIElement.toString is `tostring(table)` when it has one; this prints Object.toString (UNKNOWN whether a
    /// script prints it; the dumps do not).
    fn java_to_string(&self) -> String {
        format!(
            "zombie.ui.UIElement@{:x}",
            super::exposed_methods::hash(self)
        )
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// A double parameter: a Lua number.
fn double_arg(cf: &FrameRef, i: i32) -> Result<f64, Fail> {
    match cf.get(i) {
        Object::Number(d) => Ok(d),
        o => Err(type_error("double", &o)),
    }
}

/// A boolean parameter: a Lua boolean.
fn boolean_arg(cf: &FrameRef, i: i32) -> Result<bool, Fail> {
    match cf.get(i) {
        Object::Boolean(b) => Ok(b),
        o => Err(type_error("boolean", &o)),
    }
}

/// A UIElement (or UIElementInterface) parameter: a UIElement value, or null for nil.
fn element_arg(cf: &FrameRef, i: i32) -> Result<Option<Rc<dyn Userdata>>, Fail> {
    match cf.get(i) {
        Object::Nil => Ok(None),
        Object::Userdata(u) if ui_element(&u).is_some() => Ok(Some(u)),
        o => Err(type_error("UIElement", &o)),
    }
}

/// The receiver, a UIElement.
fn me(cf: &FrameRef) -> Result<Rc<dyn Userdata>, Fail> {
    this::<UIElementObject>(cf)
}

/// Runs `f` on the receiver's fields.
fn with<T>(cf: &FrameRef, f: impl FnOnce(&mut UIElement) -> T) -> Result<T, Fail> {
    let u = me(cf)?;
    let e = ui_element(&u).ok_or_else(npe)?;
    let r = f(&mut e.0.borrow_mut());
    Ok(r)
}

fn ret_double(cf: &FrameRef, v: f64) -> Ret {
    ret(cf, Object::Number(v))
}

/// UIManager.tableget(table, key): `table.rawget(key)` (null for a null table).
fn tableget(t: &Option<TableRef>, key: &str) -> Result<Object, Fail> {
    match t {
        Some(t) => KahluaTableImpl::rawget(t, &Object::str(key)).map_err(Fail::Lua),
        None => Ok(Object::Nil),
    }
}

/// LuaManager.caller.protectedCallVoid / pcallvoid of a table's function: errors are kept by the thread's pcall and
/// do not reach the caller.
fn call_void(thread: &mut KahluaThread, f: &Object, args: &[Object]) -> Result<(), Fail> {
    thread.pcall_args(f, args).map_err(Fail::Lua)?;
    Ok(())
}

/// UIElement.invokeOnScrollPosChanged: the table's `onScrollPosChanged(table, xScroll, yScroll)`.
fn invoke_on_scroll_pos_changed(
    thread: &mut KahluaThread,
    u: &Rc<dyn Userdata>,
) -> Result<(), Fail> {
    let e = ui_element(u).ok_or_else(npe)?;
    let (table, xs, ys) = {
        let e = e.0.borrow();
        (e.table.clone(), e.x_scroll, e.y_scroll)
    };
    let f = tableget(&table, "onScrollPosChanged")?;
    if !f.is_nil() {
        let t = table.map_or(Object::Nil, Object::Table);
        call_void(thread, &f, &[t, Object::Number(xs), Object::Number(ys)])?;
    }
    Ok(())
}

/// zombie.ui.UIElement.onResize: the anchored resize against a dirty parent, the table's `onResize(table, w, h)`,
/// the children (last first), then the size is clean.
pub fn on_resize(thread: &mut KahluaThread, u: &Rc<dyn Userdata>) -> Result<(), Fail> {
    let e = ui_element(u).ok_or_else(npe)?;
    let parent = e.0.borrow().parent();
    if let Some(p) = parent.as_ref().and_then(ui_element) {
        let (dirty, pw, plw, ph, plh) = {
            let p = p.0.borrow();
            (
                p.resize_dirty,
                f64::from(p.width),
                p.lastwidth,
                f64::from(p.height),
                p.lastheight,
            )
        };
        if dirty {
            let difx = pw - plw;
            let dify = ph - plh;
            let mut m = e.0.borrow_mut();
            if !m.anchor_top && m.anchor_bottom {
                let y = m.y + dify;
                m.set_y(y);
            }
            if m.anchor_top && m.anchor_bottom {
                let h = f64::from(m.height) + dify;
                m.set_height(h);
            }
            if !m.anchor_left && m.anchor_right {
                let x = m.x + difx;
                m.set_x(x);
            }
            if m.anchor_left && m.anchor_right {
                let w = f64::from(m.width) + difx;
                m.set_width(w);
            }
        }
    }
    let (table, w, h) = {
        let m = e.0.borrow();
        (m.table.clone(), f64::from(m.width), f64::from(m.height))
    };
    let f = tableget(&table, "onResize")?;
    if !f.is_nil() {
        let t = table.map_or(Object::Nil, Object::Table);
        call_void(thread, &f, &[t, Object::Number(w), Object::Number(h)])?;
    }
    let n = e.0.borrow().controls.len();
    for i in (0..n).rev() {
        let child = e.0.borrow().controls.get(i).cloned();
        if let Some(c) = child {
            on_resize(thread, &c)?;
        }
    }
    let mut m = e.0.borrow_mut();
    m.resize_dirty = false;
    m.lastwidth = f64::from(m.width);
    m.lastheight = f64::from(m.height);
    Ok(())
}

/// The UIManager lists of this Lua state.
pub fn ui_manager(s: &Rc<JavaStatics>) -> &RefCell<crate::zombie::ui::ui_manager::UIManager> {
    &s.state.ui_manager
}

/// UIElement's and UIManager's static methods this module owns.
pub fn static_method(class: &str, name: &str) -> Option<Imp> {
    Some(match (class, name) {
        // UIElement() and UIElement(KahluaTable) (a MultiLuaJavaInvoker: the table one for one argument).
        ("zombie.ui.UIElement", "new") => |_, _, cf, n| {
            let table: Option<TableRef> = match (n, cf.get(0)) {
                (0, _) => None,
                (1, Object::Nil) => None,
                (1, Object::Table(t)) => Some(t),
                _ => {
                    return Err(Fail::Lua(Throwable::runtime(
                        "not ported: UIElement.new with these arguments",
                    )))
                }
            };
            ret(
                cf,
                ud(Rc::new(UIElementObject(RefCell::new(UIElement::new(
                    table,
                ))))),
            )
        },
        ("zombie.ui.UITransition", "new") => |_, _, cf, n| {
            static_args(n, 0)?;
            ret(cf, ud(Rc::new(UITransitionObject::default())))
        },
        ("zombie.GameWindow", "doRenderEvent") => |s, _, cf, n| {
            static_args(n, 1)?;
            let b = boolean_arg(cf, 0)?;
            s.state.do_render_event.set(b);
            Ok(0)
        },
        ("zombie.ui.UIManager", "AddUI") => |s, _, cf, n| {
            static_args(n, 1)?;
            let el = element_arg(cf, 0)?.ok_or_else(npe)?;
            ui_manager(s).borrow_mut().add_ui(&el);
            Ok(0)
        },
        ("zombie.ui.UIManager", "RemoveElement") => |s, _, cf, n| {
            static_args(n, 1)?;
            let el = element_arg(cf, 0)?.ok_or_else(npe)?;
            ui_manager(s).borrow_mut().remove_element(&el);
            Ok(0)
        },
        ("zombie.ui.UIManager", "clearArrays") => |s, _, _, n| {
            static_args(n, 0)?;
            ui_manager(s).borrow_mut().clear_arrays();
            Ok(0)
        },
        _ => return None,
    })
}

/// UIElement's instance methods this module owns.
pub fn instance_method(class: &str, name: &str) -> Option<Imp> {
    if class == "zombie.ui.UITransition" {
        return transition_method(name);
    }
    if class != "zombie.ui.UIElement" {
        return None;
    }
    Some(match name {
        // ---- children ----
        "AddChild" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let el = element_arg(cf, 1)?;
            let u = me(cf)?;
            match el {
                Some(el) => ue::add_child(&u, &el).map_err(|_| npe())?,
                // controls.add(null), then null.setParent(this)
                None => {
                    return Err(npe());
                }
            }
            Ok(0)
        },
        "RemoveChild" | "RemoveControl" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let el = element_arg(cf, 1)?.ok_or_else(npe)?;
            let u = me(cf)?;
            ue::remove_child(ui_element(&u).ok_or_else(npe)?, &el).map_err(|_| npe())?;
            Ok(0)
        },
        "ClearChildren" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with(cf, |e| e.controls.clear())?;
            Ok(0)
        },
        "BringToTop" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let el = element_arg(cf, 1)?;
            let u = me(cf)?;
            let e = ui_element(&u).ok_or_else(npe)?;
            let mut m = e.0.borrow_mut();
            if let Some(el) = &el {
                ue::list_remove(&mut m.controls, el);
                m.controls.push(el.clone());
            }
            Ok(0)
        },
        // UIManager.pushToTop(this), and the parent's toTop list
        "bringToTop" => |s, _, cf, n| {
            want_args(n, true, 0)?;
            let u = me(cf)?;
            ui_manager(s).borrow_mut().push_to_top(&u);
            let parent = ui_element(&u).ok_or_else(npe)?.0.borrow().parent();
            if let Some(p) = parent.as_ref().and_then(ui_element) {
                p.0.borrow_mut().to_top.push(u.clone());
            }
            Ok(0)
        },
        "getParent" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let p = with(cf, |e| e.parent())?;
            ret(cf, p.map_or(Object::Nil, Object::Userdata))
        },
        "setParent" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let p = element_arg(cf, 1)?;
            with(cf, |e| e.parent = p.as_ref().map(Rc::downgrade))?;
            Ok(0)
        },
        "getTable" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let t = with(cf, |e| e.table.clone())?;
            ret(cf, t.map_or(Object::Nil, Object::Table))
        },
        "setTable" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let t = match cf.get(1) {
                Object::Nil => None,
                Object::Table(t) => Some(t),
                o => return Err(type_error("KahluaTable", &o)),
            };
            with(cf, |e| e.table = t)?;
            Ok(0)
        },
        // ---- position and size ----
        "getX" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.x)?;
            ret_double(cf, v)
        },
        "getY" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.y)?;
            ret_double(cf, v)
        },
        "getWidth" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| f64::from(e.width))?;
            ret_double(cf, v)
        },
        "getHeight" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| f64::from(e.height))?;
            ret_double(cf, v)
        },
        "setX" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = double_arg(cf, 1)?;
            with(cf, |e| e.set_x(v))?;
            Ok(0)
        },
        "setY" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = double_arg(cf, 1)?;
            with(cf, |e| e.set_y(v))?;
            Ok(0)
        },
        "setWidth" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = double_arg(cf, 1)?;
            with(cf, |e| e.set_width(v))?;
            Ok(0)
        },
        "setHeight" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = double_arg(cf, 1)?;
            with(cf, |e| e.set_height(v))?;
            Ok(0)
        },
        "setWidthSilent" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = double_arg(cf, 1)?;
            with(cf, |e| e.set_width_silent(v))?;
            Ok(0)
        },
        "setHeightSilent" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = double_arg(cf, 1)?;
            with(cf, |e| e.set_height_silent(v))?;
            Ok(0)
        },
        "setWidthOnly" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = double_arg(cf, 1)?;
            with(cf, |e| e.set_width_only(v))?;
            Ok(0)
        },
        "setHeightOnly" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = double_arg(cf, 1)?;
            with(cf, |e| e.set_height_only(v))?;
            Ok(0)
        },
        "ignoreWidthChange" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with(cf, |e| e.ignore_width_change())?;
            Ok(0)
        },
        "ignoreHeightChange" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with(cf, |e| e.ignore_height_change())?;
            Ok(0)
        },
        "getAbsoluteX" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let u = me(cf)?;
            ret_double(cf, ue::get_absolute_x(ui_element(&u).ok_or_else(npe)?))
        },
        "getAbsoluteY" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let u = me(cf)?;
            ret_double(cf, ue::get_absolute_y(ui_element(&u).ok_or_else(npe)?))
        },
        "clampToParentX" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = double_arg(cf, 1)?;
            let u = me(cf)?;
            ret_double(
                cf,
                ue::clamp_to_parent_x(ui_element(&u).ok_or_else(npe)?, v),
            )
        },
        "clampToParentY" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = double_arg(cf, 1)?;
            let u = me(cf)?;
            ret_double(
                cf,
                ue::clamp_to_parent_y(ui_element(&u).ok_or_else(npe)?, v),
            )
        },
        "onResize" => |_, thread, cf, n| {
            want_args(n, true, 0)?;
            let u = me(cf)?;
            on_resize(thread, &u)?;
            Ok(0)
        },
        "onresize" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            me(cf)?;
            Ok(0)
        },
        // ---- anchors ----
        "setAnchorTop" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with(cf, |e| e.set_anchor_top(v))?;
            Ok(0)
        },
        "setAnchorLeft" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with(cf, |e| e.set_anchor_left(v))?;
            Ok(0)
        },
        "setAnchorRight" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with(cf, |e| e.set_anchor_right(v))?;
            Ok(0)
        },
        "setAnchorBottom" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with(cf, |e| e.set_anchor_bottom(v))?;
            Ok(0)
        },
        "isAnchorTop" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.anchor_top)?;
            ret_bool(cf, v)
        },
        "isAnchorLeft" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.anchor_left)?;
            ret_bool(cf, v)
        },
        "isAnchorRight" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.anchor_right)?;
            ret_bool(cf, v)
        },
        "isAnchorBottom" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.anchor_bottom)?;
            ret_bool(cf, v)
        },
        // ---- scrolling ----
        "getXScroll" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.x_scroll)?;
            ret_double(cf, v)
        },
        "getYScroll" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.y_scroll)?;
            ret_double(cf, v)
        },
        // a change calls the table's onScrollPosChanged
        "setXScroll" => |_, thread, cf, n| {
            want_args(n, true, 1)?;
            let v = double_arg(cf, 1)?;
            let changed = with(cf, |e| {
                let c = e.x_scroll != v;
                if c {
                    e.x_scroll = v;
                }
                c
            })?;
            if changed {
                invoke_on_scroll_pos_changed(thread, &me(cf)?)?;
            }
            Ok(0)
        },
        "setYScroll" => |_, thread, cf, n| {
            want_args(n, true, 1)?;
            let v = double_arg(cf, 1)?;
            let changed = with(cf, |e| {
                let c = e.y_scroll != v;
                if c {
                    e.y_scroll = v;
                }
                c
            })?;
            if changed {
                invoke_on_scroll_pos_changed(thread, &me(cf)?)?;
            }
            Ok(0)
        },
        "getScrollHeight" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| f64::from(e.scroll_height))?;
            ret_double(cf, v)
        },
        "setScrollHeight" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = double_arg(cf, 1)?;
            with(cf, |e| e.set_scroll_height(v))?;
            Ok(0)
        },
        "getScrollChildren" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.scroll_children)?;
            ret_bool(cf, v)
        },
        "setScrollChildren" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with(cf, |e| e.scroll_children = v)?;
            Ok(0)
        },
        "getScrollWithParent" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.scroll_with_parent)?;
            ret_bool(cf, v)
        },
        "setScrollWithParent" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with(cf, |e| e.scroll_with_parent = v)?;
            Ok(0)
        },
        "getMaxDrawHeight" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| f64::from(e.max_draw_height))?;
            ret_double(cf, v)
        },
        "setMaxDrawHeight" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = double_arg(cf, 1)?;
            with(cf, |e| e.set_max_draw_height(v))?;
            Ok(0)
        },
        "clearMaxDrawHeight" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with(cf, |e| e.max_draw_height = -1)?;
            Ok(0)
        },
        "setRenderClippedChildren" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with(cf, |e| e.render_clipped_children = v)?;
            Ok(0)
        },
        // ---- flags ----
        "isVisible" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.visible)?;
            ret_bool(cf, v)
        },
        "setVisible" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with(cf, |e| e.visible = v)?;
            Ok(0)
        },
        "isReallyVisible" => |s, _, cf, n| {
            want_args(n, true, 0)?;
            let u = me(cf)?;
            let ui = ui_manager(s).borrow().ui.clone();
            ret_bool(cf, ue::is_really_visible(&u, &ui))
        },
        "isEnabled" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.enabled)?;
            ret_bool(cf, v)
        },
        "setEnabled" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with(cf, |e| e.enabled = v)?;
            Ok(0)
        },
        "isCapture" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.capture)?;
            ret_bool(cf, v)
        },
        "setCapture" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with(cf, |e| e.capture = v)?;
            Ok(0)
        },
        "isAlwaysOnTop" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.always_on_top)?;
            ret_bool(cf, v)
        },
        "setAlwaysOnTop" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with(cf, |e| e.always_on_top = v)?;
            Ok(0)
        },
        "backMost" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with(cf, |e| e.always_back = true)?;
            Ok(0)
        },
        "isBackMost" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.always_back)?;
            ret_bool(cf, v)
        },
        "isFollowGameWorld" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.follow_game_world)?;
            ret_bool(cf, v)
        },
        "setFollowGameWorld" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with(cf, |e| e.follow_game_world = v)?;
            Ok(0)
        },
        "isDefaultDraw" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.default_draw)?;
            ret_bool(cf, v)
        },
        "setDefaultDraw" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with(cf, |e| e.default_draw = v)?;
            Ok(0)
        },
        "isIgnoreLossControl" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.ignore_loss_control)?;
            ret_bool(cf, v)
        },
        "setIgnoreLossControl" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with(cf, |e| e.ignore_loss_control = v)?;
            Ok(0)
        },
        "getRenderThisPlayerOnly" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.render_this_player_only)?;
            ret_int(cf, v)
        },
        "setRenderThisPlayerOnly" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = super::exposed_methods::int_arg(cf, 1)?;
            with(cf, |e| e.render_this_player_only = v)?;
            Ok(0)
        },
        "getPlayerContext" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.player_context)?;
            ret_int(cf, v)
        },
        "setPlayerContext" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = super::exposed_methods::int_arg(cf, 1)?;
            with(cf, |e| e.player_context = v)?;
            Ok(0)
        },
        "isConsumeMouseEvents" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.consume_mouse_events)?;
            ret_bool(cf, v)
        },
        "setConsumeMouseEvents" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with(cf, |e| e.consume_mouse_events = v)?;
            Ok(0)
        },
        "isWantKeyEvents" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.want_key_events)?;
            ret_bool(cf, v)
        },
        "setWantKeyEvents" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with(cf, |e| e.want_key_events = v)?;
            Ok(0)
        },
        "isWantExtraMouseEvents" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.want_extra_mouse_events)?;
            ret_bool(cf, v)
        },
        "setWantExtraMouseEvents" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with(cf, |e| e.want_extra_mouse_events = v)?;
            Ok(0)
        },
        "isForceCursorVisible" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.force_cursor_visible)?;
            ret_bool(cf, v)
        },
        "setForceCursorVisible" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = boolean_arg(cf, 1)?;
            with(cf, |e| e.force_cursor_visible = v)?;
            Ok(0)
        },
        "getClickedValue" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let v = with(cf, |e| e.clicked_value.clone())?;
            ret_str(cf, v.as_deref())
        },
        "setClickedValue" | "ButtonClicked" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = string_arg(cf, 1)?;
            with(cf, |e| e.clicked_value = v)?;
            Ok(0)
        },
        "setUIName" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let v = string_arg(cf, 1)?;
            with(cf, |e| e.set_ui_name(v))?;
            Ok(0)
        },
        // the name, else the table's "Type: name" / "Type" (as `toString` of the values), else "UI: " + hashCode
        "getUIName" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let (name, table) = with(cf, |e| (e.uiname.clone(), e.table.clone()))?;
            if !name.is_empty() {
                return ret_str(cf, Some(&name));
            }
            if let Some(t) = &table {
                let rg = KahluaTableImpl::rawget(t, &Object::str("name")).map_err(Fail::Lua)?;
                let ty = KahluaTableImpl::rawget(t, &Object::str("Type")).map_err(Fail::Lua)?;
                if !ty.is_nil() {
                    let s = if rg.is_nil() {
                        ty.java_to_string()
                    } else {
                        format!("{}: {}", ty.java_to_string(), rg.java_to_string())
                    };
                    return ret_str(cf, Some(&s));
                }
            }
            Err(Fail::Lua(Throwable::runtime(
                "not ported: UIElement.getUIName of an element without a name (its identity hash)",
            )))
        },
        _ => return None,
    })
}
