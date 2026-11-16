//! zombie.ui.UIElement: the Java side of a Lua UI element (ISUIElement makes one in `instantiate` and keeps it as
//! `javaObject`). Its fields and the methods that only change them; the ones that call back into Lua
//! (setXScroll/setYScroll's onScrollPosChanged, onResize, update) are in the Lua layer (`lua::exposed_ui`), and
//! drawing and input are not ported (nothing renders headless).
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::se::krka::kahlua::vm::object::{TableRef, Userdata};

/// A UIElement as a Lua value (identity: the Rc). Its children and parent are other UIElement values.
pub struct UIElementObject(pub RefCell<UIElement>);

/// The UIElement a Lua value is, if it is one.
pub fn ui_element(u: &Rc<dyn Userdata>) -> Option<&UIElementObject> {
    u.as_any().downcast_ref::<UIElementObject>()
}

/// Object identity of two Java objects.
pub fn same(a: &Rc<dyn Userdata>, b: &Rc<dyn Userdata>) -> bool {
    std::ptr::addr_eq(Rc::as_ptr(a), Rc::as_ptr(b))
}

/// `ArrayList.remove(Object)` by identity (UIElement does not override equals): the first occurrence.
pub fn list_remove(list: &mut Vec<Rc<dyn Userdata>>, el: &Rc<dyn Userdata>) -> bool {
    match list.iter().position(|x| same(x, el)) {
        Some(i) => {
            list.remove(i);
            true
        }
        None => false,
    }
}

/// zombie.ui.UIElement's fields (their initialisers).
pub struct UIElement {
    pub capture: bool,
    pub ignore_loss_control: bool,
    pub clicked_value: Option<String>,
    /// `controls`: the children, UIElements.
    pub controls: Vec<Rc<dyn Userdata>>,
    pub default_draw: bool,
    pub follow_game_world: bool,
    pub render_this_player_only: i32,
    pub height: f32,
    /// `parent` (weak: the parent holds the child).
    pub parent: Option<Weak<dyn Userdata>>,
    pub visible: bool,
    pub width: f32,
    pub x: f64,
    pub y: f64,
    /// The Lua table the element belongs to (`new UIElement(KahluaTable)`); null for `new UIElement()`.
    pub table: Option<TableRef>,
    pub always_back: bool,
    pub scroll_children: bool,
    pub scroll_with_parent: bool,
    pub render_clipped_children: bool,
    pub anchor_top: bool,
    pub anchor_left: bool,
    pub anchor_right: bool,
    pub anchor_bottom: bool,
    pub player_context: i32,
    pub always_on_top: bool,
    pub max_draw_height: i32,
    /// `Double yScroll` (never null here: only the setters write it).
    pub y_scroll: f64,
    pub x_scroll: f64,
    pub scroll_height: i32,
    pub lastheight: f64,
    pub lastwidth: f64,
    pub resize_dirty: bool,
    pub enabled: bool,
    /// `toTop`: children bringToTop asked to move last at the next update.
    pub to_top: Vec<Rc<dyn Userdata>>,
    pub consume_mouse_events: bool,
    pub uiname: String,
    pub want_key_events: bool,
    pub want_extra_mouse_events: bool,
    pub force_cursor_visible: bool,
}

impl UIElement {
    /// zombie.ui.UIElement.<init>(KahluaTable) (and <init>() with `None`).
    pub fn new(table: Option<TableRef>) -> UIElement {
        UIElement {
            capture: false,
            ignore_loss_control: false,
            clicked_value: None,
            controls: Vec::new(),
            default_draw: true,
            follow_game_world: false,
            render_this_player_only: -1,
            height: 256.0,
            parent: None,
            visible: true,
            width: 256.0,
            x: 0.0,
            y: 0.0,
            table,
            always_back: false,
            scroll_children: false,
            scroll_with_parent: true,
            render_clipped_children: true,
            anchor_top: true,
            anchor_left: true,
            anchor_right: false,
            anchor_bottom: false,
            player_context: -1,
            always_on_top: false,
            max_draw_height: -1,
            y_scroll: 0.0,
            x_scroll: 0.0,
            scroll_height: 0,
            lastheight: -1.0,
            lastwidth: -1.0,
            resize_dirty: false,
            enabled: true,
            to_top: Vec::new(),
            consume_mouse_events: true,
            uiname: String::new(),
            want_key_events: false,
            want_extra_mouse_events: false,
            force_cursor_visible: false,
        }
    }

