//! zombie.iso.TileOverlays: the overlay sprites (dust, dishes, books, ...) the world puts on furniture and floor
//! tiles by room. `media/lua/server/Items/*Overlays.lua` hand their tables to `addOverlays`.
use std::collections::HashMap;

use crate::java;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::object::{cast_table, TableRef, Throwable, R};
use crate::zombie::core::math::pz_math;
use crate::zombie::util::string_utils::is_null_or_whitespace;

/// zombie.iso.TileOverlays.TileOverlayUsage
#[derive(Clone, Debug, PartialEq)]
pub struct TileOverlayUsage {
    pub usage: Option<String>,
    pub z_only: i32,
    pub z_greater_than: i32,
    pub alpha: f32,
    pub table_top: bool,
}

impl Default for TileOverlayUsage {
    fn default() -> TileOverlayUsage {
        TileOverlayUsage {
            usage: None,
            z_only: i32::MIN,
            z_greater_than: i32::MIN,
            alpha: -1.0,
            table_top: false,
        }
    }
}

impl TileOverlayUsage {
    /// zombie.iso.TileOverlays.TileOverlayUsage.parse: `usage` trimmed is kept; the untrimmed text is split on
    /// ';' into `z=N`, `z>N`, `alpha=F` (clamped to 0..1) and `tabletop...`; anything else stops the parse (false).
    pub fn parse(&mut self, usage: Option<&str>) -> R<bool> {
        let usage = usage.ok_or_else(npe)?;
        self.usage = Some(java::trim(usage).to_string());
        if is_null_or_whitespace(self.usage.as_deref()) {
            return Ok(true);
        }
        for s in java::split_char(usage, ';') {
            if let Some(n) = s.strip_prefix("z=") {
                self.z_only = parse_int(n)?;
            } else if let Some(n) = s.strip_prefix("z>") {
                self.z_greater_than = parse_int(n)?;
            } else if let Some(f) = s.strip_prefix("alpha=") {
                let alpha = java::parse_float(f).ok_or_else(|| number_format(f))?;
                self.alpha = pz_math::clamp(alpha, 0.0, 1.0);
            } else {
                if !s.starts_with("tabletop") {
                    return Ok(false);
                }
                self.table_top = true;
            }
        }
        Ok(true)
    }

    /// zombie.iso.TileOverlays.TileOverlayUsage.match
    pub fn matches(&self, z: i32) -> bool {
        if self.z_only != i32::MIN && z != self.z_only {
            false
        } else {
            self.z_greater_than == i32::MIN || z > self.z_greater_than
        }
    }
}

/// Integer.parseInt with its NumberFormatException.
fn parse_int(s: &str) -> R<i32> {
    java::parse_int(s).ok_or_else(|| number_format(s))
}

fn number_format(s: &str) -> Throwable {
    Throwable::Java {
        class: "java.lang.NumberFormatException",
        message: Some(if s.is_empty() {
            "empty String".into()
        } else {
            java::number_format_message(s)
        }),
    }
}

/// zombie.iso.TileOverlays.TileOverlayEntry
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TileOverlayEntry {
    pub room: Option<String>,
    pub chance: i32,
    /// "" for a blank or "none" tile.
    pub tiles: Vec<String>,
    pub usage: TileOverlayUsage,
}

/// zombie.iso.TileOverlays.TileOverlay
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TileOverlay {
    pub tile: String,
    pub entries: Vec<TileOverlayEntry>,
}

/// zombie.iso.TileOverlays (one instance: TileOverlays.instance).
#[derive(Debug, Default)]
pub struct TileOverlays {
    /// A THashMap in the original: its order is not observable through the class.
    pub overlay_map: HashMap<String, TileOverlay>,
    pub overlay_name_to_underlying_name: HashMap<String, Vec<String>>,
}

impl TileOverlays {
    /// zombie.iso.TileOverlays.addOverlays: every key but VERSION is a tile; its value lists room entries
    /// (`name`, `chance`, `usage`, `tiles`). A tile given again replaces the earlier one; the reverse map from an
    /// overlay sprite to its tiles only grows. With Core.debug off no texture is checked.
    pub fn add_overlays(&mut self, overlay_map: &TableRef) -> R<()> {
        for (key, value) in KahluaTableImpl::iterate_snapshot(overlay_map) {
            let key = key.java_to_string();
            if java::equals_ignore_case("VERSION", &key) {
                continue;
            }
            let mut o = TileOverlay {
                tile: key,
                entries: Vec::new(),
            };
            let overlay_table = cast_table(&value)?.ok_or_else(npe)?;
            for (_, room) in KahluaTableImpl::iterate_snapshot(&overlay_table) {
                let room_table = cast_table(&room)?.ok_or_else(npe)?;
                let mut e = TileOverlayEntry {
                    room: KahluaTableImpl::rawget_str(&room_table, "name")?,
                    chance: KahluaTableImpl::rawget_int_of(&room_table, "chance")?,
                    ..TileOverlayEntry::default()
                };
                let usage = KahluaTableImpl::rawget_str(&room_table, "usage")?;
                e.usage.parse(usage.as_deref())?;
                let tiles_table =
                    KahluaTableImpl::rawget_table(&room_table, "tiles")?.ok_or_else(npe)?;
                for (_, tile) in KahluaTableImpl::iterate_snapshot(&tiles_table) {
                    let texture_name = tile.java_to_string();
                    let texture_name = if is_null_or_whitespace(Some(&texture_name))
                        || java::equals_ignore_case("none", &texture_name)
                    {
                        String::new()
                    } else {
                        texture_name
                    };
                    e.tiles.push(texture_name);
                }
                o.entries.push(e);
            }
            let tile = o.tile.clone();
            for e in &o.entries {
                for overlay_name in &e.tiles {
                    let underlying = self
                        .overlay_name_to_underlying_name
                        .entry(overlay_name.clone())
                        .or_default();
                    if !underlying.contains(&tile) {
                        underlying.push(tile.clone());
                    }
                }
            }
            self.overlay_map.insert(tile, o);
        }
        Ok(())
    }

    /// zombie.iso.TileOverlays.Reset
    pub fn reset(&mut self) {
        self.overlay_map.clear();
        self.overlay_name_to_underlying_name.clear();
    }
}

/// A NullPointerException (the JDK's helpful message is not reproduced; it only reaches the log).
pub(crate) fn npe() -> Throwable {
    Throwable::Java {
        class: "java.lang.NullPointerException",
        message: None,
    }
}
