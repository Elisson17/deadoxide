//! zombie.ui.UIElement: the Java side of a Lua UI element (ISUIElement makes one in `instantiate` and keeps it as
//! `javaObject`). The fields and setters the Lua UI reaches so far; layout, drawing and input are not ported.
use crate::se::krka::kahlua::vm::object::TableRef;

/// zombie.ui.UIElement's fields (their initialisers).
pub struct UIElement {
    /// The Lua table the element belongs to (`new UIElement(KahluaTable)`); null for `new UIElement()`.
    pub table: Option<TableRef>,
    pub height: f32,
    pub width: f32,
    pub x: f64,
    pub y: f64,
    pub anchor_top: bool,
    pub anchor_left: bool,
    pub anchor_right: bool,
    pub anchor_bottom: bool,
    pub lastheight: f64,
    pub lastwidth: f64,
    pub resize_dirty: bool,
    pub consume_mouse_events: bool,
    pub want_key_events: bool,
    pub want_extra_mouse_events: bool,
    pub force_cursor_visible: bool,
}

impl UIElement {
    /// zombie.ui.UIElement.<init>(KahluaTable) (and <init>() with `None`).
    pub fn new(table: Option<TableRef>) -> UIElement {
        UIElement {
            table,
            height: 256.0,
            width: 256.0,
            x: 0.0,
            y: 0.0,
            anchor_top: true,
            anchor_left: true,
            anchor_right: false,
            anchor_bottom: false,
            lastheight: -1.0,
            lastwidth: -1.0,
            resize_dirty: false,
            consume_mouse_events: true,
            want_key_events: false,
            want_extra_mouse_events: false,
            force_cursor_visible: false,
        }
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
}
