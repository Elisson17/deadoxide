//! zombie.core.Core's instance methods over its options (`CoreOptions`): the plain `getValue` / `setValue` wrappers
//! (generated, `exposed_core_accessors`) and the ones with their own body (colours, array options, derived values).
//! Same conventions as `exposed_methods`.
use std::rc::Rc;

use super::exposed_core_accessors::accessor;
use super::exposed_methods::{
    bool_arg, int_arg, npe, ret, ret_bool, ret_float, ret_int, ret_str, string_arg, type_error, ud,
    want_args, with_this, ArrayListObject, CoreObject, CORE_HEIGHT, CORE_WIDTH,
};
use super::exposer::{Fail, Imp, JavaObject, JavaStatics, Ret};
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::object::{Object, Throwable};
use crate::zombie::core::core_options::{Arg, Opt, SaveContext};
use crate::zombie::core::textures::color_info::ColorInfo;

/// The generated getter of option field `field` (Java return type `ty`).
pub(super) fn get(
    _s: &Rc<JavaStatics>,
    cf: &FrameRef,
    n: i32,
    field: &'static str,
    ty: char,
) -> Ret {
    want_args(n, true, 0)?;
    with_this!(cf, CoreObject, c, {
        let options = c.options.borrow();
        match (ty, options.get(field)) {
            ('i', Opt::Int(x)) => ret_int(cf, x.value),
            ('z', Opt::Bool(x)) => ret_bool(cf, x.value),
            ('d', Opt::Double(x)) => ret(cf, Object::Number(x.value)),
            ('d', Opt::Int(x)) => ret(cf, Object::Number(f64::from(x.value))),
            ('f', Opt::Double(x)) => ret_float(cf, x.value as f32),
            ('f', Opt::Int(x)) => ret_float(cf, x.value as f32),
            ('s', Opt::Str(x)) => ret_str(cf, Some(&x.value)),
            _ => unreachable!("Core option {field} read as {ty}"),
        }
    })
}

/// The generated setter of option field `field` (Java parameter type `ty`).
pub(super) fn set(
    s: &Rc<JavaStatics>,
    cf: &FrameRef,
    n: i32,
    field: &'static str,
    ty: char,
) -> Ret {
    want_args(n, true, 1)?;
    with_this!(cf, CoreObject, c, {
        let mut log = Vec::new();
        {
            let mut options = c.options.borrow_mut();
            match (ty, options.get_mut(field)) {
                ('i', Opt::Int(x)) => {
                    let v = int_arg(cf, 1)?;
                    x.set_value(v, &mut log);
                }
                ('z', Opt::Bool(x)) => match cf.get(1) {
                    Object::Boolean(b) => x.value = b,
                    o => return Err(type_error("boolean", &o)),
                },
                ('d', Opt::Double(x)) => match cf.get(1) {
                    Object::Number(d) => x.set_value(d, &mut log),
                    o => return Err(type_error("double", &o)),
                },
                ('s', Opt::Str(x)) => {
                    let v = string_arg(cf, 1)?;
                    x.set_value(v.as_deref(), &mut log);
                }
                _ => unreachable!("Core option {field} written as {ty}"),
            }
        }
        s.loader.borrow_mut().log.extend(log);
        Ok(0)
    })
}

/// The ColorInfo a Core colour setter takes.
fn color_arg(cf: &FrameRef, i: i32) -> Result<(f32, f32, f32, f32), Fail> {
    match cf.get(i) {
        Object::Nil => Err(npe()),
        Object::Userdata(u) => match u.as_any().downcast_ref::<ColorInfo>() {
            Some(c) => Ok((c.r.get(), c.g.get(), c.b.get(), c.a.get())),
            None => Err(type_error("ColorInfo", &Object::Userdata(u))),
        },
        o => Err(type_error("ColorInfo", &o)),
    }
}

/// `Core.getXxxColor`: `option.getValueAsColorInfo(this.xxxColor)`, the Core's own ColorInfo returned.
fn get_color(
    cf: &FrameRef,
    n: i32,
    field: &'static str,
    pick: fn(&CoreObject) -> &Rc<ColorInfo>,
) -> Ret {
    want_args(n, true, 0)?;
    with_this!(cf, CoreObject, c, {
        c.options
            .borrow()
            .get(field)
            .array_value()
            .value_as_color_info(pick(c));
        ret(cf, ud(pick(c).clone()))
    })
}

