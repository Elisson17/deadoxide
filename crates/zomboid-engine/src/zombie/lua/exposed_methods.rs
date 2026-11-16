//! The Java side of the exposed API: what each exposed method the port has written does, the Java singletons the
//! scripts reach (SandboxOptions.instance, Core, BodyLocations, ...), and the conversion of arguments and results
//! (se.krka.kahlua.integration.expose.LuaJavaInvoker with KahluaConverterManager: a Lua number becomes the Java
//! number type, a Java number becomes a Lua number, null becomes nil).
//!
//! The state is the original's in a standalone LuaManager.init (the oracle scenario): no translations loaded
//! (Translator.loadFiles is the game's startup, not LuaManager's), no renderer (Texture.getSharedTexture gives
//! null), no item scripts (ScriptManager.instance is empty), not a client or a server, Core.debug off.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::exposer::{is_instance, Fail, Imp, JavaEnum, JavaObject, JavaStatics, Ret};
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
use crate::se::krka::kahlua::vm::object::{
    rstr, JavaClass, Object, TableRef, Throwable, Userdata, R,
};
use crate::zombie::characters::attached_items::attached_locations::{
    AttachedLocation, AttachedLocationGroup, AttachedLocations,
};
use crate::zombie::characters::worn_items::body_locations::{
    BodyLocation, BodyLocationGroup, BodyLocations,
};
use crate::zombie::core::textures::color_info::ColorInfo;
use crate::zombie::iso::vector2::Vector2;
use crate::zombie::lua::exposer::class_named;
use crate::zombie::lua::map_objects::MapObjects;
use crate::zombie::sandbox_options::SandboxOptions;
use crate::zombie::scripting::objects::resource_location::ResourceLocation;
use crate::zombie::world_map::symbols::map_symbol_definitions::{
    MapSymbolDefinition, MapSymbolDefinitions,
};

/// The Java state behind the exposed classes (statics in the original).
#[derive(Default)]
pub struct State {
    /// SandboxOptions.instance, made when the exposer reads the field.
    pub sandbox_options: RefCell<Option<Rc<SandboxOptionsObject>>>,
    /// Core.getInstance()
    pub core: RefCell<Option<Rc<CoreObject>>>,
    pub body_locations: RefCell<BodyLocations>,
    pub attached_locations: RefCell<AttachedLocations>,
    /// MapSymbolDefinitions.instance (made by the first getInstance()).
    pub map_symbol_definitions: RefCell<Option<Rc<MapSymbolDefinitions>>>,
    /// zombie.Lua.MapObjects' statics.
    pub map_objects: RefCell<MapObjects>,
    /// RandomizedWorldBase's static clutter lists, by field name (made when first returned).
    pub clutter: RefCell<HashMap<&'static str, Object>>,
    /// CharacterInputBindingSet.loadedBindingSets and the CharacterJoypad*Binding enum statics.
    pub input_bindings:
        RefCell<crate::zombie::characters::character_input_binding_set::InputBindings>,
}

/// zombie.SandboxOptions as a Lua value.
pub struct SandboxOptionsObject(pub RefCell<SandboxOptions>);

impl Userdata for SandboxOptionsObject {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.SandboxOptions")
    }
    fn java_to_string(&self) -> String {
        format!("zombie.SandboxOptions@{:x}", hash(self))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// zombie.core.Core, the part the scripts reach so far: the highlight colours and the options below (at their
/// defaults, as no options.ini is read in this state).
pub struct CoreObject {
    pub good_highlited_color: Rc<ColorInfo>,
    pub bad_highlited_color: Rc<ColorInfo>,
    /// optionGoodHighlightColor ("0.0,1.0,0.0") and optionBadHighlightColor ("1.0,0.0,0.0")
    pub option_good_highlight_color: [f64; 3],
    pub option_bad_highlight_color: [f64; 3],
    /// optionFontSize: `newOption("fontSize", 1, 6, 6)`, 6 is "automatic"
    pub option_font_size: i32,
    /// optionControllerButtonStyle: `newOption("controllerButtonStyle", 1, 3, 1, ...)`
    pub option_controller_button_style: i32,
    /// optionGamepadBindingPreset: `newOption("gamepadBindingPreset", "Original", ...)`
    pub option_gamepad_binding_preset: String,
}

/// zombie.core.Core.height (public static, 720 until the display is created).
pub const CORE_HEIGHT: i32 = 720;

impl CoreObject {
    /// zombie.core.Core.<init>, the fields above.
    fn new() -> CoreObject {
        CoreObject {
            good_highlited_color: Rc::new(ColorInfo::new(0.0, 1.0, 0.0, 1.0)),
            bad_highlited_color: Rc::new(ColorInfo::new(1.0, 0.0, 0.0, 1.0)),
            option_good_highlight_color: [0.0, 1.0, 0.0],
            option_bad_highlight_color: [1.0, 0.0, 0.0],
            option_font_size: 6,
            option_controller_button_style: 1,
            option_gamepad_binding_preset: "Original".into(),
        }
    }

    /// zombie.core.Core.getOptionFontSizeReal: size 6 ("automatic") picks 1..5 from the screen height
    /// (`getScreenHeight()` is Core.height), any other size is itself.
    pub fn get_option_font_size_real(&self) -> i32 {
        let mut size = self.option_font_size;
        if size == 6 {
            let scale =
                crate::zombie::core::math::pz_math::floor((CORE_HEIGHT - 1080) as f32 / 132.0)
                    as i32
                    + 1;
            let scale_clamp = crate::zombie::core::math::pz_math::clamp_i32(scale, 0, 10);
            match scale_clamp {
                0 => size = 1,
                1 => size = 2,
                2 | 3 => size = 3,
                4..=6 => size = 4,
                7..=10 => size = 5,
                _ => {}
            }
        }
        size
    }

    /// zombie.core.Core.getOptionControllerButtonStyleString
    pub fn get_option_controller_button_style_string(&self) -> &'static str {
        crate::zombie::input::controller_button_style::ControllerButtonStyle::from_option_code(
            self.option_controller_button_style,
        )
        .get_label()
    }
}

