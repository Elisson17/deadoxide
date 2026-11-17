//! fmod.fmod.FMODManager: the part of `init()` that reads the banks: the event path list and the event description map.
use std::collections::HashMap;
use std::path::Path;

use crate::java;
use crate::native::fmod::studio::StudioSystem;

/// fmod.fmod.FMOD_STUDIO_EVENT_DESCRIPTION (the `address` is a native pointer and has no value here).
#[derive(Clone, Debug)]
pub struct FmodStudioEventDescription {
    pub path: String,
    pub id: [u8; 16],
}

pub struct FmodManager {
    pub system: StudioSystem,
    /// `eventPathList`, sorted with `String::compareToIgnoreCase` after `initEvents`
    pub event_path_list: Vec<String>,
    /// `eventDescriptionMap`: the lower-cased path to the description (a later bank's event replaces an earlier one)
    pub event_description_map: HashMap<String, FmodStudioEventDescription>,
}

impl FmodManager {
    /// `FMODManager.init` up to `initEvents`: the banks are loaded in order, then the events of every bank in the order
    /// of `FMOD_Studio_System_GetBankList`.
    pub fn init(root: &Path) -> Result<FmodManager, String> {
        let system = StudioSystem::init_banks(root)?;
        let mut m = FmodManager {
            system,
            event_path_list: Vec::new(),
            event_description_map: HashMap::new(),
        };
        m.init_events()?;
        Ok(m)
    }

    /// `FMODManager.initEvents` / `initEvent`
    fn init_events(&mut self) -> Result<(), String> {
        for i in 0..self.system.bank_count() {
            for (id, path) in self.system.bank_event_list(i)? {
                self.event_description_map.insert(
                    java::to_lower_case(&path),
                    FmodStudioEventDescription {
                        path: path.clone(),
                        id,
                    },
                );
                self.event_path_list.push(path);
            }
        }
        // ArrayList.sort(String::compareToIgnoreCase): stable
        self.event_path_list
            .sort_by(|a, b| java::compare_to_ignore_case(a, b));
        Ok(())
    }

    /// `getEventDescription`: the lookup is case-insensitive
    pub fn get_event_description(&self, path: &str) -> Option<&FmodStudioEventDescription> {
        self.event_description_map.get(&java::to_lower_case(path))
    }
}
