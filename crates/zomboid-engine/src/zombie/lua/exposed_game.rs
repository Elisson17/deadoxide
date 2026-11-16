//! The exposed API the scripts reach in a client whose startup ran (GameWindow.init and initShared before LoadDirBase,
//! the luagame oracle): the fonts TextManager.Init loaded, as AngelCodeFont objects and through TextManager's
//! measuring methods. Same conventions as `exposed_methods`.
use std::path::Path;
use std::rc::Rc;

use super::exposed_client::TextManagerObject;
use super::exposed_methods::{
    hash, npe, registry_id, ret, ret_int, ret_str, string_arg, want_args, with_this,
};
use super::exposed_world::enum_arg;
use super::exposer::{class_named, Fail, Imp, JavaEnum, JavaStatics, Ret};
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::object::{JavaClass, Object, Throwable, Userdata};
use crate::zombie::core::textures::texture::{PngSize, Texture, TextureRef};
use crate::zombie::scripting::item::Value;
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

/// zombie.core.textures.Texture as a Lua value (one object per texture).
pub struct TextureObject(pub TextureRef);

impl Userdata for TextureObject {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.core.textures.Texture")
    }
    fn java_to_string(&self) -> String {
        format!("zombie.core.textures.Texture@{:x}", hash(self))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// The Lua value of `t`: the same object each time.
fn texture_object(s: &Rc<JavaStatics>, t: &TextureRef) -> Object {
    let mut textures = s.state.textures.borrow_mut();
    let key = Rc::as_ptr(t) as usize;
    let o = textures
        .entry(key)
        .or_insert_with(|| Rc::new(TextureObject(t.clone())))
        .clone();
    Object::Userdata(o)
}

/// Runs `f` on a Texture receiver with the startup's texture state (its game folder and PNGSize).
fn with_texture(
    s: &Rc<JavaStatics>,
    cf: &FrameRef,
    f: fn(&mut Texture, &Path, &mut PngSize) -> i32,
) -> Ret {
    let client = s.state.client.borrow().clone().ok_or_else(npe)?;
    with_this!(cf, TextureObject, t, {
        let mut textures = client.textures.borrow_mut();
        let root = textures.root().to_path_buf();
        let v = f(&mut t.0.borrow_mut(), &root, &mut textures.png_size);
        ret_int(cf, v)
    })
}

/// zombie.scripting.objects.Item as a Lua value: the index of the item script in the startup's ScriptManager (one
/// object per item).
pub struct ItemObject(pub usize);

impl Userdata for ItemObject {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.scripting.objects.Item")
    }
    fn java_to_string(&self) -> String {
        format!("zombie.scripting.objects.Item@{:x}", hash(self))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// The Lua value of item `i`: the same object each time.
pub(super) fn item_object(s: &Rc<JavaStatics>, i: usize) -> Object {
    let o = s
        .state
        .items
        .borrow_mut()
        .entry(i)
        .or_insert_with(|| Rc::new(ItemObject(i)))
        .clone();
    Object::Userdata(o)
}

/// Runs `f` on an Item receiver's script.
fn with_item(
    s: &Rc<JavaStatics>,
    cf: &FrameRef,
    f: impl FnOnce(&crate::zombie::scripting::item::Item) -> Ret,
) -> Ret {
    let client = s.state.client.borrow().clone().ok_or_else(npe)?;
    with_this!(cf, ItemObject, o, f(&client.scripts.items[o.0]))
}

pub fn global(name: &str) -> Option<Imp> {
    Some(match name {
        // Texture.getSharedTexture: the startup's packs and the game folder's files (null without the startup, as
        // in a Lua state made on its own)
        "getTexture" => |s, _, cf, n| {
            want_args(n, false, 1)?;
            let name = string_arg(cf, 0)?;
            let Some(client) = s.state.client.borrow().clone() else {
                return ret(cf, Object::Nil);
            };
            let t = client
                .textures
                .borrow_mut()
                .get_shared_texture(&client.fs, name.as_deref());
            match t {
                Some(t) => ret(cf, texture_object(s, &t)),
                None => ret(cf, Object::Nil),
            }
        },
        _ => return None,
    })
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
        // ---- Texture ----
        ("zombie.core.textures.Texture", "getWidth") => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_texture(s, cf, Texture::get_width)
        },
        ("zombie.core.textures.Texture", "getHeight") => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_texture(s, cf, Texture::get_height)
        },
        ("zombie.core.textures.Texture", "getWidthOrig") => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_texture(s, cf, Texture::get_width_orig)
        },
        ("zombie.core.textures.Texture", "getHeightOrig") => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_texture(s, cf, Texture::get_height_orig)
        },
        // ---- ScriptManager: the startup's scripts (none in a Lua state made on its own) ----
        ("zombie.scripting.ScriptManager", "FindItem") => |s, _, cf, _| {
            let name = string_arg(cf, 1)?.ok_or_else(npe)?;
            let Some(client) = s.state.client.borrow().clone() else {
                return ret(cf, Object::Nil);
            };
            match client.find_item(&name) {
                Some(i) => ret(cf, item_object(s, i)),
                None => ret(cf, Object::Nil),
            }
        },
        // ---- Item ----
        ("zombie.scripting.objects.Item", "getFullName") => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_item(s, cf, |i| ret_str(cf, Some(&i.full_name())))
        },
        ("zombie.scripting.objects.Item", "getName") => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_item(s, cf, |i| ret_str(cf, Some(&i.name)))
        },
        ("zombie.scripting.objects.Item", "getDisplayName") => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_item(s, cf, |i| {
                let full = i.full_name();
                let name = match i.get("displayName") {
                    Some(Value::Str(Some(d))) if !d.is_empty() => d.clone(),
                    _ => full,
                };
                ret_str(cf, Some(&name))
            })
        },
        ("zombie.scripting.objects.Item", "getUseDelta") => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_item(s, cf, |i| match i.get("useDelta") {
                Some(Value::F32(f)) => ret(cf, Object::Number(f64::from(*f))),
                _ => Err(Fail::Lua(Throwable::runtime("not ported: Item.useDelta"))),
            })
        },
        // ---- GameMode: its strings, made from the id in the constructor ----
        ("zombie.core.GameMode", "getTitle") => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let id = registry_id(cf)?;
            ret_str(cf, Some(&format!("UI_NewGame_{id}")))
        },
        ("zombie.core.GameMode", "getDescription") => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let id = registry_id(cf)?;
            ret_str(cf, Some(&format!("UI_NewGame_{id}_desc")))
        },
        ("zombie.core.GameMode", "getThumbnail") => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let id = registry_id(cf)?;
            ret_str(cf, Some(&format!("media/ui/playstyleIcons/{id}.png")))
        },
        ("zombie.core.GameMode", "getVideo") => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let id = registry_id(cf)?;
            ret_str(cf, Some(&format!("{id}.bik")))
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