impl Userdata for CoreObject {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.core.Core")
    }
    fn java_to_string(&self) -> String {
        format!("zombie.core.Core@{:x}", hash(self))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// zombie.config.ArrayConfigOption.getValueAsColorInfo for a fixed-size-3 double option: the colour object set
/// from the three elements (cast to float) and alpha 1 (a missing fourth element), then returned.
fn value_as_color_info(option: &[f64; 3], color: &Rc<ColorInfo>) -> Rc<ColorInfo> {
    color.set(option[0] as f32, option[1] as f32, option[2] as f32, 1.0);
    color.clone()
}

/// zombie.scripting.ScriptManager.instance with no scripts loaded.
pub struct ScriptManagerObject;

impl Userdata for ScriptManagerObject {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.scripting.ScriptManager")
    }
    fn java_to_string(&self) -> String {
        format!("zombie.scripting.ScriptManager@{:x}", hash(self))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// java.util.ArrayList of Java objects (Lua values as the converter hands them over).
#[derive(Default)]
pub struct ArrayListObject(pub RefCell<Vec<Object>>);

impl Userdata for ArrayListObject {
    fn class(&self) -> &'static JavaClass {
        class_named("java.util.ArrayList")
    }
    /// AbstractCollection.toString: "[a, b]".
    fn java_to_string(&self) -> String {
        let items: Vec<String> = self.0.borrow().iter().map(Object::java_to_string).collect();
        format!("[{}]", items.join(", "))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// zombie.iso.Vector2 as a Lua value.
pub struct Vector2Object(pub RefCell<Vector2>);

impl Userdata for Vector2Object {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.iso.Vector2")
    }
    /// zombie.iso.Vector2.toString
    fn java_to_string(&self) -> String {
        let v = self.0.borrow();
        format!(
            "Vector2 (X: {}, Y: {}) (L: {})",
            fmt_f(v.x),
            fmt_f(v.y),
            fmt_f(v.get_length())
        )
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// `String.format("%f", f)` as Vector2.toString uses it (six decimals, half-up on the decimal value).
fn fmt_f(f: f32) -> String {
    format!("{:.6}", f64::from(f))
}

pub(super) fn hash<T>(t: &T) -> usize {
    (t as *const T as usize >> 4) & 0x7fff_ffff
}

/// Classes whose registry constants print as their ResourceLocation (`Registries.X.getLocation(this).toString()`).
const LOCATION_REGISTRIES: [&str; 4] = [
    "zombie.scripting.objects.CharacterTrait",
    "zombie.scripting.objects.CharacterProfession",
    "zombie.scripting.objects.ItemBodyLocation",
    "zombie.scripting.objects.ItemTag",
];

/// The `toString()` of a registry constant: its ResourceLocation (traits, professions, body locations, tags),
/// its id (KeybindId, PerkFactory.Perk) or "Base." + id (ItemKey); `None` for an opaque object.
pub fn object_to_string(o: &JavaObject) -> Option<String> {
    let name = o.class.name;
    if LOCATION_REGISTRIES.contains(&name) {
        return ResourceLocation::of(o.id).ok().map(|l| l.to_string());
    }
    match name {
        "zombie.input.KeybindId" | "zombie.characters.skills.PerkFactory$Perk" => {
            Some(o.id.to_string())
        }
        "zombie.scripting.objects.ItemKey" => Some(format!("Base.{}", o.id)),
        _ => None,
    }
}

/// The object a public static field holds when the exposer reads it. Reading `SandboxOptions.instance`
/// initialises SandboxOptions, whose constructor runs Lua (see `sandbox_options_new`).
pub fn static_object(
    statics: &Rc<JavaStatics>,
    thread: &mut KahluaThread,
    owner: &str,
    field: &str,
    class: &'static JavaClass,
    id: &'static str,
) -> R<Object> {
    Ok(match (owner, field, class.name) {
        ("zombie.SandboxOptions", "instance", _) => {
            Object::Userdata(sandbox_options_instance(statics, thread)?)
        }
        (_, "instance" | "INSTANCE", c) if c == owner => singleton(statics, class),
        (_, _, "java.util.ArrayList") => Object::Userdata(Rc::new(ArrayListObject::default())),
        _ => Object::Userdata(Rc::new(JavaObject { class, id })),
    })
}

/// The one object of a singleton class (its `instance` field, `getInstance()`, the global getter): the port's
/// state for the classes it has written, an opaque object for the others.
pub fn singleton(statics: &Rc<JavaStatics>, class: &'static JavaClass) -> Object {
    statics.singleton(class.name, || match class.name {
        "zombie.scripting.ScriptManager" => ud(Rc::new(ScriptManagerObject)),
        name => super::exposed_world::make_singleton(name)
            .or_else(|| super::exposed_client::make_singleton(name))
            .unwrap_or_else(|| Object::Userdata(Rc::new(JavaObject { class, id: "" }))),
    })
}

/// SandboxOptions.instance, made on first use.
fn sandbox_options_instance(
    statics: &Rc<JavaStatics>,
    thread: &mut KahluaThread,
) -> R<Rc<SandboxOptionsObject>> {
    if let Some(s) = statics.state.sandbox_options.borrow().as_ref() {
        return Ok(s.clone());
    }
    let s = Rc::new(SandboxOptionsObject(RefCell::new(sandbox_options_new(
        statics, thread,
    )?)));
    *statics.state.sandbox_options.borrow_mut() = Some(s.clone());
    Ok(s)
}

/// zombie.SandboxOptions.<init>: runs `media/lua/shared/defines.lua`, then loadGameFile("Apocalypse") (the preset
/// file run again even if loaded, its returned table read by every option), then setDefaultsToCurrentValues.
/// CustomSandboxOptions adds nothing without mods.
pub fn sandbox_options_new(
    statics: &Rc<JavaStatics>,
    thread: &mut KahluaThread,
) -> R<SandboxOptions> {
    let mut options = SandboxOptions::new();
    let defines = statics.loader.borrow().media_file("lua/shared/defines.lua");
    statics.run_lua(thread, &defines)?;
    sandbox_load_game_file(statics, thread, &mut options, "Apocalypse")?;
    options.set_defaults_to_current_values();
    Ok(options)
}

/// zombie.SandboxOptions.loadGameFile: `media/lua/shared/Sandbox/<preset>.lua` must return a table; a missing
/// file is fine; an error is logged (ExceptionLogger) and gives false.
pub fn sandbox_load_game_file(
    statics: &Rc<JavaStatics>,
    thread: &mut KahluaThread,
    options: &mut SandboxOptions,
    preset: &str,
) -> R<bool> {
    let file = statics
        .loader
        .borrow()
        .media_file(&format!("lua/shared/Sandbox/{preset}.lua"));
    if !std::path::Path::new(&file).exists() {
        return Ok(true);
    }
    statics
        .loader
        .borrow_mut()
        .remove_loaded(&file.replace('\\', "/"));
    match statics.run_lua(thread, &file)? {
        Object::Table(t) => {
            for i in 0..options.defs().len() {
                options.from_table(i, &t)?;
            }
            Ok(true)
        }
        _ => {
            let name = file.rsplit('/').next().unwrap_or(&file).to_string();
            statics.loader.borrow_mut().log.push(format!(
                "java.lang.RuntimeException: {name} must return a SandboxVars table"
            ));
            Ok(false)
        }
    }
}

// ---- arguments and results (LuaJavaInvoker + KahluaConverterManager) ----

/// The argument type failure of LuaJavaInvoker.prepareCall.
pub(super) fn type_error(expected: &str, got: &Object) -> Fail {
    let got = match got {
        Object::Number(_) => "Double".to_string(),
        Object::String(_) => "String".to_string(),
        Object::Boolean(_) => "Boolean".to_string(),
        Object::Table(_) => "KahluaTableImpl".to_string(),
        Object::Closure(_) => "LuaClosure".to_string(),
        o => o
            .class()
            .map(|c| {
                c.name
                    .rsplit(['.', '$'])
                    .next()
                    .unwrap_or(c.name)
                    .to_string()
            })
            .unwrap_or_default(),
    };
    Fail::Lua(Throwable::runtime(format!(
        "expected argument of type {expected}, got {got}"
    )))
}

/// The argument count failure of LuaJavaInvoker.prepareCall.
pub(super) fn arg_count_error(expected: i32, got: i32) -> Fail {
    Fail::Lua(Throwable::runtime(format!(
        "expected {expected} argument{}, got {got}",
        if expected == 1 { "" } else { "s" }
    )))
}

/// The argument count check of LuaJavaInvoker.prepareCall: exactly `params` arguments after `self`.
pub(super) fn want_args(n: i32, has_self: bool, params: i32) -> Result<(), Fail> {
    let got = n - i32::from(has_self);
    if got != params {
        return Err(arg_count_error(params, got));
    }
    Ok(())
}

/// A String parameter: a Lua string, or null for nil.
pub(super) fn string_arg(cf: &FrameRef, i: i32) -> Result<Option<String>, Fail> {
    match cf.get(i) {
        Object::Nil => Ok(None),
        Object::String(s) => Ok(Some(rstr(&s))),
        o => Err(type_error("String", &o)),
    }
}

/// A String parameter the method dereferences: null is a NullPointerException.
pub(super) fn string_arg_nn(cf: &FrameRef, i: i32) -> Result<String, Fail> {
    string_arg(cf, i)?.ok_or_else(npe)
}

/// A NullPointerException in the method (logged by MethodCaller, nothing returned).
pub(super) fn npe() -> Fail {
    Fail::Java(Throwable::Java {
        class: "java.lang.NullPointerException",
        message: None,
    })
}

/// An int parameter: a Lua number through Double.intValue (KahluaNumberConverter).
pub(super) fn int_arg(cf: &FrameRef, i: i32) -> Result<i32, Fail> {
    match cf.get(i) {
        Object::Number(d) => Ok(d as i32),
        o => Err(type_error("int", &o)),
    }
}

/// A float parameter: a Lua number through Double.floatValue.
pub(super) fn float_arg(cf: &FrameRef, i: i32) -> Result<f32, Fail> {
    match cf.get(i) {
        Object::Number(d) => Ok(d as f32),
        o => Err(type_error("float", &o)),
    }
}

pub(super) fn bool_arg(cf: &FrameRef, i: i32) -> Result<bool, Fail> {
    match cf.get(i) {
        Object::Boolean(b) => Ok(b),
        o => Err(type_error("boolean", &o)),
    }
}

/// A KahluaTable parameter: a Lua table, or null for nil.
pub(super) fn table_arg(cf: &FrameRef, i: i32) -> Result<Option<TableRef>, Fail> {
    match cf.get(i) {
        Object::Nil => Ok(None),
        Object::Table(t) => Ok(Some(t)),
        o => Err(type_error("KahluaTable", &o)),
    }
}

/// The receiver of an instance method (the invoker has checked its class).
pub(super) fn this<T: 'static>(cf: &FrameRef) -> Result<Rc<dyn Userdata>, Fail> {
    match cf.get(0) {
        Object::Userdata(u) if u.as_any().is::<T>() => Ok(u),
        o => Err(type_error(std::any::type_name::<T>(), &o)),
    }
}

pub(super) fn ud<T: Userdata + 'static>(x: Rc<T>) -> Object {
    Object::Userdata(x)
}

pub(super) fn ret(cf: &FrameRef, o: Object) -> Ret {
    cf.push(o).map_err(Fail::Lua)?;
    Ok(1)
}

pub(super) fn ret_opt<T: Userdata + 'static>(cf: &FrameRef, o: Option<Rc<T>>) -> Ret {
    ret(cf, o.map_or(Object::Nil, ud))
}

pub(super) fn ret_str(cf: &FrameRef, s: Option<&str>) -> Ret {
    ret(cf, s.map_or(Object::Nil, Object::str))
}

pub(super) fn ret_int(cf: &FrameRef, v: i32) -> Ret {
    ret(cf, Object::Number(f64::from(v)))
}

pub(super) fn ret_float(cf: &FrameRef, v: f32) -> Ret {
    ret(cf, Object::Number(f64::from(v)))
}

pub(super) fn ret_bool(cf: &FrameRef, v: bool) -> Ret {
    ret(cf, Object::Boolean(v))
}

/// Runs `$body` with `$x` bound to the receiver of type `$t`.
macro_rules! with_this {
    ($cf:expr, $t:ty, $x:ident, $body:expr) => {{
        let u = $crate::zombie::lua::exposed_methods::this::<$t>($cf)?;
        let $x = u
            .as_any()
            .downcast_ref::<$t>()
            .ok_or_else($crate::zombie::lua::exposed_methods::npe)?;
        #[allow(clippy::let_and_return)]
        let r = $body;
        r
    }};
}
pub(super) use with_this;

// ---- global functions (LuaManager.GlobalObject) ----

/// The written ones of LuaManager.GlobalObject's `@LuaMethod(global = true)` functions.
pub fn global(name: &str) -> Option<Imp> {
    if let Some(f) = super::exposed_world::global(name) {
        return Some(f);
    }
    if let Some(f) = super::exposed_client::global(name) {
        return Some(f);
    }
    Some(match name {
        "getSandboxOptions" => |s, t, cf, _| {
            let o = sandbox_options_instance(s, t)?;
            ret(cf, ud(o))
        },
        "getCore" => |s, _, cf, _| ret(cf, ud(core(s))),
        "getDebug" => |s, _, cf, _| ret_bool(cf, s.debug),
        "isClient" | "isServer" | "isCoopHost" => |_, _, cf, _| ret_bool(cf, false),
        // System.getProperty("os.name").contains("OS X") / startsWith("Win") / contains("Linux")
        "isSystemMacOS" => |_, _, cf, _| ret_bool(cf, cfg!(target_os = "macos")),
        "isSystemWindows" => |_, _, cf, _| ret_bool(cf, cfg!(target_os = "windows")),
        "isSystemLinux" => |_, _, cf, _| ret_bool(cf, cfg!(target_os = "linux")),
        "getText" => |_, _, cf, n| {
            let desc = string_arg_nn(cf, 0)?;
            let text = translator_get_text(&desc, n > 1)?;
            ret_str(cf, Some(&text))
        },
        "getTextOrNull" => |_, _, cf, _| {
            string_arg_nn(cf, 0)?;
            ret(cf, Object::Nil)
        },
        // Texture.getSharedTexture: null without a renderer.
        "getTexture" | "tryGetTexture" => |_, _, cf, _| {
            string_arg(cf, 0)?;
            ret(cf, Object::Nil)
        },
        "copyTable" => |_, _, cf, n| {
            let t = if n >= 2 {
                copy_table(table_arg(cf, 0)?, table_arg(cf, 1)?.as_ref())?
            } else {
                copy_table(None, table_arg(cf, 0)?.as_ref())?
            };
            ret(cf, Object::Table(t))
        },
        // DebugType.debugln: the game's debug log, kept with the loader's log here.
        "log" => |s, _, cf, n| {
            want_args(n, false, 2)?;
            let ty = cf.get(0).java_to_string();
            let msg = string_arg(cf, 1)?.unwrap_or_else(|| "null".into());
            s.loader.borrow_mut().log.push(format!("{ty}: {msg}"));
            Ok(0)
        },
        _ => return None,
    })
}

/// zombie.core.Translator.getText in a state where no translation file is loaded: every key is missing, so the
/// key itself is the text, formatted with the arguments (`String.formatted`, which a key without `%` leaves as
/// it is) and with `<br>` turned into newlines.
fn translator_get_text(desc: &str, _has_args: bool) -> Result<String, Fail> {
    if desc.contains('%') {
        return Err(Fail::Lua(Throwable::runtime(format!(
            "not ported: Translator.getText formatting of \"{desc}\""
        ))));
    }
    Ok(if desc.contains("<br>") || desc.contains("<BR>") {
        desc.replace("<br>", "\n").replace("<BR>", "\n")
    } else {
        desc.to_string()
    })
}

/// zombie.Lua.LuaManager.copyTable: `to` wiped (or a new table), then every entry of `from`, tables copied deeply.
pub fn copy_table(to: Option<TableRef>, from: Option<&TableRef>) -> R<TableRef> {
    let to = match to {
        Some(t) => {
            t.borrow_mut().wipe();
            t
        }
        None => KahluaTableImpl::new_ref(),
    };
    if let Some(from) = from {
        for (k, v) in KahluaTableImpl::iterate_snapshot(from) {
            let v = match v {
                Object::Table(t) => Object::Table(copy_table(None, Some(&t))?),
                v => v,
            };
            KahluaTableImpl::rawset(&to, k, v);
        }
    }
    Ok(to)
}

/// Core.getInstance()
fn core(s: &Rc<JavaStatics>) -> Rc<CoreObject> {
    s.state
        .core
        .borrow_mut()
        .get_or_insert_with(|| Rc::new(CoreObject::new()))
        .clone()
}

// ---- methods ----

/// The written method `name` of class `class` (instance or static), if any.
pub fn method(class: &str, name: &str, has_self: bool) -> Option<Imp> {
    if has_self {
        if let Some(f) = common_instance_method(class, name) {
            return Some(f);
        }
        return instance_method(class, name);
    }
    static_method(class, name)
}

/// Methods every exposed object answers the same way: an enum's name/ordinal/toString, a registry constant's
/// toString and identity equals.
fn common_instance_method(class: &str, name: &str) -> Option<Imp> {
    let registry = LOCATION_REGISTRIES.contains(&class)
        || class == "zombie.input.KeybindId"
        || class == "zombie.scripting.objects.ItemKey";
    Some(match name {
        "name" => |_, _, cf, _| match cf.get(0) {
            Object::Userdata(u) => match u.as_any().downcast_ref::<JavaEnum>() {
                Some(e) => ret_str(cf, Some(e.name)),
                None => Err(npe()),
            },
            _ => Err(npe()),
        },
        "ordinal" => |_, _, cf, _| match cf.get(0) {
            Object::Userdata(u) => match u.as_any().downcast_ref::<JavaEnum>() {
                Some(e) => ret_int(cf, e.ordinal),
                None => Err(npe()),
            },
            _ => Err(npe()),
        },
        "toString" => |_, _, cf, _| ret(cf, Object::str(&cf.get(0).java_to_string())),
        "equals" if registry || is_enum_class(class) => |_, _, cf, _| {
            let same = cf.get(0).same(&cf.get(1));
            ret_bool(cf, same)
        },
        _ => return None,
    })
}

fn is_enum_class(class: &str) -> bool {
    super::exposed_table::CLASSES.iter().any(|c| {
        super::exposed_table::JAVA_CLASSES[usize::from(c.class)].name == class
            && c.statics
                .iter()
                .any(|(_, s)| matches!(s, super::exposer::Static::Enum(..)))
    })
}

/// The registry id of a JavaObject receiver.
fn registry_id(cf: &FrameRef) -> Result<&'static str, Fail> {
    match cf.get(0) {
        Object::Userdata(u) => u
            .as_any()
            .downcast_ref::<JavaObject>()
            .map(|o| o.id)
            .ok_or_else(npe),
        _ => Err(npe()),
    }
}

