//! The part of `FMOD::Studio::System` that `fmod.javafmodJNI` exposes for banks: `LoadBankFile`, the bank list and the
//! per-bank event and VCA counts, as the game's `FMODManager.init()` uses them.
use std::path::{Path, PathBuf};

use super::bank::Bank;
use super::events::{EventDetails, EventTables, ParamDef};
use super::strings::StringTable;

/// `fmod.fmod.FMODManager.init`: the banks it asks for, in order (`loadBank(name)` reads
/// `./media/sound/banks/Desktop/<name>.bank`; a bank that does not exist is logged and skipped).
pub const INIT_BANKS: [&str; 12] = [
    "ZomboidSound.strings",
    "ZomboidMusic",
    "ZomboidSound",
    "ZomboidSoundMP",
    "Animal",
    "Broadcast",
    "Character",
    "Object",
    "Vehicle",
    "Weapon",
    "World",
    "Zombie",
];

/// The folder `FMODManager.bankPath` reads banks from, under the game root.
pub const BANK_DIR: &str = "media/sound/banks/Desktop";

#[derive(Default)]
pub struct StudioSystem {
    pub banks: Vec<Bank>,
    pub paths: Vec<PathBuf>,
}

impl StudioSystem {
    /// `FMOD_Studio_System_LoadBankFile`: the bank joins the list when the file loads.
    pub fn load_bank_file(&mut self, path: &Path) -> Result<(), String> {
        let bank = Bank::open(path)?;
        self.banks.push(bank);
        self.paths.push(path.to_path_buf());
        Ok(())
    }

    /// `FMODManager.init`'s `loadBank` calls over a game root (the missing banks are skipped).
    pub fn init_banks(root: &Path) -> Result<StudioSystem, String> {
        let mut s = StudioSystem::default();
        for name in INIT_BANKS {
            let p = root.join(BANK_DIR).join(format!("{name}.bank"));
            if crate::vfs::exists(&p) {
                s.load_bank_file(&p)?;
            }
        }
        Ok(s)
    }

    /// `FMOD_Studio_System_GetBankCount`
    pub fn bank_count(&self) -> usize {
        self.banks.len()
    }

    /// `FMOD_Studio_Bank_GetEventCount` of the bank at `i` of the bank list.
    pub fn bank_event_count(&self, i: usize) -> u32 {
        self.banks[i].bnki.events
    }

    /// `FMOD_Studio_Bank_GetVCACount`: the entries of the `VCAS` list.
    pub fn bank_vca_count(&self, i: usize) -> u32 {
        self.banks[i]
            .list(b"VCAS")
            .and_then(|l| l.lcnt)
            .unwrap_or(0)
    }

    /// The string table of the loaded strings bank (`ZomboidSound.strings.bank`), the one `lookupPath`/`lookupID` use.
    pub fn strings(&self) -> Option<&StringTable> {
        self.banks.iter().find_map(|b| b.strings.as_ref())
    }

    /// `FMOD_Studio_Bank_GetEventList` of the bank at `i`: the GUIDs and paths of its events, in list order. The events
    /// the Studio API reports are the `EVNT` entries of the bank that have a path in the string table (measured over all
    /// four banks: 3215, 272, 7 of 4056, 379, 78 entries, the count of `BNKI`; the 841, 107 and 71 other entries are
    /// referenced by event instruments and have no path; the flag in the `EVTB` record that tells them apart is not
    /// decoded: UNKNOWN). `Err` when the count disagrees with `BNKI`.
    pub fn bank_event_list(&self, i: usize) -> Result<Vec<([u8; 16], String)>, String> {
        let bank = &self.banks[i];
        let mut out = Vec::new();
        if let Some(st) = self.strings() {
            for g in bank.event_guids()? {
                if let Some(path) = st.lookup_path(&g) {
                    out.push((g, path));
                }
            }
        }
        if out.len() as u32 != bank.bnki.events {
            return Err(format!(
                "{}: {} named events, BNKI says {}",
                bank.name,
                out.len(),
                bank.bnki.events
            ));
        }
        Ok(out)
    }

    /// What `FMODManager.initEvent` asks each event of the bank at `i` for (sustain point, length, parameters), in the
    /// order of `bank_event_list`.
    pub fn bank_event_details(&self, i: usize) -> Result<Vec<EventDetails>, String> {
        let bank = &self.banks[i];
        let tables = EventTables::build(bank)?;
        self.bank_event_list(i)?
            .iter()
            .map(|(g, _)| tables.details(bank, g))
            .collect()
    }

    /// `FMOD_Studio_System_GetParameterDescriptionList`: the global parameters of all loaded banks, once each by GUID
    /// (the banks that share a parameter hold the same record), in GUID order (measured: the 43 of the game's banks
    /// come in this order).
    pub fn parameter_description_list(&self) -> Result<Vec<ParamDef>, String> {
        let mut all: Vec<ParamDef> = Vec::new();
        for b in &self.banks {
            for p in EventTables::build(b)?.params {
                if p.is_global() && !all.iter().any(|q| q.guid == p.guid) {
                    all.push(p);
                }
            }
        }
        all.sort_by_key(|a| a.guid);
        Ok(all)
    }

    /// `FMOD_Studio_Bank_GetVCAList` of the bank at `i` with `FMOD_Studio_VCA_GetPath`: the paths in list order.
    pub fn bank_vca_list(&self, i: usize) -> Result<Vec<String>, String> {
        let st = self.strings();
        self.banks[i]
            .vca_guids()?
            .iter()
            .map(|g| {
                st.and_then(|t| t.lookup_path(g))
                    .ok_or_else(|| "a VCA has no path".to_string())
            })
            .collect()
    }

    /// The GUID an object path names when it starts with `prefix` (`bus:/`, `vca:/`), compared without ASCII case.
    fn typed_lookup(&self, path: &str, prefix: &str) -> Option<[u8; 16]> {
        let head = path.get(..prefix.len())?;
        if !head.eq_ignore_ascii_case(prefix) {
            return None;
        }
        self.strings()?.lookup_id(path)
    }

    /// `FMOD_Studio_System_GetBus` as the probe sees it: whether the path names a bus. The kind is taken from the
    /// `bus:/` prefix of the path (measured: every `bus:/` string of the table is a bus in any case and no string of
    /// another kind is; the GUID is not looked up in the banks' bus lists).
    pub fn get_bus(&self, path: &str) -> bool {
        self.typed_lookup(path, "bus:/").is_some()
    }

    /// `FMOD_Studio_System_GetVCA`, with the `vca:/` prefix standing for the kind as in `get_bus`.
    pub fn get_vca(&self, path: &str) -> bool {
        self.typed_lookup(path, "vca:/").is_some()
    }
}
