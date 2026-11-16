//! The exposed API the scripts reach in a client whose startup ran (GameWindow.init and initShared before LoadDirBase,
//! the luagame oracle): the fonts TextManager.Init loaded, as AngelCodeFont objects and through TextManager's
//! measuring methods. Same conventions as `exposed_methods`.
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use super::exposed_client::TextManagerObject;
use super::exposed_methods::SandboxOptionsObject;
use super::exposed_methods::{
    float_arg, hash, int_arg, npe, registry_id, ret, ret_int, ret_str, string_arg, ud, want_args,
    with_this,
};
use super::exposed_world::enum_arg;
use super::exposer::{class_named, Fail, Imp, JavaEnum, JavaStatics, Ret};
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::object::{JavaClass, Object, Throwable, Userdata};
use crate::zombie::core::textures::texture::{PngSize, Texture, TextureRef};
use crate::zombie::network::server_options::{
    Kind as ServerKind, ServerOption, ServerOptions, Value as ServerValue,
};
use crate::zombie::sandbox_options::{Kind as SandboxKind, Value as SandboxValue};
use crate::zombie::sandbox_options_table::OPTIONS;
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
    let shared = client.shared.get().ok_or_else(npe)?;
    with_this!(cf, ItemObject, o, f(&shared.scripts.items[o.0]))
}

/// zombie.network.ServerOptions as a Lua value, with its option objects (one per option, made on first use).
pub struct ServerOptionsObject {
    pub options: Rc<RefCell<ServerOptions>>,
    objects: RefCell<Vec<Option<Rc<ServerOptionObject>>>>,
}

