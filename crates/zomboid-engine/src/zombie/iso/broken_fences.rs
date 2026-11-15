//! zombie.iso.BrokenFences: which fence sprites break into which, and their debris. `media/lua/server/Items/
//! BrokenFences.lua` hands its tables to `addBrokenTiles` and `addDebrisTiles`.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::tile_overlays::npe;
use crate::java;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::object::{cast_table, TableRef, Throwable, R};

/// zombie.iso.BrokenFences.Tile (shared between the maps, so a later addDebrisTiles shows through all of them).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tile {
    pub self_: Option<Vec<String>>,
    pub left: Option<Vec<String>>,
    pub right: Option<Vec<String>>,
    pub debris_n: Option<Vec<String>>,
    pub debris_s: Option<Vec<String>>,
    pub debris_w: Option<Vec<String>>,
    pub debris_e: Option<Vec<String>>,
}

/// zombie.iso.BrokenFences (one instance: BrokenFences.getInstance()).
#[derive(Debug, Default)]
pub struct BrokenFences {
    /// THashMaps in the original: their order is not observable through the class.
    pub unbroken_map: HashMap<String, Rc<RefCell<Tile>>>,
    pub broken_left_map: HashMap<String, Rc<RefCell<Tile>>>,
    pub broken_right_map: HashMap<String, Rc<RefCell<Tile>>>,
    pub all_map: HashMap<String, Rc<RefCell<Tile>>>,
}

/// zombie.iso.BrokenFences.tableToTiles(KahluaTableImpl): the values as strings; null for a missing table and
/// also for an empty one (the list is made on the first value).
fn table_to_tiles(tiles: Option<TableRef>) -> Option<Vec<String>> {
    let tiles = tiles?;
    let mut result: Option<Vec<String>> = None;
    for (_, v) in KahluaTableImpl::iterate_snapshot(&tiles) {
        result.get_or_insert_with(Vec::new).push(v.java_to_string());
    }
    result
}

/// zombie.iso.BrokenFences.tableToTiles(KahluaTable, String)
fn table_to_tiles_at(table: &TableRef, key: &str) -> R<Option<Vec<String>>> {
    Ok(table_to_tiles(KahluaTableImpl::rawget_table(table, key)?))
}

impl BrokenFences {
    /// zombie.iso.BrokenFences.addBrokenTiles: every key but VERSION is an unbroken sprite with its `self`, `left`
    /// and `right` broken sprites; the broken ones map back to the same Tile. Then allMap takes all three maps.
    /// The original also looks the key up in IsoSpriteManager (adding a sprite for a name it does not know) and
    /// logs "Missing ThumpSound for breakable object <key>" (DebugType.Sound) when the sprite has no ThumpSound
    /// property; with no tile definitions loaded that is every key. Returns those log lines.
    pub fn add_broken_tiles(&mut self, tiles: &TableRef) -> R<Vec<String>> {
        let mut log = Vec::new();
        for (key, value) in KahluaTableImpl::iterate_snapshot(tiles) {
            let key = key.java_to_string();
            if java::equals_ignore_case("VERSION", &key) {
                continue;
            }
            let value = cast_table(&value)?.ok_or_else(npe)?;
            let tile = Rc::new(RefCell::new(Tile {
                self_: table_to_tiles_at(&value, "self")?,
                left: table_to_tiles_at(&value, "left")?,
                right: table_to_tiles_at(&value, "right")?,
                ..Tile::default()
            }));
            self.unbroken_map.insert(key.clone(), tile.clone());
            for s in tile.borrow().left.iter().flatten() {
                self.broken_left_map.insert(s.clone(), tile.clone());
            }
            for s in tile.borrow().right.iter().flatten() {
                self.broken_right_map.insert(s.clone(), tile.clone());
            }
            log.push(format!("Missing ThumpSound for breakable object {key}"));
        }
        for map in [
            &self.unbroken_map,
            &self.broken_left_map,
            &self.broken_right_map,
        ] {
            for (k, v) in map {
                self.all_map.insert(k.clone(), v.clone());
            }
        }
        Ok(log)
    }

    /// zombie.iso.BrokenFences.addDebrisTiles: every key but VERSION must already be an unbroken sprite
    /// (IllegalArgumentException otherwise); its `north`/`south`/`west`/`east` debris lists are set.
    pub fn add_debris_tiles(&mut self, tiles: &TableRef) -> R<()> {
        for (key, value) in KahluaTableImpl::iterate_snapshot(tiles) {
            let key = key.java_to_string();
            if java::equals_ignore_case("VERSION", &key) {
                continue;
            }
            let value = cast_table(&value)?.ok_or_else(npe)?;
            let tile = self
                .unbroken_map
                .get(&key)
                .cloned()
                .ok_or_else(|| Throwable::Java {
                    class: "java.lang.IllegalArgumentException",
                    message: Some("addDebrisTiles() with unknown tile".into()),
                })?;
            let mut tile = tile.borrow_mut();
            tile.debris_n = table_to_tiles_at(&value, "north")?;
            tile.debris_s = table_to_tiles_at(&value, "south")?;
            tile.debris_w = table_to_tiles_at(&value, "west")?;
            tile.debris_e = table_to_tiles_at(&value, "east")?;
        }
        Ok(())
    }

    /// zombie.iso.BrokenFences.isBreakableSprite
    pub fn is_breakable_sprite(&self, sprite: &str) -> bool {
        self.all_map.contains_key(sprite)
    }
}
