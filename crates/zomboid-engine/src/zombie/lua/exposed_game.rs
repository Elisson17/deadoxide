//! The exposed API the scripts reach in a client whose startup ran (GameWindow.init and initShared before LoadDirBase,
//! the luagame oracle): the fonts TextManager.Init loaded, as AngelCodeFont objects and through TextManager's
//! measuring methods. Same conventions as `exposed_methods`.
use std::rc::Rc;

use super::exposed_client::TextManagerObject;
use super::exposed_methods::{hash, npe, ret, ret_int, want_args, with_this};
use super::exposed_world::enum_arg;
use super::exposer::{class_named, Fail, Imp, JavaEnum, JavaStatics};
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::object::{JavaClass, Object, Throwable, Userdata};
use crate::zombie::ui::text_manager::FontRef;
use crate::zombie::ui::ui_font::UIFont;

/// zombie.core.fonts.AngelCodeFont as a Lua value (one object per font, so that `==` sees the font's identity).
pub struct AngelCodeFontObject(pub FontRef);

impl Userdata for AngelCodeFontObject {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.core.fonts.AngelCodeFont")
    }
    fn java_to_string(&self) -> String {
        format!("zombie.core.fonts.AngelCodeFont@{:x}", hash(self))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// The Lua value of `font`: the same object each time.
pub(super) fn font_object(s: &Rc<JavaStatics>, font: &FontRef) -> Object {
    let mut fonts = s.state.fonts.borrow_mut();
    if let Some((_, o)) = fonts.iter().find(|(f, _)| Rc::ptr_eq(f, font)) {
        return Object::Userdata(o.clone());
    }
    let o = Rc::new(AngelCodeFontObject(font.clone()));
    fonts.push((font.clone(), o.clone()));
    Object::Userdata(o)
}

/// A UIFont parameter (null for nil).
pub(super) fn font_arg(cf: &FrameRef, i: i32) -> Result<Option<UIFont>, Fail> {
    Ok(enum_arg(cf, i, "zombie.ui.UIFont", "UIFont")?.map(|u| {
        let ordinal = u
            .as_any()
            .downcast_ref::<JavaEnum>()
            .map_or(0, |e| usize::try_from(e.ordinal).unwrap_or(0));
        UIFont::values()[ordinal]
    }))
}

/// A String parameter as Java's UTF-16 text (null for nil).
fn text_arg(cf: &FrameRef, i: i32) -> Result<Option<Rc<[u16]>>, Fail> {
    match cf.get(i) {
        Object::Nil => Ok(None),
        Object::String(s) => Ok(Some(s)),
        o => Err(super::exposed_methods::type_error("String", &o)),
    }
}

/// A NullPointerException (or another exception) of the ported TextManager: logged, nothing returned.
fn java_fail(m: String) -> Fail {
    if m == "java.lang.NullPointerException" {
        npe()
    } else {
        Fail::Lua(Throwable::runtime(m))
    }
}

pub fn global(_name: &str) -> Option<Imp> {
    None
}

pub fn static_method(_class: &str, _name: &str) -> Option<Imp> {
    None
}

pub fn instance_method(class: &str, name: &str) -> Option<Imp> {
    Some(match (class, name) {
        // ---- TextManager: the fonts of TextManager.Init (none before it: the NullPointerException) ----
        ("zombie.ui.TextManager", "getFontFromEnum") => |s, _, cf, n| {
            want_args(n, true, 1)?;
            let font = font_arg(cf, 1)?;
            with_this!(cf, TextManagerObject, t, {
                let f = t.0.borrow().get_font_from_enum(font);
                match f {
                    None => ret(cf, Object::Nil),
                    Some(f) => ret(cf, font_object(s, &f)),
                }
            })
        },
        ("zombie.ui.TextManager", "getFontHeight") => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let font = font_arg(cf, 1)?;
            with_this!(cf, TextManagerObject, t, {
                let h = t.0.borrow().get_font_height(font).map_err(java_fail)?;
                ret_int(cf, h)
            })
        },
        ("zombie.ui.TextManager", "MeasureStringX") => |_, _, cf, n| {
            want_args(n, true, 2)?;
            let font = font_arg(cf, 1)?;
            let text = text_arg(cf, 2)?;
            with_this!(cf, TextManagerObject, t, {
                let w =
                    t.0.borrow()
                        .measure_string_x(font, text.as_deref())
                        .map_err(java_fail)?;
                ret_int(cf, w)
            })
        },
        ("zombie.ui.TextManager", "MeasureStringY") => |_, _, cf, n| {
            want_args(n, true, 2)?;
            let font = font_arg(cf, 1)?;
            let text = text_arg(cf, 2)?;
            with_this!(cf, TextManagerObject, t, {
                let h =
                    t.0.borrow()
                        .measure_string_y(font, text.as_deref())
                        .map_err(java_fail)?;
                ret_int(cf, h)
            })
        },
        // ---- AngelCodeFont ----
        ("zombie.core.fonts.AngelCodeFont", "getLineHeight") => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_this!(cf, AngelCodeFontObject, f, {
                let h = f.0.borrow().get_line_height();
                ret_int(cf, h)
            })
        },
        _ => return None,
    })
}
