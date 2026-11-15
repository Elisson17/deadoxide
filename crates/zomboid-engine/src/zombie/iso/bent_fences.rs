//! zombie.iso.BentFences: the stages a fence bends through under zombie thumping, its collapsed sprites and debris.
//! `media/lua/server/Items/BentFences.lua` hands its table to `addFenceTiles`.
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use super::iso_directions::{IsoDirections, VALUES};
use super::tile_overlays::npe;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::object::{cast_table, TableRef, Throwable, R};

/// zombie.iso.BentFences.Entry
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub dir: IsoDirections,
    pub health: i32,
    /// HashMap<Integer, ArrayList<String>>: read by stage number only.
    pub stages: HashMap<i32, Vec<String>>,
    pub collapsed: Vec<String>,
    pub debris: Vec<String>,
    pub length: i32,
    pub collapsed_offset: i32,
    pub collapsed_size_x: i32,
    pub collapsed_size_y: i32,
    pub do_smash: bool,
}

/// zombie.iso.BentFences (one instance: BentFences.getInstance()).
#[derive(Debug, Default)]
pub struct BentFences {
    pub entries: Vec<Rc<Entry>>,
    /// HashSets/HashMap in the original: their order is not observable through the class.
    pub collapsed_tiles: HashSet<String>,
    pub debris_tiles: HashSet<String>,
    pub fence_map: HashMap<String, Vec<Rc<Entry>>>,
}

/// zombie.iso.BentFences.tableToTiles(KahluaTable, String): the values of `table[key]` as strings, empty for a
/// missing table.
fn table_to_tiles(table: &TableRef, key: &str) -> R<Vec<String>> {
    let mut result = Vec::new();
    if let Some(tiles) = KahluaTableImpl::rawget_table(table, key)? {
        for (_, v) in KahluaTableImpl::iterate_snapshot(&tiles) {
            result.push(v.java_to_string());
        }
    }
    Ok(result)
}

/// `IsoDirections.valueOf(name)`
fn iso_directions_value_of(name: Option<&str>) -> R<IsoDirections> {
    let name = name.ok_or_else(|| Throwable::npe("Name is null"))?;
    VALUES
        .iter()
        .copied()
        .find(|d| d.name() == name)
        .ok_or_else(|| Throwable::Java {
            class: "java.lang.IllegalArgumentException",
            message: Some(format!("No enum constant zombie.iso.IsoDirections.{name}")),
        })
}

impl BentFences {
    /// zombie.iso.BentFences.addFenceTiles: one entry per value (`dir`, `health` (100 when absent),
    /// `collapsedOffset`, `collapsedSizeX/Y`, `doSmash`, `stages = { { stage, tiles } }`, `collapsed`, `debris`).
    /// An entry without stages, or whose collapsed list does not hold sizeX * sizeY sprites, is dropped before its
    /// debris is read; one whose stages turn out empty is dropped after. A kept entry is listed under every sprite
    /// of stages 0 .. count-1. `version` is not read.
    pub fn add_fence_tiles(&mut self, _version: i32, tiles: &TableRef) -> R<()> {
        for (_, value) in KahluaTableImpl::iterate_snapshot(tiles) {
            let value = cast_table(&value)?.ok_or_else(npe)?;
            let dir =
                iso_directions_value_of(KahluaTableImpl::rawget_str(&value, "dir")?.as_deref())?;
            let health = match KahluaTableImpl::rawget_int_of(&value, "health")? {
                -1 => 100,
                h => h,
            };
            let mut entry = Entry {
                dir,
                health,
                stages: HashMap::new(),
                collapsed: Vec::new(),
                debris: Vec::new(),
                length: -1,
                collapsed_offset: KahluaTableImpl::rawget_int_of(&value, "collapsedOffset")?,
                collapsed_size_x: KahluaTableImpl::rawget_int_of(&value, "collapsedSizeX")?,
                collapsed_size_y: KahluaTableImpl::rawget_int_of(&value, "collapsedSizeY")?,
                do_smash: KahluaTableImpl::rawget_bool(&value, "doSmash")?,
            };
            let stage_table = KahluaTableImpl::rawget_table(&value, "stages")?;
            let collapsed_table = KahluaTableImpl::rawget_table(&value, "collapsed")?;
            let debris_table = KahluaTableImpl::rawget_table(&value, "debris")?;
            let Some(stage_table) = stage_table else {
                continue;
            };
            for (_, next) in KahluaTableImpl::iterate_snapshot(&stage_table) {
                let next = cast_table(&next)?.ok_or_else(npe)?;
                let stage = KahluaTableImpl::rawget_int_of(&next, "stage")?;
                entry.stages.insert(stage, table_to_tiles(&next, "tiles")?);
            }
            if collapsed_table.is_some() {
                entry.collapsed.extend(table_to_tiles(&value, "collapsed")?);
                if entry.collapsed_size_x.wrapping_mul(entry.collapsed_size_y)
                    != entry.collapsed.len() as i32
                {
                    continue;
                }
                self.collapsed_tiles.extend(entry.collapsed.iter().cloned());
            }
            if debris_table.is_some() {
                entry.debris.extend(table_to_tiles(&value, "debris")?);
                self.debris_tiles.extend(entry.debris.iter().cloned());
            }
            if !entry.stages.is_empty() {
                entry.length = entry.stages.get(&0).ok_or_else(npe)?.len() as i32;
                let count = entry.stages.len() as i32;
                let entry = Rc::new(entry);
                self.entries.push(entry.clone());
                for i in 0..count {
                    for sprite_name in entry.stages.get(&i).ok_or_else(npe)? {
                        self.fence_map
                            .entry(sprite_name.clone())
                            .or_default()
                            .push(entry.clone());
                    }
                }
            }
        }
        Ok(())
    }
}
