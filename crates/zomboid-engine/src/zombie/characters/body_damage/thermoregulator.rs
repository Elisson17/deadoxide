//! zombie.characters.BodyDamage.Thermoregulator: the body temperature simulation of a player: a thermal node per body
//! part (core, skin, clothing insulation, wetness), the metabolic rate, the set point and what the climate does to them.
use super::body_part::BodyPart;
use super::body_part_type;
use super::metabolics::Metabolics;
use crate::java_nio::ByteBuffer;
use crate::zombie::characters::stats::{CharacterStat, Stats};
use crate::zombie::core::math::pz_math;
use crate::zombie::iso::weather::temperature;

/// The node of `BodyPartType.Torso_Upper`, the core.
pub const CORE: usize = body_part_type::TORSO_UPPER;

/// What `ThermalNode.calculateInsulation` reads of one worn `Clothing` item on a node.
#[derive(Clone, Debug, PartialEq)]
pub struct ClothingLayer {
    /// `getInsulation()`, `getWindresistance()`
    pub insulation: f32,
    pub windresistance: f32,
    /// `getWetness()` (0..100) and `getCurrentCondition()` (0..100)
    pub wetness: f32,
    pub condition: f32,
    /// `getVisual().getHole(bloodBpt) > 0.0F`
    pub has_hole: bool,
}

/// zombie.characters.BodyDamage.Thermoregulator.ThermalNode: the saved fields and the clothing on the node.
#[derive(Clone, Debug, PartialEq)]
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
    pub clothing: Vec<ClothingLayer>,
}

impl ThermalNode {
    /// `new ThermalNode(initTemperature, bodyPart, ...)`: `skinCelcius` 33.
    pub fn new(init_temperature: f32) -> Self {
        ThermalNode {
            celcius: init_temperature,
            skin_celcius: 33.0,
            heat_delta: 0.0,
            primary_delta: 0.0,
            secondary_delta: 0.0,
            insulation: 0.0,
            windresist: 0.0,
            body_wetness: 0.0,
            clothing_wetness: 0.0,
            clothing: Vec::new(),
        }
    }

    /// zombie.characters.BodyDamage.Thermoregulator.ThermalNode.calculateInsulation. `part_wetness` is the node's
    /// `bodyPart.getWetness()`.
    fn calculate_insulation(&mut self, part_wetness: f32) {
        let layers = self.clothing.len();
        self.insulation = 0.0;
        self.windresist = 0.0;
        self.clothing_wetness = 0.0;
        self.body_wetness = part_wetness * 0.01;
        self.body_wetness = pz_math::clamp_01(self.body_wetness);
        if layers > 0 {
            for item in &self.clothing {
                let item_wetness = pz_math::clamp(item.wetness * 0.01, 0.0, 1.0);
                self.clothing_wetness += item_wetness;
                if !item.has_hole {
                    let mut item_insulation =
                        temperature::get_true_insulation_value(item.insulation);
                    let mut item_wind_resist =
                        temperature::get_true_windresistance_value(item.windresistance);
                    let mut item_condition = pz_math::clamp(item.condition * 0.01, 0.0, 1.0);
                    item_condition = 0.5 + 0.5 * item_condition;
                    item_insulation *= (1.0 - item_wetness * 0.75) * item_condition;
                    item_wind_resist *= (1.0 - item_wetness * 0.45) * item_condition;
                    self.insulation += item_insulation;
                    self.windresist += item_wind_resist;
                }
            }
            self.clothing_wetness /= layers as f32;
            self.insulation += layers as f32 * 0.05;
            self.windresist += layers as f32 * 0.05;
        }
    }
}

