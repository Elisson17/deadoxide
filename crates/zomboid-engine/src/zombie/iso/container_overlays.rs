//! zombie.iso.ContainerOverlays: the "full"/"few items" sprites of shelves and other containers by room.
//! `media/lua/server/Items/WorldFiller.lua` hands its table to `addOverlays`.
use std::collections::HashMap;

use super::tile_overlays::npe;
use crate::java;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::object::{
    cast_string, cast_table, rstr, Object, TableRef, Throwable, R,
};
use crate::zombie::util::string_utils::is_null_or_whitespace;

/// zombie.iso.ContainerOverlays.ContainerOverlayEntry
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContainerOverlayEntry {
    pub room: Option<String>,
    pub many_items: Option<String>,
    pub few_items: Option<String>,
}

/// zombie.iso.ContainerOverlays.ContainerOverlay
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContainerOverlay {
    pub name: String,
    pub entries: Vec<ContainerOverlayEntry>,
}

/// zombie.iso.ContainerOverlays (one instance: ContainerOverlays.instance).
#[derive(Debug, Default)]
pub struct ContainerOverlays {
    /// A THashMap in the original: its order is not observable through the class.
    pub overlay_map: HashMap<String, ContainerOverlay>,
    pub overlay_name_to_underlying_name: HashMap<String, Vec<String>>,
}

impl ContainerOverlays {
    /// zombie.iso.ContainerOverlays.addOverlays: no VERSION is the old format, VERSION 1 the current one, any
    /// other a RuntimeException.
    pub fn add_overlays(&mut self, overlay_map: &TableRef) -> R<()> {
        let version = KahluaTableImpl::rawget_int_of(overlay_map, "VERSION")?;
        if version == -1 {
            self.parse_container_overlay_map_v0(overlay_map)
        } else if version != 1 {
            Err(Throwable::runtime(format!(
                "unknown overlayMap.VERSION {version}"
            )))
        } else {
            self.parse_container_overlay_map_v1(overlay_map)
        }
    }

    /// zombie.iso.ContainerOverlays.parseContainerOverlayMapV0: `{ sprite = { room = { many, few } } }`, the
    /// texts as given (no "none" check), and no reverse map.
    fn parse_container_overlay_map_v0(&mut self, overlay_map_table: &TableRef) -> R<()> {
        for (key, value) in KahluaTableImpl::iterate_snapshot(overlay_map_table) {
            let name = key.java_to_string();
            let entries = cast_table(&value)?.ok_or_else(npe)?;
            let mut o = ContainerOverlay {
                name: name.clone(),
                entries: Vec::new(),
            };
            for (room, tiles) in KahluaTableImpl::iterate_snapshot(&entries) {
                let room = room.java_to_string();
                let tiles = cast_table(&tiles)?.ok_or_else(npe)?;
                let a = text_at(&tiles, 1)?;
                let b = text_at(&tiles, 2)?;
                o.entries.push(ContainerOverlayEntry {
                    room: Some(room),
                    many_items: a,
                    few_items: b,
                });
            }
            self.overlay_map.insert(name, o);
        }
        Ok(())
    }

    /// zombie.iso.ContainerOverlays.parseContainerOverlayMapV1: every key but VERSION is a sprite; its value lists
    /// room entries (`name`, `tiles = { many, few }`, blank or "none" for no sprite). A sprite given again replaces
    /// the earlier one; the reverse map (few first, then many) only grows.
    fn parse_container_overlay_map_v1(&mut self, overlays_table: &TableRef) -> R<()> {
        for (key, value) in KahluaTableImpl::iterate_snapshot(overlays_table) {
            let key = key.java_to_string();
            if java::equals_ignore_case("VERSION", &key) {
                continue;
            }
            let mut o = ContainerOverlay {
                name: key,
                entries: Vec::new(),
            };
            let overlay_table = cast_table(&value)?.ok_or_else(npe)?;
            for (_, room) in KahluaTableImpl::iterate_snapshot(&overlay_table) {
                let room_table = cast_table(&room)?.ok_or_else(npe)?;
                let room = KahluaTableImpl::rawget_str(&room_table, "name")?;
                let tile_table =
                    KahluaTableImpl::rawget_table(&room_table, "tiles")?.ok_or_else(npe)?;
                let many =
                    cast_string(&KahluaTableImpl::rawget_int(&tile_table, 1)?)?.map(|s| rstr(&s));
                let few =
                    cast_string(&KahluaTableImpl::rawget_int(&tile_table, 2)?)?.map(|s| rstr(&s));
                o.entries.push(ContainerOverlayEntry {
                    room,
                    many_items: none_to_null(many),
                    few_items: none_to_null(few),
                });
            }
            let name = o.name.clone();
            for ex in &o.entries {
                for overlay_name in [&ex.few_items, &ex.many_items].into_iter().flatten() {
                    let underlying = self
                        .overlay_name_to_underlying_name
                        .entry(overlay_name.clone())
                        .or_default();
                    if !underlying.contains(&name) {
                        underlying.push(name.clone());
                    }
                }
            }
            self.overlay_map.insert(name, o);
        }
        Ok(())
    }

    /// zombie.iso.ContainerOverlays.Reset
    pub fn reset(&mut self) {
        self.overlay_map.clear();
        self.overlay_name_to_underlying_name.clear();
    }
}

/// V0: `tiles.delegate.containsKey(i) ? tiles.rawget(i).toString() : null`.
fn text_at(tiles: &TableRef, i: i32) -> R<Option<String>> {
    Ok(
        match tiles.borrow().get_direct(&Object::Number(f64::from(i))) {
            Object::Nil => None,
            v => Some(v.java_to_string()),
        },
    )
}

/// V1: a blank or "none" sprite is null.
fn none_to_null(s: Option<String>) -> Option<String> {
    match s {
        Some(s) if !is_null_or_whitespace(Some(&s)) && !java::equals_ignore_case("none", &s) => {
            Some(s)
        }
        _ => None,
    }
}
