//! zombie.characters.BodyDamage.BodyDamage: the body parts, colds, infection and overall health of a character, as a
//! save stores them, with the thermoregulator.
use super::super::stats::{CharacterStat, Stats};
use super::body_part::{BodyPart, BodySandbox};
use super::body_part_type;
pub use super::thermoregulator::{ThermalNode, Thermoregulator};
use crate::java;
use crate::java_nio::{read_string, ByteBuffer};

/// zombie.characters.BodyDamage.BodyDamage, the saved fields.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyDamage {
    pub body_parts: Vec<BodyPart>,
    pub catch_a_cold: f32,
    pub has_a_cold: bool,
    pub cold_strength: f32,
    /// Field initialiser -1; an int in saves since world version 222.
    pub time_to_sneeze_or_cough: f32,
    pub reduce_fake_infection: bool,
    pub health_from_food_timer: f32,
    pub pain_reduction: f32,
    pub cold_reduction: f32,
    pub infection_time: f32,
    pub infection_mortality_duration: f32,
    pub cold_damage_stage: f32,
    pub overall_body_health: f32,
    /// `None` where the save has none (the character keeps its constructed one).
    pub thermoregulator: Option<Thermoregulator>,
    // the fields the save does not hold, at their initialisers in zombie.characters.BodyDamage.BodyDamage
    pub damage_mod_count: i32,
    pub infection_growth_rate: f32,
    pub is_infected: bool,
    pub is_fake_infected: bool,
    pub standard_health_addition: f32,
    pub reduced_health_addition: f32,
    pub severly_reduced_health_addition: f32,
    pub sleeping_health_addition: f32,
    pub health_from_food: f32,
    pub health_reduction_from_severe_bad_moodles: f32,
    pub standard_health_from_food_time: i32,
    pub boredom_decrease_from_reading: f32,
    pub initial_thump_pain: f32,
    pub initial_scratch_pain: f32,
    pub initial_bite_pain: f32,
    pub initial_wound_pain: f32,
    pub continual_pain_increase: f32,
    pub pain_reduction_from_meds: f32,
    pub standard_pain_reduction_when_well: f32,
    pub old_num_zombies_visible: i32,
    pub current_num_zombies_visible: i32,
    pub panic_increase_value: f32,
    pub panic_increase_value_frame: f32,
    pub panic_reduction_value: f32,
    pub drunk_increase_value: f32,
    pub drunk_reduction_value: f32,
    pub is_on_fire: bool,
    pub burnt_to_death: bool,
    pub cold_progression_rate: f32,
    pub mild_cold_sneeze_timer_min: i32,
    pub mild_cold_sneeze_timer_max: i32,
    pub cold_sneeze_timer_min: i32,
    pub cold_sneeze_timer_max: i32,
    pub nasty_cold_sneeze_timer_min: i32,
    pub nasty_cold_sneeze_timer_max: i32,
    pub sneeze_cough_active: i32,
    pub sneeze_cough_time: i32,
    pub sneeze_cough_delay: i32,
    pub was_dragging_corpse: bool,
    pub started_dragging_corpse: bool,
}

impl BodyDamage {
    /// `new BodyDamage(parent)` of a character: 17 new parts at full health, every field at its initialiser, a
    /// thermoregulator for a player.
    pub fn new(is_player: bool) -> Self {
        BodyDamage {
            body_parts: (0..body_part_type::MAX as i32).map(BodyPart::new).collect(),
            catch_a_cold: 0.0,
            has_a_cold: false,
            cold_strength: 0.0,
            time_to_sneeze_or_cough: -1.0,
            reduce_fake_infection: false,
            health_from_food_timer: 0.0,
            pain_reduction: 0.0,
            cold_reduction: 0.0,
            infection_time: -1.0,
            infection_mortality_duration: -1.0,
            cold_damage_stage: 0.0,
            overall_body_health: 100.0,
            thermoregulator: is_player.then(Thermoregulator::default),
            damage_mod_count: 60,
            infection_growth_rate: 0.001,
            is_infected: false,
            is_fake_infected: false,
            standard_health_addition: 0.002,
            reduced_health_addition: 0.0013,
            severly_reduced_health_addition: 8.0E-4,
            sleeping_health_addition: 0.02,
            health_from_food: 0.015,
            health_reduction_from_severe_bad_moodles: 0.0165,
            standard_health_from_food_time: 1600,
            boredom_decrease_from_reading: 0.5,
            initial_thump_pain: 14.0,
            initial_scratch_pain: 18.0,
            initial_bite_pain: 25.0,
            initial_wound_pain: 80.0,
            continual_pain_increase: 0.001,
            pain_reduction_from_meds: 30.0,
            standard_pain_reduction_when_well: 0.01,
            old_num_zombies_visible: 0,
            current_num_zombies_visible: 0,
            panic_increase_value: 7.0,
            panic_increase_value_frame: 0.035,
            panic_reduction_value: 0.06,
            drunk_increase_value: 400.0,
            drunk_reduction_value: 0.0042,
            is_on_fire: false,
            burnt_to_death: false,
            cold_progression_rate: 0.0112,
            mild_cold_sneeze_timer_min: 600,
            mild_cold_sneeze_timer_max: 800,
            cold_sneeze_timer_min: 300,
            cold_sneeze_timer_max: 600,
            nasty_cold_sneeze_timer_min: 200,
            nasty_cold_sneeze_timer_max: 300,
            sneeze_cough_active: 0,
            sneeze_cough_time: 0,
            sneeze_cough_delay: 25,
            was_dragging_corpse: false,
            started_dragging_corpse: false,
        }
    }

