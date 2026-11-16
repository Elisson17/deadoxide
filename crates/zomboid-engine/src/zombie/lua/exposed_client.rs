//! The exposed API the game's `media/lua/client` reaches while loading (measured with LuaLoadCallsProbe): the
//! music configuration (MusicIntensityConfig, MusicThreatConfig), the controller bindings (CharacterInputBindingSet,
//! CharacterJoypadButtonBinding, the Joypad* enums, the controller slots), the UI fonts of TextManager, Core's
//! font size, button style and gamepad preset options, CGlobalObjects, the window states, org.joml.Vector3f,
//! DebugType's log calls and a few constant globals. Same conventions as `exposed_methods`.
use std::cell::RefCell;
use std::rc::Rc;

use super::exposed_methods::{
    arg_count_error, float_arg, int_arg, ret, ret_bool, ret_float, ret_int, ret_str, singleton,
    string_arg, type_error, ud, with_this, CoreObject,
};
use super::exposed_world::{enum_arg, state_object, table_impl_arg};
use super::exposer::{class_named, Fail, Imp, JavaEnum, JavaStatics, Ret};
use crate::org::joml::vector3f::Vector3f;
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::object::{JavaClass, Object, Throwable, Userdata};
use crate::zombie::audio::music_intensity_config::MusicIntensityConfig;
use crate::zombie::audio::music_threat_config::MusicThreatConfig;
use crate::zombie::characters::character_input_binding_set::CharacterInputBindingSet;
use crate::zombie::ui::text_manager::TextManager;
use crate::zombie::ui::ui_font::UIFont;

state_object!(
    MusicIntensityConfigObject,
    MusicIntensityConfig,
    "zombie.audio.MusicIntensityConfig"
);
state_object!(
    MusicThreatConfigObject,
    MusicThreatConfig,
    "zombie.audio.MusicThreatConfig"
);
state_object!(TextManagerObject, TextManager, "zombie.ui.TextManager");

/// org.joml.Vector3f as a Lua value.
pub struct Vector3fObject(pub RefCell<Vector3f>);

impl Userdata for Vector3fObject {
    fn class(&self) -> &'static JavaClass {
        class_named("org.joml.Vector3f")
    }
    fn java_to_string(&self) -> String {
        self.0.borrow().java_to_string()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// The singletons whose state this module keeps; `None` for the others.
pub fn make_singleton(class: &str) -> Option<Object> {
    Some(match class {
        "zombie.audio.MusicIntensityConfig" => ud(Rc::new(MusicIntensityConfigObject::default())),
        "zombie.audio.MusicThreatConfig" => ud(Rc::new(MusicThreatConfigObject::default())),
        "zombie.ui.TextManager" => ud(Rc::new(TextManagerObject::default())),
        _ => return None,
    })
}

fn the(s: &Rc<JavaStatics>, class: &str) -> Object {
    singleton(s, class_named(class))
}

/// The argument count check of LuaJavaInvoker.prepareCall for a static method with `params` parameters: fewer
/// arguments fail, and so do more, except one argument to a method without parameters (a `Class:method()` call
/// of a static method).
pub(super) fn static_args(n: i32, params: i32) -> Result<(), Fail> {
    if n < params || (n != params && !(n == 1 && params == 0)) {
        return Err(arg_count_error(params, n));
    }
    Ok(())
}

/// An enum constant `name` of `class` as the exposer published it (one object per constant); nil for null.
fn enum_constant(s: &Rc<JavaStatics>, class: &str, name: Option<&str>) -> Object {
    name.map_or(Object::Nil, |n| s.field(class, n))
}

/// The JavaEnum receiver of an enum's instance method.
fn enum_this(cf: &FrameRef) -> Result<Rc<dyn Userdata>, Fail> {
    match cf.get(0) {
        Object::Userdata(u) if u.as_any().is::<JavaEnum>() => Ok(u),
        o => Err(type_error("Enum", &o)),
    }
}

fn ordinal_of(u: &Rc<dyn Userdata>) -> usize {
    u.as_any()
        .downcast_ref::<JavaEnum>()
        .map_or(0, |e| usize::try_from(e.ordinal).unwrap_or(0))
}

/// The name of an enum argument of class `class` (`simple` is its simple name), or None for nil.
fn enum_name(
    cf: &FrameRef,
    i: i32,
    class: &str,
    simple: &str,
) -> Result<Option<&'static str>, Fail> {
    Ok(enum_arg(cf, i, class, simple)?
        .and_then(|u| u.as_any().downcast_ref::<JavaEnum>().map(|e| e.name)))
}

