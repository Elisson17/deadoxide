//! The exposed API the game's `media/lua/server` reaches while loading: the tile, container and fence registries
//! (TileOverlays, ContainerOverlays, BentFences, BrokenFences), MapObjects, WorldGenUtils, ItemPickerJava.KeyNamer,
//! RandomizedWorldBase's clutter lists (through getWorld()), LuaEventManager.AddEvent, SGlobalObjects and the
//! character state singletons. Same conventions as `exposed_methods`.
use std::cell::RefCell;
use std::rc::Rc;

use super::exposed_methods::{
    int_arg, npe, ret, ret_int, ret_str, singleton, string_arg, string_arg_nn, type_error, ud,
    want_args, with_this, ArrayListObject, State,
};
use super::exposer::{class_named, Fail, Imp, JavaEnum, JavaStatics, Ret};
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::object::{JavaClass, Object, TableRef, Throwable, Userdata};
use crate::zombie::iso::bent_fences::BentFences;
use crate::zombie::iso::broken_fences::BrokenFences;
use crate::zombie::iso::container_overlays::ContainerOverlays;
use crate::zombie::iso::tile_overlays::TileOverlays;
use crate::zombie::iso::worldgen::world_gen_utils::WorldGenUtils;

/// A Java singleton whose state the port keeps (`$state`), shown to Lua as class `$class`.
macro_rules! state_object {
    ($name:ident, $state:ty, $class:literal) => {
        #[doc = concat!($class, " as a Lua value.")]
        #[derive(Default)]
        pub struct $name(pub RefCell<$state>);

        impl Userdata for $name {
            fn class(&self) -> &'static JavaClass {
                class_named($class)
            }
            fn java_to_string(&self) -> String {
                format!(concat!($class, "@{:x}"), super::exposed_methods::hash(self))
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }
    };
}

pub(super) use state_object;

state_object!(TileOverlaysObject, TileOverlays, "zombie.iso.TileOverlays");
state_object!(
    ContainerOverlaysObject,
    ContainerOverlays,
    "zombie.iso.ContainerOverlays"
);
state_object!(BentFencesObject, BentFences, "zombie.iso.BentFences");
state_object!(BrokenFencesObject, BrokenFences, "zombie.iso.BrokenFences");
state_object!(
    WorldGenUtilsObject,
    WorldGenUtils,
    "zombie.iso.worldgen.WorldGenUtils"
);

/// The singletons whose state the port keeps; `None` for the opaque ones.
pub fn make_singleton(class: &str) -> Option<Object> {
    Some(match class {
        "zombie.iso.TileOverlays" => ud(Rc::new(TileOverlaysObject::default())),
        "zombie.iso.ContainerOverlays" => ud(Rc::new(ContainerOverlaysObject::default())),
        "zombie.iso.BentFences" => ud(Rc::new(BentFencesObject::default())),
        "zombie.iso.BrokenFences" => ud(Rc::new(BrokenFencesObject::default())),
        "zombie.iso.worldgen.WorldGenUtils" => ud(Rc::new(WorldGenUtilsObject::default())),
        _ => return None,
    })
}

fn the(s: &Rc<JavaStatics>, class: &str) -> Object {
    singleton(s, class_named(class))
}

/// A `KahluaTableImpl` parameter: a Lua table, or null for nil.
pub(super) fn table_impl_arg(cf: &FrameRef, i: i32) -> Result<Option<TableRef>, Fail> {
    match cf.get(i) {
        Object::Nil => Ok(None),
        Object::Table(t) => Ok(Some(t)),
        o => Err(type_error("KahluaTableImpl", &o)),
    }
}

/// An enum parameter of class `class` (`simple` is its simple name for the error), or null for nil.
pub(super) fn enum_arg(
    cf: &FrameRef,
    i: i32,
    class: &str,
    simple: &str,
) -> Result<Option<Rc<dyn Userdata>>, Fail> {
    match cf.get(i) {
        Object::Nil => Ok(None),
        Object::Userdata(u)
            if u.as_any()
                .downcast_ref::<JavaEnum>()
                .is_some_and(|e| e.class.name == class) =>
        {
            Ok(Some(u))
        }
        o => Err(type_error(simple, &o)),
    }
}

/// A `LuaClosure` parameter: a Lua function, or null for nil.
fn closure_arg(cf: &FrameRef, i: i32) -> Result<Object, Fail> {
    match cf.get(i) {
        o @ (Object::Nil | Object::Closure(_)) => Ok(o),
        o => Err(type_error("LuaClosure", &o)),
    }
}

