//! zombie.characters.Moodles.MoodleStat: the stat thresholds the moodle levels are read at.
use crate::zombie::scripting::objects::moodle_type::MoodleType;

/// zombie.characters.Moodles.MoodleStat: (minimum, lowest, moderate, highest, maximum) thresholds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoodleStat {
    pub minimum: f32,
    pub lowest: f32,
    pub moderate: f32,
    pub highest: f32,
    pub maximum: f32,
}

const fn stat(minimum: f32, lowest: f32, moderate: f32, highest: f32, maximum: f32) -> MoodleStat {
    MoodleStat {
        minimum,
        lowest,
        moderate,
        highest,
        maximum,
    }
}

impl MoodleStat {
    /// `MoodleStat.get(moodleType)`: the `register` calls of the class; `None` for a type with no stat.
    pub fn get(t: MoodleType) -> Option<MoodleStat> {
        Some(match t {
            MoodleType::Endurance => stat(0.75, 0.5, 0.25, 0.1, 0.0),
            MoodleType::Angry => stat(0.0, 0.1, 0.25, 0.5, 0.75),
            MoodleType::Tired => stat(0.0, 0.6, 0.7, 0.8, 0.9),
            MoodleType::Hungry => stat(0.0, 0.15, 0.25, 0.45, 0.7),
            MoodleType::Panic => stat(0.0, 6.0, 30.0, 65.0, 80.0),
            MoodleType::Sick => stat(0.0, 0.25, 0.5, 0.75, 0.9),
            MoodleType::Bored => stat(0.0, 25.0, 50.0, 75.0, 90.0),
            MoodleType::Unhappy => stat(0.0, 20.0, 45.0, 60.0, 80.0),
            MoodleType::Stress => stat(0.0, 0.25, 0.5, 0.75, 0.9),
            MoodleType::Thirst => stat(0.0, 0.12, 0.25, 0.7, 0.84),
            MoodleType::Pain => stat(0.0, 10.0, 20.0, 50.0, 75.0),
            MoodleType::Wet => stat(0.0, 15.0, 40.0, 70.0, 90.0),
            MoodleType::HasACold => stat(0.0, 20.0, 40.0, 60.0, 75.0),
            MoodleType::Injured => stat(0.0, 20.0, 40.0, 60.0, 75.0),
            MoodleType::Drunk => stat(0.0, 10.0, 30.0, 50.0, 70.0),
            MoodleType::Uncomfortable => stat(0.0, 20.0, 40.0, 60.0, 80.0),
            MoodleType::NoxiousSmell => stat(0.0, 0.001, 0.001, 0.002, 0.002),
            MoodleType::Hypothermia => stat(0.0, 30.0, 70.0, 9.0, 100.0),
            MoodleType::Windchill => stat(0.0, 5.0, 10.0, 15.0, 20.0),
            MoodleType::HeavyLoad => stat(0.0, 1.0, 1.25, 1.5, 1.75),
            _ => return None,
        })
    }
}
