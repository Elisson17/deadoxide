//! The exposed API the game's boot events reach (OnLoadSoundBanks, OnGameBoot, OnGameStateEnter; measured with a
//! counting probe, docs/COMPATIBILITY.md): the sound bank and script manager globals, the character registries the
//! OnGameBoot handlers fill (ObservationFactory, SurvivorFactory's names, SurvivorDesc's colours), the trait and
//! profession definitions, PerkFactory's names, `instanceof`, TermsOfServiceState, Core's screen size and terms
//! version, and the UIElement a Lua window makes. Same conventions as `exposed_methods`.
use std::cell::RefCell;
use std::rc::Rc;

use super::exposed_client::static_args;
use super::exposed_methods::{
    core, hash, npe, ret, ret_bool, ret_int, ret_str, singleton, string_arg, translator_get_text,
    type_error, ud, want_args, with_this, ArrayListObject, CoreObject, ScriptManagerObject,
    CORE_HEIGHT, CORE_WIDTH,
};
use super::exposer::{class_named, Fail, Imp, JavaObject, JavaStatics, Ret};
use crate::java::float_to_string;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::object::{classes, JavaClass, Object, Throwable, Userdata};
use crate::zombie::characters::traits::observation_factory::ObservationFactory;
use crate::zombie::core::color::Color;
use crate::zombie::core::immutable_color::ImmutableColor;
use crate::zombie::core::textures::color_info::ColorInfo;
use crate::zombie::game_states::terms_of_service_state::TermsOfServiceState;

static COLOR_CLASS: JavaClass = JavaClass {
    name: "zombie.core.Color",
    superclass: Some(&classes::OBJECT),
    interfaces: &[],
};
static IMMUTABLE_COLOR_CLASS: JavaClass = JavaClass {
    name: "zombie.core.ImmutableColor",
    superclass: Some(&classes::OBJECT),
    interfaces: &[],
};

/// zombie.core.Color as a Lua value.
pub struct ColorObject(pub RefCell<Color>);