/// zombie.characters.BodyDamage.Thermoregulator: the saved fields, with the rest of the state the update keeps.
#[derive(Clone, Debug, PartialEq)]
pub struct Thermoregulator {
    pub set_point: f32,
    pub metabolic_rate: f32,
    pub metabolic_rate_real: f32,
    pub metabolic_target: f32,
    pub body_heat_delta: f32,
    pub core_heat_delta: f32,
    pub thermal_damage: f32,
    pub damage_counter: f32,
    pub thermal_duration: f32,
    /// The nodes by body part index (`getNodeForType(BodyPartType.FromIndex(i))`; MAX has no node), in the order of
    /// the Java `nodes` array.
    pub nodes: std::collections::BTreeMap<usize, ThermalNode>,
    /// `fluidsMultiplier`, `energyMultiplier`, `fatigueMultiplier`: field initialiser 1.0
    pub fluids_multiplier: f64,
    pub energy_multiplier: f64,
    pub fatigue_multiplier: f64,
    /// `thermalChevronUp`: field initialiser true
    pub thermal_chevron_up: bool,
    pub core_rate_of_change: f32,
    pub total_heat_raw: f32,
    pub total_heat: f32,
    pub prim_total: f32,
    pub sec_total: f32,
    /// field initialiser 27.0
    pub external_air_temperature: f32,
    pub air_temperature: f32,
    pub air_and_wind_temp: f32,
    pub rate_of_change_counter: f32,
    /// field initialiser 37.0
    pub core_celcius_cache: f32,
}

impl Default for Thermoregulator {
    /// `new Thermoregulator(bodyDamage)`: the field initialisers and `initNodes` (a node at the set point per part
    /// but MAX, the core at `setPoint`).
    fn default() -> Self {
        let default_met = Metabolics::Default.get_met();
        let mut nodes = std::collections::BTreeMap::new();
        for i in 0..body_part_type::MAX {
            nodes.insert(i, ThermalNode::new(37.0));
        }
        Thermoregulator {
            set_point: 37.0,
            metabolic_rate: default_met,
            metabolic_rate_real: default_met,
            metabolic_target: default_met,
            body_heat_delta: 0.0,
            core_heat_delta: 0.0,
            thermal_damage: 0.0,
            damage_counter: 0.0,
            thermal_duration: 0.0,
            nodes,
            fluids_multiplier: 1.0,
            energy_multiplier: 1.0,
            fatigue_multiplier: 1.0,
            thermal_chevron_up: true,
            core_rate_of_change: 0.0,
            total_heat_raw: 0.0,
            total_heat: 0.0,
            prim_total: 0.0,
            sec_total: 0.0,
            external_air_temperature: 27.0,
            air_temperature: 0.0,
            air_and_wind_temp: 0.0,
            rate_of_change_counter: 0.0,
            core_celcius_cache: 37.0,
        }
    }
}

