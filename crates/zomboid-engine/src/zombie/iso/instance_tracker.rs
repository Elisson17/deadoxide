//! zombie.iso.InstanceTracker: counters by group and key (item spawns, container rolls, stats) that a save keeps in
//! `iTrack.bin` and that saving exports to `ItemTracker.log`.
use crate::java::{is_whitespace, string_compare_to};
use crate::java_nio::{read_string, ByteBuffer};

/// zombie.iso.InstanceTracker.DEFAULT
pub const DEFAULT: &str = "";
/// zombie.iso.InstanceTracker.ITEMS
pub const ITEMS: &str = "Item Spawns";
/// zombie.iso.InstanceTracker.CONTAINERS
pub const CONTAINERS: &str = "Container Rolls";
/// zombie.iso.InstanceTracker.STATS
pub const STATS: &str = "Stats";

/// zombie.iso.InstanceTracker.Format
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Text,
    Csv,
}

/// zombie.iso.InstanceTracker.Sort
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    /// The key set's own order: Trove's `TObjectIntHashMap` order, not reproduced (first-put order here).
    None,
    /// `sorted()`: `String.compareTo`.
    Key,
    /// Highest count first; keys with equal counts keep the key set's order (not reproduced).
    Count,
}

/// java.lang.String.isBlank
fn is_blank(s: &str) -> bool {
    s.chars().all(is_whitespace)
}

/// zombie.iso.InstanceTracker: the static `InstanceGroups` (`HashMap<String, TObjectIntHashMap<String>>`) as a
/// value; groups and keys in first-put order (a missing key counts 0, as Trove's `get` gives).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InstanceTracker {
    pub instance_groups: Vec<(String, Vec<(String, i32)>)>,
}

impl InstanceTracker {
    fn group(&self, group: &str) -> Option<&Vec<(String, i32)>> {
        self.instance_groups
            .iter()
            .find(|(g, _)| g == group)
            .map(|(_, m)| m)
    }

    /// `InstanceGroups.putIfAbsent(group, new TObjectIntHashMap())` then `get(group)`.
    fn group_or_create(&mut self, group: &str) -> &mut Vec<(String, i32)> {
        let i = match self.instance_groups.iter().position(|(g, _)| g == group) {
            Some(i) => i,
            None => {
                self.instance_groups.push((group.to_string(), Vec::new()));
                self.instance_groups.len() - 1
            }
        };
        &mut self.instance_groups[i].1
    }

    /// zombie.iso.InstanceTracker.get(String, String)
    pub fn get(&self, group: &str, key: &str) -> i32 {
        self.group(group)
            .and_then(|m| m.iter().find(|(k, _)| k == key))
            .map_or(0, |(_, v)| *v)
    }

    /// `TObjectIntHashMap.put`
    fn put(map: &mut Vec<(String, i32)>, key: &str, value: i32) {
        match map.iter_mut().find(|(k, _)| k == key) {
            Some(e) => e.1 = value,
            None => map.push((key.to_string(), value)),
        }
    }

    /// zombie.iso.InstanceTracker.set(String, String, int): ignored for a blank key.
    pub fn set(&mut self, group: &str, key: &str, value: i32) {
        if !is_blank(key) {
            Self::put(self.group_or_create(group), key, value);
        }
    }

    /// zombie.iso.InstanceTracker.adj(String, String, int): the int sum wraps before `PZMath.clamp` to the int range,
    /// which then changes nothing.
    pub fn adj(&mut self, group: &str, key: &str, value: i32) {
        let v = self.get(group, key).wrapping_add(value);
        Self::put(self.group_or_create(group), key, v);
    }

    /// zombie.iso.InstanceTracker.inc(String, String)
    pub fn inc(&mut self, group: &str, key: &str) {
        self.adj(group, key, 1);
    }

    /// zombie.iso.InstanceTracker.dec(String, String)
    pub fn dec(&mut self, group: &str, key: &str) {
        self.adj(group, key, -1);
    }

