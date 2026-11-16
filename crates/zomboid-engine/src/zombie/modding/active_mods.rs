//! zombie.modding.ActiveMods: named lists of mod ids (and map folders), e.g. "default" (the main menu's choice,
//! `mods/default.txt`) or a server's; `s_loaded` is the copy of the list last loaded. The class's statics are one
//! `ActiveModsState` value; lists are referred to by their index in `s_activeMods`.
use crate::java;
use crate::zombie::core::translator::Translator;
use crate::zombie::game_states::choose_game_info::ChooseGameInfo;
use crate::zombie::util::string_utils::is_null_or_whitespace;
use crate::zombie::zomboid_file_system::ZomboidFileSystem;

/// An exception out of ActiveMods (or a part of the original the port does not have).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveModsError {
    /// the Java exception's simple class name
    pub class: &'static str,
    pub message: String,
}

impl ActiveModsError {
    /// A step of the original that is not ported.
    pub fn not_ported(what: &str) -> ActiveModsError {
        ActiveModsError {
            class: "NotPorted",
            message: format!("{what} is not ported"),
        }
    }
}

/// zombie.modding.ActiveMods
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveMods {
    id: String,
    mods: Vec<String>,
    map_order: Vec<String>,
}

/// zombie.modding.ActiveMods.requireValidId: IllegalArgumentException for an id that is empty or whitespace.
fn require_valid_id(id: &str) -> Result<(), ActiveModsError> {
    if is_null_or_whitespace(Some(id)) {
        return Err(ActiveModsError {
            class: "IllegalArgumentException",
            message: "id is null or whitespace".into(),
        });
    }
    Ok(())
}

impl ActiveMods {
    /// zombie.modding.ActiveMods.<init>
    pub fn new(id: &str) -> Result<ActiveMods, ActiveModsError> {
        require_valid_id(id)?;
        Ok(ActiveMods {
            id: id.to_string(),
            mods: Vec::new(),
            map_order: Vec::new(),
        })
    }

    /// The list's id, as created (`id`).
    pub fn id(&self) -> &str {
        &self.id
    }

    /// zombie.modding.ActiveMods.clear
    pub fn clear(&mut self) {
        self.mods.clear();
        self.map_order.clear();
    }

    /// zombie.modding.ActiveMods.getMods
    pub fn get_mods(&self) -> &[String] {
        &self.mods
    }

    /// zombie.modding.ActiveMods.getMods, to change.
    pub fn get_mods_mut(&mut self) -> &mut Vec<String> {
        &mut self.mods
    }

    /// zombie.modding.ActiveMods.getMapOrder
    pub fn get_map_order(&self) -> &[String] {
        &self.map_order
    }

    /// zombie.modding.ActiveMods.getMapOrder, to change.
    pub fn get_map_order_mut(&mut self) -> &mut Vec<String> {
        &mut self.map_order
    }

    /// zombie.modding.ActiveMods.copyFrom
    pub fn copy_from(&mut self, other: &ActiveMods) {
        self.mods.clear();
        self.map_order.clear();
        self.mods.extend(other.mods.iter().cloned());
        self.map_order.extend(other.map_order.iter().cloned());
    }

    /// zombie.modding.ActiveMods.setModActive: the trimmed id added at the end (once) or removed (its first
    /// occurrence); an empty or whitespace id changes nothing.
    pub fn set_mod_active(&mut self, mod_id: &str, active: bool) {
        let mod_id = java::trim(mod_id);
        if is_null_or_whitespace(Some(mod_id)) {
            return;
        }
        if active {
            if !self.mods.iter().any(|m| m == mod_id) {
                self.mods.push(mod_id.to_string());
            }
        } else {
            self.remove_first(mod_id);
        }
    }

    fn remove_first(&mut self, mod_id: &str) {
        if let Some(i) = self.mods.iter().position(|m| m == mod_id) {
            self.mods.remove(i);
        }
    }

    /// zombie.modding.ActiveMods.isModActive
    pub fn is_mod_active(&self, mod_id: &str) -> bool {
        let mod_id = java::trim(mod_id);
        !is_null_or_whitespace(Some(mod_id)) && self.mods.iter().any(|m| m == mod_id)
    }