/// The written ones of LuaManager.GlobalObject's global functions this module owns.
pub fn global(name: &str) -> Option<Imp> {
    Some(match name {
        "getTileOverlays" => |s, _, cf, _| ret(cf, the(s, "zombie.iso.TileOverlays")),
        "getContainerOverlays" => |s, _, cf, _| ret(cf, the(s, "zombie.iso.ContainerOverlays")),
        "getTextManager" => |s, _, cf, _| ret(cf, the(s, "zombie.ui.TextManager")),
        "getWorld" => |s, _, cf, _| ret(cf, the(s, "zombie.iso.IsoWorld")),
        _ => return None,
    })
}

/// The static methods this module owns.
pub fn static_method(class: &str, name: &str) -> Option<Imp> {
    Some(match (class, name) {
        ("zombie.iso.BentFences", "getInstance") => {
            |s, _, cf, _| ret(cf, the(s, "zombie.iso.BentFences"))
        }
        ("zombie.iso.BrokenFences", "getInstance") => {
            |s, _, cf, _| ret(cf, the(s, "zombie.iso.BrokenFences"))
        }
        ("zombie.ai.states.ClimbDownSheetRopeState", "instance") => {
            |s, _, cf, _| ret(cf, the(s, "zombie.ai.states.ClimbDownSheetRopeState"))
        }
        ("zombie.ai.states.ClimbOverFenceState", "instance") => {
            |s, _, cf, _| ret(cf, the(s, "zombie.ai.states.ClimbOverFenceState"))
        }
        ("zombie.ai.states.ClimbOverWallState", "instance") => {
            |s, _, cf, _| ret(cf, the(s, "zombie.ai.states.ClimbOverWallState"))
        }
        ("zombie.ai.states.ClimbSheetRopeState", "instance") => {
            |s, _, cf, _| ret(cf, the(s, "zombie.ai.states.ClimbSheetRopeState"))
        }
        ("zombie.ai.states.ClimbThroughWindowState", "instance") => {
            |s, _, cf, _| ret(cf, the(s, "zombie.ai.states.ClimbThroughWindowState"))
        }
        // zombie.inventory.ItemPickerJava.KeyNamer.clear: the six public lists emptied.
        ("zombie.inventory.ItemPickerJava$KeyNamer", "clear") => |s, _, _, _| {
            for name in [
                "badZones",
                "bigBuildingRooms",
                "restaurantSubstrings",
                "restaurants",
                "roomSubstrings",
                "rooms",
            ] {
                if let Object::Userdata(u) =
                    s.field("zombie.inventory.ItemPickerJava$KeyNamer", name)
                {
                    if let Some(l) = u.as_any().downcast_ref::<ArrayListObject>() {
                        l.0.borrow_mut().clear();
                    }
                }
            }
            Ok(0)
        },
        ("zombie.Lua.LuaEventManager", "AddEvent") => |s, _, cf, n| {
            want_args(n, false, 1)?;
            let name = string_arg_nn(cf, 0)?;
            let event = s.events.add_event(&name, &s.env()?)?;
            ret(cf, Object::Userdata(event))
        },
        // OnNewWithSprite / OnLoadWithSprite (String | KahluaTable, LuaClosure, int)
        ("zombie.Lua.MapObjects", "OnNewWithSprite") => {
            |s, _, cf, n| map_objects_add(s, cf, n, false)
        }
        ("zombie.Lua.MapObjects", "OnLoadWithSprite") => {
            |s, _, cf, n| map_objects_add(s, cf, n, true)
        }
        // No system is registered before the game starts (OnSGlobalObjectSystemInit).
        ("zombie.globalObjects.SGlobalObjects", "getSystemCount") => |_, _, cf, _| ret_int(cf, 0),
        ("zombie.globalObjects.SGlobalObjects", "getSystemByIndex") => |_, _, cf, _| {
            int_arg(cf, 0)?;
            ret(cf, Object::Nil)
        },
        ("zombie.globalObjects.SGlobalObjects", "getSystemByName") => |_, _, cf, _| {
            string_arg(cf, 0)?;
            ret(cf, Object::Nil)
        },
        _ => return None,
    })
}

/// zombie.Lua.MapObjects.OnNewWithSprite / OnLoadWithSprite: the KahluaTable overload for a table, else the
/// String one.
fn map_objects_add(s: &Rc<JavaStatics>, cf: &FrameRef, n: i32, on_load: bool) -> Ret {
    want_args(n, false, 3)?;
    let function = closure_arg(cf, 1)?;
    let priority = int_arg(cf, 2)?;
    let mut m = s.state.map_objects.borrow_mut();
    match cf.get(0) {
        Object::Table(t) if on_load => m.on_load_with_sprites(Some(&t), &function, priority)?,
        Object::Table(t) => m.on_new_with_sprites(Some(&t), &function, priority)?,
        _ => {
            let name = string_arg(cf, 0)?;
            if on_load {
                m.on_load_with_sprite(name.as_deref(), &function, priority)?
            } else {
                m.on_new_with_sprite(name.as_deref(), &function, priority)?
            }
        }
    }
    Ok(0)
}