/// The written ones of LuaManager.GlobalObject's global functions this module owns.
pub fn global(name: &str) -> Option<Imp> {
    Some(match name {
        // SteamUtils.isSteamModeEnabled: `steamEnabled`, set by SteamUtils.init (the game's startup, with
        // -Dzomboid.steam=1); false before it.
        "getSteamModeActive" => |_, _, cf, n| {
            static_args(n, 0)?;
            ret_bool(cf, false)
        },
        "isDemo" => |_, _, cf, n| {
            static_args(n, 0)?;
            ret_bool(cf, false)
        },
        // BoxedStaticValues.toDouble(256.0) / (8.0)
        "getCellSizeInSquares" => |_, _, cf, n| {
            static_args(n, 0)?;
            ret(cf, Object::Number(256.0))
        },
        "getChunkSizeInSquares" => |_, _, cf, n| {
            static_args(n, 0)?;
            ret(cf, Object::Number(8.0))
        },
        "getMaxActivePlayers" => |_, _, cf, n| {
            static_args(n, 0)?;
            ret_int(cf, 4)
        },
        // GameWindow.GameInput.getControllerCount() is the 16 controller slots of zombie.core.input.Input; a slot
        // holds a Controller once GLFW reports one (Input.checkConnectDisconnect, the game loop), so every slot is
        // null here and the count of a slot is 0.
        "getButtonCount" => |_, _, cf, n| {
            static_args(n, 1)?;
            int_arg(cf, 0)?;
            ret_int(cf, 0)
        },
        // JoypadManager.isJoypadConnected: `GameInput.getController(index) != null` for 0 <= index < 16.
        "isJoypadConnected" => |_, _, cf, n| {
            static_args(n, 1)?;
            int_arg(cf, 0)?;
            ret_bool(cf, false)
        },
        _ => return None,
    })
}

/// The static methods this module owns.
pub fn static_method(class: &str, name: &str) -> Option<Imp> {
    Some(match (class, name) {
        ("zombie.ai.states.CloseWindowState", "instance") => {
            |s, _, cf, _| ret(cf, the(s, "zombie.ai.states.CloseWindowState"))
        }
        ("zombie.ai.states.OpenWindowState", "instance") => {
            |s, _, cf, _| ret(cf, the(s, "zombie.ai.states.OpenWindowState"))
        }
        ("zombie.audio.MusicIntensityConfig", "getInstance") => {
            |s, _, cf, _| ret(cf, the(s, "zombie.audio.MusicIntensityConfig"))
        }
        ("zombie.audio.MusicThreatConfig", "getInstance") => {
            |s, _, cf, _| ret(cf, the(s, "zombie.audio.MusicThreatConfig"))
        }
        // The CharacterInputBindingSet[] becomes a new table 1..n (KahluaArrayConverter).
        ("zombie.characters.CharacterInputBindingSet", "getLoadedBindingSets") => |s, _, cf, n| {
            static_args(n, 0)?;
            let dir = s.cache_dir()?;
            let mut b = s.state.input_bindings.borrow_mut();
            let sets = b.get_loaded_binding_sets(&dir)?;
            ret(cf, binding_sets_table(s, sets))
        },
        ("zombie.characters.CharacterInputBindingSet", "reloadAll") => |s, _, cf, n| {
            static_args(n, 0)?;
            let dir = s.cache_dir()?;
            let mut b = s.state.input_bindings.borrow_mut();
            let sets = b.reload_all(&dir)?;
            ret(cf, binding_sets_table(s, sets))
        },
        ("zombie.characters.CharacterInputBindingSet", "resetAllToDefault") => |s, _, _, n| {
            static_args(n, 0)?;
            s.state.input_bindings.borrow_mut().reset_all_to_default();
            Ok(0)
        },
        ("zombie.characters.CharacterJoypadButtonBinding", "setAllToDefault") => |s, _, _, n| {
            static_args(n, 0)?;
            s.state
                .input_bindings
                .borrow_mut()
                .button_bindings
                .set_all_to_default();
            Ok(0)
        },
        ("zombie.characters.CharacterJoypadAxis2dBinding", "setAllToDefault") => |s, _, _, n| {
            static_args(n, 0)?;
            s.state
                .input_bindings
                .borrow_mut()
                .axis2d_bindings
                .set_all_to_default();
            Ok(0)
        },
        // Vector3f() only; the other 11 constructors are not ported.
        ("org.joml.Vector3f", "new") => |_, _, cf, n| {
            if n != 0 {
                return Err(Fail::Lua(Throwable::runtime(format!(
                    "not ported: org.joml.Vector3f.new with {n} arguments"
                ))));
            }
            ret(
                cf,
                ud(Rc::new(Vector3fObject(RefCell::new(Vector3f::default())))),
            )
        },
        // No system is registered before the game starts (registerSystem runs from OnCGlobalObjectSystemInit).
        ("zombie.globalObjects.CGlobalObjects", "getSystemCount") => |_, _, cf, n| {
            static_args(n, 0)?;
            ret_int(cf, 0)
        },
        ("zombie.globalObjects.CGlobalObjects", "getSystemByIndex") => |_, _, cf, n| {
            static_args(n, 1)?;
            int_arg(cf, 0)?;
            ret(cf, Object::Nil)
        },
        ("zombie.globalObjects.CGlobalObjects", "getSystemByName") => |_, _, cf, n| {
            static_args(n, 1)?;
            string_arg(cf, 0)?;
            ret(cf, Object::Nil)
        },
        _ => return None,
    })
}

