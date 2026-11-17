//! zombie.scripting.objects.MoodleType: the moodles of the base game, in registration order.

/// zombie.scripting.objects.MoodleType (the `registerBase` constants; the index is the registry position).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum MoodleType {
    Endurance,
    Tired,
    Hungry,
    Panic,
    Sick,
    Bored,
    Unhappy,
    Bleeding,
    Wet,
    HasACold,
    Angry,
    Stress,
    Thirst,
    Injured,
    Pain,
    HeavyLoad,
    Drunk,
    Dead,
    Zombie,
    Hyperthermia,
    Hypothermia,
    Windchill,
    CantSprint,
    Uncomfortable,
    NoxiousSmell,
    FoodEaten,
}

/// `Registries.MOODLE_TYPE.values()`
pub const ALL_MOODLE_TYPES: [MoodleType; 26] = [
    MoodleType::Endurance,
    MoodleType::Tired,
    MoodleType::Hungry,
    MoodleType::Panic,
    MoodleType::Sick,
    MoodleType::Bored,
    MoodleType::Unhappy,
    MoodleType::Bleeding,
    MoodleType::Wet,
    MoodleType::HasACold,
    MoodleType::Angry,
    MoodleType::Stress,
    MoodleType::Thirst,
    MoodleType::Injured,
    MoodleType::Pain,
    MoodleType::HeavyLoad,
    MoodleType::Drunk,
    MoodleType::Dead,
    MoodleType::Zombie,
    MoodleType::Hyperthermia,
    MoodleType::Hypothermia,
    MoodleType::Windchill,
    MoodleType::CantSprint,
    MoodleType::Uncomfortable,
    MoodleType::NoxiousSmell,
    MoodleType::FoodEaten,
];

impl MoodleType {
    /// `MoodleType.getTranslationName`, the id the type was registered with.
    pub fn translation_name(self) -> &'static str {
        const NAMES: [&str; 26] = [
            "Endurance",
            "Tired",
            "Hungry",
            "Panic",
            "Sick",
            "Bored",
            "Unhappy",
            "Bleeding",
            "Wet",
            "HasACold",
            "Angry",
            "Stress",
            "Thirst",
            "Injured",
            "Pain",
            "HeavyLoad",
            "Drunk",
            "Dead",
            "Zombie",
            "Hyperthermia",
            "Hypothermia",
            "Windchill",
            "CantSprint",
            "Uncomfortable",
            "NoxiousSmell",
            "FoodEaten",
        ];
        NAMES[self as usize]
    }
}