impl Userdata for ServerOptionsObject {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.network.ServerOptions")
    }
    fn java_to_string(&self) -> String {
        format!("zombie.network.ServerOptions@{:x}", hash(self))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ServerOptionsObject {
    /// The Lua value of option `i`: the same object each time.
    fn option(&self, i: usize) -> Object {
        let mut objects = self.objects.borrow_mut();
        if objects.len() <= i {
            objects.resize(i + 1, None);
        }
        let o = objects[i].get_or_insert_with(|| {
            Rc::new(ServerOptionObject {
                options: self.options.clone(),
                index: i,
            })
        });
        Object::Userdata(o.clone())
    }
}

/// A ServerOptions$*ServerOption of a ServerOptions object.
pub struct ServerOptionObject {
    pub options: Rc<RefCell<ServerOptions>>,
    pub index: usize,
}

impl Userdata for ServerOptionObject {
    fn class(&self) -> &'static JavaClass {
        let o = self.options.borrow();
        class_named(match o.options[self.index].def.kind {
            ServerKind::Boolean(_) => "zombie.network.ServerOptions$BooleanServerOption",
            ServerKind::Double { .. } => "zombie.network.ServerOptions$DoubleServerOption",
            ServerKind::Integer { .. } => "zombie.network.ServerOptions$IntegerServerOption",
            ServerKind::Enum { .. } => "zombie.network.ServerOptions$EnumServerOption",
            ServerKind::Str { .. } => "zombie.network.ServerOptions$StringServerOption",
            ServerKind::Text { .. } => "zombie.network.ServerOptions$TextServerOption",
        })
    }
    fn java_to_string(&self) -> String {
        format!("{}@{:x}", self.class().name, hash(self))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Runs `f` on a server option receiver.
fn with_server_option(
    cf: &FrameRef,
    f: impl FnOnce(&ServerOption, &ServerOptionObject) -> Ret,
) -> Ret {
    with_this!(cf, ServerOptionObject, o, {
        let options = o.options.borrow();
        f(&options.options[o.index], o)
    })
}

/// The translator of the startup (a server option's tooltip in a Lua state made on its own is not ported).
fn with_translator<T>(
    s: &Rc<JavaStatics>,
    f: impl FnOnce(&mut crate::zombie::core::translator::Translator) -> Result<T, String>,
) -> Result<T, Fail> {
    let client = s.state.client.borrow().clone().ok_or_else(|| {
        Fail::Lua(Throwable::runtime(
            "not ported: Translator before loadFiles",
        ))
    })?;
    let mut t = client.translator.borrow_mut();
    f(&mut t).map_err(|e| Fail::Lua(Throwable::runtime(e)))
}

/// A SandboxOptions$*SandboxOption of SandboxOptions.instance (its index). The two StrongEnumSandboxOption
/// options are EnumSandboxOption to Lua (the class is not exposed; Kahlua takes the superclass's metatable).
pub struct SandboxOptionObject(pub usize);

impl Userdata for SandboxOptionObject {
    fn class(&self) -> &'static JavaClass {
        class_named(match OPTIONS[self.0].kind {
            SandboxKind::Boolean(_) => "zombie.SandboxOptions$BooleanSandboxOption",
            SandboxKind::Double { .. } => "zombie.SandboxOptions$DoubleSandboxOption",
            SandboxKind::Integer { .. } => "zombie.SandboxOptions$IntegerSandboxOption",
            SandboxKind::Enum { .. } => "zombie.SandboxOptions$EnumSandboxOption",
            SandboxKind::Str { .. } => "zombie.SandboxOptions$StringSandboxOption",
        })
    }
    fn java_to_string(&self) -> String {
        format!("{}@{:x}", self.class().name, hash(self))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// The Lua value of option `i` of SandboxOptions.instance: the same object each time.
fn sandbox_option_object(s: &Rc<JavaStatics>, i: usize) -> Object {
    let o = s
        .state
        .sandbox_option_objects
        .borrow_mut()
        .entry(i)
        .or_insert_with(|| Rc::new(SandboxOptionObject(i)))
        .clone();
    Object::Userdata(o)
}

/// SandboxOptions.instance's options for a SandboxOptions receiver; another SandboxOptions object (made by
/// `SandboxOptions.new()`) is not ported here.
fn with_sandbox_instance(
    s: &Rc<JavaStatics>,
    cf: &FrameRef,
    f: impl FnOnce(&crate::zombie::sandbox_options::SandboxOptions) -> Ret,
) -> Ret {
    let instance = s.state.sandbox_options.borrow().clone().ok_or_else(npe)?;
    let receiver = super::exposed_methods::this::<SandboxOptionsObject>(cf)?;
    let same = receiver
        .as_any()
        .downcast_ref::<SandboxOptionsObject>()
        .is_some_and(|r| std::ptr::eq(r, &*instance));
    if !same {
        return Err(Fail::Lua(Throwable::runtime(
            "not ported: the options of a SandboxOptions other than SandboxOptions.instance",
        )));
    }
    let options = instance.0.borrow();
    f(&options)
}

/// Runs `f` on a sandbox option receiver: SandboxOptions.instance and the option's index.
fn with_sandbox_option(
    s: &Rc<JavaStatics>,
    cf: &FrameRef,
    f: impl FnOnce(&crate::zombie::sandbox_options::SandboxOptions, usize) -> Ret,
) -> Ret {
    let i = with_this!(cf, SandboxOptionObject, o, o.0);
    let instance = s.state.sandbox_options.borrow().clone().ok_or_else(npe)?;
    let options = instance.0.borrow();
    f(&options, i)
}

/// The methods every sandbox option class has (from ConfigOption and SandboxOptions.SandboxOption).
fn sandbox_option_method(name: &str) -> Option<Imp> {
    Some(match name {
        "asConfigOption" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_this!(cf, SandboxOptionObject, _o, ret(cf, cf.get(0)))
        },
        "getName" => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_sandbox_option(s, cf, |_, i| ret_str(cf, Some(OPTIONS[i].name)))
        },
        "getType" => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_sandbox_option(s, cf, |o, i| ret_str(cf, Some(o.get_type(i))))
        },
        "getValueAsString" => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_sandbox_option(s, cf, |o, i| ret_str(cf, Some(&o.value_as_string(i))))
        },
        // isCustom: only CustomSandboxOptions (mods) sets it
        "isCustom" => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_sandbox_option(s, cf, |_, _| ret(cf, Object::Boolean(false)))
        },
        // getPageName: only setPageName (custom options) sets it
        "getPageName" => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_sandbox_option(s, cf, |_, _| ret(cf, Object::Nil))
        },
        "getDefaultValue" => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_sandbox_option(s, cf, |o, i| match o.default_value(i) {
                SandboxValue::Boolean(b) => ret(cf, Object::Boolean(*b)),
                SandboxValue::Double(d) => ret(cf, Object::Number(*d)),
                SandboxValue::Integer(v) => ret_int(cf, *v),
                SandboxValue::Str(v) => ret_str(cf, Some(v)),
            })
        },
        "getTranslatedName" => |s, _, cf, n| {
            want_args(n, true, 0)?;
            let i = with_this!(cf, SandboxOptionObject, o, o.0);
            let instance = s.state.sandbox_options.borrow().clone().ok_or_else(npe)?;
            let name = with_translator(s, |t| instance.0.borrow().get_translated_name(i, t))?;
            ret_str(cf, Some(&name))
        },
        "getTooltip" => |s, _, cf, n| {
            want_args(n, true, 0)?;
            let i = with_this!(cf, SandboxOptionObject, o, o.0);
            let instance = s.state.sandbox_options.borrow().clone().ok_or_else(npe)?;
            let tip = with_translator(s, |t| instance.0.borrow().get_tooltip(i, t))?;
            ret_str(cf, tip.as_deref())
        },
        _ => return None,
    })
}

/// The value of a server option class's methods: every ServerOptions$*ServerOption.
const SERVER_OPTION_CLASSES: [&str; 6] = [
    "zombie.network.ServerOptions$BooleanServerOption",
    "zombie.network.ServerOptions$DoubleServerOption",
    "zombie.network.ServerOptions$IntegerServerOption",
    "zombie.network.ServerOptions$EnumServerOption",
    "zombie.network.ServerOptions$StringServerOption",
    "zombie.network.ServerOptions$TextServerOption",
];