impl Userdata for ColorObject {
    fn class(&self) -> &'static JavaClass {
        &COLOR_CLASS
    }
    /// zombie.core.Color.toString: `"Color (" + r + "," + g + "," + b + "," + a + ")"`.
    fn java_to_string(&self) -> String {
        let c = self.0.borrow();
        format!(
            "Color ({},{},{},{})",
            float_to_string(c.r),
            float_to_string(c.g),
            float_to_string(c.b),
            float_to_string(c.a)
        )
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// zombie.core.ImmutableColor as a Lua value.
pub struct ImmutableColorObject(pub ImmutableColor);

impl Userdata for ImmutableColorObject {
    fn class(&self) -> &'static JavaClass {
        &IMMUTABLE_COLOR_CLASS
    }
    /// zombie.core.ImmutableColor.toString: `"ImmutableColor (" + r + "," + g + "," + b + "," + a + ")"`.
    fn java_to_string(&self) -> String {
        let c = &self.0;
        format!(
            "ImmutableColor ({},{},{},{})",
            float_to_string(c.r),
            float_to_string(c.g),
            float_to_string(c.b),
            float_to_string(c.a)
        )
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// ObservationFactory.observationMap (a java.util.HashMap) as a Lua value: the factory's state lives in it.
/// AbstractMap.toString is not reproduced (its values print identity hashes); this prints Object.toString.
#[derive(Default)]
pub struct ObservationMapObject(pub RefCell<ObservationFactory>);

impl Userdata for ObservationMapObject {
    fn class(&self) -> &'static JavaClass {
        class_named("java.util.HashMap")
    }
    fn java_to_string(&self) -> String {
        format!("java.util.HashMap@{:x}", hash(self))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// zombie.gameStates.TermsOfServiceState as a Lua value.
#[derive(Default)]
pub struct TermsOfServiceStateObject(pub RefCell<TermsOfServiceState>);

impl Userdata for TermsOfServiceStateObject {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.gameStates.TermsOfServiceState")
    }
    fn java_to_string(&self) -> String {
        format!("zombie.gameStates.TermsOfServiceState@{:x}", hash(self))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// The ObservationFactory state (the object in ObservationFactory.observationMap).
fn observations(s: &Rc<JavaStatics>) -> Result<Rc<dyn Userdata>, Fail> {
    match s.field(
        "zombie.characters.traits.ObservationFactory",
        "observationMap",
    ) {
        Object::Userdata(u) if u.as_any().is::<ObservationMapObject>() => Ok(u),
        _ => Err(npe()),
    }
}

/// `list.add(o)` on the ArrayList a public static field `class.field` holds.
fn add_to_static_list(s: &Rc<JavaStatics>, class: &str, field: &str, o: Object) -> Ret {
    match s.field(class, field) {
        Object::Userdata(u) => match u.as_any().downcast_ref::<ArrayListObject>() {
            Some(l) => {
                l.0.borrow_mut().push(o);
                Ok(0)
            }
            None => Err(npe()),
        },
        _ => Err(npe()),
    }
}

/// SurvivorFactory's add*name: the translated `prefix + name` (Java string concatenation: null is "null")
/// appended to the static list `field`.
fn survivor_name(s: &Rc<JavaStatics>, cf: &FrameRef, n: i32, prefix: &str, field: &str) -> Ret {
    static_args(n, 1)?;
    let name = string_arg(cf, 0)?;
    let text = translator_get_text(
        s,
        &format!("{prefix}{}", name.as_deref().unwrap_or("null")),
        &[],
    )?;
    add_to_static_list(
        s,
        "zombie.characters.SurvivorFactory",
        field,
        Object::str(&text),
    )
}

/// A ColorInfo parameter, or null for nil.
fn color_info_arg(cf: &FrameRef, i: i32) -> Result<Option<Rc<dyn Userdata>>, Fail> {
    match cf.get(i) {
        Object::Nil => Ok(None),
        Object::Userdata(u) if u.as_any().is::<ColorInfo>() => Ok(Some(u)),
        o => Err(type_error("ColorInfo", &o)),
    }
}

/// A parameter of a registry class (`class`, `simple` its simple name) whose constants are JavaObjects: the
/// object, or None for nil.
fn registry_arg(
    cf: &FrameRef,
    i: i32,
    class: &str,
    simple: &str,
) -> Result<Option<Rc<dyn Userdata>>, Fail> {
    match cf.get(i) {
        Object::Nil => Ok(None),
        Object::Userdata(u) if u.class().name == class => Ok(Some(u)),
        o => Err(type_error(simple, &o)),
    }
}

/// zombie.Lua.LuaManager.GlobalObject.instof (`instanceof`) for the objects the port has: false for null; a name
/// in the exposer's typeMap (each exposed class by the text after the last '.' of `Class.toString()`, plus
/// "function" and "table") is `isInstance`; else the LuaClosure / LuaJavaInvoker / KahluaTableImpl names. The
/// port's classes carry no superclass chain, so a userdata is an instance of its own class only: an instanceof
/// by a superclass or interface name answers false here (UNKNOWN whether the original agrees, not measured).
fn instance_of_name(o: &Object, name: &str) -> bool {
    match o {
        Object::Closure(_) => name == "function" || name == "LuaClosure",
        Object::Table(_) => name == "table" || name == "KahluaTableImpl",
        Object::Function(f) => {
            name == "LuaJavaInvoker"
                && f.class().name == "se.krka.kahlua.integration.expose.LuaJavaInvoker"
        }
        Object::Userdata(u) => u.class().name.rsplit('.').next() == Some(name),
        _ => false,
    }
}

/// The written ones of LuaManager.GlobalObject's global functions this module owns.
pub fn global(name: &str) -> Option<Imp> {
    Some(match name {
        // FMODSoundBank.instance (BaseSoundBank.instance): made by GameWindow.mainThreadInit (an FMODSoundBank, a
        // DummySoundBank with sound disabled, as in the `-nosound` client whose startup ran); null in a state
        // without that startup.
        "getFMODSoundBank" | "getBaseSoundBank" => |s, _, cf, n| {
            static_args(n, 0)?;
            ret(cf, super::exposed_game::base_sound_bank(s))
        },
        "getScriptManager" => |s, _, cf, n| {
            static_args(n, 0)?;
            ret(
                cf,
                singleton(s, class_named("zombie.scripting.ScriptManager")),
            )
        },
        "instanceof" => |_, _, cf, n| {
            static_args(n, 2)?;
            let name = string_arg(cf, 1)?;
            let o = cf.get(0);
            if o.is_nil() {
                return ret_bool(cf, false);
            }
            let name = name.ok_or_else(npe)?;
            ret_bool(cf, instance_of_name(&o, &name))
        },
        // null unless Core.debug and DebugType.ISUIStackTrace are on (the stack trace text is not ported).
        "getISUIStackTrace" => |s, _, cf, n| {
            static_args(n, 1)?;
            match cf.get(0) {
                Object::Number(_) => {}
                o => return Err(type_error("int", &o)),
            }
            if s.debug {
                return Err(Fail::Lua(Throwable::runtime(
                    "not ported: LuaManager$GlobalObject.getISUIStackTrace with Core.debug",
                )));
            }
            ret(cf, Object::Nil)
        },
        _ => return None,
    })
}

/// The static methods this module owns.
pub fn static_method(class: &str, name: &str) -> Option<Imp> {
    Some(match (class, name) {
        // `new ArrayList<>(characterTraitDefinitions.values())`: the map is filled by ScriptManager.Load (the
        // character_trait scripts), which this state does not run, so the list is new and empty.
        ("zombie.characters.traits.CharacterTraitDefinition", "getTraits")
        | ("zombie.characters.professions.CharacterProfessionDefinition", "getProfessions") => {
            |_, _, cf, n| {
                static_args(n, 0)?;
                ret(cf, ud(Rc::new(ArrayListObject::default())))
            }
        }
        ("zombie.characters.traits.ObservationFactory", "addObservation") => |s, _, cf, n| {
            static_args(n, 3)?;
            let (ty, name, desc) = (string_arg(cf, 0)?, string_arg(cf, 1)?, string_arg(cf, 2)?);
            let u = observations(s)?;
            if let Some(m) = u.as_any().downcast_ref::<ObservationMapObject>() {
                m.0.borrow_mut()
                    .add_observation(ty.as_deref(), name.as_deref(), desc.as_deref());
            }
            Ok(0)
        },
        ("zombie.characters.traits.ObservationFactory", "setMutualExclusive") => |s, _, cf, n| {
            static_args(n, 2)?;
            let (a, b) = (string_arg(cf, 0)?, string_arg(cf, 1)?);
            let u = observations(s)?;
            if let Some(m) = u.as_any().downcast_ref::<ObservationMapObject>() {
                m.0.borrow_mut()
                    .set_mutual_exclusive(a.as_deref(), b.as_deref())?;
            }
            Ok(0)
        },
        // SurvivorFactory.addSurname / addFemaleForename / addMaleForename:
        // `<list>.add(Translator.getText("SurvivorSurname_" / "SurvivorName_" + name))` (no translation file is
        // loaded in this state, so the text is the key).
        ("zombie.characters.SurvivorFactory", "addSurname") => {
            |s, _, cf, n| survivor_name(s, cf, n, "SurvivorSurname_", "Surnames")
        }
        ("zombie.characters.SurvivorFactory", "addFemaleForename") => {
            |s, _, cf, n| survivor_name(s, cf, n, "SurvivorName_", "FemaleForenames")
        }
        ("zombie.characters.SurvivorFactory", "addMaleForename") => {
            |s, _, cf, n| survivor_name(s, cf, n, "SurvivorName_", "MaleForenames")
        }
        // SurvivorDesc.addTrouserColor: `TrouserCommonColors.add(color.toColor())`.
        ("zombie.characters.SurvivorDesc", "addTrouserColor") => |s, _, cf, n| {
            static_args(n, 1)?;
            let u = color_info_arg(cf, 0)?.ok_or_else(npe)?;
            let c = u
                .as_any()
                .downcast_ref::<ColorInfo>()
                .ok_or_else(npe)?
                .to_color();
            let o = ud(Rc::new(ColorObject(RefCell::new(c))));
            add_to_static_list(
                s,
                "zombie.characters.SurvivorDesc",
                "TrouserCommonColors",
                o,
            )
        },
        // SurvivorDesc.addHairColor: `HairCommonColors.add(color.toImmutableColor())`.
        ("zombie.characters.SurvivorDesc", "addHairColor") => |s, _, cf, n| {
            static_args(n, 1)?;
            let u = color_info_arg(cf, 0)?.ok_or_else(npe)?;
            let c = u
                .as_any()
                .downcast_ref::<ColorInfo>()
                .ok_or_else(npe)?
                .to_immutable_color();
            let o = ud(Rc::new(ImmutableColorObject(c)));
            add_to_static_list(s, "zombie.characters.SurvivorDesc", "HairCommonColors", o)
        },
        // PerkFactory.getPerkName: `type.getName()`. A Perk's name is its id until PerkFactory.init translates
        // it (GameWindow.initShared, run only in a client's startup).
        ("zombie.characters.skills.PerkFactory", "getPerkName") => |s, _, cf, n| {
            static_args(n, 1)?;
            let u = registry_arg(cf, 0, "zombie.characters.skills.PerkFactory$Perk", "Perk")?
                .ok_or_else(npe)?;
            let id = u
                .as_any()
                .downcast_ref::<JavaObject>()
                .map(|o| o.id)
                .ok_or_else(npe)?;
            let client = s.state.client.borrow().clone();
            let name = match client.as_ref().and_then(|c| c.shared.get()) {
                Some(shared) => shared.perks.get_name(id).to_string(),
                None => id.to_string(),
            };
            ret(cf, Object::str(&name))
        },
        _ => return None,
    })
}

/// The instance methods this module owns.
pub fn instance_method(class: &str, name: &str) -> Option<Imp> {
    Some(match (class, name) {
        // ScriptManager.getItemsTag: getAllItemsWithTag, the list cached per tag: the items of getAllItems that
        // `hasTag(itemTag)`; with no scripts loaded (no client startup) every list is empty.
        ("zombie.scripting.ScriptManager", "getItemsTag") => |s, _, cf, n| {
            want_args(n, true, 1)?;
            let tag = registry_arg(cf, 1, "zombie.scripting.objects.ItemTag", "ItemTag")?
                .map_or(Object::Nil, Object::Userdata);
            with_this!(cf, ScriptManagerObject, m, {
                let found = m
                    .tag_to_item_map
                    .borrow()
                    .iter()
                    .find(|(k, _)| k.same(&tag))
                    .map(|(_, v)| v.clone());
                let list = match found {
                    Some(l) => l,
                    None => {
                        let items = super::exposed_game::items_with_tag(s, &tag);
                        let l = ud(Rc::new(ArrayListObject(RefCell::new(items))));
                        m.tag_to_item_map.borrow_mut().push((tag, l.clone()));
                        l
                    }
                };
                ret(cf, list)
            })
        },
        // Core.getScreenModes: the lines of <Lua cache dir>/screenresolution.ini at 1..n. A missing file is created
        // and filled from `Display.getAvailableDisplayModes()`; with no display (this headless state) there are no
        // modes (UNKNOWN whether that call throws: either way the table stays empty, the exception is logged and the
        // file stays empty).
        ("zombie.core.Core", "getScreenModes") => |s, _, cf, n| {
            want_args(n, true, 0)?;
            let t = s.platform.new_table();
            // LuaManager.getLuaCacheDir: <cache>/Lua, made with mkdir
            let dir = std::path::Path::new(&s.cache_dir()?).join("Lua");
            if !dir.exists() {
                let _ = std::fs::create_dir(&dir);
            }
            let file = dir.join("screenresolution.ini");
            if !file.exists() {
                // createNewFile; a failure (no Lua folder) is logged and leaves the table empty
                let _ = std::fs::File::create(&file);
            } else if let Ok(bytes) = std::fs::read(&file) {
                let text = String::from_utf8_lossy(&bytes).into_owned();
                let mut c = 1.0;
                let mut rest: &str = &text;
                while !rest.is_empty() {
                    let (line, tail) = match rest.find(['\n', '\r']) {
                        Some(i) => {
                            let skip = if rest[i..].starts_with("\r\n") { 2 } else { 1 };
                            (&rest[..i], &rest[i + skip..])
                        }
                        None => (rest, ""),
                    };
                    KahluaTableImpl::rawset(
                        &t,
                        Object::Number(c),
                        Object::str(crate::java::trim(line)),
                    );
                    c += 1.0;
                    rest = tail;
                }
            }
            ret(cf, Object::Table(t))
        },
        ("zombie.core.Core", "getVersion") => |_, _, cf, n| {
            want_args(n, true, 0)?;
            ret_str(cf, Some(&crate::zombie::core::core::get_version()))
        },
        ("zombie.core.Core", "getGameAndBuildVersion") => |_, _, cf, n| {
            want_args(n, true, 0)?;
            ret_str(
                cf,
                Some(&crate::zombie::core::core::get_game_and_build_version()),
            )
        },
        ("zombie.core.Core", "getBulletVersion") => |_, _, cf, n| {
            want_args(n, true, 0)?;
            ret_str(cf, Some(crate::zombie::core::core::BULLET_VERSION))
        },
        ("zombie.core.Core", "getGitSha") => |_, _, cf, n| {
            want_args(n, true, 0)?;
            ret_str(cf, Some(crate::zombie::core::core::GIT_REVISION))
        },
        ("zombie.core.Core", "getScreenWidth") => |s, _, cf, n| {
            want_args(n, true, 0)?;
            let _ = core(s);
            ret_int(cf, CORE_WIDTH)
        },
        ("zombie.core.Core", "getScreenHeight") => |s, _, cf, n| {
            want_args(n, true, 0)?;
            let _ = core(s);
            ret_int(cf, CORE_HEIGHT)
        },
        ("zombie.core.Core", "getTermsOfServiceVersion") => |_, _, cf, n| {
            want_args(n, true, 0)?;
            with_this!(
                cf,
                CoreObject,
                c,
                ret_int(cf, c.option_terms_of_service_version)
            )
        },
        ("zombie.gameStates.TermsOfServiceState", "fromLua0") => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let func = string_arg(cf, 1)?;
            with_this!(cf, TermsOfServiceStateObject, t, {
                t.0.borrow_mut()
                    .from_lua0(func.as_deref())
                    .map_err(Fail::Java)?;
                ret(cf, Object::Nil)
            })
        },
        _ => return None,
    })
}
