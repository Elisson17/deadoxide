//! zombie.Lua.MapObjects: Lua functions run when an object with a given sprite is created (OnNewWithSprite) or
//! loaded (OnLoadWithSprite). `media/lua/server/Map/MapObjects/MO*.lua` register them while loading.
use std::collections::HashMap;

use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::object::{Object, TableRef, Throwable, R};

/// zombie.Lua.MapObjects.Callback: the functions of one sprite, highest priority first.
#[derive(Clone, Debug, Default)]
pub struct Callback {
    pub sprite_name: String,
    /// LuaClosures
    pub functions: Vec<Object>,
    /// A TShortArrayList: priorities are stored as short.
    pub priority: Vec<i16>,
}

/// The statics of zombie.Lua.MapObjects.
#[derive(Debug, Default)]
pub struct MapObjects {
    /// HashMaps in the original: their order is not observable through the class.
    pub on_new: HashMap<String, Callback>,
    pub on_load: HashMap<String, Callback>,
}

fn illegal_argument(msg: impl Into<String>) -> Throwable {
    Throwable::Java {
        class: "java.lang.IllegalArgumentException",
        message: Some(msg.into()),
    }
}

/// The insertion of OnNewWithSprite/OnLoadWithSprite(String, LuaClosure, int): before the first function of
/// lower priority, in place of one of equal priority, else at the end.
fn add(
    map: &mut HashMap<String, Callback>,
    sprite_name: Option<&str>,
    function: &Object,
    priority: i32,
) -> R<()> {
    let sprite_name = match sprite_name {
        Some(s) if !s.is_empty() => s,
        _ => return Err(illegal_argument("invalid sprite name")),
    };
    if function.is_nil() {
        return Err(Throwable::npe("function is null"));
    }
    let callback = map
        .entry(sprite_name.to_string())
        .or_insert_with(|| Callback {
            sprite_name: sprite_name.to_string(),
            ..Callback::default()
        });
    for i in 0..callback.functions.len() {
        if i32::from(callback.priority[i]) < priority {
            callback.functions.insert(i, function.clone());
            callback.priority.insert(i, priority as i16);
            return Ok(());
        }
        if i32::from(callback.priority[i]) == priority {
            callback.functions[i] = function.clone();
            callback.priority[i] = priority as i16;
            return Ok(());
        }
    }
    callback.functions.push(function.clone());
    callback.priority.push(priority as i16);
    Ok(())
}

/// The KahluaTable overloads: every value must be a string (IllegalArgumentException at the first that is not,
/// after the earlier ones were added).
fn add_all(
    map: &mut HashMap<String, Callback>,
    sprite_names: Option<&TableRef>,
    function: &Object,
    priority: i32,
) -> R<()> {
    let sprite_names = match sprite_names {
        Some(t) if !t.borrow().is_empty() => t,
        _ => return Err(illegal_argument("invalid sprite-name table")),
    };
    if function.is_nil() {
        return Err(Throwable::npe("function is null"));
    }
    for (_, value) in KahluaTableImpl::iterate_snapshot(sprite_names) {
        let Object::String(s) = &value else {
            return Err(illegal_argument(format!(
                "expected string but got \"{}\"",
                value.java_to_string()
            )));
        };
        add(
            map,
            Some(&crate::se::krka::kahlua::vm::object::rstr(s)),
            function,
            priority,
        )?;
    }
    Ok(())
}

impl MapObjects {
    /// zombie.Lua.MapObjects.OnNewWithSprite(String, LuaClosure, int)
    pub fn on_new_with_sprite(
        &mut self,
        sprite_name: Option<&str>,
        function: &Object,
        priority: i32,
    ) -> R<()> {
        add(&mut self.on_new, sprite_name, function, priority)
    }

    /// zombie.Lua.MapObjects.OnNewWithSprite(KahluaTable, LuaClosure, int)
    pub fn on_new_with_sprites(
        &mut self,
        sprite_names: Option<&TableRef>,
        function: &Object,
        priority: i32,
    ) -> R<()> {
        add_all(&mut self.on_new, sprite_names, function, priority)
    }

    /// zombie.Lua.MapObjects.OnLoadWithSprite(String, LuaClosure, int)
    pub fn on_load_with_sprite(
        &mut self,
        sprite_name: Option<&str>,
        function: &Object,
        priority: i32,
    ) -> R<()> {
        add(&mut self.on_load, sprite_name, function, priority)
    }

    /// zombie.Lua.MapObjects.OnLoadWithSprite(KahluaTable, LuaClosure, int)
    pub fn on_load_with_sprites(
        &mut self,
        sprite_names: Option<&TableRef>,
        function: &Object,
        priority: i32,
    ) -> R<()> {
        add_all(&mut self.on_load, sprite_names, function, priority)
    }
}