/// The methods every server option class has (from ConfigOption and ServerOptions.ServerOption).
fn server_option_method(name: &str) -> Option<Imp> {
    Some(match name {
        // asConfigOption: the option itself
        "asConfigOption" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_this!(cf, ServerOptionObject, _o, ret(cf, cf.get(0)))
        },
        "getName" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_server_option(cf, |o, _| ret_str(cf, Some(o.def.name)))
        },
        "getType" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_server_option(cf, |o, _| ret_str(cf, Some(o.get_type())))
        },
        "getValueAsString" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_server_option(cf, |o, _| ret_str(cf, Some(&o.get_value_as_string())))
        },
        // getDefaultValue: boolean, double, int (a Lua number) or String
        "getDefaultValue" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_server_option(cf, |o, _| match &o.default {
                ServerValue::Boolean(b) => ret(cf, Object::Boolean(*b)),
                ServerValue::Double(d) => ret(cf, Object::Number(*d)),
                ServerValue::Integer(i) => ret_int(cf, *i),
                ServerValue::Str(v) => ret_str(cf, Some(v)),
            })
        },
        "getTooltip" => |s, _, cf, n| {
            want_args(n, true, 0)?;
            let (options, index) =
                with_this!(cf, ServerOptionObject, o, (o.options.clone(), o.index));
            let tip = with_translator(s, |t| options.borrow().options[index].get_tooltip(t))?;
            ret_str(cf, tip.as_deref())
        },
        _ => return None,
    })
}

