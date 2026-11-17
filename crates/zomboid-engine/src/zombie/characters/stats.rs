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

/// zombie.characters.CharacterStat: the registered stats, in `ORDERED_STATS` order (the index is the position in
/// `Stats::values`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum CharacterStat {
    Anger,
    Boredom,
    Discomfort,
    Endurance,
    Fatigue,
    Fitness,
    FoodSickness,
    Hunger,
    Idleness,
    Intoxication,
    Morale,
    NicotineWithdrawal,
    Pain,
    Panic,
    Poison,
    Sanity,
    Sickness,
    Stress,
    Temperature,
    Thirst,
    Unhappiness,
    Wetness,
    ZombieFever,
    ZombieInfection,
}

impl CharacterStat {
    /// `CharacterStat.getById`
    pub fn by_id(id: &str) -> Option<CharacterStat> {
        ORDERED_STATS
            .iter()
            .position(|s| s.0 == id)
            .map(|i| ALL_STATS[i])
    }

    /// `getId`
    pub fn id(self) -> &'static str {
        ORDERED_STATS[self as usize].0
    }

    /// `getMinimumValue`
    pub fn minimum(self) -> f32 {
        ORDERED_STATS[self as usize].1
    }

    /// `getMaximumValue`
    pub fn maximum(self) -> f32 {
        ORDERED_STATS[self as usize].2
    }

    /// `getDefaultValue`
    pub fn default_value(self) -> f32 {
        ORDERED_STATS[self as usize].3
    }

    /// `CharacterStat.clamp`: `PZMath.clamp(value, min, max)`
    pub fn clamp(self, value: f32) -> f32 {
        pz_math::clamp(value, self.minimum(), self.maximum())
    }

    /// `isAtMinimum(value)`
    pub fn is_at_minimum(self, value: f32) -> bool {
        value <= self.minimum()
    }

    /// `isAtMaximum(value)`
    pub fn is_at_maximum(self, value: f32) -> bool {
        value >= self.maximum()
    }
}

/// `CharacterStat.ORDERED_STATS`
pub const ALL_STATS: [CharacterStat; 24] = [
    CharacterStat::Anger,
    CharacterStat::Boredom,
    CharacterStat::Discomfort,
    CharacterStat::Endurance,
    CharacterStat::Fatigue,
    CharacterStat::Fitness,
    CharacterStat::FoodSickness,
    CharacterStat::Hunger,
    CharacterStat::Idleness,
    CharacterStat::Intoxication,
    CharacterStat::Morale,
    CharacterStat::NicotineWithdrawal,
    CharacterStat::Pain,
    CharacterStat::Panic,
    CharacterStat::Poison,
    CharacterStat::Sanity,
    CharacterStat::Sickness,
    CharacterStat::Stress,
    CharacterStat::Temperature,
    CharacterStat::Thirst,
    CharacterStat::Unhappiness,
    CharacterStat::Wetness,
    CharacterStat::ZombieFever,
    CharacterStat::ZombieInfection,
];

impl Default for Stats {
    /// `new Stats()`: every stat at its default (`get` answers the default of an unset stat).
    fn default() -> Self {
        let mut values = [0.0f32; 24];
        for (v, s) in values.iter_mut().zip(ORDERED_STATS) {
            *v = s.3;
        }
        Stats { values }
    }
}

impl Stats {
    /// `Stats.get`
    pub fn get(&self, stat: CharacterStat) -> f32 {
        self.values[stat as usize]
    }

    /// `Stats.set`: clamped; true when the value changed.
    pub fn set(&mut self, stat: CharacterStat, value: f32) -> bool {
        let old = self.get(stat);
        let new = stat.clamp(value);
        self.values[stat as usize] = new;
        new != old
    }

    /// `Stats.add`
    pub fn add(&mut self, stat: CharacterStat, amount: f32) -> bool {
        self.set(stat, self.get(stat) + amount)
    }

    /// `Stats.remove`
    pub fn remove(&mut self, stat: CharacterStat, amount: f32) -> bool {
        self.add(stat, -amount)
    }

    /// `Stats.reset`
    pub fn reset(&mut self, stat: CharacterStat) -> bool {
        self.set(stat, stat.default_value())
    }

    /// `Stats.isAtMinimum`
    pub fn is_at_minimum(&self, stat: CharacterStat) -> bool {
        stat.is_at_minimum(self.get(stat))
    }

    /// `Stats.isAtMaximum`
    pub fn is_at_maximum(&self, stat: CharacterStat) -> bool {
        stat.is_at_maximum(self.get(stat))
    }

    /// `Stats.isAboveMinimum`
    pub fn is_above_minimum(&self, stat: CharacterStat) -> bool {
        !self.is_at_minimum(stat)
    }

    /// `Stats.getNicotineStress`
    pub fn get_nicotine_stress(&self) -> f32 {
        let nicotine_stress =
            self.get(CharacterStat::Stress) + self.get(CharacterStat::NicotineWithdrawal);
        CharacterStat::Stress.clamp(nicotine_stress)
    }
}