    /// zombie.ui.UIElement.getParent
    pub fn parent(&self) -> Option<Rc<dyn Userdata>> {
        self.parent.as_ref().and_then(Weak::upgrade)
    }

    /// zombie.ui.UIElement.setX: `this.x = (float)x` (the double field keeps the float's value).
    pub fn set_x(&mut self, x: f64) {
        self.x = f64::from(x as f32);
    }

    /// zombie.ui.UIElement.setY: `this.y = (float)y`.
    pub fn set_y(&mut self, y: f64) {
        self.y = f64::from(y as f32);
    }

    /// zombie.ui.UIElement.setWidth: a change marks the size dirty; the old width is kept as lastwidth.
    pub fn set_width(&mut self, width: f64) {
        if f64::from(self.width) != width {
            self.resize_dirty = true;
        }
        self.lastwidth = f64::from(self.width);
        self.width = width as f32;
    }

    /// zombie.ui.UIElement.setHeight: a change marks the size dirty; the old height is kept as lastheight.
    pub fn set_height(&mut self, height: f64) {
        if f64::from(self.height) != height {
            self.resize_dirty = true;
        }
        self.lastheight = f64::from(self.height);
        self.height = height as f32;
    }

    /// zombie.ui.UIElement.setHeightSilent
    pub fn set_height_silent(&mut self, height: f64) {
        self.lastheight = f64::from(self.height);
        self.height = height as f32;
    }

    /// zombie.ui.UIElement.setWidthSilent
    pub fn set_width_silent(&mut self, width: f64) {
        self.lastwidth = f64::from(self.width);
        self.width = width as f32;
    }

    /// zombie.ui.UIElement.setHeightOnly
    pub fn set_height_only(&mut self, height: f64) {
        self.height = height as f32;
    }

    /// zombie.ui.UIElement.setWidthOnly
    pub fn set_width_only(&mut self, width: f64) {
        self.width = width as f32;
    }

    /// zombie.ui.UIElement.ignoreWidthChange
    pub fn ignore_width_change(&mut self) {
        self.lastwidth = f64::from(self.width);
    }

    /// zombie.ui.UIElement.ignoreHeightChange
    pub fn ignore_height_change(&mut self) {
        self.lastheight = f64::from(self.height);
    }

    /// The common tail of setAnchorTop/Left/Right/Bottom: the current size becomes the last size.
    fn anchor_changed(&mut self) {
        self.lastwidth = f64::from(self.width);
        self.lastheight = f64::from(self.height);
    }

    /// zombie.ui.UIElement.setAnchorTop
    pub fn set_anchor_top(&mut self, b: bool) {
        self.anchor_top = b;
        self.anchor_changed();
    }

    /// zombie.ui.UIElement.setAnchorLeft
    pub fn set_anchor_left(&mut self, b: bool) {
        self.anchor_left = b;
        self.anchor_changed();
    }

    /// zombie.ui.UIElement.setAnchorRight
    pub fn set_anchor_right(&mut self, b: bool) {
        self.anchor_right = b;
        self.anchor_changed();
    }

    /// zombie.ui.UIElement.setAnchorBottom
    pub fn set_anchor_bottom(&mut self, b: bool) {
        self.anchor_bottom = b;
        self.anchor_changed();
    }

    /// zombie.ui.UIElement.setScrollHeight: `(int)h`.
    pub fn set_scroll_height(&mut self, h: f64) {
        self.scroll_height = h as i32;
    }

    /// zombie.ui.UIElement.setMaxDrawHeight: `(int)height`.
    pub fn set_max_draw_height(&mut self, h: f64) {
        self.max_draw_height = h as i32;
    }

    /// zombie.ui.UIElement.setUIName: null is "".
    pub fn set_ui_name(&mut self, name: Option<String>) {
        self.uiname = name.unwrap_or_default();
    }
}