/// `Core.setXxxColor(ColorInfo)`: `option.setValueVarArgs((double)r, (double)g, (double)b)`, then
/// `this.xxxColor.set(colorInfo)`.
fn set_color(
    s: &Rc<JavaStatics>,
    cf: &FrameRef,
    n: i32,
    field: &'static str,
    pick: fn(&CoreObject) -> &Rc<ColorInfo>,
) -> Ret {
    want_args(n, true, 1)?;
    with_this!(cf, CoreObject, c, {
        let (r, g, b, a) = color_arg(cf, 1)?;
        let mut log = Vec::new();
        c.options
            .borrow_mut()
            .get_mut(field)
            .array_mut()
            .set_value_var_args(
                &[
                    Arg::Double(f64::from(r)),
                    Arg::Double(f64::from(g)),
                    Arg::Double(f64::from(b)),
                ],
                &mut log,
            );
        s.loader.borrow_mut().log.extend(log);
        pick(c).set(r, g, b, a);
        Ok(0)
    })
}

/// The Lua-exposed instance methods of Core over its options.
pub fn instance_method(class: &str, name: &str) -> Option<Imp> {
    if class != "zombie.core.Core" {
        return None;
    }
    if let Some(f) = accessor(name) {
        return Some(f);
    }
    Some(match name {
        "getObjectHighlitedColor" => |_, _, cf, n| {
            get_color(cf, n, "optionObjectHighlightColor", |c| {
                &c.object_highlited_color
            })
        },
        "setObjectHighlitedColor" => |s, _, cf, n| {
            set_color(s, cf, n, "optionObjectHighlightColor", |c| {
                &c.object_highlited_color
            })
        },
        "getWorldItemHighlightColor" => |_, _, cf, n| {
            get_color(cf, n, "optionWorldItemHighlightColor", |c| {
                &c.world_item_highlight_color
            })
        },
        "setWorldItemHighlightColor" => |s, _, cf, n| {
            set_color(s, cf, n, "optionWorldItemHighlightColor", |c| {
                &c.world_item_highlight_color
            })
        },
        "getGoodHighlitedColor" => |_, _, cf, n| {
            get_color(cf, n, "optionGoodHighlightColor", |c| {
                &c.good_highlited_color
            })
        },
        "setGoodHighlitedColor" => |s, _, cf, n| {
            set_color(s, cf, n, "optionGoodHighlightColor", |c| {
                &c.good_highlited_color
            })
        },
        "getBadHighlitedColor" => {
            |_, _, cf, n| get_color(cf, n, "optionBadHighlightColor", |c| &c.bad_highlited_color)
        }
        "setBadHighlitedColor" => |s, _, cf, n| {
            set_color(s, cf, n, "optionBadHighlightColor", |c| {
                &c.bad_highlited_color
            })
        },
        "getTargetColor" => {
            |_, _, cf, n| get_color(cf, n, "optionTargetColor", |c| &c.target_color)
        }
        "setTargetColor" => {
            |s, _, cf, n| set_color(s, cf, n, "optionTargetColor", |c| &c.target_color)
        }
        "getNoTargetColor" => {
            |_, _, cf, n| get_color(cf, n, "optionNoTargetColor", |c| &c.no_target_color)
        }
        "setNoTargetColor" => {
            |s, _, cf, n| set_color(s, cf, n, "optionNoTargetColor", |c| &c.no_target_color)
        }
        // Core.getOptionZoomLevels1x/2x: `optionZoomLevelsNx.getValueAsString()`
        // Core.allowOptionTextureCompression: `RenderThread.invokeQueryOnRenderContext(..)` with no render thread (it
        // spins until `isInitialized`, which never becomes true headless) returns false; so does the option getter
        // (`allow && value`).
        "allowOptionTextureCompression" | "getOptionTextureCompression" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            ret_bool(cf, false)
        },
        // Core.getOptionUsePhysicsHitReaction: `!GameClient.client && !GameServer.server ? option : false`; neither
        // is set in the client
        "getOptionUsePhysicsHitReaction" => {
            |s, _, cf, n| get(s, cf, n, "optionUsePhysicsHitReaction", 'z')
        }
        "isAzerty" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_this!(cf, CoreObject, c, ret_bool(cf, c.is_azerty.get()))
        },
        "setAzerty" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            with_this!(cf, CoreObject, c, {
                c.is_azerty.set(bool_arg(cf, 1)?);
                Ok(0)
            })
        },
        "getPerfSkyboxOnLoad" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_this!(cf, CoreObject, c, ret_int(cf, c.i_perf_skybox.get()))
        },
        "getPerfReflectionsOnLoad" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_this!(cf, CoreObject, c, ret_bool(cf, c.perf_reflections.get()))
        },
        "getPerfPuddlesOnLoad" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_this!(cf, CoreObject, c, ret_int(cf, c.i_perf_puddles.get()))
        },
        // Core.getOptionVoiceRecordDevice: `Core.soundDisabled` (this headless client is the `-nosound` one) gives 0
        "getOptionVoiceRecordDevice" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            ret_int(cf, 0)
        },
        // Core.getDefaultZoomLevels: `offscreenBuffer.getDefaultZoomLevels()`, Math.round(level * 100.0F) of
        // zoomLevelsDefault
        "getDefaultZoomLevels" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            let levels = [2.5f32, 2.25, 2.0, 1.75, 1.5, 1.25, 1.0, 0.75, 0.5, 0.25];
            let items = levels
                .iter()
                .map(|l| Object::Number(f64::from(crate::java::math_round_f32(l * 100.0))))
                .collect();
            ret(
                cf,
                ud(Rc::new(ArrayListObject(std::cell::RefCell::new(items)))),
            )
        },
        // Core.getOptionSingleContextMenu / getAutoZoom: `((BooleanConfigOption)option.getElement(playerIndex)).getValue()`
        "getOptionSingleContextMenu" => {
            |_, _, cf, n| array_element_get(cf, n, "optionSingleContextMenu")
        }
        "getAutoZoom" => |_, _, cf, n| array_element_get(cf, n, "autoZoom"),
        "setOptionSingleContextMenu" => {
            |_, _, cf, n| array_element_set(cf, n, "optionSingleContextMenu")
        }
        // Core.reinitKeyMaps
        "reinitKeyMaps" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_this!(cf, CoreObject, c, {
                c.key_maps.borrow_mut().reinit();
                Ok(0)
            })
        },
        // Core.getKey(String | KeybindId): `getKeyBinding(keyName).keyValue()`
        "getKey" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            with_this!(cf, CoreObject, c, {
                let name = key_name_arg(cf, 1)?;
                let binding = c.key_maps.borrow().get_key_binding(name.as_deref());
                ret_int(cf, binding.key_value)
            })
        },
        // Core.getAltKey(String | KeybindId): `getKeyBinding(keyName).altKey()`
        "getAltKey" => |_, _, cf, n| {
            want_args(n, true, 1)?;
            with_this!(cf, CoreObject, c, {
                let name = key_name_arg(cf, 1)?;
                let binding = c.key_maps.borrow().get_key_binding(name.as_deref());
                ret_int(cf, binding.alt_key)
            })
        },
        // Core.addKeyBinding(String | KeybindId, int key, int altKey, boolean shift, boolean ctrl, boolean alt)
        "addKeyBinding" => |_, _, cf, n| {
            want_args(n, true, 6)?;
            with_this!(cf, CoreObject, c, {
                let name = key_name_arg(cf, 1)?;
                let key = int_arg(cf, 2)?;
                let alt_key = int_arg(cf, 3)?;
                let shift = bool_arg(cf, 4)?;
                let ctrl = bool_arg(cf, 5)?;
                let alt = bool_arg(cf, 6)?;
                c.key_maps.borrow_mut().add_key_binding(
                    name.as_deref(),
                    key,
                    alt_key,
                    shift,
                    ctrl,
                    alt,
                );
                Ok(0)
            })
        },
        // Core.saveOptions: `<cache>/options.ini` (ConfigFile.write)
        "saveOptions" => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_this!(cf, CoreObject, c, {
                let path = std::path::Path::new(&s.cache_dir()?).join("options.ini");
                let ctx = SaveContext {
                    file_version: 8,
                    screen_width: CORE_WIDTH,
                    screen_height: CORE_HEIGHT,
                    performance: &s.state.performance_settings.borrow(),
                    // the measurement host is macOS (os.name "Mac OS X")
                    os_x: cfg!(target_os = "macos"),
                    active_controller_guids: &[],
                };
                let mut log = Vec::new();
                c.options.borrow_mut().save_options(&ctx, &path, &mut log);
                s.loader.borrow_mut().log.extend(log);
                Ok(0)
            })
        },
        "getOptionZoomLevels1x" => |_, _, cf, n| zoom_levels_get(cf, n, "optionZoomLevels1x"),
        "getOptionZoomLevels2x" => |_, _, cf, n| zoom_levels_get(cf, n, "optionZoomLevels2x"),
        // Core.setOptionZoomLevels1x/2x: `optionZoomLevelsNx.parse(levels == null ? "" : levels)`
        "setOptionZoomLevels1x" => |s, _, cf, n| zoom_levels_set(s, cf, n, "optionZoomLevels1x"),
        "setOptionZoomLevels2x" => |s, _, cf, n| zoom_levels_set(s, cf, n, "optionZoomLevels2x"),
        _ => return None,
    })
}

