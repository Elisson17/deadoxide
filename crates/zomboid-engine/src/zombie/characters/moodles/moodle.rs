//! zombie.characters.Moodles.Moodle: one moodle and the rule that turns the character's state into its level.
use super::moodle_stat::MoodleStat;
use crate::zombie::characters::body_damage::body_damage::BodyDamage;
use crate::zombie::characters::body_damage::body_sim::{BodyWorld, CharCtx};
use crate::zombie::characters::stats::{CharacterStat, Stats};
use crate::zombie::scripting::objects::moodle_type::MoodleType;

/// `Moodle.MoodleLevel`
pub const MIN_LEVEL: i32 = 0;
pub const LOW_LEVEL: i32 = 1;
pub const MODERATE_LEVEL: i32 = 2;
pub const HIGH_LEVEL: i32 = 3;
pub const MAX_LEVEL: i32 = 4;

const CANT_SPRINT_TIMER_DURATION: i32 = 300;
const PAIN_TIMER_DURATION: i32 = 120;

/// The colours a chevron takes: `Color.white`, `colorNeg` (0.88235295, 0.15686275, 0.15686275) and `colorPos`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChevronColor {
    White,
    Neg,
    Pos,
}

/// zombie.characters.Moodles.Moodle
#[derive(Clone, Debug, PartialEq)]
pub struct Moodle {
    pub moodle_type: MoodleType,
    pub level: i32,
    pub pain_timer: i32,
    pub chevron_color: ChevronColor,
    pub chevron_is_up: bool,
    pub chevron_count: i32,
    pub cant_sprint_timer: i32,
}

/// What `Moodle.Update` reads of the character it belongs to.
pub struct MoodleCtx<'a> {
    pub stats: &'a Stats,
    pub body: &'a BodyDamage,
    pub ch: &'a mut CharCtx,
    pub world: &'a BodyWorld,
}

fn threshold(t: MoodleType) -> Result<MoodleStat, String> {
    MoodleStat::get(t)
        .ok_or_else(|| "java.lang.NullPointerException: no MoodleStat for the moodle".to_string())
}

/// The level of a stat that rises through the thresholds (`> lowest` ... `> maximum`, each overriding the last).
fn rising(value: f32, s: &MoodleStat, level: i32) -> i32 {
    let mut l = level;
    if value > s.lowest {
        l = LOW_LEVEL;
    }
    if value > s.moderate {
        l = MODERATE_LEVEL;
    }
    if value > s.highest {
        l = HIGH_LEVEL;
    }
    if value > s.maximum {
        l = MAX_LEVEL;
    }
    l
}

impl Moodle {
    /// `new Moodle(type, character)`
    pub fn new(moodle_type: MoodleType) -> Self {
        Moodle {
            moodle_type,
            level: MIN_LEVEL,
            pain_timer: 0,
            chevron_color: ChevronColor::White,
            chevron_is_up: true,
            chevron_count: 0,
            cant_sprint_timer: CANT_SPRINT_TIMER_DURATION,
        }
    }

    /// `Moodle.chevronDifference`
    fn chevron_difference(&self, count: i32, is_up: bool, col: ChevronColor) -> bool {
        count != self.chevron_count || is_up != self.chevron_is_up || col != self.chevron_color
    }

    /// `Moodle.setChevron`
    fn set_chevron(&mut self, count: i32, is_up: bool, col: ChevronColor) {
        self.chevron_count = count.clamp(0, 3);
        self.chevron_is_up = is_up;
        self.chevron_color = col;
    }

    /// `Moodle.updateMoodleLevel`
    fn update_moodle_level(&mut self, level: i32) -> bool {
        if level != self.level {
            self.level = MIN_LEVEL.max(level.min(MAX_LEVEL));
            true
        } else {
            false
        }
    }

