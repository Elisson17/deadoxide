//! zombie.characters.traits.CharacterTraits: the traits a character has, as a save stores them.
use std::collections::BTreeSet;

use crate::java_nio::{read_string, ByteBuffer};
use crate::zombie::scripting::objects::resource_location::ResourceLocation;

/// zombie.characters.traits.CharacterTraits: `knownTraits` in the order they were added (a trait added twice is in
/// it twice, `remove` takes out the first one) and the `traits` map's true entries (`get`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CharacterTraits {
    pub known_traits: Vec<ResourceLocation>,
    pub flags: BTreeSet<ResourceLocation>,
}

impl CharacterTraits {
    /// zombie.characters.traits.CharacterTraits.load: `reset()`, then each saved trait through
    /// `CharacterTrait.get(ResourceLocation.of(name))` and `add`. The trait registry is not modelled: the original
    /// throws on a trait it does not know.
    pub fn load(b: &mut ByteBuffer) -> Result<Self, String> {
        let mut t = CharacterTraits::default();
        let n = b.get_int()?;
        for _ in 0..n {
            let id = ResourceLocation::of(&read_string(b)?)?;
            t.set(id, true);
        }
        Ok(t)
    }

    /// `CharacterTraits.get`
    pub fn get(&self, id: &ResourceLocation) -> bool {
        self.flags.contains(id)
    }

    /// `CharacterTraits.set`: true when the flag changed.
    pub fn set(&mut self, id: ResourceLocation, value: bool) -> bool {
        let old = self.get(&id);
        if value {
            self.flags.insert(id.clone());
            self.known_traits.push(id);
        } else {
            self.flags.remove(&id);
            if let Some(i) = self.known_traits.iter().position(|t| *t == id) {
                self.known_traits.remove(i);
            }
        }
        value != old
    }

    /// `CharacterTraits.add`
    pub fn add(&mut self, id: ResourceLocation) {
        self.set(id, true);
    }

    /// `CharacterTraits.remove`
    pub fn remove(&mut self, id: ResourceLocation) {
        self.set(id, false);
    }
}
