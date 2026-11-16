//! zombie.characters.SurvivorDesc: a character's name, gender, profession, extras, XP boosts and voice.
use super::survivor_factory::SurvivorType;
use crate::java::JavaRandom;
use crate::java_nio::{read_string, ByteBuffer};
use crate::zombie::core::random::rand_abstract::RandAbstract;

/// zombie.characters.SurvivorDesc, the saved fields. The profession and perk names are kept as saved (the original
/// resolves them through its registries; an unknown perk is dropped there).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurvivorDesc {
    pub id: i32,
    pub forename: String,
    pub surname: String,
    pub torso: String,
    pub female: bool,
    pub profession: String,
    pub extra: Vec<String>,
    /// (perk name, level) in save order
    pub xp_boosts: Vec<(String, i32)>,
    pub voice_prefix: String,
    pub voice_pitch: f32,
    pub voice_type: i32,
    /// `type` (the survivor factory's kind); only a descriptor made by `new` has one set.
    pub survivor_type: SurvivorType,
    /// The eight stats `doStats` rolls. `load` does not run the `doStats` the original calls (its draws belong to the
    /// global Rand, which a save reader does not have), so a loaded descriptor keeps the defaults here.
    pub bravery: f32,
    pub aggressiveness: f32,
    pub compassion: f32,
    pub loner: f32,
    pub temper: f32,
    pub friendliness: f32,
    pub favourindoors: f32,
    pub loyalty: f32,
}

impl SurvivorDesc {
    /// zombie.characters.SurvivorDesc(): `id = idCount++`, then `doStats` (six draws). The field initialisers are the
    /// Java ones (gender FEMALE, voice prefix "VoiceFemale", torso "Base_Torso", names "None", stats 1/0 before
    /// `doStats`). `IsoWorld.instance.survivorDescriptors.put(id, this)` is the caller's registry, not modelled.
    pub fn new<R: JavaRandom>(id_count: &mut i32, rand: &mut RandAbstract<R>) -> Self {
        let id = *id_count;
        *id_count = id_count.wrapping_add(1);
        let mut d = SurvivorDesc {
            id,
            forename: "None".to_string(),
            surname: "None".to_string(),
            torso: "Base_Torso".to_string(),
            female: true,
            voice_prefix: "VoiceFemale".to_string(),
            survivor_type: SurvivorType::Neutral,
            bravery: 1.0,
            aggressiveness: 1.0,
            compassion: 1.0,
            ..SurvivorDesc::default()
        };
        d.do_stats(rand);
        d
    }

    /// zombie.characters.SurvivorDesc.doStats
    fn do_stats<R: JavaRandom>(&mut self, rand: &mut RandAbstract<R>) {
        self.bravery = if rand.next(2) == 0 { 10.0 } else { 0.0 };
        self.aggressiveness = if rand.next(2) == 0 { 10.0 } else { 0.0 };
        self.compassion = 10.0 - self.aggressiveness;
        self.loner = if rand.next(2) == 0 { 10.0 } else { 0.0 };
        self.temper = if rand.next(2) == 0 { 10.0 } else { 0.0 };
        self.friendliness = 10.0 - self.loner;
        self.favourindoors = if rand.next(2) == 0 { 10.0 } else { 0.0 };
        self.loyalty = if rand.next(2) == 0 { 10.0 } else { 0.0 };
    }

    /// zombie.characters.SurvivorDesc.load. World versions before 208 (which draw a random voice) are refused.
    pub fn load(b: &mut ByteBuffer, world_version: i32) -> Result<Self, String> {
        let mut d = SurvivorDesc {
            id: b.get_int()?,
            ..SurvivorDesc::default()
        };
        d.forename = read_string(b)?;
        d.surname = read_string(b)?;
        d.torso = read_string(b)?;
        d.female = b.get_int()? == 1;
        d.profession = read_string(b)?;
        if b.get_int()? == 1 {
            let size = b.get_int()?;
            for _ in 0..size {
                d.extra.push(read_string(b)?);
            }
        }
        let size = b.get_int()?;
        for _ in 0..size {
            // loadPerk
            let perk = read_string(b)?;
            let level = b.get_int()?;
            d.xp_boosts.push((perk, level));
        }
        if world_version < 208 {
            return Err("survivor descriptors of world versions before 208 are not ported".into());
        }
        d.voice_prefix = read_string(b)?;
        d.voice_pitch = b.get_float()?;
        d.voice_type = b.get_int()?;
        Ok(d)
    }
}