/// KahluaArrayConverter: a Java array as a new table with the elements at 1..n. A loaded set has no Lua form yet
/// (none can be loaded, see CharacterInputBindingSet.reloadAll).
fn binding_sets_table(s: &Rc<JavaStatics>, sets: &[CharacterInputBindingSet]) -> Object {
    let t = s.platform.new_table();
    debug_assert!(sets.is_empty());
    Object::Table(t)
}

/// The instance methods this module owns.
pub fn instance_method(class: &str, name: &str) -> Option<Imp> {
    Some(match (class, name) {
        // ---- MusicIntensityConfig / MusicThreatConfig ----
        ("zombie.audio.MusicIntensityConfig", "initEvents") => |_, _, cf, n| {
            want_self_args(n, 1)?;
            let t = table_impl_arg(cf, 1)?;
            with_this!(cf, MusicIntensityConfigObject, o, {
                let t = t.ok_or_else(npe_java)?;
                o.0.borrow_mut().init_events(&t)?;
                Ok(0)
            })
        },
        ("zombie.audio.MusicThreatConfig", "initStatuses") => |_, _, cf, n| {
            want_self_args(n, 1)?;
            let t = table_impl_arg(cf, 1)?;
            with_this!(cf, MusicThreatConfigObject, o, {
                let t = t.ok_or_else(npe_java)?;
                o.0.borrow_mut().init_statuses(&t)?;
                Ok(0)
            })
        },
        ("zombie.audio.MusicThreatConfig", "getStatusCount") => |_, _, cf, n| {
            want_self_args(n, 0)?;
            with_this!(
                cf,
                MusicThreatConfigObject,
                o,
                ret_int(cf, o.0.borrow().get_status_count())
            )
        },
        ("zombie.audio.MusicThreatConfig", "getStatusIntensity") => |_, _, cf, n| {
            want_self_args(n, 1)?;
            let id = string_arg(cf, 1)?;
            with_this!(cf, MusicThreatConfigObject, o, {
                let v = id.map_or(0.0, |id| o.0.borrow().get_status_intensity(&id));
                ret_float(cf, v)
            })
        },
        ("zombie.audio.MusicThreatConfig", "getStatusIntensityOverride") => |_, _, cf, n| {
            want_self_args(n, 1)?;
            let id = string_arg(cf, 1)?;
            with_this!(cf, MusicThreatConfigObject, o, {
                let v = id.map_or(0.0, |id| o.0.borrow().get_status_intensity_override(&id));
                ret_float(cf, v)
            })
        },
        ("zombie.audio.MusicThreatConfig", "isStatusIntensityOverridden") => |_, _, cf, n| {
            want_self_args(n, 1)?;
            let id = string_arg(cf, 1)?;
            with_this!(cf, MusicThreatConfigObject, o, {
                let v = id.is_some_and(|id| o.0.borrow().is_status_intensity_overridden(&id));
                ret_bool(cf, v)
            })
        },
        ("zombie.audio.MusicThreatConfig", "setStatusIntensityOverride") => |_, _, cf, n| {
            want_self_args(n, 2)?;
            let id = string_arg(cf, 1)?;
            let v = float_arg(cf, 2)?;
            with_this!(cf, MusicThreatConfigObject, o, {
                if let Some(id) = id {
                    o.0.borrow().set_status_intensity_override(&id, v);
                }
                Ok(0)
            })
        },
        // ---- TextManager (no font before TextManager.Init) ----
        ("zombie.ui.TextManager", "getCurrentCodeFont") => |s, _, cf, n| {
            want_self_args(n, 0)?;
            with_this!(cf, TextManagerObject, t, {
                let font = t.0.borrow().get_current_code_font();
                ret(cf, s.field("zombie.ui.UIFont", font.name()))
            })
        },
        ("zombie.ui.TextManager", "getFontFromEnum") => |_, _, cf, n| {
            want_self_args(n, 1)?;
            let font = enum_arg(cf, 1, "zombie.ui.UIFont", "UIFont")?;
            with_this!(cf, TextManagerObject, t, {
                let font = font.as_ref().map(|u| UIFont::values()[ordinal_of(u)]);
                match t.0.borrow().get_font_from_enum(font) {
                    None => ret(cf, Object::Nil),
                    // Only reachable after TextManager.Init, which the Lua state never runs.
                    Some(_) => Err(Fail::Lua(Throwable::runtime(
                        "not ported: an AngelCodeFont as a Lua value".to_string(),
                    ))),
                }
            })
        },
        // ---- Core options ----
        ("zombie.core.Core", "getOptionFontSizeReal") => |_, _, cf, n| {
            want_self_args(n, 0)?;
            with_this!(
                cf,
                CoreObject,
                c,
                ret_int(cf, c.get_option_font_size_real())
            )
        },
        ("zombie.core.Core", "getOptionControllerButtonStyleString") => |_, _, cf, n| {
            want_self_args(n, 0)?;
            with_this!(
                cf,
                CoreObject,
                c,
                ret_str(cf, Some(c.get_option_controller_button_style_string()))
            )
        },
        ("zombie.core.Core", "getOptionGamepadBindingPreset") => |_, _, cf, n| {
            want_self_args(n, 0)?;
            with_this!(
                cf,
                CoreObject,
                c,
                ret_str(cf, Some(&c.option_gamepad_binding_preset))
            )
        },
        // ---- CharacterJoypadButtonBinding ----
        ("zombie.characters.CharacterJoypadButtonBinding", "setBinding") => {
            |s, _, cf, n| set_binding(s, cf, n)
        }
        ("zombie.characters.CharacterJoypadButtonBinding", "setDefault") => |s, _, cf, n| {
            want_self_args(n, 0)?;
            let o = ordinal_of(&enum_this(cf)?);
            s.state
                .input_bindings
                .borrow_mut()
                .button_bindings
                .set_default(o);
            Ok(0)
        },
        ("zombie.characters.CharacterJoypadButtonBinding", "getJoypadButton") => |s, _, cf, n| {
            want_self_args(n, 0)?;
            let o = ordinal_of(&enum_this(cf)?);
            let b = s
                .state
                .input_bindings
                .borrow()
                .button_bindings
                .get_joypad_button(o);
            ret(cf, enum_constant(s, "zombie.input.JoypadButton", b))
        },
        ("zombie.characters.CharacterJoypadButtonBinding", "getJoypadAxis1d") => |s, _, cf, n| {
            want_self_args(n, 0)?;
            let o = ordinal_of(&enum_this(cf)?);
            let b = s
                .state
                .input_bindings
                .borrow()
                .button_bindings
                .get_joypad_axis1d(o);
            ret(cf, enum_constant(s, "zombie.input.JoypadAxis1d", b))
        },
        ("zombie.characters.CharacterJoypadButtonBinding", "getJoypadAxis2d") => |s, _, cf, n| {
            want_self_args(n, 0)?;
            let o = ordinal_of(&enum_this(cf)?);
            let b = s
                .state
                .input_bindings
                .borrow()
                .button_bindings
                .get_joypad_axis2d(o);
            ret(cf, enum_constant(s, "zombie.input.JoypadAxis2d", b))
        },
        // ---- CharacterJoypadAxis2dBinding ----
        ("zombie.characters.CharacterJoypadAxis2dBinding", "getJoypadAxis" | "getBinding") => {
            |s, _, cf, n| {
                want_self_args(n, 0)?;
                let o = ordinal_of(&enum_this(cf)?);
                let b = s.state.input_bindings.borrow().axis2d_bindings.binding[o];
                ret(cf, enum_constant(s, "zombie.input.JoypadAxis2d", b))
            }
        }
        ("zombie.characters.CharacterJoypadAxis2dBinding", "setBinding") => |s, _, cf, n| {
            want_self_args(n, 1)?;
            let axis = enum_name(cf, 1, "zombie.input.JoypadAxis2d", "JoypadAxis2d")?;
            let o = ordinal_of(&enum_this(cf)?);
            s.state.input_bindings.borrow_mut().axis2d_bindings.binding[o] = axis;
            Ok(0)
        },
        // ---- DebugType (the debug log; kept with the loader's log) ----
        ("zombie.debug.DebugType", "debugln" | "warn") => |s, _, cf, n| {
            if n < 2 {
                return Err(arg_count_error(1, n - 1));
            }
            let ty = cf.get(0).java_to_string();
            let msg = cf.get(1).java_to_string();
            s.loader.borrow_mut().log.push(format!("{ty}: {msg}"));
            Ok(0)
        },
        _ => return None,
    })
}

