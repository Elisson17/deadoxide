//! zombie.characters.traits.ObservationFactory: what a survivor's look tells about them ("Tough", "Nervous", ...),
//! filled by the game's Lua (`BaseGameCharacterDetails.DoObservations`, an OnGameBoot handler).
use std::collections::HashMap;

use crate::se::krka::kahlua::vm::object::Throwable;

/// zombie.characters.traits.ObservationFactory.Observation
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Observation {
    pub trait_id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub mutually_exclusive: Vec<Option<String>>,
}

/// zombie.characters.traits.ObservationFactory's static `observationMap` (a HashMap with null keys allowed).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObservationFactory {
    pub observation_map: HashMap<Option<String>, Observation>,
}

impl ObservationFactory {
    /// zombie.characters.traits.ObservationFactory.addObservation: `observationMap.put(type, new Observation(type,
    /// name, desc))`.
    pub fn add_observation(&mut self, ty: Option<&str>, name: Option<&str>, desc: Option<&str>) {
        self.observation_map.insert(
            ty.map(str::to_string),
            Observation {
                trait_id: ty.map(str::to_string),
                name: name.map(str::to_string),
                description: desc.map(str::to_string),
                mutually_exclusive: Vec::new(),
            },
        );
    }

    /// zombie.characters.traits.ObservationFactory.setMutualExclusive: each one's list gets the other; a type not
    /// added is a NullPointerException (after `a`'s list got `b` when only `b` is missing).
    pub fn set_mutual_exclusive(
        &mut self,
        a: Option<&str>,
        b: Option<&str>,
    ) -> Result<(), Throwable> {
        let npe = || Throwable::Java {
            class: "java.lang.NullPointerException",
            message: None,
        };
        let ka = a.map(str::to_string);
        let kb = b.map(str::to_string);
        self.observation_map
            .get_mut(&ka)
            .ok_or_else(npe)?
            .mutually_exclusive
            .push(kb.clone());
        self.observation_map
            .get_mut(&kb)
            .ok_or_else(npe)?
            .mutually_exclusive
            .push(ka);
        Ok(())
    }
}
