//! zombie.characters.traits.CharacterTraits: the traits a character has, as a save stores them.
use crate::java_nio::{read_string, ByteBuffer};
use crate::zombie::scripting::objects::resource_location::ResourceLocation;

/// zombie.characters.traits.CharacterTraits, `knownTraits` in the order they were added.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CharacterTraits {
    pub known_traits: Vec<ResourceLocation>,
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
            // set(trait, true): knownTraits.add, even when it is already there
            t.known_traits.push(id);
        }
        Ok(t)
    }
}