fn static_method(class: &str, name: &str) -> Option<Imp> {
    if let Some(f) = super::exposed_world::static_method(class, name) {
        return Some(f);
    }
    if let Some(f) = super::exposed_client::static_method(class, name) {
        return Some(f);
    }
    Some(match (class, name) {
        ("zombie.characters.WornItems.BodyLocations", "getGroup") => |s, _, cf, n| {
            want_args(n, false, 1)?;
            let id = string_arg_nn(cf, 0)?;
            let g = s.state.body_locations.borrow_mut().get_group(&id)?;
            ret(cf, ud(g))
        },
        ("zombie.characters.AttachedItems.AttachedLocations", "getGroup") => |s, _, cf, n| {
            want_args(n, false, 1)?;
            let id = string_arg_nn(cf, 0)?;
            let g = s.state.attached_locations.borrow_mut().get_group(&id)?;
            ret(cf, ud(g))
        },
        ("zombie.worldMap.symbols.MapSymbolDefinitions", "getInstance") => |s, _, cf, _| {
            let m = s
                .state
                .map_symbol_definitions
                .borrow_mut()
                .get_or_insert_with(|| Rc::new(MapSymbolDefinitions::default()))
                .clone();
            ret(cf, ud(m))
        },
        ("zombie.core.Core", "getInstance") => |s, _, cf, _| ret(cf, ud(core(s))),
        // ArrayList(), ArrayList(int), ArrayList(Collection)
        ("java.util.ArrayList", "new") => |_, _, cf, n| {
            let list = ArrayListObject::default();
            if n == 1 {
                match cf.get(0) {
                    Object::Number(d) if d < 0.0 => {
                        return Err(Throwable::Java {
                            class: "java.lang.IllegalArgumentException",
                            message: Some(format!("Illegal Capacity: {}", d as i32)),
                        }
                        .into())
                    }
                    Object::Number(_) => {}
                    Object::Userdata(u) => match u.as_any().downcast_ref::<ArrayListObject>() {
                        Some(other) => *list.0.borrow_mut() = other.0.borrow().clone(),
                        None => return Err(type_error("Collection", &Object::Userdata(u.clone()))),
                    },
                    o => return Err(type_error("int", &o)),
                }
            }
            ret(cf, ud(Rc::new(list)))
        },
        // Vector2(), Vector2(float, float), Vector2(Vector2)
        ("zombie.iso.Vector2", "new") => |_, _, cf, n| {
            let v = match n {
                0 => Vector2::default(),
                2 => Vector2::new(float_arg(cf, 0)?, float_arg(cf, 1)?),
                _ => match cf.get(0) {
                    Object::Userdata(u) => match u.as_any().downcast_ref::<Vector2Object>() {
                        Some(o) => *o.0.borrow(),
                        None => return Err(type_error("Vector2", &Object::Userdata(u.clone()))),
                    },
                    o => return Err(type_error("Vector2", &o)),
                },
            };
            ret(cf, ud(Rc::new(Vector2Object(RefCell::new(v)))))
        },
        // ColorInfo(), ColorInfo(float, float, float, float)
        ("zombie.core.textures.ColorInfo", "new") => |_, _, cf, n| {
            let c = if n == 0 {
                ColorInfo::new(1.0, 1.0, 1.0, 1.0)
            } else {
                want_args(n, false, 4)?;
                ColorInfo::new(
                    float_arg(cf, 0)?,
                    float_arg(cf, 1)?,
                    float_arg(cf, 2)?,
                    float_arg(cf, 3)?,
                )
            };
            ret(cf, ud(Rc::new(c)))
        },
        _ => return None,
    })
}

