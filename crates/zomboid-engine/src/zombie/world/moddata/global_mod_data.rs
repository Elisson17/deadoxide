//! zombie.world.moddata.GlobalModData: the world's Lua tables that the game's and mods' Lua keep by tag
//! (`ModData.getOrCreate`), saved in `global_mod_data.bin`.
use crate::java::hash_map_order;
use crate::java_nio::{read_string, ByteBuffer};
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTable;

/// zombie.world.moddata.GlobalModData: the `HashMap<String, KahluaTable>`, kept in first-put order (a later put of
/// the same tag replaces the table in place); `collect_table_names` gives the HashMap's iteration order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GlobalModData {
    pub mod_data: Vec<(String, KahluaTable)>,
}

impl GlobalModData {
    /// zombie.world.moddata.GlobalModData.reset (the static `lastBlockSize`, which only sizes saving, is not modelled).
    pub fn reset(&mut self) {
        self.mod_data.clear();
    }

    /// zombie.world.moddata.GlobalModData.get
    pub fn get(&self, tag: &str) -> Option<&KahluaTable> {
        self.mod_data.iter().find(|(t, _)| t == tag).map(|(_, v)| v)
    }

    /// zombie.world.moddata.GlobalModData.exists
    pub fn exists(&self, tag: &str) -> bool {
        self.get(tag).is_some()
    }

    /// `HashMap.put`
    fn put(&mut self, tag: String, table: KahluaTable) {
        match self.mod_data.iter_mut().find(|(t, _)| *t == tag) {
            Some(e) => e.1 = table,
            None => self.mod_data.push((tag, table)),
        }
    }

    /// zombie.world.moddata.GlobalModData.collectTableNames: the tags in the HashMap's iteration order.
    pub fn collect_table_names(&self) -> Vec<String> {
        let keys: Vec<&str> = self.mod_data.iter().map(|(t, _)| t.as_str()).collect();
        hash_map_order(&keys)
            .into_iter()
            .map(|i| keys[i].to_string())
            .collect()
    }

    /// zombie.world.moddata.GlobalModData.load over the bytes of the save's `global_mod_data.bin` (`None`: no file,
    /// nothing changes). The tables are dropped, then the world version, the count, and per table its block size
    /// (unused), tag and `KahluaTableImpl.load`. `Err` where the original throws (it rethrows as
    /// `IOException("Error loading GlobalModData.")`); the tables read before stay. `Core.isNoSave` is not
    /// modelled (false).
    pub fn load(&mut self, data: Option<&[u8]>) -> Result<(), String> {
        let Some(data) = data else {
            return Ok(());
        };
        self.mod_data.clear();
        let mut bb = ByteBuffer::wrap(data);
        let world_version = bb.get_int()?;
        let size = bb.get_int()?;
        for _ in 0..size {
            let _data_block_size = bb.get_int()?;
            let tag = read_string(&mut bb)?;
            let table = KahluaTable::load(&mut bb, world_version)?;
            self.put(tag, table);
        }
        Ok(())
    }
}