/// What `Thermoregulator.update` reads of the player and the world each tick.
pub struct ThermoInputs<'a> {
    pub stats: &'a mut Stats,
    /// the body parts (their `getWetness()` feeds the nodes)
    pub parts: &'a [BodyPart],
    /// the levels of the HYPOTHERMIA and HYPERTHERMIA moodles
    pub hypothermia: i32,
    pub hyperthermia: i32,
    /// `getAirTemperatureForCharacter(character, false)` and `(character, true)`
    pub air_temperature: f32,
    pub air_and_wind: f32,
    /// `GameTime.getMultiplier()` and `getGameWorldSecondsSinceLastUpdate()`
    pub multiplier: f32,
    pub gwsslu: f32,
    /// `ClimateManager.getHumidity()`
    pub humidity: f32,
    /// `getNutrition().getWeight()`
    pub weight: f64,
    /// `getInventory().getCapacityWeight()` and `getMaxWeight()`
    pub capacity_weight: f32,
    pub max_weight: i32,
    pub player_moving: bool,
    pub sprinting: bool,
    pub running: bool,
    pub sneaking: bool,
    /// `IsoPlayer.currentSpeed`
    pub current_speed: f32,
    /// `isAttacking()`
    pub attacking: bool,
    /// `getFitness().getCurrentExe() != null`
    pub fitness_exercise: bool,
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
            t.metabolic_rate_real = b.get_float()?;
        }
        t.metabolic_target = b.get_float()?;
        t.body_heat_delta = b.get_float()?;
        t.core_heat_delta = b.get_float()?;
        t.thermal_damage = b.get_float()?;
        t.damage_counter = b.get_float()?;
        if world_version >= 249 {
            t.thermal_duration = b.get_float()?;
        }
        let count = b.get_int()?;
        for _ in 0..count {
            let node_index = b.get_int()?;
            let celcius = b.get_float()?;
            let skin_celcius = b.get_float()?;
            let heat_delta = b.get_float()?;
            let primary_delta = b.get_float()?;
            let secondary_delta = b.get_float()?;
            let mut insulation = 0.0;
            if world_version >= 241 {
                insulation = b.get_float()?;
            }
            let (mut windresist, mut body_wetness, mut clothing_wetness) = (0.0, 0.0, 0.0);
            if world_version >= 243 {
                windresist = b.get_float()?;
                body_wetness = b.get_float()?;
                clothing_wetness = b.get_float()?;
            }
            let part = body_part_type::from_index(node_index);
            if part < body_part_type::MAX {
                if let Some(n) = t.nodes.get_mut(&part) {
                    n.celcius = celcius;
                    n.skin_celcius = skin_celcius;
                    n.heat_delta = heat_delta;
                    n.primary_delta = primary_delta;
                    n.secondary_delta = secondary_delta;
                    n.insulation = insulation;
                    n.windresist = windresist;
                    n.body_wetness = body_wetness;
                    n.clothing_wetness = clothing_wetness;
                }
            }
        }
        Ok(t)
    }

    /// zombie.characters.BodyDamage.Thermoregulator.reset
    pub fn reset(&mut self) {
        self.set_point = 37.0;
        self.metabolic_rate = Metabolics::Default.get_met();
        self.metabolic_target = self.metabolic_rate;
        if let Some(c) = self.nodes.get_mut(&CORE) {
            c.celcius = 37.0;
        }
        self.body_heat_delta = 0.0;
        self.core_heat_delta = 0.0;
        self.thermal_damage = 0.0;
        for (i, node) in self.nodes.iter_mut() {
            if *i != CORE {
                node.celcius = 35.0;
            }
            node.primary_delta = 0.0;
            node.secondary_delta = 0.0;
            node.skin_celcius = 33.0;
            node.heat_delta = 0.0;
        }
    }

    fn core(&self) -> &ThermalNode {
        &self.nodes[&CORE]
    }

    /// zombie.characters.BodyDamage.Thermoregulator.getCoreTemperature
    pub fn get_core_temperature(&self) -> f32 {
        self.core().celcius
    }

    /// zombie.characters.BodyDamage.Thermoregulator.thermalChevronUp
    pub fn thermal_chevron_up(&self) -> bool {
        self.thermal_chevron_up
    }

    /// zombie.characters.BodyDamage.Thermoregulator.thermalChevronCount
    pub fn thermal_chevron_count(&self) -> i32 {
        if self.core_rate_of_change > 0.01 {
            3
        } else if self.core_rate_of_change > 0.001 {
            2
        } else if self.core_rate_of_change > 1.0E-4 {
            1
        } else {
            0
        }
    }

    /// zombie.characters.BodyDamage.Thermoregulator.getMovementModifier
    pub fn get_movement_modifier(hypothermia: i32, hyperthermia: i32) -> f32 {
        let mut delta = 1.0;
        if hypothermia == 2 {
            delta = 0.66;
        } else if hypothermia == 3 {
            delta = 0.33;
        } else if hypothermia == 4 {
            delta = 0.0;
        }
        if hyperthermia == 2 {
            delta = 0.66;
        } else if hyperthermia == 3 {
            delta = 0.33;
        } else if hyperthermia == 4 {
            delta = 0.0;
        }
        delta
    }

    /// zombie.characters.BodyDamage.Thermoregulator.getCatchAColdDelta
    pub fn get_catch_a_cold_delta(&self, hypothermia: i32) -> f32 {
        let mut delta = 0.0f32;
        for (i, node) in &self.nodes {
            let mut skin = 0.0f32;
            if node.skin_celcius < 33.0 {
                skin = (node.skin_celcius - 20.0) / 13.0;
                skin = 1.0 - skin;
                skin *= skin;
            }
            let mut add = 0.25 * skin * body_part_type::get_skin_surface(*i);
            if node.body_wetness > 0.0 {
                add *= 1.0 + node.body_wetness * 1.0;
            }
            if node.clothing_wetness > 0.5 {
                add *= 1.0 + (node.clothing_wetness - 0.5) * 2.0;
            }
            if *i == body_part_type::NECK {
                add *= 8.0;
            } else if *i == body_part_type::TORSO_UPPER {
                add *= 16.0;
            } else if *i == body_part_type::HEAD {
                add *= 4.0;
            }
            delta += add;
        }
        if hypothermia > 1 {
            delta *= hypothermia as f32;
        }
        delta
    }

    /// zombie.characters.BodyDamage.Thermoregulator.setMetabolicTarget (float)
    pub fn set_metabolic_target(&mut self, target: f32) {
        if !(target < 0.0) && !(target < self.metabolic_target) {
            self.metabolic_target = target;
            if self.metabolic_target > Metabolics::Max.get_met() {
                self.metabolic_target = Metabolics::Max.get_met();
            }
        }
    }

    /// zombie.characters.BodyDamage.Thermoregulator.getSimulationMultiplier(Multiplier): `GameTime.getMultiplier()`
    /// times the constant of the multiplier times the static `simulationMultiplier` (1.0). The decompiled source drops
    /// the game time factor; the bytecode multiplies by it (`fload_2` of `getMultiplier()` before each `ldc`).
    fn sim(mult: f32, c: f32) -> f32 {
        mult * c * 1.0
    }

    /// zombie.characters.BodyDamage.Thermoregulator.getEnergy
    pub fn get_energy(stats: &Stats) -> f32 {
        let hunger = stats.get(CharacterStat::Hunger);
        let fatigue = stats.get(CharacterStat::Fatigue);
        let h = 1.0 - (0.4 * hunger + 0.6 * hunger * hunger);
        let f = 1.0 - (0.4 * fatigue + 0.6 * fatigue * fatigue);
        0.6 * h + 0.4 * f
    }

    /// zombie.characters.BodyDamage.Thermoregulator.getBodyFluids
    pub fn get_body_fluids(stats: &Stats) -> f32 {
        1.0 - stats.get(CharacterStat::Thirst)
    }

    /// zombie.characters.BodyDamage.Thermoregulator.update. Returns the thermal damage the player's BodyDamage takes as
    /// its `coldDamageStage` (`setColdDamageStage`).
    pub fn update(&mut self, i: &mut ThermoInputs) -> Result<f32, String> {
        self.air_temperature = i.air_temperature;
        self.air_and_wind_temp = i.air_and_wind;
        self.external_air_temperature = self.air_temperature;
        self.update_set_point(i.stats);
        self.update_core_rate_of_change(i.multiplier);
        self.update_metabolic_rate(i)?;
        // updateClothing: the nodes' `clothing` lists are set by the caller from the worn items
        self.update_nodes_heat_delta(i);
        self.update_heat_deltas(i.stats, i.multiplier);
        self.update_nodes(i.multiplier);
        self.update_body_multipliers();
        Ok(self.update_thermal_damage(i, i.air_and_wind))
    }

    fn update_set_point(&mut self, stats: &Stats) {
        self.set_point = 37.0;
        if stats.is_above_minimum(CharacterStat::Sickness) {
            self.set_point += stats.get(CharacterStat::Sickness) * 2.0;
        }
    }

    fn update_core_rate_of_change(&mut self, multiplier: f32) {
        self.rate_of_change_counter += multiplier;
        if self.rate_of_change_counter > 100.0 {
            self.rate_of_change_counter = 0.0;
            self.core_rate_of_change = self.core().celcius - self.core_celcius_cache;
            self.thermal_chevron_up = self.core_rate_of_change >= 0.0;
            self.core_rate_of_change = pz_math::abs(self.core_rate_of_change);
            self.core_celcius_cache = self.core().celcius;
        }
    }

    fn update_metabolic_rate(&mut self, i: &mut ThermoInputs) -> Result<(), String> {
        self.set_metabolic_target(Metabolics::Default.get_met());
        if i.attacking {
            return Err(
                "not ported: Thermoregulator.updateMetabolicRate while attacking (WeaponType)"
                    .into(),
            );
        }
        if i.player_moving {
            if i.sprinting {
                self.set_metabolic_target(Metabolics::Running15kmh.get_met());
            } else if i.running {
                self.set_metabolic_target(Metabolics::Running10kmh.get_met());
            } else if i.sneaking {
                self.set_metabolic_target(Metabolics::Walking2kmh.get_met());
            } else if i.current_speed > 0.0 {
                self.set_metabolic_target(Metabolics::Walking5kmh.get_met());
            }
        }
        if i.fitness_exercise {
            self.set_metabolic_target(Metabolics::Fitness.get_met());
        }
        let excercise = pz_math::clamp_01(1.0 - i.stats.get(CharacterStat::Endurance))
            * Metabolics::DefaultExercise.get_met();
        self.set_metabolic_target(excercise * Self::get_energy(i.stats));
        let weight = pz_math::clamp_01(i.capacity_weight / i.max_weight as f32);
        let ratio = 1.0 + weight * weight * 0.35;
        self.set_metabolic_target(self.metabolic_target * ratio);
        if !pz_math::equal_default(self.metabolic_rate, self.metabolic_target) {
            let rate = self.metabolic_target - self.metabolic_rate;
            if self.metabolic_target > self.metabolic_rate {
                self.metabolic_rate += rate * Self::sim(i.multiplier, 0.001);
            } else {
                self.metabolic_rate += rate * Self::sim(i.multiplier, 4.0E-4);
            }
        }
        let mut meta_mod = 1.0;
        if i.hypothermia >= 1 {
            meta_mod = Self::get_movement_modifier(i.hypothermia, i.hyperthermia);
        }
        self.metabolic_rate_real =
            self.metabolic_rate * (0.2 + 0.8 * Self::get_energy(i.stats) * meta_mod);
        self.metabolic_target = -1.0;
        Ok(())
    }

    fn update_nodes_heat_delta(&mut self, i: &ThermoInputs) {
        let mut weight_delta =
            pz_math::clamp_01((((i.weight / 75.0) - 0.5) * f64::from(0.666f32)) as f32);
        weight_delta = (weight_delta - 0.5) * 2.0;
        let fitness_delta = i.stats.get(CharacterStat::Fitness);
        let mut prim_heat_mod = 1.0f32;
        if self.air_and_wind_temp > self.set_point - 2.0 {
            if self.air_temperature < self.set_point + 2.0 {
                prim_heat_mod = (self.air_temperature - (self.set_point - 2.0)) / 4.0;
                prim_heat_mod = 1.0 - prim_heat_mod;
            } else {
                prim_heat_mod = 0.0;
            }
        }
        let mut humidity_mod = 1.0f32;
        if i.humidity > 0.5 {
            let hum = (i.humidity - 0.5) * 2.0;
            humidity_mod -= hum;
        }
        let mut nodes_total_heat = 0.0f32;
        let (air, air_wind, set_point) =
            (self.air_temperature, self.air_and_wind_temp, self.set_point);
        let _ = set_point;
        for (idx, node) in self.nodes.iter_mut() {
            node.calculate_insulation(i.parts[*idx].wetness);
            let skin_surface = body_part_type::get_skin_surface(*idx);
            let mut external_temp = air;
            if air_wind < air {
                external_temp -= (air - air_wind) / (1.0 + node.windresist);
            }
            let mut node_heat_delta = external_temp - node.skin_celcius;
            if node_heat_delta <= 0.0 {
                node_heat_delta *= 1.0 + 0.75 * node.body_wetness;
            } else {
                node_heat_delta /= 1.0 + 3.0 * node.body_wetness;
            }
            node_heat_delta *= 0.3;
            node_heat_delta /= 1.0 + node.insulation;
            node.heat_delta = node_heat_delta * skin_surface;
            if node.primary_delta > 0.0 {
                let fluids = 0.2 + 0.8 * Self::get_body_fluids(i.stats);
                let mut contribution =
                    Metabolics::Default.get_met() * node.primary_delta * skin_surface
                        / (1.0 + node.insulation);
                contribution *= fluids * (0.1 + 0.9 * prim_heat_mod);
                contribution *= humidity_mod;
                contribution *= 1.0 - 0.2 * weight_delta;
                contribution *= 1.0 + 0.2 * fitness_delta;
                node.heat_delta -= contribution;
            } else {
                let energy = 0.2 + 0.8 * Self::get_energy(i.stats);
                let mut contribution =
                    Metabolics::Default.get_met() * pz_math::abs(node.primary_delta) * skin_surface;
                contribution *= energy;
                contribution *= 1.0 + 0.2 * weight_delta;
                contribution *= 1.0 + 0.2 * fitness_delta;
                node.heat_delta += contribution;
            }
            if node.secondary_delta > 0.0 {
                let fluids = 0.1 + 0.9 * Self::get_body_fluids(i.stats);
                let mut v = Metabolics::Max.get_met() * 0.75 * node.secondary_delta * skin_surface
                    / (1.0 + node.insulation);
                v *= fluids;
                v *= 0.85 + 0.15 * humidity_mod;
                v *= 1.0 - 0.2 * weight_delta;
                v *= 1.0 + 0.2 * fitness_delta;
                node.heat_delta -= v;
            } else {
                let energy = 0.1 + 0.9 * Self::get_energy(i.stats);
                let mut v = Metabolics::Default.get_met()
                    * pz_math::abs(node.secondary_delta)
                    * skin_surface;
                v *= energy;
                v *= 1.0 + 0.2 * weight_delta;
                v *= 1.0 + 0.2 * fitness_delta;
                node.heat_delta += v;
            }
            nodes_total_heat += node.heat_delta;
        }
        self.total_heat_raw = nodes_total_heat;
        nodes_total_heat += self.metabolic_rate_real;
        self.total_heat = nodes_total_heat;
    }

    fn update_heat_deltas(&mut self, stats: &mut Stats, multiplier: f32) {
        self.core_heat_delta = self.total_heat * Self::sim(multiplier, 2.5E-4);
        let core_celcius = self.core().celcius;
        if self.core_heat_delta < 0.0 {
            if core_celcius > self.set_point {
                self.core_heat_delta *= 1.0 + (core_celcius - self.set_point) / 2.0;
            }
        } else if core_celcius < self.set_point {
            self.core_heat_delta *= 1.0 + (self.set_point - core_celcius) / 4.0;
        }
        let mut core = core_celcius + self.core_heat_delta;
        core = pz_math::clamp(core, 20.0, 42.0);
        let current_temperature = stats.get(CharacterStat::Temperature);
        if pz_math::abs(current_temperature - core) > 0.001 {
            core = pz_math::lerp(core, current_temperature, 0.5);
        }
        if let Some(c) = self.nodes.get_mut(&CORE) {
            c.celcius = core;
        }
        stats.set(CharacterStat::Temperature, core);
        self.body_heat_delta = 0.0;
        if core > self.set_point {
            self.body_heat_delta = core - self.set_point;
        } else if core < self.set_point {
            self.body_heat_delta = core - self.set_point;
        }
        if self.body_heat_delta < 0.0 {
            let mut bhd = pz_math::abs(self.body_heat_delta);
            if bhd <= 1.0 {
                self.body_heat_delta *= 0.8;
            } else {
                bhd = pz_math::clamp(bhd, 1.0, 11.0) - 1.0;
                bhd /= 10.0;
                self.body_heat_delta = -0.8 + -0.2 * bhd;
            }
        }
        self.body_heat_delta = pz_math::clamp(self.body_heat_delta, -1.0, 1.0);
    }

    fn update_nodes(&mut self, multiplier: f32) {
        let mut prim = 0.0f32;
        let mut sec = 0.0f32;
        let core_celcius = self.core().celcius;
        let (body_heat_delta, air) = (self.body_heat_delta, self.air_temperature);
        for (idx, node) in self.nodes.iter_mut() {
            let dist_to_core = body_part_type::get_dist_to_core(*idx);
            let skin_surface = body_part_type::get_skin_surface(*idx);
            let insulation = 1.0 + node.insulation;
            if body_heat_delta < 0.0 {
                node.primary_delta = body_heat_delta * (1.0 + dist_to_core);
            } else {
                node.primary_delta = body_heat_delta * (1.0 + (1.0 - dist_to_core));
            }
            node.primary_delta = pz_math::clamp(node.primary_delta, -1.0, 1.0);
            node.secondary_delta = node.primary_delta
                * pz_math::abs(node.primary_delta)
                * pz_math::abs(node.primary_delta);
            prim += node.primary_delta * skin_surface;
            sec += node.secondary_delta * skin_surface;
            node.primary_delta = pz_math::clamp(node.primary_delta, -1.0, 1.0);
            let mut skin_min = core_celcius - 20.0;
            let skin_max = core_celcius;
            if skin_min < air {
                if air < 33.0 {
                    skin_min = air;
                } else {
                    let dist_core = 0.4 + 0.6 * (1.0 - dist_to_core);
                    let skinmod = (air - 33.0) / 6.0;
                    skin_min = 33.0;
                    skin_min += 4.0 * skinmod * dist_core;
                    skin_min = pz_math::clamp(skin_min, 33.0, air);
                    if skin_min > skin_max {
                        skin_min = skin_max - 0.25;
                    }
                }
            }
            let mut skin_target = core_celcius - 4.0;
            if node.primary_delta < 0.0 {
                let dist_core = 0.4 + 0.6 * dist_to_core;
                let target = skin_target - 12.0 * dist_core / insulation;
                skin_target =
                    pz_math::c_lerp(skin_target, target, pz_math::abs(node.primary_delta));
            } else {
                let dist_core = 0.4 + 0.6 * (1.0 - dist_to_core);
                let mut target_add = 4.0 * dist_core;
                target_add *= java_max(insulation * 0.5 * dist_core, 1.0);
                let v = java_min(skin_target + target_add, skin_max);
                skin_target = pz_math::c_lerp(skin_target, v, node.primary_delta);
            }
            skin_target = pz_math::clamp(skin_target, skin_min, skin_max);
            let skin_delta = skin_target - node.skin_celcius;
            let mut multipl = Self::sim(multiplier, 0.0025);
            if skin_delta < 0.0 && node.skin_celcius > 33.0 {
                multipl *= 3.0;
            } else if skin_delta > 0.0 && node.skin_celcius < 33.0 {
                multipl *= 3.0;
            }
            if multipl > 1.0 {
                multipl = 1.0;
            }
            node.skin_celcius += skin_delta * multipl;
            if *idx != CORE {
                if node.skin_celcius >= core_celcius {
                    node.celcius = core_celcius;
                } else {
                    node.celcius = pz_math::lerp(node.skin_celcius, core_celcius, 0.5);
                }
            }
        }
        self.prim_total = prim;
        self.sec_total = sec;
    }

    fn update_body_multipliers(&mut self) {
        self.energy_multiplier = 1.0;
        self.fluids_multiplier = 1.0;
        self.fatigue_multiplier = 1.0;
        let mut modv = pz_math::abs(self.prim_total);
        modv *= modv;
        if self.prim_total < 0.0 {
            self.energy_multiplier += f64::from(0.05f32 * modv);
            self.fatigue_multiplier += f64::from(0.25f32 * modv);
        } else if self.prim_total > 0.0 {
            self.fluids_multiplier += f64::from(0.25f32 * modv);
            self.fatigue_multiplier += f64::from(0.25f32 * modv);
        }
        modv = pz_math::abs(self.sec_total);
        modv *= modv;
        if self.sec_total < 0.0 {
            self.energy_multiplier += f64::from(0.1f32 * modv);
            self.fatigue_multiplier += f64::from(0.75f32 * modv);
        } else if self.sec_total > 0.0 {
            self.fluids_multiplier += f64::from(3.75f32 * modv);
            self.fatigue_multiplier += f64::from(1.75f32 * modv);
        }
    }

    fn update_thermal_damage(&mut self, i: &ThermoInputs, air_temperature: f32) -> f32 {
        self.damage_counter += i.gwsslu;
        if self.damage_counter > 1.0 {
            let core_celcius = self.core().celcius;
            if i.hypothermia == 4
                && air_temperature < 0.0
                && core_celcius - self.core_celcius_cache <= 0.0
            {
                let mut frost_speed = (core_celcius - 20.0) / 5.0;
                frost_speed = 1.0 - frost_speed;
                let mut seconds = 120.0f32;
                seconds += 480.0 * frost_speed;
                self.thermal_damage +=
                    1.0 / seconds * pz_math::clamp_01(pz_math::abs(air_temperature) / 10.0);
                self.thermal_damage *= 1.2;
                self.thermal_duration += self.damage_counter;
                self.thermal_damage = java_min(self.thermal_damage, self.thermal_duration / 7200.0);
            } else if i.hyperthermia == 4
                && air_temperature > 37.0
                && core_celcius - self.core_celcius_cache >= 0.0
            {
                let burnspeed = (core_celcius - 41.0) / 1.0;
                let mut seconds = 120.0f32;
                seconds += 480.0 * burnspeed;
                self.thermal_damage +=
                    1.0 / seconds * pz_math::clamp_01((air_temperature - 37.0) / 8.0);
                self.thermal_damage = java_min(self.thermal_damage, 0.3);
                self.thermal_duration = 0.0;
            } else {
                self.thermal_damage -= 0.011111111;
                self.thermal_duration = 0.0;
            }
            self.damage_counter = 0.0;
            self.thermal_damage = pz_math::clamp_01(self.thermal_damage);
        }
        self.thermal_damage
    }
}

/// `Math.min (float, float)`
fn java_min(a: f32, b: f32) -> f32 {
    crate::java::math_min_f32(a, b)
}

/// `Math.max (float, float)`
fn java_max(a: f32, b: f32) -> f32 {
    crate::java::math_max_f32(a, b)
}
