//! zombie.characters.Stats with zombie.characters.CharacterStat: hunger, thirst, fatigue, panic and the other
//! character stats, as a save stores them.
use crate::java_nio::ByteBuffer;
use crate::zombie::core::math::pz_math;

/// zombie.characters.CharacterStat.ORDERED_STATS: (id, minimum, maximum, default) in save order.
pub const ORDERED_STATS: [(&str, f32, f32, f32); 24] = [
    ("Anger", 0.0, 1.0, 0.0),
    ("Boredom", 0.0, 100.0, 0.0),
    ("Discomfort", 0.0, 100.0, 0.0),
    ("Endurance", 0.0, 1.0, 1.0),
    ("Fatigue", 0.0, 1.0, 0.0),
    ("Fitness", -1.0, 1.0, 0.0),
    ("FoodSickness", 0.0, 100.0, 0.0),
    ("Hunger", 0.0, 1.0, 0.0),
    ("Idleness", 0.0, 1.0, 0.0),
    ("Intoxication", 0.0, 100.0, 0.0),
    ("Morale", 0.0, 1.0, 1.0),
    ("NicotineWithdrawal", 0.0, 0.51, 0.0),
    ("Pain", 0.0, 100.0, 0.0),
    ("Panic", 0.0, 100.0, 0.0),
    ("Poison", 0.0, 100.0, 0.0),
    ("Sanity", 0.0, 1.0, 1.0),
    ("Sickness", 0.0, 1.0, 0.0),
    ("Stress", 0.0, 1.0, 0.0),
    ("Temperature", 20.0, 40.0, 37.0),
    ("Thirst", 0.0, 1.0, 0.0),
    ("Unhappiness", 0.0, 100.0, 0.0),
    ("Wetness", 0.0, 100.0, 0.0),
    ("ZombieFever", 0.0, 100.0, 0.0),
    ("ZombieInfection", 0.0, 100.0, 0.0),
];

/// zombie.characters.Stats, the saved values in `ORDERED_STATS` order.
#[derive(Clone, Debug, PartialEq)]
pub struct Stats {
    pub values: [f32; 24],
}

impl Stats {
    /// zombie.characters.Stats.load: one float per stat, through `set` (`CharacterStat.clamp`).
    pub fn load(b: &mut ByteBuffer) -> Result<Self, String> {
        let mut values = [0.0; 24];
        for (v, (_, min, max, _)) in values.iter_mut().zip(ORDERED_STATS) {
            *v = pz_math::clamp(b.get_float()?, min, max);
        }
        Ok(Stats { values })
    }
}