    /// zombie.modding.ActiveMods.removeMod: the trimmed id's first occurrence.
    pub fn remove_mod(&mut self, mod_id: &str) {
        self.remove_first(java::trim(mod_id));
    }

    /// zombie.modding.ActiveMods.removeMapOrder
    pub fn remove_map_order(&mut self, folder: &str) {
        if let Some(i) = self.map_order.iter().position(|m| m == folder) {
            self.map_order.remove(i);
        }
    }
}

/// The statics of zombie.modding.ActiveMods: `s_activeMods` and `s_loaded`.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveModsState {
    s_active_mods: Vec<ActiveMods>,
    s_loaded: ActiveMods,
}

impl Default for ActiveModsState {
    fn default() -> Self {
        ActiveModsState {
            s_active_mods: Vec::new(),
            s_loaded: ActiveMods {
                id: "loaded".into(),
                mods: Vec::new(),
                map_order: Vec::new(),
            },
        }
    }
}

impl ActiveModsState {
    /// zombie.modding.ActiveMods.count
    pub fn count(&self) -> usize {
        self.s_active_mods.len()
    }

    /// zombie.modding.ActiveMods.getByIndex
    pub fn get(&self, index: usize) -> &ActiveMods {
        &self.s_active_mods[index]
    }

    /// zombie.modding.ActiveMods.getByIndex, to change.
    pub fn get_mut(&mut self, index: usize) -> &mut ActiveMods {
        &mut self.s_active_mods[index]
    }

    /// Every list, in creation order (`s_activeMods`).
    pub fn all(&self) -> &[ActiveMods] {
        &self.s_active_mods
    }

    /// `s_loaded`: the list last loaded.
    pub fn loaded(&self) -> &ActiveMods {
        &self.s_loaded
    }

    /// zombie.modding.ActiveMods.getById: the list whose id equals the trimmed `id` ignoring case, else a new list
    /// under `id` as given (untrimmed).
    pub fn get_by_id(&mut self, id: &str) -> Result<usize, ActiveModsError> {
        match self.index_of(id)? {
            -1 => self.create(id),
            i => Ok(i as usize),
        }
    }

    /// zombie.modding.ActiveMods.indexOf
    pub fn index_of(&self, id: &str) -> Result<i32, ActiveModsError> {
        let id = java::trim(id);
        require_valid_id(id)?;
        Ok(self
            .s_active_mods
            .iter()
            .position(|a| java::equals_ignore_case(&a.id, id))
            .map_or(-1, |i| i as i32))
    }

    /// zombie.modding.ActiveMods.create
    fn create(&mut self, id: &str) -> Result<usize, ActiveModsError> {
        require_valid_id(id)?;
        if self.index_of(id)? != -1 {
            return Err(ActiveModsError {
                class: "IllegalStateException",
                message: format!("id \"{id}\" exists"),
            });
        }
        self.s_active_mods.push(ActiveMods::new(id)?);
        Ok(self.s_active_mods.len() - 1)
    }

    /// zombie.modding.ActiveMods.setLoadedMods: `s_loaded` becomes a copy of the list.
    pub fn set_loaded_mods(&mut self, index: usize) {
        let list = self.s_active_mods[index].clone();
        self.s_loaded.copy_from(&list);
    }

    /// zombie.modding.ActiveMods.requiresResetLua: the list's mods differ from the loaded ones.
    pub fn requires_reset_lua(&self, index: usize) -> bool {
        self.s_loaded.mods != self.s_active_mods[index].mods
    }

    /// zombie.modding.ActiveMods.Reset: `s_loaded` cleared.
    pub fn reset(&mut self) {
        self.s_loaded.clear();
    }

    /// zombie.modding.ActiveMods.checkMissingMods: from the last to the first, each mod that
    /// `ChooseGameInfo.getAvailableModDetails` does not find available is removed.
    pub fn check_missing_mods(
        &mut self,
        index: usize,
        cgi: &mut ChooseGameInfo,
        fs: &mut ZomboidFileSystem,
        tr: &mut Translator,
    ) {
        let mut i = self.s_active_mods[index].mods.len();
        while i > 0 {
            i -= 1;
            let mod_id = self.s_active_mods[index].mods[i].clone();
            if cgi.get_available_mod_details(fs, tr, &mod_id).is_none() {
                self.s_active_mods[index].mods.remove(i);
            }
        }
    }
}