/// The argument count check of an instance method with `params` parameters (after `self`).
fn want_self_args(n: i32, params: i32) -> Result<(), Fail> {
    super::exposed_methods::want_args(n, true, params)
}

/// A NullPointerException inside the method.
fn npe_java() -> Fail {
    super::exposed_methods::npe()
}

/// CharacterJoypadButtonBinding.setBinding's five overloads through MultiLuaJavaInvoker: the overloads sorted by
/// parameter count (most first); one whose parameters are all instances of the arguments wins, else one whose
/// arguments convert (a number to a float parameter, nil to an object). Argument combinations no overload takes
/// are not ported (the original throws the last overload's argument failure or "No implementation found").
fn set_binding(s: &Rc<JavaStatics>, cf: &FrameRef, n: i32) -> Ret {
    let this = enum_this(cf)?;
    let o = ordinal_of(&this);
    let is_enum = |i: i32, class: &str| match cf.get(i) {
        Object::Userdata(u) => u
            .as_any()
            .downcast_ref::<JavaEnum>()
            .is_some_and(|e| e.class.name == class),
        _ => false,
    };
    let mut b = s.state.input_bindings.borrow_mut();
    let b = &mut b.button_bindings;
    match n - 1 {
        // setBinding(JoypadButton): an instance, or nil (converts to null)
        1 if is_enum(1, "zombie.input.JoypadButton") || cf.get(1).is_nil() => {
            let button = enum_name(cf, 1, "zombie.input.JoypadButton", "JoypadButton")?;
            b.set_binding_button(o, button);
        }
        // setBinding(JoypadAxis1d, float [, float]) / setBinding(JoypadAxis2d, float [, float])
        args @ (2 | 3) if is_enum(1, "zombie.input.JoypadAxis1d") => {
            let axis = enum_name(cf, 1, "zombie.input.JoypadAxis1d", "JoypadAxis1d")?;
            let min = float_arg(cf, 2)?;
            let max = if args == 3 {
                float_arg(cf, 3)?
            } else {
                f32::MAX
            };
            b.set_binding_axis1d(o, axis, min, max);
        }
        args @ (2 | 3) if is_enum(1, "zombie.input.JoypadAxis2d") => {
            let axis = enum_name(cf, 1, "zombie.input.JoypadAxis2d", "JoypadAxis2d")?;
            let min = float_arg(cf, 2)?;
            let max = if args == 3 {
                float_arg(cf, 3)?
            } else {
                f32::MAX
            };
            b.set_binding_axis2d(o, axis, min, max);
        }
        _ => {
            return Err(Fail::Lua(Throwable::runtime(
                "not ported: CharacterJoypadButtonBinding.setBinding with these arguments",
            )))
        }
    }
    Ok(0)
}