pub fn global(name: &str) -> Option<Imp> {
    Some(match name {
        // Input.getKeyName(code)
        "getKeyName" => |s, _, cf, n| {
            want_args(n, false, 1)?;
            let code = int_arg(cf, 0)?;
            let Some(client) = s.state.client.borrow().clone() else {
                return Err(Fail::Lua(Throwable::runtime(
                    "not ported: getKeyName without a client startup",
                )));
            };
            let name = crate::zombie::core::input::input::get_key_name(
                &mut client.translator.borrow_mut(),
                code,
            )
            .map_err(|e| Fail::Lua(Throwable::runtime(e)))?;
            ret_str(cf, name.as_deref())
        },
        // TextureID.useFiltering = bUse: the filter new textures are made with (not observable headless)
        "useTextureFiltering" => |s, _, cf, n| {
            want_args(n, false, 1)?;
            let b = super::exposed_methods::bool_arg(cf, 0)?;
            s.state.use_texture_filtering.set(b);
            Ok(0)
        },
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

pub fn static_method(class: &str, name: &str) -> Option<Imp> {
    Some(match (class, name) {
        // ---- Translator ----
        // Translator.getAvailableLanguage: `Languages.instance.getLanguages()`, the same list every time
        ("zombie.core.Translator", "getAvailableLanguage") => |s, _, cf, n| {
            want_args(n, false, 0)?;
            let client = s.state.client.borrow().clone().ok_or_else(|| {
                Fail::Lua(Throwable::runtime(
                    "not ported: zombie.core.Translator.getAvailableLanguage".to_string(),
                ))
            })?;
            if let Some(list) = s.state.available_language.borrow().clone() {
                return ret(cf, ud(list));
            }
            let languages = client.translator.borrow_mut().get_available_language();
            let items = languages.iter().map(|l| language_object(s, l)).collect();
            let list = Rc::new(super::exposed_methods::ArrayListObject(RefCell::new(items)));
            *s.state.available_language.borrow_mut() = Some(list.clone());
            ret(cf, ud(list))
        },
        // Translator.getLanguage
        ("zombie.core.Translator", "getLanguage") => |s, _, cf, n| {
            want_args(n, false, 0)?;
            let client = s.state.client.borrow().clone().ok_or_else(|| {
                Fail::Lua(Throwable::runtime(
                    "not ported: zombie.core.Translator.getLanguage".to_string(),
                ))
            })?;
            let language = client.translator.borrow_mut().get_language();
            ret(cf, language_object(s, &language))
        },
        // ---- SystemDisabler ----
        ("zombie.SystemDisabler", "setDoCharacterStats") => |s, _, cf, n| {
            want_args(n, false, 1)?;
            let b = super::exposed_methods::bool_arg(cf, 0)?;
            s.state.system_disabler.borrow_mut().do_character_stats = b;
            Ok(0)
        },
        ("zombie.SystemDisabler", "setDoZombieCreation") => |s, _, cf, n| {
            want_args(n, false, 1)?;
            let b = super::exposed_methods::bool_arg(cf, 0)?;
            s.state.system_disabler.borrow_mut().do_zombie_creation = b;
            Ok(0)
        },
        ("zombie.SystemDisabler", "setDoSurvivorCreation") => |s, _, cf, n| {
            want_args(n, false, 1)?;
            let b = super::exposed_methods::bool_arg(cf, 0)?;
            s.state.system_disabler.borrow_mut().do_survivor_creation = b;
            Ok(0)
        },
        ("zombie.SystemDisabler", "setDoPlayerCreation") => |s, _, cf, n| {
            want_args(n, false, 1)?;
            let b = super::exposed_methods::bool_arg(cf, 0)?;
            s.state.system_disabler.borrow_mut().do_player_creation = b;
            Ok(0)
        },
        ("zombie.SystemDisabler", "setOverridePOVCharacters") => |s, _, cf, n| {
            want_args(n, false, 1)?;
            let b = super::exposed_methods::bool_arg(cf, 0)?;
            s.state
                .system_disabler
                .borrow_mut()
                .do_override_pov_characters = b;
            Ok(0)
        },
        ("zombie.SystemDisabler", "setVehiclesEverywhere") => |s, _, cf, n| {
            want_args(n, false, 1)?;
            let b = super::exposed_methods::bool_arg(cf, 0)?;
            s.state.system_disabler.borrow_mut().do_vehicles_everywhere = b;
            Ok(0)
        },
        ("zombie.SystemDisabler", "setWorldSyncEnable") => |s, _, cf, n| {
            want_args(n, false, 1)?;
            let b = super::exposed_methods::bool_arg(cf, 0)?;
            s.state.system_disabler.borrow_mut().do_world_sync_enable = b;
            Ok(0)
        },
        ("zombie.SystemDisabler", "setEnableAdvancedSoundOptions") => |s, _, cf, n| {
            want_args(n, false, 1)?;
            let b = super::exposed_methods::bool_arg(cf, 0)?;
            s.state
                .system_disabler
                .borrow_mut()
                .enable_advanced_sound_options = b;
            Ok(0)
        },
        ("zombie.SystemDisabler", "setUncappedFPS") => |s, _, cf, n| {
            want_args(n, false, 1)?;
            let b = super::exposed_methods::bool_arg(cf, 0)?;
            s.state.system_disabler.borrow_mut().uncapped_fps = b;
            Ok(0)
        },
        ("zombie.SystemDisabler", "getEnableAdvancedSoundOptions") => |s, _, cf, n| {
            want_args(n, false, 0)?;
            let b = s
                .state
                .system_disabler
                .borrow()
                .enable_advanced_sound_options;
            super::exposed_methods::ret_bool(cf, b)
        },
        ("zombie.SystemDisabler", "getUncappedFPS") => |s, _, cf, n| {
            want_args(n, false, 0)?;
            let b = s.state.system_disabler.borrow().uncapped_fps;
            super::exposed_methods::ret_bool(cf, b)
        },
        ("zombie.SystemDisabler", "getDoMainLoopDealWithNetData") => |s, _, cf, n| {
            want_args(n, false, 0)?;
            let b = s
                .state
                .system_disabler
                .borrow()
                .do_main_loop_deal_with_net_data;
            super::exposed_methods::ret_bool(cf, b)
        },
        (
            "zombie.SystemDisabler",
            "getdoHighFriction" | "getdoVehicleLowRider" | "printDetailedInfo",
        ) => |_, _, cf, n| {
            want_args(n, false, 0)?;
            super::exposed_methods::ret_bool(cf, false)
        },
        ("zombie.SystemDisabler", "Reset") => |s, _, _, n| {
            want_args(n, false, 0)?;
            s.state.system_disabler.borrow_mut().reset();
            Ok(0)
        },
        // Keyboard.getKeyName(code): `keyName[code]` (null until Keyboard.initKeyNames)
        ("org.lwjglx.input.Keyboard", "getKeyName") => |_, _, cf, n| {
            want_args(n, false, 1)?;
            let code = int_arg(cf, 0)?;
            let name = crate::zombie::core::input::input::keyboard_get_key_name(code)
                .map_err(|e| Fail::Lua(Throwable::runtime(e)))?;
            ret_str(cf, name.as_deref())
        },
        // MapGroups(): empty groups and directories
        ("zombie.MapGroups", "new") => |_, _, cf, n| {
            want_args(n, false, 0)?;
            ret(cf, ud(Rc::new(MapGroupsObject::default())))
        },
        // MapGroups.addMissingVanillaDirectories(mapName)
        ("zombie.MapGroups", "addMissingVanillaDirectories") => |s, _, cf, n| {
            want_args(n, false, 1)?;
            let name = string_arg(cf, 0)?.ok_or_else(npe)?;
            let client = s.state.client.borrow().clone().ok_or_else(|| {
                Fail::Lua(Throwable::runtime(
                    "not ported: zombie.MapGroups.addMissingVanillaDirectories without a client startup",
                ))
            })?;
            let r = crate::zombie::map_groups::add_missing_vanilla_directories(&client.fs, &name);
            ret_str(cf, Some(&r))
        },
        // ServerOptions(): its random defaults come from Rand (the startup's RandStandard)
        ("zombie.network.ServerOptions", "new") => |s, _, cf, n| {
            want_args(n, false, 0)?;
            let Some(client) = s.state.client.borrow().clone() else {
                return Err(Fail::Lua(Throwable::runtime(
                    "not ported: zombie.network.ServerOptions.new",
                )));
            };
            let options = ServerOptions::new(&mut client.rand_standard.borrow_mut());
            ret(
                cf,
                Object::Userdata(Rc::new(ServerOptionsObject {
                    options: Rc::new(RefCell::new(options)),
                    objects: RefCell::new(Vec::new()),
                })),
            )
        },
        _ => return None,
    })
}

pub fn instance_method(class: &str, name: &str) -> Option<Imp> {
    if class == "zombie.core.Language" {
        return language_method(name);
    }
    if class.starts_with("zombie.SandboxOptions$") {
        if let Some(f) = sandbox_option_method(name) {
            return Some(f);
        }
    }
    if SERVER_OPTION_CLASSES.contains(&class) {
        if let Some(f) = server_option_method(name) {
            return Some(f);
        }
    }
    Some(match (class, name) {
        // ---- MapGroups ----
        ("zombie.MapGroups", "getNumberOfGroups") => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_this!(cf, MapGroupsObject, m, {
                let v = m.0.borrow().get_number_of_groups();
                ret_int(cf, v)
            })
        },
        ("zombie.MapGroups", "getAllMapsInOrder") => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_this!(cf, MapGroupsObject, m, {
                let names = m.0.borrow().get_all_maps_in_order();
                ret(cf, string_list(&names))
            })
        },
        ("zombie.MapGroups", "getMapDirectoriesInGroup") => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let i = int_arg(cf, 1)?;
            with_this!(cf, MapGroupsObject, m, {
                let names =
                    m.0.borrow()
                        .get_map_directories_in_group(i)
                        .map_err(|e| Fail::Lua(Throwable::runtime(e)))?;
                ret(cf, string_list(&names))
            })
        },
        ("zombie.MapGroups", "getMapConflicts") => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let name = string_arg(cf, 1)?.ok_or_else(npe)?;
            with_this!(cf, MapGroupsObject, m, {
                match m.0.borrow().get_map_conflicts(&name) {
                    Some(c) => ret(cf, string_list(&c)),
                    None => ret(cf, Object::Nil),
                }
            })
        },
        // ---- DummySoundBank: BaseSoundBank.instance of a client without sound; every method does nothing ----
        ("zombie.audio.DummySoundBank", "addVoice") => |_, _, cf, n| {
            want_args(n, true, 3)?;
            string_arg(cf, 1)?;
            string_arg(cf, 2)?;
            float_arg(cf, 3)?;
            Ok(0)
        },
        ("zombie.audio.DummySoundBank", "addFootstep") => |_, _, cf, n| {
            want_args(n, true, 5)?;
            for i in 1..=5 {
                string_arg(cf, i)?;
            }
            Ok(0)
        },
        ("zombie.audio.DummySoundBank", "getVoice" | "getFootstep") => |_, _, cf, n| {
            want_args(n, true, 1)?;
            string_arg(cf, 1)?;
            ret(cf, Object::Nil)
        },
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
        // ---- SandboxOptions: the options of SandboxOptions.instance ----
        ("zombie.SandboxOptions", "getNumOptions") => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_sandbox_instance(s, cf, |o| ret_int(cf, o.get_num_options() as i32))
        },
        // getOptionByIndex: ArrayList.get (IndexOutOfBoundsException outside the list)
        ("zombie.SandboxOptions", "getOptionByIndex") => |s, _, cf, n| {
            want_args(n, true, 1)?;
            let i = int_arg(cf, 1)?;
            with_sandbox_instance(s, cf, |o| {
                let len = o.get_num_options();
                if i < 0 || i as usize >= len {
                    return Err(Fail::Java(Throwable::Java {
                        class: "java.lang.IndexOutOfBoundsException",
                        message: Some(format!("Index {i} out of bounds for length {len}")),
                    }));
                }
                Ok(0)
            })?;
            ret(cf, sandbox_option_object(s, i as usize))
        },
        ("zombie.SandboxOptions", "getOptionByName") => |s, _, cf, n| {
            want_args(n, true, 1)?;
            let name = string_arg(cf, 1)?;
            let mut found = None;
            with_sandbox_instance(s, cf, |o| {
                found = name.and_then(|name| o.get_option_by_name(&name));
                Ok(0)
            })?;
            match found {
                Some(i) => ret(cf, sandbox_option_object(s, i)),
                None => ret(cf, Object::Nil),
            }
        },
        // getFirstYear: 1993
        ("zombie.SandboxOptions", "getFirstYear") => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_sandbox_instance(s, cf, |_| Ok(0))?;
            ret_int(cf, 1993)
        },
        ("zombie.SandboxOptions$EnumSandboxOption", "getNumValues") => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_sandbox_option(s, cf, |o, i| ret_int(cf, o.get_num_values(i).unwrap_or(0)))
        },
        ("zombie.SandboxOptions$EnumSandboxOption", "getValueTranslationByIndex") => {
            |s, _, cf, n| {
                want_args(n, true, 1)?;
                let index = int_arg(cf, 1)?;
                let i = with_this!(cf, SandboxOptionObject, o, o.0);
                let instance = s.state.sandbox_options.borrow().clone().ok_or_else(npe)?;
                let client = s.state.client.borrow().clone().ok_or_else(npe)?;
                let r = {
                    let mut t = client.translator.borrow_mut();
                    instance
                        .0
                        .borrow()
                        .get_value_translation_by_index(i, index, false, &mut t)
                };
                match r {
                    Ok(v) => ret_str(cf, v.as_deref()),
                    Err(e) if e == "java.lang.ArrayIndexOutOfBoundsException" => {
                        Err(Fail::Java(Throwable::Java {
                            class: "java.lang.ArrayIndexOutOfBoundsException",
                            message: None,
                        }))
                    }
                    Err(e) => Err(Fail::Lua(Throwable::runtime(e))),
                }
            }
        }
        // StringConfigOption.getValue
        ("zombie.SandboxOptions$StringSandboxOption", "getValue") => |s, _, cf, n| {
            want_args(n, true, 0)?;
            with_sandbox_option(s, cf, |o, i| ret_str(cf, Some(&o.value_as_string(i))))
        },
        // ---- ServerOptions ----
        ("zombie.network.ServerOptions", "getNumOptions") => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_this!(cf, ServerOptionsObject, o, {
                let len = o.options.borrow().get_num_options();
                ret_int(cf, len as i32)
            })
        },
        // getOptionByIndex: ArrayList.get (IndexOutOfBoundsException outside the list)
        ("zombie.network.ServerOptions", "getOptionByIndex") => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let i = int_arg(cf, 1)?;
            with_this!(cf, ServerOptionsObject, o, {
                let len = o.options.borrow().get_num_options();
                if i < 0 || i as usize >= len {
                    return Err(Fail::Java(Throwable::Java {
                        class: "java.lang.IndexOutOfBoundsException",
                        message: Some(format!("Index {i} out of bounds for length {len}")),
                    }));
                }
                ret(cf, o.option(i as usize))
            })
        },
        ("zombie.network.ServerOptions", "getOptionByName") => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let name = string_arg(cf, 1)?;
            with_this!(cf, ServerOptionsObject, o, {
                let i = name.and_then(|name| o.options.borrow().get_option_by_name(&name));
                match i {
                    Some(i) => ret(cf, o.option(i)),
                    None => ret(cf, Object::Nil),
                }
            })
        },
        ("zombie.network.ServerOptions$EnumServerOption", "getNumValues") => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_server_option(cf, |o, _| ret_int(cf, o.get_num_values().unwrap_or(0)))
        },
        ("zombie.network.ServerOptions$EnumServerOption", "getValueTranslationByIndex") => {
            |s, _, cf, n| {
                want_args(n, true, 1)?;
                let index = int_arg(cf, 1)?;
                let (options, i) =
                    with_this!(cf, ServerOptionObject, o, (o.options.clone(), o.index));
                let r = {
                    let client = s.state.client.borrow().clone().ok_or_else(npe)?;
                    let mut t = client.translator.borrow_mut();
                    options.borrow().options[i].get_value_translation_by_index(&mut t, index)
                };
                match r {
                    Ok(v) => ret_str(cf, v.as_deref()),
                    Err(e) if e == "java.lang.ArrayIndexOutOfBoundsException" => {
                        Err(Fail::Java(Throwable::Java {
                            class: "java.lang.ArrayIndexOutOfBoundsException",
                            message: None,
                        }))
                    }
                    Err(e) => Err(Fail::Lua(Throwable::runtime(e))),
                }
            }
        }
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