fn zoom_levels_get(cf: &FrameRef, n: i32, field: &'static str) -> Ret {
    want_args(n, true, 0)?;
    with_this!(cf, CoreObject, c, {
        let v = c.options.borrow().get(field).value_as_string();
        ret_str(cf, Some(&v))
    })
}

fn zoom_levels_set(s: &Rc<JavaStatics>, cf: &FrameRef, n: i32, field: &'static str) -> Ret {
    want_args(n, true, 1)?;
    with_this!(cf, CoreObject, c, {
        let levels = string_arg(cf, 1)?.unwrap_or_default();
        let mut log = Vec::new();
        c.options
            .borrow_mut()
            .get_mut(field)
            .parse(&levels, &mut log);
        s.loader.borrow_mut().log.extend(log);
        Ok(0)
    })
}

/// The key name a `String` / `KeybindId` overload takes: the string, or the KeybindId's id (`getId()`); None for nil.
fn key_name_arg(cf: &FrameRef, i: i32) -> Result<Option<String>, Fail> {
    match cf.get(i) {
        Object::Nil => Ok(None),
        Object::String(_) => string_arg(cf, i),
        Object::Userdata(u) => match u.as_any().downcast_ref::<JavaObject>() {
            Some(o) if o.class.name == "zombie.input.KeybindId" => Ok(Some(o.id.to_string())),
            _ => Err(type_error("String", &Object::Userdata(u))),
        },
        o => Err(type_error("String", &o)),
    }
}

