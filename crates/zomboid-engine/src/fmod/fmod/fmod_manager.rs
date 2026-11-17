//! fmod.fmod.FMODManager: the part of `init()` that reads the banks: the event path list and the event description map.
use std::collections::HashMap;
use std::path::Path;

use crate::java;
use crate::native::fmod::events::EventParam;
use crate::native::fmod::studio::StudioSystem;

/// fmod.fmod.FMOD_STUDIO_PARAMETER_DESCRIPTION (`name`, `id`, `flags`, `globalIndex`). The id of a parameter the
/// event records never mention (the global ones no event uses: 4 in the game's banks) is a hash the banks do not
/// hold; it stays `None` (UNKNOWN) there.
#[derive(Clone, Debug)]
pub struct FmodStudioParameterDescription {
    pub name: String,
    pub id: Option<[u8; 8]>,
    pub flags: i32,
    pub global_index: usize,
}

/// fmod.fmod.FMOD_STUDIO_EVENT_DESCRIPTION (the `address` is a native pointer and has no value here). `parameters`
/// holds indexes into `FmodManager::parameters` (the Java list holds the shared description objects).
#[derive(Clone, Debug)]
pub struct FmodStudioEventDescription {
    pub path: String,
    pub id: [u8; 16],
    pub has_sustain_points: bool,
    pub length: u64,
    pub parameters: Vec<usize>,
    /// what `GetParameterDescriptionByIndex` returned for each of them (the name, flags and id of this event's record;
    /// the Java object keeps only the shared description)
    pub parameter_records: Vec<EventParam>,
}

pub struct FmodManager {
    pub system: StudioSystem,
    /// `eventPathList`, sorted with `String::compareToIgnoreCase` after `initEvents`
    pub event_path_list: Vec<String>,
    /// `eventDescriptionMap`: the lower-cased path to the description (a later bank's event replaces an earlier one)
    pub event_description_map: HashMap<String, FmodStudioEventDescription>,
    /// `parameterDescriptionMap` values in insertion order (`globalIndex` is the position)
    pub parameters: Vec<FmodStudioParameterDescription>,
    /// `parameterDescriptionMap`: the name to the position in `parameters`
    pub parameter_description_map: HashMap<String, usize>,
}

impl FmodManager {
    /// `FMODManager.init` up to `initEvents`: the banks are loaded in order, then the global parameters, then the
    /// events of every bank in the order of `FMOD_Studio_System_GetBankList`.
    pub fn init(root: &Path) -> Result<FmodManager, String> {
        let system = StudioSystem::init_banks(root)?;
        let mut m = FmodManager {
            system,
            event_path_list: Vec::new(),
            event_description_map: HashMap::new(),
            parameters: Vec::new(),
            parameter_description_map: HashMap::new(),
        };
        m.init_global_parameters()?;
        m.init_events()?;
        Ok(m)
    }

    /// `FMODManager.initGlobalParameters`: `GetParameterDescriptionList` and `initParameterDescription` for each.
    fn init_global_parameters(&mut self) -> Result<(), String> {
        for p in self.system.parameter_description_list()? {
            self.init_parameter_description(&p.name, p.flags(), None);
        }
        Ok(())
    }

    /// `FMODManager.initParameterDescription`: the first description of a name stays; `globalIndex` is the map size
    /// at that moment. `id` is what the caller knows of the parameter's id: the global list gives none here, the
    /// event records give it, and a description created without one takes the first id an event reports.
    fn init_parameter_description(&mut self, name: &str, flags: i32, id: Option<[u8; 8]>) -> usize {
        if let Some(&ix) = self.parameter_description_map.get(name) {
            let p = &mut self.parameters[ix];
            if p.id.is_none() {
                p.id = id;
            }
            return ix;
        }
        let ix = self.parameters.len();
        self.parameters.push(FmodStudioParameterDescription {
            name: name.to_string(),
            id,
            flags,
            global_index: ix,
        });
        self.parameter_description_map.insert(name.to_string(), ix);
        ix
    }

    /// `FMODManager.initEvents` / `initEvent`
    fn init_events(&mut self) -> Result<(), String> {
        for i in 0..self.system.bank_count() {
            let list = self.system.bank_event_list(i)?;
            let details = self.system.bank_event_details(i)?;
            for ((id, path), d) in list.into_iter().zip(details) {
                let mut parameters = Vec::with_capacity(d.parameters.len());
                for p in &d.parameters {
                    parameters.push(self.init_parameter_description(&p.name, p.flags, Some(p.id)));
                }
                self.event_description_map.insert(
                    java::to_lower_case(&path),
                    FmodStudioEventDescription {
                        path: path.clone(),
                        id,
                        has_sustain_points: d.has_sustain_points,
                        length: d.length,
                        parameters,
                        parameter_records: d.parameters,
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

    /// `getParameterDescription`
    pub fn get_parameter_description(&self, name: &str) -> Option<&FmodStudioParameterDescription> {
        self.parameter_description_map
            .get(name)
            .map(|&i| &self.parameters[i])
    }
}