fn location_arg(cf: &FrameRef, i: i32) -> Object {
    cf.get(i)
}

fn instance_method(class: &str, name: &str) -> Option<Imp> {
    if let Some(f) = super::exposed_world::instance_method(class, name) {
        return Some(f);
    }
    if let Some(f) = super::exposed_client::instance_method(class, name) {
        return Some(f);
    }
    Some(match (class, name) {
        ("zombie.SandboxOptions", "initSandboxVars") => |s, _, cf, _| {
            let env = s.env()?;
            let vars = match KahluaTableImpl::rawget(&env, &Object::str("SandboxVars"))? {
                Object::Table(t) => t,
                Object::Nil => return Err(npe()),
                o => return Err(Throwable::class_cast(&o, "se.krka.kahlua.vm.KahluaTable").into()),
            };
            with_this!(
                cf,
                SandboxOptionsObject,
                o,
                o.0.borrow_mut().init_sandbox_vars(&vars)
            )?;
            Ok(0)
        },
        // ---- BodyLocationGroup ----
        ("zombie.characters.WornItems.BodyLocationGroup", "getId") => {
            |_, _, cf, _| with_this!(cf, BodyLocationGroup, g, ret_str(cf, Some(&g.id)))
        }
        ("zombie.characters.WornItems.BodyLocationGroup", "getLocation") => |_, _, cf, _| {
            with_this!(
                cf,
                BodyLocationGroup,
                g,
                ret_opt(cf, g.get_location(&location_arg(cf, 1)))
            )
        },
        ("zombie.characters.WornItems.BodyLocationGroup", "getOrCreateLocation") => {
            |_, _, cf, n| {
                want_args(n, true, 1)?;
                with_this!(cf, BodyLocationGroup, g, {
                    let l = g.get_or_create_location(&location_arg(cf, 1));
                    ret(cf, ud(l))
                })
            }
        }
        ("zombie.characters.WornItems.BodyLocationGroup", "getLocationByIndex") => |_, _, cf, _| {
            with_this!(
                cf,
                BodyLocationGroup,
                g,
                ret_opt(cf, g.get_location_by_index(int_arg(cf, 1)?))
            )
        },
        ("zombie.characters.WornItems.BodyLocationGroup", "moveLocationToIndex") => {
            |_, _, cf, _| {
                with_this!(cf, BodyLocationGroup, g, {
                    g.move_location_to_index(&location_arg(cf, 1), int_arg(cf, 2)?);
                    Ok(0)
                })
            }
        }
        ("zombie.characters.WornItems.BodyLocationGroup", "size") => {
            |_, _, cf, _| with_this!(cf, BodyLocationGroup, g, ret_int(cf, g.size()))
        }
        ("zombie.characters.WornItems.BodyLocationGroup", "indexOf") => |_, _, cf, _| {
            with_this!(
                cf,
                BodyLocationGroup,
                g,
                ret_int(cf, g.index_of(&location_arg(cf, 1)))
            )
        },
        ("zombie.characters.WornItems.BodyLocationGroup", "setExclusive") => |_, _, cf, n| {
            want_args(n, true, 2)?;
            with_this!(
                cf,
                BodyLocationGroup,
                g,
                g.set_exclusive(&location_arg(cf, 1), &location_arg(cf, 2))
            )?;
            Ok(0)
        },
        ("zombie.characters.WornItems.BodyLocationGroup", "isExclusive") => |_, _, cf, _| {
            let b = with_this!(
                cf,
                BodyLocationGroup,
                g,
                g.is_exclusive(&location_arg(cf, 1), &location_arg(cf, 2))
            )?;
            ret_bool(cf, b)
        },
        ("zombie.characters.WornItems.BodyLocationGroup", "setHideModel") => |_, _, cf, n| {
            want_args(n, true, 2)?;
            with_this!(
                cf,
                BodyLocationGroup,
                g,
                g.set_hide_model(&location_arg(cf, 1), &location_arg(cf, 2))
            )?;
            Ok(0)
        },
        ("zombie.characters.WornItems.BodyLocationGroup", "isHideModel") => |_, _, cf, _| {
            let b = with_this!(
                cf,
                BodyLocationGroup,
                g,
                g.is_hide_model(&location_arg(cf, 1), &location_arg(cf, 2))
            )?;
            ret_bool(cf, b)
        },
        ("zombie.characters.WornItems.BodyLocationGroup", "setAltModel") => |_, _, cf, n| {
            want_args(n, true, 2)?;
            with_this!(
                cf,
                BodyLocationGroup,
                g,
                g.set_alt_model(&location_arg(cf, 1), &location_arg(cf, 2))
            )?;
            Ok(0)
        },
        ("zombie.characters.WornItems.BodyLocationGroup", "isAltModel") => |_, _, cf, _| {
            let b = with_this!(
                cf,
                BodyLocationGroup,
                g,
                g.is_alt_model(&location_arg(cf, 1), &location_arg(cf, 2))
            )?;
            ret_bool(cf, b)
        },
        ("zombie.characters.WornItems.BodyLocationGroup", "setMultiItem") => |_, _, cf, n| {
            want_args(n, true, 2)?;
            let multi = bool_arg(cf, 2)?;
            with_this!(
                cf,
                BodyLocationGroup,
                g,
                g.set_multi_item(&location_arg(cf, 1), multi)
            )?;
            Ok(0)
        },
        ("zombie.characters.WornItems.BodyLocationGroup", "isMultiItem") => |_, _, cf, _| {
            let b = with_this!(
                cf,
                BodyLocationGroup,
                g,
                g.is_multi_item(&location_arg(cf, 1))
            )?;
            ret_bool(cf, b)
        },
        // ---- BodyLocation ----
        ("zombie.characters.WornItems.BodyLocation", "getId") => {
            |_, _, cf, _| with_this!(cf, BodyLocation, l, ret(cf, l.id.clone()))
        }
        ("zombie.characters.WornItems.BodyLocation", "isMultiItem") => {
            |_, _, cf, _| with_this!(cf, BodyLocation, l, ret_bool(cf, l.multi_item.get()))
        }
        ("zombie.characters.WornItems.BodyLocation", "setMultiItem") => |_, _, cf, _| {
            let multi = bool_arg(cf, 1)?;
            with_this!(cf, BodyLocation, l, l.multi_item.set(multi));
            ret(cf, cf.get(0))
        },
        ("zombie.characters.WornItems.BodyLocation", "setExclusive") => |_, _, cf, _| {
            with_this!(cf, BodyLocation, l, l.set_exclusive(&location_arg(cf, 1)));
            ret(cf, cf.get(0))
        },
        ("zombie.characters.WornItems.BodyLocation", "setHideModel") => |_, _, cf, _| {
            with_this!(cf, BodyLocation, l, l.set_hide_model(&location_arg(cf, 1)));
            ret(cf, cf.get(0))
        },
        ("zombie.characters.WornItems.BodyLocation", "setAltModel") => |_, _, cf, _| {
            with_this!(cf, BodyLocation, l, l.set_alt_model(&location_arg(cf, 1)));
            ret(cf, cf.get(0))
        },
        ("zombie.characters.WornItems.BodyLocation", "isExclusive") => |_, _, cf, _| {
            with_this!(
                cf,
                BodyLocation,
                l,
                ret_bool(cf, l.is_exclusive(&location_arg(cf, 1)))
            )
        },
        ("zombie.characters.WornItems.BodyLocation", "isHideModel") => |_, _, cf, _| {
            with_this!(
                cf,
                BodyLocation,
                l,
                ret_bool(cf, l.is_hide_model(&location_arg(cf, 1)))
            )
        },
        ("zombie.characters.WornItems.BodyLocation", "isAltModel") => |_, _, cf, _| {
            with_this!(
                cf,
                BodyLocation,
                l,
                ret_bool(cf, l.is_alt_model(&location_arg(cf, 1)))
            )
        },
        ("zombie.characters.WornItems.BodyLocation", "isId") => |_, _, cf, _| {
            with_this!(
                cf,
                BodyLocation,
                l,
                ret_bool(cf, l.is_id(&location_arg(cf, 1)))
            )
        },
        // ---- AttachedLocationGroup / AttachedLocation ----
        ("zombie.characters.AttachedItems.AttachedLocationGroup", "getLocation") => {
            |_, _, cf, _| {
                let id = string_arg(cf, 1)?;
                with_this!(
                    cf,
                    AttachedLocationGroup,
                    g,
                    ret_opt(cf, id.and_then(|id| g.get_location(&id)))
                )
            }
        }
        ("zombie.characters.AttachedItems.AttachedLocationGroup", "getOrCreateLocation") => {
            |_, _, cf, n| {
                want_args(n, true, 1)?;
                let id = string_arg_nn(cf, 1)?;
                let l = with_this!(cf, AttachedLocationGroup, g, g.get_or_create_location(&id))?;
                ret(cf, ud(l))
            }
        }
        ("zombie.characters.AttachedItems.AttachedLocationGroup", "getLocationByIndex") => {
            |_, _, cf, _| {
                let i = int_arg(cf, 1)?;
                with_this!(
                    cf,
                    AttachedLocationGroup,
                    g,
                    ret_opt(cf, g.get_location_by_index(i))
                )
            }
        }
        ("zombie.characters.AttachedItems.AttachedLocationGroup", "size") => {
            |_, _, cf, _| with_this!(cf, AttachedLocationGroup, g, ret_int(cf, g.size()))
        }
        ("zombie.characters.AttachedItems.AttachedLocationGroup", "indexOf") => |_, _, cf, _| {
            let id = string_arg(cf, 1)?;
            with_this!(
                cf,
                AttachedLocationGroup,
                g,
                ret_int(cf, id.map_or(-1, |id| g.index_of(&id)))
            )
        },
        ("zombie.characters.AttachedItems.AttachedLocationGroup", "checkValid") => |_, _, cf, _| {
            let id = string_arg_nn(cf, 1)?;
            with_this!(cf, AttachedLocationGroup, g, g.check_valid(&id))?;
            Ok(0)
        },
        ("zombie.characters.AttachedItems.AttachedLocation", "setAttachmentName") => {
            |_, _, cf, n| {
                want_args(n, true, 1)?;
                let name = string_arg(cf, 1)?;
                with_this!(cf, AttachedLocation, l, l.set_attachment_name(name))?;
                Ok(0)
            }
        }
        ("zombie.characters.AttachedItems.AttachedLocation", "getAttachmentName") => {
            |_, _, cf, _| {
                with_this!(cf, AttachedLocation, l, {
                    let name = l.attachment_name.borrow().clone();
                    ret_str(cf, name.as_deref())
                })
            }
        }
        ("zombie.characters.AttachedItems.AttachedLocation", "getId") => {
            |_, _, cf, _| with_this!(cf, AttachedLocation, l, ret_str(cf, Some(&l.id)))
        }
        // ---- MapSymbolDefinitions ----
        // addTexture(id, path), addTexture(id, path, tab), addTexture(id, path, width, height, tab)
        ("zombie.worldMap.symbols.MapSymbolDefinitions", "addTexture") => |_, _, cf, n| {
            let id = string_arg(cf, 1)?.unwrap_or_default();
            let path = string_arg(cf, 2)?.unwrap_or_default();
            with_this!(cf, MapSymbolDefinitions, m, {
                match n - 1 {
                    2 => m.add_texture_sized_by(&id, &path, None, false),
                    3 => m.add_texture_sized_by(&id, &path, string_arg(cf, 3)?.as_deref(), false),
                    5 => m.add_texture(
                        &id,
                        &path,
                        int_arg(cf, 3)?,
                        int_arg(cf, 4)?,
                        string_arg(cf, 5)?.as_deref(),
                    ),
                    got => return Err(arg_count_error(3, got)),
                }
                Ok(0)
            })
        },
        ("zombie.worldMap.symbols.MapSymbolDefinitions", "getSymbolCount") => |_, _, cf, _| {
            with_this!(
                cf,
                MapSymbolDefinitions,
                m,
                ret_int(cf, m.get_symbol_count())
            )
        },
        ("zombie.worldMap.symbols.MapSymbolDefinitions", "getSymbolByIndex") => |_, _, cf, _| {
            let i = int_arg(cf, 1)?;
            with_this!(cf, MapSymbolDefinitions, m, {
                let s = usize::try_from(i)
                    .ok()
                    .and_then(|i| m.symbol_list.borrow().get(i).cloned());
                match s {
                    Some(s) => ret(cf, ud(s)),
                    None => Err(Throwable::Java {
                        class: "java.lang.IndexOutOfBoundsException",
                        message: Some(format!(
                            "Index {i} out of bounds for length {}",
                            m.symbol_list.borrow().len()
                        )),
                    }
                    .into()),
                }
            })
        },
        ("zombie.worldMap.symbols.MapSymbolDefinitions", "getSymbolById") => |_, _, cf, _| {
            let id = string_arg(cf, 1)?;
            with_this!(cf, MapSymbolDefinitions, m, {
                let s = id.and_then(|id| m.symbol_by_id.borrow().get(&id).cloned());
                ret_opt(cf, s)
            })
        },
        ("zombie.worldMap.symbols.MapSymbolDefinitions$MapSymbolDefinition", "getId") => {
            |_, _, cf, _| with_this!(cf, MapSymbolDefinition, d, ret_str(cf, Some(&d.id)))
        }
        ("zombie.worldMap.symbols.MapSymbolDefinitions$MapSymbolDefinition", "getTexturePath") => {
            |_, _, cf, _| {
                with_this!(
                    cf,
                    MapSymbolDefinition,
                    d,
                    ret_str(cf, Some(&d.texture_path))
                )
            }
        }
        ("zombie.worldMap.symbols.MapSymbolDefinitions$MapSymbolDefinition", "getWidth") => {
            |_, _, cf, _| with_this!(cf, MapSymbolDefinition, d, ret_int(cf, d.width))
        }
        ("zombie.worldMap.symbols.MapSymbolDefinitions$MapSymbolDefinition", "getHeight") => {
            |_, _, cf, _| with_this!(cf, MapSymbolDefinition, d, ret_int(cf, d.height))
        }
        ("zombie.worldMap.symbols.MapSymbolDefinitions$MapSymbolDefinition", "getTab") => {
            |_, _, cf, _| with_this!(cf, MapSymbolDefinition, d, ret_str(cf, d.tab.as_deref()))
        }
        // ---- ScriptManager (no scripts in this state) ----
        ("zombie.scripting.ScriptManager", "FindItem" | "getItem") => |_, _, cf, _| {
            string_arg(cf, 1)?;
            ret(cf, Object::Nil)
        },
        // ---- Core ----
        ("zombie.core.Core", "getGoodHighlitedColor") => |_, _, cf, _| {
            with_this!(cf, CoreObject, c, {
                let col =
                    value_as_color_info(&c.option_good_highlight_color, &c.good_highlited_color);
                ret(cf, ud(col))
            })
        },
        ("zombie.core.Core", "getBadHighlitedColor") => |_, _, cf, _| {
            with_this!(cf, CoreObject, c, {
                let col =
                    value_as_color_info(&c.option_bad_highlight_color, &c.bad_highlited_color);
                ret(cf, ud(col))
            })
        },
        // ---- ColorInfo ----
        ("zombie.core.textures.ColorInfo", "getR") => {
            |_, _, cf, _| with_this!(cf, ColorInfo, c, ret_float(cf, c.r.get()))
        }
        ("zombie.core.textures.ColorInfo", "getG") => {
            |_, _, cf, _| with_this!(cf, ColorInfo, c, ret_float(cf, c.g.get()))
        }
        ("zombie.core.textures.ColorInfo", "getB") => {
            |_, _, cf, _| with_this!(cf, ColorInfo, c, ret_float(cf, c.b.get()))
        }
        ("zombie.core.textures.ColorInfo", "getA") => {
            |_, _, cf, _| with_this!(cf, ColorInfo, c, ret_float(cf, c.a.get()))
        }
        ("zombie.core.textures.ColorInfo", "interp") => |_, _, cf, n| {
            want_args(n, true, 3)?;
            let to = cf.get(1);
            let dest = cf.get(3);
            let delta = float_arg(cf, 2)?;
            let (Object::Userdata(to), Object::Userdata(dest)) = (to, dest) else {
                return Err(npe());
            };
            let (Some(to), Some(dest)) = (
                to.as_any().downcast_ref::<ColorInfo>(),
                dest.as_any().downcast_ref::<ColorInfo>(),
            ) else {
                return Err(npe());
            };
            with_this!(cf, ColorInfo, c, c.interp(to, delta, dest));
            Ok(0)
        },
        // ---- ArrayList ----
        // add(E) -> true, add(int, E)
        ("java.util.ArrayList", "add") => |_, _, cf, n| {
            with_this!(cf, ArrayListObject, l, {
                if n - 1 == 2 {
                    let i = int_arg(cf, 1)?;
                    let len = l.0.borrow().len();
                    match usize::try_from(i) {
                        Ok(i) if i <= len => l.0.borrow_mut().insert(i, cf.get(2)),
                        _ => {
                            return Err(Throwable::Java {
                                class: "java.lang.IndexOutOfBoundsException",
                                message: Some(format!("Index: {i}, Size: {len}")),
                            }
                            .into())
                        }
                    }
                    Ok(0)
                } else {
                    want_args(n, true, 1)?;
                    l.0.borrow_mut().push(cf.get(1));
                    ret_bool(cf, true)
                }
            })
        },
        ("java.util.ArrayList", "size") => |_, _, cf, _| {
            with_this!(
                cf,
                ArrayListObject,
                l,
                ret_int(cf, l.0.borrow().len() as i32)
            )
        },
        ("java.util.ArrayList", "isEmpty") => |_, _, cf, _| {
            with_this!(
                cf,
                ArrayListObject,
                l,
                ret_bool(cf, l.0.borrow().is_empty())
            )
        },
        ("java.util.ArrayList", "clear") => |_, _, cf, _| {
            with_this!(cf, ArrayListObject, l, l.0.borrow_mut().clear());
            Ok(0)
        },
        ("java.util.ArrayList", "get") => |_, _, cf, _| {
            let i = int_arg(cf, 1)?;
            with_this!(cf, ArrayListObject, l, {
                let len = l.0.borrow().len();
                match usize::try_from(i) {
                    Ok(u) if u < len => {
                        let v = l.0.borrow()[u].clone();
                        ret(cf, v)
                    }
                    _ => Err(Throwable::Java {
                        class: "java.lang.IndexOutOfBoundsException",
                        message: Some(format!("Index {i} out of bounds for length {len}")),
                    }
                    .into()),
                }
            })
        },
        ("java.util.ArrayList", "contains") => |_, _, cf, _| {
            let x = cf.get(1);
            with_this!(
                cf,
                ArrayListObject,
                l,
                ret_bool(cf, l.0.borrow().iter().any(|y| java_equals(y, &x)))
            )
        },
        ("java.util.ArrayList", "indexOf") => |_, _, cf, _| {
            let x = cf.get(1);
            with_this!(cf, ArrayListObject, l, {
                let i =
                    l.0.borrow()
                        .iter()
                        .position(|y| java_equals(y, &x))
                        .map_or(-1, |i| i as i32);
                ret_int(cf, i)
            })
        },
        // ---- Vector2 ----
        ("zombie.iso.Vector2", "getX") => {
            |_, _, cf, _| with_this!(cf, Vector2Object, v, ret_float(cf, v.0.borrow().x))
        }
        ("zombie.iso.Vector2", "getY") => {
            |_, _, cf, _| with_this!(cf, Vector2Object, v, ret_float(cf, v.0.borrow().y))
        }
        ("zombie.iso.Vector2", "getLength") => |_, _, cf, _| {
            with_this!(
                cf,
                Vector2Object,
                v,
                ret_float(cf, v.0.borrow().get_length())
            )
        },
        // ---- registry constants ----
        ("zombie.input.KeybindId", "getId") => |_, _, cf, _| ret_str(cf, Some(registry_id(cf)?)),
        (
            "zombie.scripting.objects.CharacterTrait"
            | "zombie.scripting.objects.CharacterProfession",
            "getName",
        ) => |_, _, cf, _| {
            let id = registry_id(cf)?;
            let path = ResourceLocation::of(id).map_err(Throwable::runtime)?.path;
            ret_str(cf, Some(&path))
        },
        (
            "zombie.scripting.objects.ItemTag" | "zombie.scripting.objects.ItemBodyLocation",
            "getTranslationName",
        ) => |_, _, cf, _| ret_str(cf, Some(registry_id(cf)?)),
        ("zombie.scripting.objects.ItemKey", "id") => {
            |_, _, cf, _| ret_str(cf, Some(registry_id(cf)?))
        }
        _ => return None,
    })
}

/// `Object.equals` for list searches: numbers, strings and booleans by value, everything else by identity.
fn java_equals(a: &Object, b: &Object) -> bool {
    match (a, b) {
        (Object::Number(x), Object::Number(y)) => {
            x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan())
        }
        (Object::String(x), Object::String(y)) => x[..] == y[..],
        _ => a.same(b),
    }
}

/// `clazz.isInstance` for argument checks the methods above do not make themselves.
pub fn instance_of(o: &Object, class: &str) -> bool {
    is_instance(o, class_named(class))
}
