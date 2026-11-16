//! Statistic, StatisticType, StatisticCategory and Achievement of the statistics package, zombie.AchievementManager and
//! zombie.statistics.StatisticsManager: the read side of `statistics.bin`. Not ported: `incrementStatistic` /
//! `setStatistic` (the writers' callers), `save`, the debug text.
use crate::java::string_compare_to;
use crate::java_nio::{read_utf, ByteBuffer};

/// zombie.statistics.StatisticType
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatisticType {
    Player,
    Zombie,
    World,
    Server,
    Undefined,
}

impl StatisticType {
    /// `name()`
    pub fn name(self) -> &'static str {
        match self {
            StatisticType::Player => "Player",
            StatisticType::Zombie => "Zombie",
            StatisticType::World => "World",
            StatisticType::Server => "Server",
            StatisticType::Undefined => "Undefined",
        }
    }

    /// `valueOf`: the exact name or `IllegalArgumentException`.
    pub fn value_of(name: &str) -> Result<Self, String> {
        [
            StatisticType::Player,
            StatisticType::Zombie,
            StatisticType::World,
            StatisticType::Server,
            StatisticType::Undefined,
        ]
        .into_iter()
        .find(|t| t.name() == name)
        .ok_or_else(|| {
            format!("java.lang.IllegalArgumentException: No enum constant zombie.statistics.StatisticType.{name}")
        })
    }
}

/// zombie.statistics.StatisticCategory
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatisticCategory {
    General,
    Combat,
    Exploration,
    Health,
    Wellness,
    Skill,
    Perks,
    Traits,
    Travel,
    Items,
    Survival,
    Vehicle,
    Pvp,
    Events,
    Undefined,
}

impl StatisticCategory {
    const ALL: [StatisticCategory; 15] = [
        StatisticCategory::General,
        StatisticCategory::Combat,
        StatisticCategory::Exploration,
        StatisticCategory::Health,
        StatisticCategory::Wellness,
        StatisticCategory::Skill,
        StatisticCategory::Perks,
        StatisticCategory::Traits,
        StatisticCategory::Travel,
        StatisticCategory::Items,
        StatisticCategory::Survival,
        StatisticCategory::Vehicle,
        StatisticCategory::Pvp,
        StatisticCategory::Events,
        StatisticCategory::Undefined,
    ];

    /// `name()`
    pub fn name(self) -> &'static str {
        match self {
            StatisticCategory::General => "General",
            StatisticCategory::Combat => "Combat",
            StatisticCategory::Exploration => "Exploration",
            StatisticCategory::Health => "Health",
            StatisticCategory::Wellness => "Wellness",
            StatisticCategory::Skill => "Skill",
            StatisticCategory::Perks => "Perks",
            StatisticCategory::Traits => "Traits",
            StatisticCategory::Travel => "Travel",
            StatisticCategory::Items => "Items",
            StatisticCategory::Survival => "Survival",
            StatisticCategory::Vehicle => "Vehicle",
            StatisticCategory::Pvp => "PVP",
            StatisticCategory::Events => "Events",
            StatisticCategory::Undefined => "Undefined",
        }
    }

    /// `valueOf`: the exact name or `IllegalArgumentException`.
    pub fn value_of(name: &str) -> Result<Self, String> {
        Self::ALL.into_iter().find(|c| c.name() == name).ok_or_else(|| {
            format!(
                "java.lang.IllegalArgumentException: No enum constant zombie.statistics.StatisticCategory.{name}"
            )
        })
    }
}

/// zombie.statistics.Statistic
#[derive(Clone, Debug, PartialEq)]
pub struct Statistic {
    pub name: String,
    pub statistic_type: StatisticType,
    pub statistic_category: StatisticCategory,
    pub value: f32,
}

/// zombie.statistics.Achievement (the part that is state: whether it is unlocked).
#[derive(Clone, Debug, PartialEq)]
pub struct Achievement {
    pub name: &'static str,
    pub threshold: f32,
    pub unlocked: bool,
}

/// zombie.statistics.StatisticsManager with the `AchievementManager` it informs.
#[derive(Clone, Debug, PartialEq)]
pub struct StatisticsManager {
    /// `statisticHashMap`, in first-put order (the original's HashMap order is not observable through any caller
    /// here; callers sort by name).
    pub statistics: Vec<Statistic>,
    /// `AchievementManager.achievementHashMap`: the four built-in achievements.
    pub achievements: Vec<Achievement>,
}

impl Default for StatisticsManager {
    /// The `AchievementManager` constructor's four achievements, all locked, and no statistics.
    fn default() -> Self {
        let a = |name| Achievement {
            name,
            threshold: 10.0,
            unlocked: false,
        };
        StatisticsManager {
            statistics: Vec::new(),
            achievements: vec![
                a("Distance Walked"),
                a("Distance Ran"),
                a("Distance Driven"),
                a("Shots Fired"),
            ],
        }
    }
}

impl StatisticsManager {
    /// `statistics` sorted by name (`String.compareTo`), for printing.
    pub fn sorted(&self) -> Vec<&Statistic> {
        let mut v: Vec<&Statistic> = self.statistics.iter().collect();
        v.sort_by(|a, b| string_compare_to(&a.name, &b.name));
        v
    }

    /// `HashMap.put(name, statistic)`: a name already there is replaced.
    fn put(&mut self, s: Statistic) {
        match self.statistics.iter_mut().find(|e| e.name == s.name) {
            Some(e) => *e = s,
            None => self.statistics.push(s),
        }
    }

    /// `AchievementManager.checkAchievementsOnStatisticLoad`
    fn check_achievements_on_statistic_load(&mut self, name: &str, value: f32) {
        for a in &mut self.achievements {
            if name == a.name && !a.unlocked && value >= a.threshold {
                a.unlocked = true;
                return;
            }
        }
    }

    /// zombie.statistics.StatisticsManager.load over the bytes of `statistics.bin` (`None` is a missing file): the
    /// map is cleared and the achievements reset first, then an int count and per statistic three `readUTF`
    /// strings (name, type, category) and a float. The original logs a failure and keeps the statistics read before
    /// it; this returns the error text.
    pub fn load(&mut self, data: Option<&[u8]>) -> Result<(), String> {
        self.statistics.clear();
        for a in &mut self.achievements {
            a.unlocked = false;
        }
        let Some(data) = data else {
            return Ok(());
        };
        let mut b = ByteBuffer::wrap(data);
        let size = b.get_int()?;
        for _ in 0..size.max(0) {
            let name = read_utf(&mut b)?;
            let statistic_type = StatisticType::value_of(&read_utf(&mut b)?)?;
            let statistic_category = StatisticCategory::value_of(&read_utf(&mut b)?)?;
            let value = b.get_float()?;
            self.check_achievements_on_statistic_load(&name, value);
            self.put(Statistic {
                name,
                statistic_type,
                statistic_category,
                value,
            });
        }
        Ok(())
    }
}