    /// zombie.iso.InstanceTracker.sort: the group's keys; `None` for a group that does not exist (the original's
    /// `NullPointerException`).
    pub fn sort(&self, group: &str, sort: Sort) -> Option<Vec<String>> {
        let m = self.group(group)?;
        let mut keys: Vec<&(String, i32)> = m.iter().collect();
        match sort {
            Sort::None => {}
            Sort::Key => keys.sort_by(|a, b| string_compare_to(&a.0, &b.0)),
            Sort::Count => keys.sort_by_key(|e| std::cmp::Reverse(e.1)),
        }
        Some(keys.into_iter().map(|(k, _)| k.clone()).collect())
    }

    /// zombie.iso.InstanceTracker.exportGroup: "" for a missing group; TEXT is a "[group] ; N entries" header and
    /// "key = count" lines, CSV is "group,key,count" lines; then an empty line. `new_line` is
    /// `System.lineSeparator()`.
    pub fn export_group(&self, group: &str, format: Format, sort: Sort, new_line: &str) -> String {
        let (Some(m), Some(keys)) = (self.group(group), self.sort(group, sort)) else {
            return String::new();
        };
        let mut sb = String::new();
        match format {
            Format::Text => {
                sb.push_str(&format!("[{group}] ; {} entries{new_line}", m.len()));
                for key in &keys {
                    sb.push_str(&format!("{key} = {}{new_line}", self.get(group, key)));
                }
            }
            Format::Csv => {
                for key in &keys {
                    sb.push_str(&format!("{group},{key},{}{new_line}", self.get(group, key)));
                }
            }
        }
        sb.push_str(new_line);
        sb
    }

    /// zombie.iso.InstanceTracker.load over the bytes of the save's `iTrack.bin` (`None`: no file). The groups are
    /// dropped, then the world version (unused), the group count, and per group its name (the group's counters are
    /// replaced by an empty map) and its key-count pairs through `set` (blank keys skipped). `Err` where the
    /// original throws and logs; what was read before stays. `Core.isNoSave` is not modelled (false).
    pub fn load(&mut self, data: Option<&[u8]>) -> Result<(), String> {
        self.instance_groups.clear();
        let Some(data) = data else {
            return Ok(());
        };
        let mut bb = ByteBuffer::wrap(data);
        let _world_version = bb.get_int()?;
        let instance_groups_size = bb.get_int()?;
        for _ in 0..instance_groups_size {
            let group = read_string(&mut bb)?;
            let size = bb.get_int()?;
            match self.instance_groups.iter_mut().find(|(g, _)| *g == group) {
                Some(e) => e.1.clear(),
                None => self.instance_groups.push((group.clone(), Vec::new())),
            }
            for _ in 0..size {
                let key = read_string(&mut bb)?;
                let count = bb.get_int()?;
                self.set(&group, &key, count);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Blank keys are not set; `adj` wraps at the int range; the text export sorts keys by `String.compareTo`
    /// (upper case before lower case).
    #[test]
    fn counters_and_text_export() {
        let mut t = InstanceTracker::default();
        t.set(ITEMS, " \t", 5);
        t.set(ITEMS, "b", 2);
        t.set(ITEMS, "B", 1);
        t.set(ITEMS, "a", i32::MAX);
        t.inc(ITEMS, "a");
        t.dec(ITEMS, "c");
        assert_eq!(t.get(ITEMS, "a"), i32::MIN);
        assert_eq!(t.get(ITEMS, " \t"), 0);
        assert_eq!(
            t.export_group(ITEMS, Format::Text, Sort::Key, "\n"),
            "[Item Spawns] ; 4 entries\nB = 1\na = -2147483648\nb = 2\nc = -1\n\n"
        );
        assert_eq!(t.export_group(STATS, Format::Csv, Sort::Key, "\n"), "");
    }
}