/// The instance methods this module owns.
pub fn instance_method(class: &str, name: &str) -> Option<Imp> {
    if class == "zombie.randomizedWorld.RandomizedWorldBase" {
        return super::exposed_table::randomized_world_base_list(name);
    }
    Some(match (class, name) {
        ("zombie.iso.TileOverlays", "addOverlays") => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let t = table_impl_arg(cf, 1)?.ok_or_else(npe)?;
            with_this!(cf, TileOverlaysObject, o, o.0.borrow_mut().add_overlays(&t))?;
            Ok(0)
        },
        ("zombie.iso.ContainerOverlays", "addOverlays") => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let t = table_impl_arg(cf, 1)?.ok_or_else(npe)?;
            with_this!(
                cf,
                ContainerOverlaysObject,
                o,
                o.0.borrow_mut().add_overlays(&t)
            )?;
            Ok(0)
        },
        ("zombie.iso.BentFences", "addFenceTiles") => |_, _, cf, n| {
            want_args(n, true, 2)?;
            let version = int_arg(cf, 1)?;
            let t = table_impl_arg(cf, 2)?.ok_or_else(npe)?;
            with_this!(
                cf,
                BentFencesObject,
                o,
                o.0.borrow_mut().add_fence_tiles(version, &t)
            )?;
            Ok(0)
        },
        ("zombie.iso.BrokenFences", "addBrokenTiles") => |s, _, cf, n| {
            want_args(n, true, 1)?;
            let t = table_impl_arg(cf, 1)?.ok_or_else(npe)?;
            let log = with_this!(
                cf,
                BrokenFencesObject,
                o,
                o.0.borrow_mut().add_broken_tiles(&t)
            )?;
            s.loader
                .borrow_mut()
                .log
                .extend(log.into_iter().map(|l| format!("Sound: {l}")));
            Ok(0)
        },
        ("zombie.iso.BrokenFences", "addDebrisTiles") => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let t = table_impl_arg(cf, 1)?.ok_or_else(npe)?;
            with_this!(
                cf,
                BrokenFencesObject,
                o,
                o.0.borrow_mut().add_debris_tiles(&t)
            )?;
            Ok(0)
        },
        ("zombie.iso.worldgen.WorldGenUtils", "getFiles") => |s, _, cf, n| {
            want_args(n, true, 1)?;
            let base_path = string_arg_nn(cf, 1)?;
            let root = s.loader.borrow().game_root().to_path_buf();
            let log = with_this!(
                cf,
                WorldGenUtilsObject,
                o,
                o.0.borrow_mut().get_files(&root, &base_path)
            );
            if let Some(l) = log {
                s.loader.borrow_mut().log.push(format!("General: {l}"));
            }
            Ok(0)
        },
        ("zombie.iso.worldgen.WorldGenUtils", "getFilesNum") => |_, _, cf, _| {
            with_this!(
                cf,
                WorldGenUtilsObject,
                o,
                ret_int(cf, o.0.borrow().get_files_num())
            )
        },
        ("zombie.iso.worldgen.WorldGenUtils", "getFile") => |_, _, cf, n| {
            want_args(n, true, 1)?;
            let i = int_arg(cf, 1)?;
            with_this!(cf, WorldGenUtilsObject, o, {
                let w = o.0.borrow();
                match w.get_file(i) {
                    Some(f) => ret_str(cf, Some(f)),
                    None => Err(Throwable::Java {
                        class: "java.lang.IndexOutOfBoundsException",
                        message: Some(format!(
                            "Index {i} out of bounds for length {}",
                            w.files.len()
                        )),
                    }
                    .into()),
                }
            })
        },
        ("zombie.iso.IsoWorld", "getRandomizedWorldBase") => {
            |s, _, cf, _| ret(cf, the(s, "zombie.randomizedWorld.RandomizedWorldBase"))
        }
        _ => return None,
    })
}

/// One of RandomizedWorldBase's `ArrayList<String> getXClutter()`: the static list `field`, one per class.
pub fn clutter(s: &Rc<JavaStatics>, cf: &FrameRef, field: &'static str) -> Ret {
    let list = list_of(&s.state, field);
    ret(cf, list)
}

fn list_of(state: &State, field: &'static str) -> Object {
    state
        .clutter
        .borrow_mut()
        .entry(field)
        .or_insert_with(|| ud(Rc::new(ArrayListObject::default())))
        .clone()
}