    /// zombie.characters.Moodles.Moodle.Update: true when the level changed. The paths that need the thermoregulator's
    /// chevrons, a building or a toxic-air protection answer `Err("not ported: ...")` when reached.
    pub fn update(&mut self, c: &mut MoodleCtx) -> Result<bool, String> {
        use MoodleType as T;
        let mut level = MIN_LEVEL;
        let t = self.moodle_type;
        let dead = c.ch.is_dead;
        if dead && t != T::Dead && t != T::Zombie {
            return Ok(self.update_moodle_level(level));
        }
        let health = c.body.overall_body_health;
        let alive = health != 0.0;
        let stats = c.stats;
        if t == T::CantSprint && c.ch.moodle_cant_sprint {
            level = LOW_LEVEL;
            self.cant_sprint_timer -= 1;
            if self.cant_sprint_timer == 0 {
                level = MIN_LEVEL;
                self.cant_sprint_timer = CANT_SPRINT_TIMER_DURATION;
                c.ch.moodle_cant_sprint = false;
            }
        }
        if t == T::Endurance && alive {
            let endurance = stats.get(CharacterStat::Endurance);
            let s = threshold(T::Endurance)?;
            level = if endurance > s.minimum {
                MIN_LEVEL
            } else if endurance > s.lowest {
                LOW_LEVEL
            } else if endurance > s.moderate {
                MODERATE_LEVEL
            } else if endurance > s.highest {
                HIGH_LEVEL
            } else {
                MAX_LEVEL
            };
        }
        if t == T::Angry {
            let anger = stats.get(CharacterStat::Anger);
            let s = threshold(T::Angry)?;
            if anger > s.maximum {
                level = MAX_LEVEL;
            } else if anger > s.highest {
                level = HIGH_LEVEL;
            } else if anger > s.moderate {
                level = MODERATE_LEVEL;
            } else if anger > s.lowest {
                level = LOW_LEVEL;
            }
        }
        let simple_stat = match t {
            T::Tired => Some(CharacterStat::Fatigue),
            T::Hungry => Some(CharacterStat::Hunger),
            T::Panic => Some(CharacterStat::Panic),
            T::Bored => Some(CharacterStat::Boredom),
            T::Unhappy => Some(CharacterStat::Unhappiness),
            T::Wet => Some(CharacterStat::Wetness),
            T::Drunk => Some(CharacterStat::Intoxication),
            _ => None,
        };
        if let Some(st) = simple_stat {
            if alive {
                level = rising(stats.get(st), &threshold(t)?, level);
            }
        }
        if t == T::Sick && alive {
            let sickness = c.body.get_apparent_infection_level(stats) / 100.0
                + stats.get(CharacterStat::Sickness);
            level = rising(sickness, &threshold(T::Sick)?, level);
        }
        if t == T::Stress {
            let effective = stats.get_nicotine_stress();
            let s = threshold(T::Stress)?;
            if effective > s.maximum {
                level = MAX_LEVEL;
            } else if effective > s.highest {
                level = HIGH_LEVEL;
            } else if effective > s.moderate {
                level = MODERATE_LEVEL;
            } else if effective > s.lowest {
                level = LOW_LEVEL;
            }
        }
        if t == T::Thirst {
            level = rising(
                stats.get(CharacterStat::Thirst),
                &threshold(T::Thirst)?,
                level,
            );
        }
        if t == T::Bleeding && alive {
            level = c.body.get_num_parts_bleeding();
            if c.body.is_neck_bleeding() {
                level = MAX_LEVEL;
            }
            if level > MAX_LEVEL {
                level = MAX_LEVEL;
            }
        }
        if t == T::HasACold && alive {
            level = rising(c.body.cold_strength, &threshold(T::HasACold)?, level);
        }
        if t == T::Injured && alive {
            level = rising(100.0 - health, &threshold(T::Injured)?, level);
        }
        if t == T::Pain {
            self.pain_timer += 1;
            if self.pain_timer < PAIN_TIMER_DURATION {
                return Ok(false);
            }
            self.pain_timer = 0;
            if alive {
                level = rising(stats.get(CharacterStat::Pain), &threshold(T::Pain)?, level);
            }
            // the voice line `playerVoiceSound("PainMoodle")` is a local-player sound, not state
        }
        if t == T::HeavyLoad {
            let ratio = c.ch.capacity_weight / c.ch.max_weight as f32;
            if alive {
                let s = threshold(T::HeavyLoad)?;
                if ratio >= s.maximum {
                    level = MAX_LEVEL;
                } else if ratio >= s.highest {
                    level = HIGH_LEVEL;
                } else if ratio >= s.moderate {
                    level = MODERATE_LEVEL;
                } else if ratio > s.lowest {
                    level = LOW_LEVEL;
                }
            }
        }
        if t == T::Dead && dead {
            level = MAX_LEVEL;
            if !c.body.is_fake_infected && stats.get(CharacterStat::ZombieInfection) >= 0.001 {
                level = MIN_LEVEL;
            }
        }
        if t == T::Zombie
            && dead
            && !c.body.is_fake_infected
            && stats.get(CharacterStat::ZombieInfection) >= 0.001
        {
            level = MAX_LEVEL;
        }
        if t == T::FoodEaten && alive {
            let timer = c.body.health_from_food_timer;
            let standard = c.body.standard_health_from_food_time as f32;
            if timer > 0.0 {
                level = LOW_LEVEL;
            }
            if timer > standard {
                level = MODERATE_LEVEL;
            }
            if timer > standard * 2.0 {
                level = HIGH_LEVEL;
            }
            if timer > standard * 3.0 {
                level = MAX_LEVEL;
            }
        }
        let mut chev_count = self.chevron_count;
        let mut chev_is_up = self.chevron_is_up;
        let mut chev_col = self.chevron_color;
        let temperature = stats.get(CharacterStat::Temperature);
        if t == T::Hyperthermia || t == T::Hypothermia {
            let below = temperature < 36.5;
            let above = temperature > 37.5;
            if !below && !above {
                chev_count = 0;
            } else {
                match &c.body.thermoregulator {
                    None => chev_count = 0,
                    Some(th) => {
                        chev_is_up = th.thermal_chevron_up();
                        chev_count = th.thermal_chevron_count();
                    }
                }
            }
        }
        if t == T::Hyperthermia {
            if chev_count > 0 {
                chev_col = if chev_is_up {
                    ChevronColor::Neg
                } else {
                    ChevronColor::Pos
                };
            }
            if temperature != 0.0 {
                if temperature > 37.5 {
                    level = LOW_LEVEL;
                }
                if temperature > 39.0 {
                    level = MODERATE_LEVEL;
                }
                if temperature > 40.0 {
                    level = HIGH_LEVEL;
                }
                if temperature > 41.0 {
                    level = MAX_LEVEL;
                }
            }
            if level != self.level
                || (level > MIN_LEVEL && self.chevron_difference(chev_count, chev_is_up, chev_col))
            {
                self.set_chevron(chev_count, chev_is_up, chev_col);
            }
        }
        if t == T::Hypothermia {
            if chev_count > 0 {
                chev_col = if chev_is_up {
                    ChevronColor::Pos
                } else {
                    ChevronColor::Neg
                };
            }
            if temperature != 0.0 {
                let s = threshold(T::Hypothermia)?;
                let intox = stats.get(CharacterStat::Intoxication);
                if temperature < 36.5 && intox <= s.lowest {
                    level = LOW_LEVEL;
                }
                if temperature < 35.0 && intox <= s.moderate {
                    level = MODERATE_LEVEL;
                }
                if temperature < 30.0 {
                    level = HIGH_LEVEL;
                }
                if temperature < 25.0 {
                    level = MAX_LEVEL;
                }
            }
            if level != self.level
                || (level > MIN_LEVEL && self.chevron_difference(chev_count, chev_is_up, chev_col))
            {
                self.set_chevron(chev_count, chev_is_up, chev_col);
            }
        }
        if t == T::Windchill {
            let amount = c.ch.wind_chill_amount(c.world);
            let s = threshold(T::Windchill)?;
            if amount > s.highest {
                level = LOW_LEVEL;
            }
            if amount > s.moderate {
                level = MODERATE_LEVEL;
            }
            if amount > s.highest {
                level = HIGH_LEVEL;
            }
            if amount > s.maximum {
                level = MAX_LEVEL;
            }
        }
        if t == T::Uncomfortable && alive {
            let discomfort = stats.get(CharacterStat::Discomfort);
            let s = threshold(T::Uncomfortable)?;
            if discomfort >= s.maximum {
                level = MAX_LEVEL;
            } else if discomfort >= s.highest {
                level = HIGH_LEVEL;
            } else if discomfort >= s.moderate {
                level = MODERATE_LEVEL;
            } else if discomfort >= s.lowest {
                level = LOW_LEVEL;
            }
        }
        if t == T::NoxiousSmell && alive {
            if c.ch.in_toxic_building {
                return Err(
                    "not ported: Moodle NOXIOUS_SMELL in a toxic building (isProtectedFromToxic)"
                        .into(),
                );
            } else if c.world.sandbox_int("DecayingCorpseHealthImpact") != 1 {
                let rate = c.ch.corpse_sickness_rate;
                if rate > 0.0 {
                    let s = threshold(T::NoxiousSmell)?;
                    if rate > s.highest {
                        level = HIGH_LEVEL;
                    } else if rate > s.moderate {
                        level = MODERATE_LEVEL;
                    } else {
                        level = LOW_LEVEL;
                    }
                }
            }
        }
        Ok(self.update_moodle_level(level))
    }
}