/// `((BooleanConfigOption)option.getElement(playerIndex)).getValue()` of an array option.
fn array_element_get(cf: &FrameRef, n: i32, field: &'static str) -> Ret {
    want_args(n, true, 1)?;
    with_this!(cf, CoreObject, c, {
        let index = int_arg(cf, 1)?;
        let options = c.options.borrow();
        let a = options.get(field).array_value();
        match usize::try_from(index).ok().and_then(|i| a.value.get(i)) {
            Some(o) => ret_bool(cf, o.bool_value()),
            None => Err(index_out_of_bounds(index, a.value.len())),
        }
    })
}

/// `((BooleanConfigOption)option.getElement(playerIndex)).setValue(b)` of an array option.
fn array_element_set(cf: &FrameRef, n: i32, field: &'static str) -> Ret {
    want_args(n, true, 2)?;
    with_this!(cf, CoreObject, c, {
        let index = int_arg(cf, 1)?;
        let b = bool_arg(cf, 2)?;
        let mut options = c.options.borrow_mut();
        let a = options.get_mut(field).array_mut();
        let size = a.value.len();
        match usize::try_from(index).ok().and_then(|i| a.value.get_mut(i)) {
            Some(Opt::Bool(o)) => {
                o.value = b;
                Ok(0)
            }
            _ => Err(index_out_of_bounds(index, size)),
        }
    })
}

/// An `IndexOutOfBoundsException` of `ArrayList.get`.
pub(super) fn index_out_of_bounds(index: i32, size: usize) -> Fail {
    Fail::Java(Throwable::Java {
        class: "java.lang.IndexOutOfBoundsException",
        message: Some(format!("Index {index} out of bounds for length {size}")),
    })
}