// ---- the statics initShared and GameWindow.enter fill ----

/// The registry constants of `class` (`registerBase("Id")` fields of the exposed class), as locations.
pub(super) fn registry_locations(
    class: &str,
) -> std::collections::HashSet<crate::zombie::scripting::objects::resource_location::ResourceLocation>
{
    use super::exposed_table::{CLASSES, JAVA_CLASSES};
    use super::exposer::Static;
    use crate::zombie::scripting::objects::resource_location::ResourceLocation;
    let mut out = std::collections::HashSet::new();
    for e in CLASSES.iter() {
        if JAVA_CLASSES[e.class as usize].name != class {
            continue;
        }
        for (_, st) in e.statics {
            if let Static::Object(ci, id) = st {
                if JAVA_CLASSES[*ci as usize].name == class && !id.is_empty() {
                    if let Ok(l) = ResourceLocation::of(id) {
                        out.insert(l);
                    }
                }
            }
        }
    }
    out
}

/// GameWindow.TexturePack as a Lua value (an element of `GameWindow.texturePacks`; the class is not exposed).
pub struct TexturePackObject(pub String);

static TEXTURE_PACK_CLASS: JavaClass = JavaClass {
    name: "zombie.GameWindow$TexturePack",
    superclass: Some(&crate::se::krka::kahlua::vm::object::classes::OBJECT),
    interfaces: &[],
};