/// zombie.ui.UIElement.AddChild: `controls.add(el)`, then `el.setParent(this)`.
pub fn add_child(this: &Rc<dyn Userdata>, el: &Rc<dyn Userdata>) -> Result<(), &'static str> {
    let me = ui_element(this).ok_or("java.lang.NullPointerException")?;
    me.0.borrow_mut().controls.push(el.clone());
    let child = ui_element(el).ok_or("java.lang.NullPointerException")?;
    child.0.borrow_mut().parent = Some(Rc::downgrade(this));
    Ok(())
}

/// zombie.ui.UIElement.RemoveChild (and RemoveControl): `controls.remove(el)`, then `el.setParent(null)`.
pub fn remove_child(this: &UIElementObject, el: &Rc<dyn Userdata>) -> Result<(), &'static str> {
    list_remove(&mut this.0.borrow_mut().controls, el);
    let child = ui_element(el).ok_or("java.lang.NullPointerException")?;
    child.0.borrow_mut().parent = None;
    Ok(())
}

/// `Double.intValue()` of a double: Java's (int) cast.
fn int_value(d: f64) -> f64 {
    f64::from(d as i32)
}

/// zombie.ui.UIElement.getAbsoluteX
pub fn get_absolute_x(this: &UIElementObject) -> f64 {
    let e = this.0.borrow();
    match e.parent().as_ref().and_then(ui_element) {
        Some(p) => {
            let pe = p.0.borrow();
            let (scroll_children, x_scroll) = (pe.scroll_children, pe.x_scroll);
            drop(pe);
            let base = get_absolute_x(p) + int_value(e.x);
            if scroll_children && e.scroll_with_parent {
                base + int_value(x_scroll)
            } else {
                base
            }
        }
        None => int_value(e.x),
    }
}

/// zombie.ui.UIElement.getAbsoluteY
pub fn get_absolute_y(this: &UIElementObject) -> f64 {
    let e = this.0.borrow();
    match e.parent().as_ref().and_then(ui_element) {
        Some(p) => {
            let pe = p.0.borrow();
            let (scroll_children, y_scroll) = (pe.scroll_children, pe.y_scroll);
            drop(pe);
            let base = get_absolute_y(p) + int_value(e.y);
            if scroll_children && e.scroll_with_parent {
                base + int_value(y_scroll)
            } else {
                base
            }
        }
        None => int_value(e.y),
    }
}

/// zombie.ui.UIElement.clampToParentX
pub fn clamp_to_parent_x(this: &UIElementObject, x: f64) -> f64 {
    let parent = this.0.borrow().parent();
    let Some(p) = parent.as_ref().and_then(ui_element) else {
        return x;
    };
    let x1 = clamp_to_parent_x(p, get_absolute_x(p));
    let x2 = clamp_to_parent_x(p, x1 + int_value(f64::from(p.0.borrow().width)));
    let mut x = x;
    if x < x1 {
        x = x1;
    }
    if x > x2 {
        x = x2;
    }
    x
}

/// zombie.ui.UIElement.clampToParentY
pub fn clamp_to_parent_y(this: &UIElementObject, y: f64) -> f64 {
    let parent = this.0.borrow().parent();
    let Some(p) = parent.as_ref().and_then(ui_element) else {
        return y;
    };
    let y1 = clamp_to_parent_y(p, get_absolute_y(p));
    let y2 = clamp_to_parent_y(p, y1 + int_value(f64::from(p.0.borrow().height)));
    let mut y = y;
    if y < y1 {
        y = y1;
    }
    if y > y2 {
        y = y2;
    }
    y
}

/// zombie.ui.UIElement.isReallyVisible: visible, and in its parent's controls (and the parent really visible), or
/// a top element in UIManager's `UI` list.
pub fn is_really_visible(this: &Rc<dyn Userdata>, ui: &[Rc<dyn Userdata>]) -> bool {
    let Some(me) = ui_element(this) else {
        return false;
    };
    let e = me.0.borrow();
    if !e.visible {
        return false;
    }
    match e.parent() {
        Some(p) => {
            let Some(pe) = ui_element(&p) else {
                return false;
            };
            if !pe.0.borrow().controls.iter().any(|c| same(c, this)) {
                return false;
            }
            drop(e);
            is_really_visible(&p, ui)
        }
        None => ui.iter().any(|u| same(u, this)),
    }
}