    /// zombie.characters.BodyDamage.BodyDamage.getApparentInfectionLevel
    pub fn get_apparent_infection_level(&self, stats: &Stats) -> f32 {
        let infection_level = java::math_max_f32(
            stats.get(CharacterStat::ZombieFever),
            stats.get(CharacterStat::ZombieInfection),
        );
        java::math_max_f32(stats.get(CharacterStat::FoodSickness), infection_level)
    }

    /// zombie.characters.BodyDamage.BodyDamage.getNumPartsBleeding
    pub fn get_num_parts_bleeding(&self) -> i32 {
        self.body_parts.iter().filter(|p| p.bleeding).count() as i32
    }

    /// zombie.characters.BodyDamage.BodyDamage.isNeckBleeding
    pub fn is_neck_bleeding(&self) -> bool {
        self.body_parts[body_part_type::NECK].bleeding
    }

    /// zombie.characters.BodyDamage.BodyDamage.getNumPartsScratched
    pub fn get_num_parts_scratched(&self) -> i32 {
        self.body_parts.iter().filter(|p| p.scratched).count() as i32
    }

    /// zombie.characters.BodyDamage.BodyDamage.getNumPartsBitten
    pub fn get_num_parts_bitten(&self) -> i32 {
        self.body_parts.iter().filter(|p| p.bitten).count() as i32
    }
}

impl BodyDamage {
    /// zombie.characters.BodyDamage.BodyDamage.load over a fresh BodyDamage (17 new parts), then
    /// `setBodyPartsLastState`, `loadMainFields` and the thermoregulator. `sleeping_pills_taken` is the player's
    /// (`getDamageFromPills`).
    pub fn load(
        b: &mut ByteBuffer,
        world_version: i32,
        sb: &BodySandbox,
        sleeping_pills_taken: i32,
    ) -> Result<Self, String> {
        let mut parts: Vec<BodyPart> = (0..body_part_type::MAX as i32).map(BodyPart::new).collect();
        for p in &mut parts {
            p.set_cut(b.get()? != 0)?;
            p.set_bitten(b.get()? != 0, sb)?;
            p.set_scratched(b.get()? != 0)?;
            p.set_bandaged(b.get()? != 0, 0.0);
            p.set_bleeding(b.get()? != 0);
            p.set_deep_wounded(b.get()? != 0);
            p.is_fake_infected = b.get()? != 0;
            p.is_infected = b.get()? != 0;
            p.health = b.get_float()?;
            if p.bandaged {
                p.set_bandage_life(b.get_float()?);
            }
            p.infected_wound = b.get()? != 0;
            if p.infected_wound {
                p.set_wound_infection_level(b.get_float()?);
            }
            p.set_cut_time(b.get_float()?);
            p.bite_time = b.get_float()?;
            p.set_scratch_time(b.get_float()?);
            p.set_bleeding_time(b.get_float()?);
            p.alcohol_level = b.get_float()?;
            p.additional_pain = b.get_float()?;
            p.deep_wound_time = b.get_float()?;
            p.have_glass = b.get()? != 0;
            p.get_bandage_xp = b.get()? != 0;
            p.set_stitched(b.get()? != 0)?;
            p.stitch_time = b.get_float()?;
            p.get_stitch_xp = b.get()? != 0;
            p.get_splint_xp = b.get()? != 0;
            p.fracture_time = b.get_float()?;
            p.set_splint(b.get()? != 0, 0.0);
            if p.splint {
                p.splint_factor = b.get_float()?;
            }
            p.set_have_bullet(b.get()? != 0, sb)?;
            p.burn_time = b.get_float()?;
            p.set_need_burn_wash(b.get()? != 0);
            p.last_time_burn_wash = b.get_float()?;
            p.splint_item = Some(read_string(b)?);
            p.bandage_type = Some(read_string(b)?);
            p.set_cut_time(b.get_float()?);
            p.set_wetness(b.get_float()?);
            p.set_stiffness(b.get_float()?);
            if world_version >= 227 {
                p.set_comfrey_factor(b.get_float()?);
                p.set_garlic_factor(b.get_float()?);
                p.set_plantain_factor(b.get_float()?);
            }
        }
        // loadMainFields
        let catch_a_cold = b.get_float()?;
        let has_a_cold = b.get()? != 0;
        let cold_strength = b.get_float()?;
        let time_to_sneeze_or_cough = if world_version >= 222 {
            b.get_int()? as f32
        } else {
            -1.0
        };
        let reduce_fake_infection = b.get()? != 0;
        let health_from_food_timer = b.get_float()?;
        let pain_reduction = b.get_float()?;
        let cold_reduction = b.get_float()?;
        let infection_time = b.get_float()?;
        let infection_mortality_duration = b.get_float()?;
        let cold_damage_stage = b.get_float()?;
        // calculateOverallHealth
        let mut total_damage = 0.0f32;
        for (i, p) in parts.iter().enumerate() {
            total_damage += (100.0 - p.health) * body_part_type::get_damage_modifyer(i as i32);
        }
        total_damage += match sleeping_pills_taken {
            10 => 40.0,
            11 => 80.0,
            n if n >= 12 => 100.0,
            _ => 0.0,
        };
        if total_damage > 100.0 {
            total_damage = 100.0;
        }
        let thermoregulator = if b.get()? != 0 {
            Some(Thermoregulator::load(b, world_version)?)
        } else {
            None
        };
        Ok(BodyDamage {
            thermoregulator,
            overall_body_health: 100.0 - total_damage,
            body_parts: parts,
            catch_a_cold,
            has_a_cold,
            cold_strength,
            time_to_sneeze_or_cough,
            reduce_fake_infection,
            health_from_food_timer,
            pain_reduction,
            cold_reduction,
            infection_time,
            infection_mortality_duration,
            cold_damage_stage,
            ..BodyDamage::new(false)
        })
    }
}
