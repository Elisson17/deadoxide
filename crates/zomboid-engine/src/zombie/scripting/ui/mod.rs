//! Package `zombie.scripting.ui`: the XUI skins, their Lua styles and style vars (XuiManager.ParseScripts). The
//! layout side (XuiScript, XuiTableScript, XuiReference, VectorPosAlign, TextAlign) is not ported: the vanilla
//! scripts have no xuiLayout, xuiStyle or xuiDefaultStyle block.
pub mod xui_auto_apply;
pub mod xui_lua_style;
pub mod xui_manager;
pub mod xui_script_type;
pub mod xui_skin;
pub mod xui_var_type;

use crate::java;

/// java.util.HashMap<String, V> filled by `put` without removals: lookups by key, and iteration in the original's
/// order (`java::hash_map_order` of the keys in first-insertion order; a `put` of an existing key keeps its place).
/// `clear` here forgets the capacity the original keeps, which only matters to a later iteration of a reloaded map.
#[derive(Clone, Debug, PartialEq)]
pub struct JHashMap<V> {
    entries: Vec<(String, V)>,
}

impl<V> Default for JHashMap<V> {
    fn default() -> Self {
        JHashMap {
            entries: Vec::new(),
        }
    }
}

impl<V> JHashMap<V> {
    pub fn new() -> Self {
        Self::default()
    }

    /// HashMap.size
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// HashMap.isEmpty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The position of a key in insertion order.
    pub fn index_of(&self, key: &str) -> Option<usize> {
        self.entries.iter().position(|(k, _)| k == key)
    }

    /// HashMap.containsKey
    pub fn contains_key(&self, key: &str) -> bool {
        self.index_of(key).is_some()
    }

    /// HashMap.get
    pub fn get(&self, key: &str) -> Option<&V> {
        self.index_of(key).map(|i| &self.entries[i].1)
    }

    /// HashMap.get, for changing the value in place.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        let i = self.index_of(key)?;
        Some(&mut self.entries[i].1)
    }

    /// HashMap.put: replaces the value of an existing key in place, else adds the key.
    pub fn put(&mut self, key: &str, value: V) {
        match self.index_of(key) {
            Some(i) => self.entries[i].1 = value,
            None => self.entries.push((key.to_string(), value)),
        }
    }

    /// HashMap.clear
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// The entry at a position in insertion order.
    pub fn entry_at(&self, i: usize) -> (&str, &V) {
        let (k, v) = &self.entries[i];
        (k.as_str(), v)
    }

    /// The value at a position in insertion order.
    pub fn value_at_mut(&mut self, i: usize) -> &mut V {
        &mut self.entries[i].1
    }

    /// The positions (in insertion order) of the entries in the order HashMap.entrySet() iterates them.
    pub fn java_order(&self) -> Vec<usize> {
        let keys: Vec<&str> = self.entries.iter().map(|(k, _)| k.as_str()).collect();
        java::hash_map_order(&keys)
    }

    /// HashMap.entrySet() in the original's iteration order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.java_order().into_iter().map(|i| self.entry_at(i))
    }

    /// The entries sorted by key (String.compareTo), for dumps.
    pub fn sorted(&self) -> Vec<(&str, &V)> {
        let mut v: Vec<(&str, &V)> = self.entries.iter().map(|(k, v)| (k.as_str(), v)).collect();
        v.sort_by(|a, b| java::string_compare_to(a.0, b.0));
        v
    }
}
