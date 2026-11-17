//! The part of `FMOD::Studio::System` that `fmod.javafmodJNI` exposes for banks: `LoadBankFile`, the bank list and the
//! per-bank event and VCA counts, as the game's `FMODManager.init()` uses them.
use std::path::{Path, PathBuf};

use super::bank::Bank;

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
}