impl Userdata for TexturePackObject {
    fn class(&self) -> &'static JavaClass {
        &TEXTURE_PACK_CLASS
    }
    fn java_to_string(&self) -> String {
        format!("zombie.GameWindow$TexturePack@{:x}", hash(self))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// The ArrayList a public static field holds, as the exposer published it.
fn static_list(s: &Rc<JavaStatics>, class: &str, field: &str) -> Option<Rc<dyn Userdata>> {
    match s.field(class, field) {
        Object::Userdata(u) if u.as_any().is::<super::exposed_methods::ArrayListObject>() => {
            Some(u)
        }
        _ => None,
    }
}

/// `list.add(o)` on a static ArrayList.
fn list_add(list: &Option<Rc<dyn Userdata>>, o: Object) {
    if let Some(l) = list.as_ref().and_then(|u| {
        u.as_any()
            .downcast_ref::<super::exposed_methods::ArrayListObject>()
    }) {
        l.0.borrow_mut().push(o);
    }
}

/// The Item objects of ScriptManager.getAllItems that `hasTag(tag)` (`itemTags.contains`: an entry the ItemTag
/// registry does not hold is a null there, which only a null tag matches); none without a client's startup.
pub fn items_with_tag(s: &Rc<JavaStatics>, tag: &Object) -> Vec<Object> {
    use crate::zombie::scripting::objects::resource_location::ResourceLocation;
    let client = s.state.client.borrow().clone();
    let Some(shared) = client.as_ref().and_then(|c| c.shared.get()) else {
        return Vec::new();
    };
    let registered = registry_locations("zombie.scripting.objects.ItemTag");
    let want: Option<ResourceLocation> = match tag {
        Object::Userdata(u) => u
            .as_any()
            .downcast_ref::<super::exposer::JavaObject>()
            .and_then(|o| ResourceLocation::of(o.id).ok()),
        _ => None,
    };
    let has = |t: &ResourceLocation| match &want {
        Some(w) => registered.contains(t) && t == w,
        None => !registered.contains(t),
    };
    shared
        .all_items
        .iter()
        .filter(|i| shared.scripts.items[**i].tags.iter().any(has))
        .map(|i| item_object(s, *i))
        .collect()
}

/// zombie.MapGroups as a Lua value.
#[derive(Default)]
pub struct MapGroupsObject(pub RefCell<crate::zombie::map_groups::MapGroups>);

impl Userdata for MapGroupsObject {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.MapGroups")
    }
    fn java_to_string(&self) -> String {
        format!("zombie.MapGroups@{:x}", hash(self))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// A new java.util.ArrayList<String> (the converter hands the strings to Lua as they are).
fn string_list(items: &[String]) -> Object {
    Object::Userdata(Rc::new(super::exposed_methods::ArrayListObject(
        RefCell::new(items.iter().map(|s| Object::str(s)).collect()),
    )))
}

/// BaseSoundBank.instance: GameWindow.mainThreadInit's `new DummySoundBank()` (Core.soundDisabled) in a client
/// whose startup ran, else null.
pub fn base_sound_bank(s: &Rc<JavaStatics>) -> Object {
    if s.state.client.borrow().is_none() {
        return Object::Nil;
    }
    s.singleton("zombie.audio.BaseSoundBank", || {
        Object::Userdata(Rc::new(super::exposer::JavaObject {
            class: class_named("zombie.audio.DummySoundBank"),
            id: "",
        }))
    })
}

/// zombie.GameWindow.initShared on the Lua state of a client whose startup ran: the port's initShared
/// (`ClientStartup::init_shared`), with what it adds to the published statics: `GameWindow.texturePacks` (each
/// LoadTexturePack) and `PerkFactory.PerkList` (each AddPerk, the Perks constants themselves).
pub fn init_shared(s: &Rc<JavaStatics>) -> Result<(), String> {
    let Some(client) = s.state.client.borrow().clone() else {
        return Ok(());
    };
    let traits = registry_locations("zombie.scripting.objects.CharacterTrait");
    let professions = registry_locations("zombie.scripting.objects.CharacterProfession");
    let registries = crate::zombie::game_window::Registries {
        character_trait: &|l| traits.contains(l),
        character_profession: &|l| professions.contains(l),
    };
    client.init_shared(&registries)?;
    let packs = static_list(s, "zombie.GameWindow", "texturePacks");
    for p in crate::zombie::game_window::ClientStartup::UI_PACKS {
        list_add(
            &packs,
            Object::Userdata(Rc::new(TexturePackObject(p.to_string()))),
        );
    }
    let perk_list = static_list(s, "zombie.characters.skills.PerkFactory", "PerkList");
    if let Some(shared) = client.shared.get() {
        for p in &shared.perks.perk_list {
            list_add(
                &perk_list,
                s.field("zombie.characters.skills.PerkFactory$Perks", p.id),
            );
        }
    }
    Ok(())
}

/// zombie.GameWindow.enter's steps before OnGameBoot (`ClientStartup::enter`), with the tile packs it adds to
/// `GameWindow.texturePacks`.
pub fn enter(s: &Rc<JavaStatics>) -> Result<(), String> {
    let Some(client) = s.state.client.borrow().clone() else {
        return Ok(());
    };
    let packs = static_list(s, "zombie.GameWindow", "texturePacks");
    for p in crate::zombie::game_window::ClientStartup::TILE_PACKS {
        list_add(
            &packs,
            Object::Userdata(Rc::new(TexturePackObject(p.to_string()))),
        );
    }
    client.enter()
}

/// A static map the startup fills, as a Lua value: its size is read from the ported state each time.
pub struct StartupMapObject {
    pub class: &'static JavaClass,
    pub client: Rc<crate::zombie::game_window::ClientStartup>,
    pub which: StartupMap,
}

/// Which static map a StartupMapObject is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupMap {
    /// GameWindow.texturePackTextures
    TexturePackTextures,
    /// CharacterTraitDefinition.characterTraitDefinitions
    CharacterTraitDefinitions,
    /// CharacterProfessionDefinition.characterProfessionDefinitions
    CharacterProfessionDefinitions,
    /// Item.netIdToItem
    NetIdToItem,
    /// Item.netItemToId
    NetItemToId,
}

impl StartupMap {
    /// The map a public static field holds, for the ones the startup fills.
    pub fn of(owner: &str, field: &str) -> Option<StartupMap> {
        Some(match (owner, field) {
            ("zombie.GameWindow", "texturePackTextures") => StartupMap::TexturePackTextures,
            ("zombie.characters.traits.CharacterTraitDefinition", "characterTraitDefinitions") => {
                StartupMap::CharacterTraitDefinitions
            }
            (
                "zombie.characters.professions.CharacterProfessionDefinition",
                "characterProfessionDefinitions",
            ) => StartupMap::CharacterProfessionDefinitions,
            ("zombie.scripting.objects.Item", "netIdToItem") => StartupMap::NetIdToItem,
            ("zombie.scripting.objects.Item", "netItemToId") => StartupMap::NetItemToId,
            _ => return None,
        })
    }
}

impl StartupMapObject {
    /// `Map.size()`
    pub fn size(&self) -> usize {
        let c = &self.client;
        if self.which == StartupMap::TexturePackTextures {
            return c.textures.borrow().texture_pack_textures.len();
        }
        let Some(shared) = c.shared.get() else {
            return 0;
        };
        match self.which {
            StartupMap::TexturePackTextures => 0,
            StartupMap::CharacterTraitDefinitions => shared.traits.definitions.len(),
            StartupMap::CharacterProfessionDefinitions => shared.professions.definitions.len(),
            StartupMap::NetIdToItem => shared.scripts.item_net_id_counts().0,
            StartupMap::NetItemToId => shared.scripts.item_net_id_counts().1,
        }
    }
}

impl Userdata for StartupMapObject {
    fn class(&self) -> &'static JavaClass {
        self.class
    }
    fn java_to_string(&self) -> String {
        format!("{}@{:x}", self.class.name, hash(self))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// zombie.core.Language as a Lua value.
pub struct LanguageObject(pub Rc<crate::zombie::core::language::Language>);

impl Userdata for LanguageObject {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.core.Language")
    }
    /// Language.toString: its name.
    fn java_to_string(&self) -> String {
        self.0.name.clone()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// The one Language object of `language` (a Language is a single object in Java, equal to itself by identity).
fn language_object(
    s: &Rc<JavaStatics>,
    language: &Rc<crate::zombie::core::language::Language>,
) -> Object {
    let mut objects = s.state.language_objects.borrow_mut();
    if let Some((_, o)) = objects.iter().find(|(l, _)| Rc::ptr_eq(l, language)) {
        return ud(o.clone());
    }
    let o = Rc::new(LanguageObject(language.clone()));
    objects.push((language.clone(), o.clone()));
    ud(o)
}

/// zombie.core.Language's methods.
fn language_method(name: &str) -> Option<Imp> {
    Some(match name {
        "name" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_this!(cf, LanguageObject, l, ret_str(cf, Some(&l.0.name)))
        },
        "text" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_this!(cf, LanguageObject, l, ret_str(cf, l.0.text.as_deref()))
        },
        "base" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_this!(cf, LanguageObject, l, ret_str(cf, l.0.base.as_deref()))
        },
        "isAzerty" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_this!(
                cf,
                LanguageObject,
                l,
                super::exposed_methods::ret_bool(cf, l.0.azerty)
            )
        },
        "toString" => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_this!(cf, LanguageObject, l, ret_str(cf, Some(&l.0.name)))
        },
        _ => return None,
    })
}
