//! zombie.characters.BodyDamage.BodyDamage: the body parts, colds, infection and overall health of a character, as a
//! save stores them, with the thermoregulator.
use super::body_part::{BodyPart, BodySandbox};
use super::body_part_type;
use crate::java_nio::{read_string, ByteBuffer};

/// zombie.characters.BodyDamage.Thermoregulator.ThermalNode, the saved fields.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ThermalNode {
    pub celcius: f32,
    pub skin_celcius: f32,
    pub heat_delta: f32,
    pub primary_delta: f32,
    pub secondary_delta: f32,
    pub insulation: f32,
    pub windresist: f32,
    pub body_wetness: f32,
    pub clothing_wetness: f32,
}

/// zombie.characters.BodyDamage.Thermoregulator, the saved fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Thermoregulator {
    pub set_point: f32,
    pub metabolic_rate: f32,
    /// `None` before world version 243 (the field keeps its value).
    pub metabolic_rate_real: Option<f32>,
    pub metabolic_target: f32,
    pub body_heat_delta: f32,
    pub core_heat_delta: f32,
    pub thermal_damage: f32,
    pub damage_counter: f32,
    /// `None` before world version 249.
    pub thermal_duration: Option<f32>,
    /// Loaded nodes by body part index (`getNodeForType(BodyPartType.FromIndex(i))`; MAX has no node).
    pub nodes: std::collections::BTreeMap<usize, ThermalNode>,
}

impl Thermoregulator {
    /// zombie.characters.BodyDamage.Thermoregulator.load
    pub fn load(b: &mut ByteBuffer, world_version: i32) -> Result<Self, String> {
        let mut t = Thermoregulator {
            set_point: b.get_float()?,
            metabolic_rate: b.get_float()?,
            ..Thermoregulator::default()
        };
        if world_version >= 243 {
            t.metabolic_rate_real = Some(b.get_float()?);
        }
        t.metabolic_target = b.get_float()?;
        t.body_heat_delta = b.get_float()?;
        t.core_heat_delta = b.get_float()?;
        t.thermal_damage = b.get_float()?;
        t.damage_counter = b.get_float()?;
        if world_version >= 249 {
            t.thermal_duration = Some(b.get_float()?);
        }
        let count = b.get_int()?;
        for _ in 0..count {
            let node_index = b.get_int()?;
            let mut n = ThermalNode {
                celcius: b.get_float()?,
                skin_celcius: b.get_float()?,
                heat_delta: b.get_float()?,
                primary_delta: b.get_float()?,
                secondary_delta: b.get_float()?,
                ..ThermalNode::default()
            };
            if world_version >= 241 {
                n.insulation = b.get_float()?;
            }
            if world_version >= 243 {
                n.windresist = b.get_float()?;
                n.body_wetness = b.get_float()?;
                n.clothing_wetness = b.get_float()?;
            }
            let part = body_part_type::from_index(node_index);
            if part < body_part_type::MAX {
                t.nodes.insert(part, n);
            }
        }
        Ok(t)
    }
}

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
            overall_body_health: 100.0 - total_damage,
            thermoregulator,
        })
    }
}
