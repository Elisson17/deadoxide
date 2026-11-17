//! zombie.characters.BodyDamage.BodyDamage.Update and what it runs each tick: the Update* steps, the health
//! bookkeeping, `BodyPart.DamageUpdate`, `ClothingWetness.updateWetness` and the thermoregulator update. A path that
//! needs a world object the port does not have (splatting blood on the floor, a vehicle, worn clothing, Lua events
//! with listeners) returns `Err("not ported: ...")` instead of a made-up value.
// The conditions are the Java ones, written as they are (`!(a <= b)` differs from `a > b` for NaN; the index loops walk
// the parallel per-part arrays of the original), so the style lints that would rewrite them are off (the identical branches are the original's too).
#![allow(
    clippy::neg_cmp_op_on_partial_ord,
    clippy::needless_range_loop,
    clippy::manual_clamp,
    clippy::if_same_then_else
)]
use super::body_part_type;
use super::body_sim::PlayerBody;
use super::metabolics::Metabolics;
use super::thermoregulator::ThermoInputs;
use crate::java;
use crate::zombie::characters::stats::CharacterStat;
use crate::zombie::core::math::pz_math;
use crate::zombie::scripting::objects::moodle_type::MoodleType;

type R = Result<(), String>;

fn not_ported(what: &str) -> String {
    format!("not ported: {what}")
}

/// zombie.core.random.RandInterface.AdjustForFramerate on a dedicated server: `(int)(chance * 0.33333334F)`.
fn adjust_for_framerate(chance: i32) -> i32 {
    (chance as f32 * 0.33333334f32) as i32
}

impl PlayerBody {
    fn lvl(&self, t: MoodleType) -> i32 {
        self.moodles.moodles[t as usize].level
    }

    fn mult_f(&self) -> f32 {
        self.world.game_time.get_multiplier()
    }

    fn thirty_fps(&self) -> f32 {
        self.world.game_time.get_thirty_fps_multiplier()
    }

    fn injury_severity(&self) -> i32 {
        self.world.sandbox_int("InjurySeverity")
    }

    /// IsoGameCharacter.isRunning: not while the ENDURANCE moodle is at level 3 or more.
    pub fn is_running(&self) -> bool {
        if self.lvl(MoodleType::Endurance) >= 3 {
            false
        } else {
            self.ch.running
        }
    }

    /// IsoGameCharacter.canSprint (no equipped item stops sprinting)
    pub fn can_sprint(&self) -> bool {
        self.ch.allow_sprint && self.lvl(MoodleType::Endurance) < 2
    }

    /// IsoGameCharacter.isSprinting
    pub fn is_sprinting(&self) -> bool {
        if self.ch.sprinting && !self.can_sprint() {
            false
        } else {
            self.ch.sprinting
        }
    }

    /// zombie.characters.BodyDamage.BodyDamage.calculateOverallHealth
    pub fn calculate_overall_health(&mut self) {
        let mut total_damage = 0.0f32;
        for i in 0..body_part_type::MAX {
            let part = &self.body.body_parts[i];
            total_damage += (100.0 - part.health) * body_part_type::get_damage_modifyer(i as i32);
        }
        total_damage += self.get_damage_from_pills();
        if total_damage > 100.0 {
            total_damage = 100.0;
        }
        self.body.overall_body_health = 100.0 - total_damage;
    }

    /// zombie.characters.BodyDamage.BodyDamage.getDamageFromPills
    fn get_damage_from_pills(&self) -> f32 {
        match self.ch.sleeping_pills_taken {
            10 => 40.0,
            11 => 80.0,
            n if n >= 12 => 100.0,
            _ => 0.0,
        }
    }

    /// zombie.characters.BodyDamage.BodyDamage.AddGeneralHealth
    pub fn add_general_health(&mut self, val: f32) {
        let damaged = (0..body_part_type::MAX)
            .filter(|&i| self.body.body_parts[i].health < 100.0)
            .count() as i32;
        if damaged > 0 {
            let per_part = val / damaged as f32;
            for i in 0..body_part_type::MAX {
                if self.body.body_parts[i].health < 100.0 {
                    self.body.body_parts[i].add_health(per_part);
                }
            }
        }
    }

    /// zombie.characters.BodyDamage.BodyDamage.ReduceGeneralHealth
    pub fn reduce_general_health(&mut self, val: f32) {
        if self.body.overall_body_health <= 10.0 && self.ch.asleep {
            // IsoGameCharacter.forceAwake
            self.ch.force_wake_up = true;
        }
        if !(val <= 0.0) {
            let per_part = val / body_part_type::MAX as f32;
            for i in 0..body_part_type::MAX {
                self.body.body_parts[i]
                    .reduce_health(per_part / body_part_type::get_damage_modifyer(i as i32));
            }
        }
    }

    /// zombie.characters.BodyDamage.BodyDamage.getGeneralWoundInfectionLevel
    pub fn get_general_wound_infection_level(&self) -> f32 {
        let factor = self.world.sandbox_double("WoundInfectionFactor");
        if factor <= 0.0 {
            0.0
        } else {
            let mut level = 0.0f32;
            for bp in &self.body.body_parts {
                if bp.infected_wound {
                    level += bp.wound_infection_level;
                }
            }
            level *= 10.0;
            level *= factor as f32;
            java::math_min_f32(level, 100.0)
        }
    }

    /// zombie.characters.BodyDamage.BodyDamage.pickMortalityDuration
    pub fn pick_mortality_duration(&mut self) -> f32 {
        let mut del = 1.0f32;
        if self.ch.has_trait("Resilient") {
            del = 1.25;
        }
        if self.ch.has_trait("ProneToIllness") {
            del = 0.75;
        }
        let r = &mut self.world.rand;
        match self
            .world
            .sandbox
            .get_int("ZombieLore.Mortality")
            .unwrap_or(0)
        {
            1 => 0.0,
            2 => r.next_float(0.0, 30.0) / 3600.0 * del,
            3 => r.next_float(0.5, 1.0) / 60.0 * del,
            4 => r.next_float(3.0, 12.0) * del,
            5 => r.next_float(2.0, 3.0) * 24.0 * del,
            6 => r.next_float(1.0, 2.0) * 7.0 * 24.0 * del,
            _ => -1.0,
        }
    }

    /// zombie.characters.BodyDamage.BodyDamage.getSicknessFromCorpsesRate
    fn get_sickness_from_corpses_rate(&self, corpse_count: i32) -> f32 {
        let impact = self.world.sandbox_int("DecayingCorpseHealthImpact");
        if impact == 1 {
            return 0.0;
        }
        if corpse_count > 5 {
            let decrease = self.world.globals.field("foodSicknessDecrease");
            let mut inc = (decrease as f32) * 0.07;
            match impact {
                2 => inc = (decrease as f32) * 0.01,
                4 => inc = (decrease as f32) * 0.11,
                5 => inc = decrease as f32,
                _ => {}
            }
            // FliesSound.maxCorpseCount is 25
            let cap = std::cmp::min(corpse_count - 5, 25 - 5);
            inc * cap as f32
        } else {
            0.0
        }
    }

    /// zombie.characters.BodyDamage.BodyDamage.getCurrentTimeForInfection (an IsoPlayer: its hours survived)
    fn get_current_time_for_infection(&self) -> f32 {
        self.ch.hours_survived as f32
    }

    /// zombie.characters.BodyDamage.BodyDamage.UpdateDraggingCorpse
    fn update_dragging_corpse(&mut self) {
        let dragging = self.ch.dragging_corpse;
        if dragging != self.body.was_dragging_corpse {
            self.body.started_dragging_corpse = dragging;
            self.body.was_dragging_corpse = dragging;
        } else {
            self.body.started_dragging_corpse = false;
        }
    }

    /// zombie.characters.BodyDamage.BodyDamage.UpdatePanicState
    fn update_panic_state(&mut self) -> R {
        let visible = self.stats.num_visible_zombies;
        let old = self.body.old_num_zombies_visible;
        self.body.old_num_zombies_visible = visible;
        let new_zombies = visible - old;
        if self.ch.has_trait("Desensitized") {
            self.stats.reset(CharacterStat::Panic);
        } else {
            let mut increase = 0;
            if new_zombies > 0 {
                increase += new_zombies;
            }
            if increase > 0 {
                self.increase_panic(increase)?;
            } else {
                self.reduce_panic();
            }
        }
        Ok(())
    }

    /// zombie.characters.BodyDamage.BodyDamage.IncreasePanic
    fn increase_panic(&mut self, mut num_new_zombies_seen: i32) -> R {
        if self.ch.vehicle_inside_temperature.is_some() {
            num_new_zombies_seen /= 2;
        }
        let mut del = 1.0f32;
        if self.ch.beta_effect > 0.0 {
            del -= self.ch.beta_delta;
            if del > 1.0 {
                del = 1.0;
            }
            if del < 0.0 {
                del = 0.0;
            }
        }
        if self.ch.has_trait("Cowardly") {
            del *= 2.0;
        }
        if self.ch.has_trait("Brave") {
            del *= 0.3;
        }
        if self.ch.has_trait("Desensitized") {
            del *= 0.15;
        }
        self.stats.add(
            CharacterStat::Panic,
            self.body.panic_increase_value * num_new_zombies_seen as f32 * del,
        );
        if self.ch.has_trait("Desensitized") {
            self.stats.reset(CharacterStat::Panic);
        }
        Ok(())
    }

    /// zombie.characters.BodyDamage.BodyDamage.ReducePanic
    fn reduce_panic(&mut self) {
        if !self.stats.is_at_minimum(CharacterStat::Panic) {
            let mut delta = self.body.panic_reduction_value * self.thirty_fps();
            let mut month_survived = pz_math::fastfloor_f64(f64::from(
                ((self.ch.hours_survived as i32) / 24 / 30) as f32,
            ));
            if month_survived > 5 {
                month_survived = 5;
            }
            delta += self.body.panic_reduction_value * month_survived as f32;
            if self.ch.asleep {
                delta *= 2.0;
            }
            self.stats.remove(CharacterStat::Panic, delta);
        }
    }

    /// zombie.characters.BodyDamage.BodyDamage.UpdateStrength
    fn update_strength(&mut self) {
        let mut reducers = 0;
        match self.lvl(MoodleType::Hungry) {
            2 => reducers += 1,
            3 => reducers += 2,
            4 => reducers += 2,
            _ => {}
        }
        match self.lvl(MoodleType::Thirst) {
            2 => reducers += 1,
            3 => reducers += 2,
            4 => reducers += 2,
            _ => {}
        }
        match self.lvl(MoodleType::Sick) {
            2 => reducers += 1,
            3 => reducers += 2,
            4 => reducers += 3,
            _ => {}
        }
        if matches!(self.lvl(MoodleType::Bleeding), 2..=4) {
            reducers += 1;
        }
        match self.lvl(MoodleType::Injured) {
            2 => reducers += 1,
            3 => reducers += 2,
            4 => reducers += 3,
            _ => {}
        }
        // IsoGameCharacter.getWeightMod
        let weight_mod = match self.ch.perk_strength {
            1 => 0.9f32,
            2 => 1.07,
            3 => 1.24,
            4 => 1.41,
            5 => 1.58,
            6 => 1.75,
            7 => 1.92,
            8 => 2.09,
            9 => 2.26,
            10 => 2.5,
            _ => 0.8,
        };
        self.ch.max_weight = (self.ch.max_weight_base as f32 * weight_mod) as i32 - reducers;
        if self.ch.max_weight < 0 {
            self.ch.max_weight = 0;
        }
        self.ch.max_weight = (self.ch.max_weight as f32 * self.ch.max_weight_delta) as i32;
    }

    /// zombie.characters.BodyDamage.BodyDamage.UpdateIllness
    fn update_illness(&mut self) {
        if self.world.sandbox_int("DecayingCorpseHealthImpact") != 1 {
            let mut rate = self.get_sickness_from_corpses_rate(self.ch.corpse_count);
            if rate > 0.0 {
                // getCorpseSicknessDefense of a character wearing nothing: 0
                if self.ch.has_trait("Resilient") {
                    rate *= 0.75;
                } else if self.ch.has_trait("ProneToIllness") {
                    rate *= 1.25;
                }
                if rate > 0.0 {
                    let m = self.mult_f();
                    self.stats.add(CharacterStat::FoodSickness, rate * m);
                    self.ch.corpse_sickness_rate = rate;
                    return;
                }
            }
        }
        self.ch.corpse_sickness_rate = 0.0;
        if self.stats.is_at_minimum(CharacterStat::Poison)
            && self.stats.is_above_minimum(CharacterStat::FoodSickness)
        {
            let d = self.world.globals.field("foodSicknessDecrease") as f32 * self.mult_f();
            self.stats.remove(CharacterStat::FoodSickness, d);
        }
    }

    /// zombie.characters.BodyDamage.BodyDamage.UpdateTemperatureState
    fn update_temperature_state(&mut self) {
        if self.body.cold_damage_stage > 0.0 {
            let max_health = 100.0 - self.body.cold_damage_stage * 100.0;
            if max_health <= 0.0 {
                // IsoGameCharacter.setHealth(0)
                self.ch.health = 0.0;
                return;
            }
            if self.body.overall_body_health > max_health {
                let d = self.body.overall_body_health - max_health;
                self.reduce_general_health(d);
            }
        }
        // isoPlayer.setMoveSpeed(0.06F)
        self.ch.move_speed = 0.06;
    }

    /// zombie.characters.BodyDamage.BodyDamage.UpdateDiscomfort
    fn update_discomfort(&mut self) {
        let dragging_mod = if self.ch.dragging_corpse { 0.3f32 } else { 0.0 };
        let clothing_mod = self.ch.clothing_discomfort_modifier;
        let mut bed_mod = 0.0f32;
        if self.ch.asleep {
            match self.ch.bed_type.as_str() {
                "badBed" => bed_mod = 0.3,
                "badBedPillow" => bed_mod = 0.2,
                "floor" => bed_mod = 0.5,
                "floorPillow" => bed_mod = 0.4,
                _ => {}
            }
        }
        let drunk_mod = 1.0 - 0.5 * (self.stats.get(CharacterStat::Intoxication) / 100.0);
        let hypo_mod = 0.1 * self.lvl(MoodleType::Hypothermia) as f32;
        let hyper_mod = 0.1 * self.lvl(MoodleType::Hyperthermia) as f32;
        let wet_mod = 0.1 * self.lvl(MoodleType::Wet) as f32;
        // IsoGameCharacter.getVehicleDiscomfortModifier: 0 without a vehicle
        let vehicle_mod = 0.0f32;
        let mut discomfort_mod = 0.0f32;
        discomfort_mod += bed_mod;
        discomfort_mod += clothing_mod;
        discomfort_mod += dragging_mod;
        discomfort_mod += hypo_mod;
        discomfort_mod += hyper_mod;
        discomfort_mod += wet_mod;
        discomfort_mod += vehicle_mod;
        discomfort_mod *= drunk_mod;
        let discomfort_target = pz_math::clamp(discomfort_mod, 0.0, 1.0) * 100.0;
        let mut step = 0.005f32 * self.mult_f();
        if discomfort_target > self.stats.get(CharacterStat::Discomfort) {
            step *= 0.025;
        }
        if self.ch.asleep {
            self.stats.set(CharacterStat::Discomfort, discomfort_target);
        } else {
            let discomfort = self.stats.get(CharacterStat::Discomfort);
            if !pz_math::equal(discomfort, discomfort_target, step) {
                self.stats.set(
                    CharacterStat::Discomfort,
                    pz_math::lerp(discomfort, discomfort_target, step),
                );
            } else if discomfort != discomfort_target {
                self.stats.set(CharacterStat::Discomfort, discomfort_target);
            }
        }
        if self.lvl(MoodleType::Uncomfortable) >= 1
            && self.stats.get(CharacterStat::Stress) < CharacterStat::Stress.maximum()
        {
            let malus = if discomfort_mod > 1.0 {
                3.0 + (discomfort_mod - 1.0) * 9.0
            } else {
                3.0f32
            };
            let g = self.world.globals.field("stressFromDiscomfort");
            let v = g
                * f64::from(self.stats.get(CharacterStat::Discomfort) * malus)
                * f64::from(self.mult_f())
                * f64::from(self.world.game_time.get_delta_minutes_per_day());
            self.stats.add(CharacterStat::Stress, v as f32);
        }
    }

    /// zombie.characters.BodyDamage.BodyDamage.UpdateBoredom
    fn update_boredom(&mut self) -> R {
        if self.ch.asleep {
            return Ok(());
        }
        let g = |s: &PlayerBody, n: &str| s.world.globals.field(n);
        let mult = f64::from(self.mult_f());
        let in_room = self.ch.current_square.is_some_and(|s| s.in_room);
        if !in_room && !(self.ch.idle_square_time >= 1800.0) {
            if self.ch.vehicle_inside_temperature.is_some() {
                return Err(not_ported("UpdateBoredom in a vehicle"));
            }
            let v = (g(self, "boredomDecreaseRate") * f64::from(0.1f32) * mult) as f32;
            self.stats.remove(CharacterStat::Boredom, v);
        } else {
            let idleness = self.stats.get(CharacterStat::Idleness);
            if self.is_currently_idle() {
                let v = (g(self, "boredomIncreaseRate") * f64::from(idleness) * mult) as f32;
                self.stats.add(CharacterStat::Boredom, v);
            } else {
                let v = (g(self, "boredomIncreaseRate") / 10.0 * f64::from(idleness) * mult) as f32;
                self.stats.add(CharacterStat::Boredom, v);
            }
            if self.ch.speaking && !self.ch.call_out {
                let v = (g(self, "boredomDecreaseRate") * mult) as f32;
                self.stats.remove(CharacterStat::Boredom, v);
            }
            if self.ch.num_survivors_in_vicinity > 0 {
                let v = (g(self, "boredomDecreaseRate") * f64::from(0.1f32) * mult) as f32;
                self.stats.remove(CharacterStat::Boredom, v);
            }
            if self.is_currently_busy() && self.stats.get(CharacterStat::Idleness) < 0.1 {
                let v = (g(self, "boredomDecreaseRate") * 0.5 * mult) as f32;
                self.stats.remove(CharacterStat::Boredom, v);
            }
        }
        if self.stats.get(CharacterStat::Intoxication) > 20.0 {
            let v = (g(self, "boredomDecreaseRate") * 2.0 * mult) as f32;
            self.stats.remove(CharacterStat::Boredom, v);
        }
        if self.stats.get(CharacterStat::Panic) > 5.0 {
            self.stats.reset(CharacterStat::Boredom);
        }
        let bored = self.lvl(MoodleType::Bored);
        if bored > 1 && !self.ch.reading {
            let v = (g(self, "unhappinessIncrease") * f64::from(bored) * mult) as f32;
            self.stats.add(CharacterStat::Unhappiness, v);
        }
        let stress = self.lvl(MoodleType::Stress);
        if stress > 1 && !self.ch.reading {
            let v = (g(self, "unhappinessIncrease") / 2.0 * f64::from(stress) * mult) as f32;
            self.stats.add(CharacterStat::Unhappiness, v);
        }
        if self.ch.has_trait("Smoker") {
            let m = self.mult_f();
            self.ch.time_since_last_smoke =
                pz_math::clamp(self.ch.time_since_last_smoke + 1.0E-4 * m, 0.0, 10.0);
            if self.ch.time_since_last_smoke > 1.0 {
                let mut last = f64::from(
                    pz_math::fastfloor(self.ch.time_since_last_smoke / 10.0) as f32 + 1.0,
                );
                if last > 10.0 {
                    last = 10.0;
                }
                let v = (g(self, "stressFromBiteOrScratch") / 8.0 * last * mult) as f32;
                self.stats.add(CharacterStat::NicotineWithdrawal, v);
            }
        }
        Ok(())
    }

    /// IsoGameCharacter.isCurrentlyBusy of a player
    fn is_currently_busy(&self) -> bool {
        if self.ch.asleep || self.ch.reading {
            true
        } else {
            self.ch.has_actions || self.ch.currently_in_combat
        }
    }

    /// zombie.characters.BodyDamage.BodyDamage.UpdateCold
    fn update_cold(&mut self) -> R {
        if self.body.has_a_cold {
            let mut recovering = true;
            let in_room = self.ch.current_square.is_some_and(|s| s.in_room);
            if !in_room
                || self.lvl(MoodleType::Wet) > 0
                || self.lvl(MoodleType::Hypothermia) >= 1
                || self.stats.get(CharacterStat::Fatigue) > 0.5
                || self.stats.get(CharacterStat::Hunger) > 0.25
                || self.stats.get(CharacterStat::Thirst) > 0.25
            {
                recovering = false;
            }
            let m = self.mult_f();
            if self.body.cold_reduction > 0.0 {
                recovering = true;
                self.body.cold_reduction -= 0.005 * m;
                if self.body.cold_reduction < 0.0 {
                    self.body.cold_reduction = 0.0;
                }
            }
            if recovering {
                let mut delta = 1.0f32;
                if self.ch.has_trait("ProneToIllness") {
                    delta = 0.5;
                }
                if self.ch.has_trait("Resilient") {
                    delta = 1.5;
                }
                self.body.cold_strength -= self.body.cold_progression_rate * delta * m;
                if self.body.cold_reduction > 0.0 {
                    self.body.cold_strength -= self.body.cold_progression_rate * delta * m;
                }
                if self.body.cold_strength < 0.0 {
                    self.body.cold_strength = 0.0;
                    self.body.has_a_cold = false;
                    self.body.catch_a_cold = 0.0;
                }
            } else {
                let mut delta = 1.0f32;
                if self.ch.has_trait("ProneToIllness") {
                    delta = 1.2;
                }
                if self.ch.has_trait("Resilient") {
                    delta = 0.8;
                }
                self.body.cold_strength += self.body.cold_progression_rate * delta * m;
                if self.body.cold_strength > 100.0 {
                    self.body.cold_strength = 100.0;
                }
            }
            if self.body.sneeze_cough_time > 0 {
                self.body.sneeze_cough_time -= 1;
                if self.body.sneeze_cough_time == 0 {
                    self.body.sneeze_cough_active = 0;
                }
            }
            if self.lvl(MoodleType::HasACold) > 1
                && self.body.time_to_sneeze_or_cough >= 0.0
                && !self.ch.speaking
            {
                self.body.time_to_sneeze_or_cough -= 1.0;
                if self.body.time_to_sneeze_or_cough <= 0.0 {
                    return Err(not_ported("BodyDamage.TriggerSneezeCough (a sound)"));
                }
            }
        } else if self.ch.has_trait("Smoker") {
            if self.body.sneeze_cough_time > 0 {
                self.body.sneeze_cough_time -= 1;
                if self.body.sneeze_cough_time == 0 {
                    self.body.sneeze_cough_active = 0;
                }
            }
            if self.body.time_to_sneeze_or_cough >= 0.0 {
                if !self.ch.speaking {
                    let g = self
                        .world
                        .game_time
                        .get_game_world_seconds_since_last_update();
                    self.body.time_to_sneeze_or_cough -= g;
                    if self.body.time_to_sneeze_or_cough <= 0.0 {
                        return Err(not_ported("BodyDamage.TriggerSneezeCough (a sound)"));
                    }
                }
            } else {
                // smokerSneezeTimerMin 43200, smokerSneezeTimerMax 129600
                self.body.time_to_sneeze_or_cough =
                    43200.0 + self.world.rand.next(129600 - 43200) as f32;
            }
        }
        Ok(())
    }

    /// zombie.characters.BodyDamage.BodyDamage.UpdateWetness
    fn update_wetness(&mut self) -> R {
        let square = self.ch.current_square;
        let mut is_outside = square.is_none_or(|s| !s.in_room && !s.have_roof);
        if self.ch.vehicle_inside_temperature.is_some() {
            return Err(not_ported("UpdateWetness in a vehicle"));
        }
        // a sleeping or sitting character in a tent: no bed is placed for a headless player
        if is_outside
            && (self.ch.asleep || self.ch.sitting || self.ch.resting)
            && self.ch.bed_is_tent
        {
            is_outside = false;
        }
        let mut wetness_increase = 0.0f32;
        let mut wetness_decrease = 0.0f32;
        if is_outside && self.world.climate.is_raining() {
            let mut val = self.world.climate.get_rain_intensity();
            if val < 0.1 {
                val = 0.0;
            }
            wetness_increase = val;
        } else if !is_outside || !self.world.climate.is_raining() {
            let temperature = self.ch.air_temperature_for_character(&self.world, false);
            let mut val = 0.1f32;
            if temperature > 5.0 {
                val += (temperature - 5.0) / 10.0;
            }
            val -= 0.0;
            if val < 0.0 {
                val = 0.0;
            }
            wetness_decrease = val;
        }
        self.clothing_wetness_update(wetness_increase, wetness_decrease)?;
        let current_wetness = self.stats.get(CharacterStat::Wetness);
        let mut average = 0.0f32;
        if !self.body.body_parts.is_empty() {
            for bp in &self.body.body_parts {
                average += bp.wetness;
            }
            average /= self.body.body_parts.len() as f32;
        }
        let target = average + (current_wetness - average) * 0.1;
        for bp in &mut self.body.body_parts {
            bp.set_wetness(target);
        }
        self.stats.set(CharacterStat::Wetness, target);
        let mut delta = 0.0f32;
        let hypo = self.lvl(MoodleType::Hypothermia);
        if let Some(t) = &self.body.thermoregulator {
            delta = t.get_catch_a_cold_delta(hypo);
        }
        if !self.body.has_a_cold && delta > 0.1 {
            if self.ch.has_trait("ProneToIllness") {
                delta *= 1.7;
            }
            if self.ch.has_trait("Resilient") {
                delta *= 0.45;
            }
            if self.ch.has_trait("Outdoorsman") {
                delta *= 0.25;
            }
            let rate = self.world.globals.field("catchAColdIncreaseRate") as f32;
            self.body.catch_a_cold += rate * delta * self.mult_f();
            if self.body.catch_a_cold >= 100.0 {
                self.body.catch_a_cold = 0.0;
                self.body.has_a_cold = true;
                self.body.cold_strength = 20.0;
                self.body.time_to_sneeze_or_cough = 0.0;
            }
        }
        if delta <= 0.1 {
            self.body.catch_a_cold -= self.world.globals.field("catchAColdDecreaseRate") as f32;
            if self.body.catch_a_cold <= 0.0 {
                self.body.catch_a_cold = 0.0;
            }
        }
        Ok(())
    }

    /// zombie.characters.ClothingWetness.updateWetness of a character wearing nothing and holding no umbrella: the
    /// body parts are wetted or dried, perspiring from the thermal nodes of the previous tick.
    fn clothing_wetness_update(&mut self, outer_inc: f32, outer_dec: f32) -> R {
        if self.ch.has_worn_clothing {
            return Err(not_ported(
                "ClothingWetness.updateWetness with worn clothing",
            ));
        }
        let m = self.mult_f();
        let base_increase = (self.world.globals.field("wetnessIncrease") as f32) * m;
        let base_decrease = (self.world.globals.field("wetnessDecrease") as f32) * m;
        let mut perspiring = [0i32; body_part_type::MAX];
        let Some(thermo) = self.body.thermoregulator.as_ref() else {
            return Ok(());
        };
        for ix in 0..body_part_type::MAX {
            let node = &thermo.nodes[&ix];
            let mut base_delta = 0.0f32;
            let mut perspiration = pz_math::clamp(node.secondary_delta, 0.0, 1.0);
            perspiration *= perspiration;
            perspiration *= 0.2 + 0.8 * (1.0 - body_part_type::get_dist_to_core(ix));
            if perspiration > 0.1 {
                base_delta += perspiration;
            } else {
                let mut body_heat = (node.skin_celcius - 20.0) / 22.0;
                body_heat *= body_heat;
                body_heat -= outer_inc;
                body_heat = java::math_max_f32(0.0, body_heat);
                base_delta -= body_heat;
                if outer_inc > 0.0 {
                    base_delta = 0.0;
                }
            }
            perspiring[ix] = i32::from(base_delta > 0.0);
            if base_delta != 0.0 {
                if base_delta > 0.0 {
                    base_delta *= base_increase;
                } else {
                    base_delta *= base_decrease;
                }
                let part = &mut self.body.body_parts[ix];
                part.set_wetness(part.wetness + base_delta);
            }
        }
        for ix in 0..body_part_type::MAX {
            // no umbrella: maxWetness 100
            let max_wetness = 100.0f32;
            let mut base_delta = 0.0f32;
            if outer_inc > 0.0 {
                base_delta = outer_inc * base_increase;
            } else {
                base_delta -= outer_dec * base_decrease;
            }
            let part = &mut self.body.body_parts[ix];
            if (base_delta < 0.0 && perspiring[ix] == 0) || part.wetness <= max_wetness {
                part.set_wetness(part.wetness + base_delta);
            }
        }
        Ok(())
    }

    /// zombie.characters.BodyDamage.Thermoregulator.update with the player's state
    fn update_thermoregulator(&mut self) -> R {
        let Some(mut t) = self.body.thermoregulator.take() else {
            return Ok(());
        };
        let air = self.ch.air_temperature_for_character(&self.world, false);
        let air_wind = self.ch.air_temperature_for_character(&self.world, true);
        let (hypo, hyper) = (
            self.lvl(MoodleType::Hypothermia),
            self.lvl(MoodleType::Hyperthermia),
        );
        let running = self.is_running();
        let sprinting = self.is_sprinting();
        let result = {
            let mut i = ThermoInputs {
                stats: &mut self.stats,
                parts: &self.body.body_parts,
                hypothermia: hypo,
                hyperthermia: hyper,
                air_temperature: air,
                air_and_wind: air_wind,
                multiplier: self.world.game_time.get_multiplier(),
                gwsslu: self
                    .world
                    .game_time
                    .get_game_world_seconds_since_last_update(),
                humidity: self.world.climate.get_humidity(),
                weight: self.nutrition.weight,
                capacity_weight: self.ch.capacity_weight,
                max_weight: self.ch.max_weight,
                player_moving: self.ch.player_moving,
                sprinting,
                running,
                sneaking: self.ch.sneaking,
                current_speed: self.ch.current_speed,
                attacking: self.ch.attacking,
                fitness_exercise: self.ch.fitness_exercise,
            };
            t.update(&mut i)
        };
        self.body.thermoregulator = Some(t);
        // Thermoregulator.updateThermalDamage ends in setColdDamageStage(thermalDamage)
        self.body.cold_damage_stage = result?;
        Ok(())
    }

    /// zombie.characters.BodyDamage.BodyPart.DamageUpdate of the part `idx`
    fn damage_update(&mut self, idx: usize) -> R {
        let m = self.mult_f();
        let invulnerable = self.ch.god_mode;
        let ds = self.body.body_parts[idx].damage_scaler;
        // CombatManager.applyDamage(BodyPart, float)
        macro_rules! apply_damage {
            ($v:expr) => {
                if !invulnerable {
                    self.body.body_parts[idx].reduce_health($v);
                }
            };
        }
        macro_rules! p {
            () => {
                self.body.body_parts[idx]
            };
        }
        let dbl = |c: f64| (c * f64::from(m)) as f32;
        if p!().deep_wound_time > 0.0 && !p!().stitched {
            let mut damage = 3.125f32 * ds * m;
            if p!().bandaged {
                damage = 1.5625f32 * ds * m;
            }
            apply_damage!(damage);
        }
        if p!().scratch_time > 0.0 && !p!().bandaged {
            apply_damage!(0.9375f32 * ds * m);
        }
        if p!().cut_time > 0.0 && !p!().bandaged {
            apply_damage!(1.875f32 * ds * m);
        }
        if p!().bite_time > 0.0 && !p!().bandaged {
            apply_damage!(2.1875f32 * ds * m);
        }
        if p!().bleeding_time > 0.0 && !p!().bandaged {
            let final_bleed = 0.2857143f32 * ds * m * (p!().bleeding_time / 10.0);
            self.reduce_general_health(final_bleed);
            // Rand.NextBool(Rand.AdjustForFramerate(1000)) then IsoGameCharacter.addBlood
            if self.world.rand.next_bool(adjust_for_framerate(1000)) {
                // IsoGameCharacter.addBlood(part, false, false, true): the blood of the `HumanVisual` and of the worn
                // clothing (`OutfitRNG`, not `Rand`); nothing the body simulation reads
            }
        }
        if p!().have_bullet {
            let mut damage = 3.125f32 * ds * m;
            if p!().bandaged {
                damage = 1.5625f32 * ds * m;
            }
            apply_damage!(damage);
        }
        if p!().burn_time > 0.0 && !p!().bandaged {
            apply_damage!(3.75f32 * ds * m);
        }
        if p!().fracture_time > 0.0 && !p!().splint {
            apply_damage!(3.125f32 * ds * m);
        }
        if p!().bite_time > 0.0 {
            if p!().bandaged {
                p!().bite_time -= dbl(1.0E-4);
                let bl = p!().bandage_life - dbl(1.0E-4);
                p!().set_bandage_life(bl);
            } else {
                p!().bite_time -= dbl(5.0E-6);
            }
        }
        if p!().burn_time > 0.0 {
            if p!().bandaged {
                p!().burn_time -= dbl(1.0E-4);
                let bl = p!().bandage_life - dbl(1.0E-4);
                p!().set_bandage_life(bl);
            } else {
                p!().burn_time -= dbl(5.0E-6);
            }
            if p!().last_time_burn_wash - p!().burn_time >= 20.0 {
                p!().last_time_burn_wash = 0.0;
                p!().set_need_burn_wash(true);
            }
        }
        if p!().bleeding_time > 0.0 {
            if p!().bandaged {
                let bt = p!().bleeding_time - dbl(2.0E-4);
                p!().set_bleeding_time(bt);
                if p!().deep_wound_time > 0.0 {
                    let bl = p!().bandage_life - dbl(0.005);
                    p!().set_bandage_life(bl);
                } else {
                    let bl = p!().bandage_life - dbl(3.0E-4);
                    p!().set_bandage_life(bl);
                }
            } else {
                let bt = p!().bleeding_time - dbl(2.0E-5);
                p!().set_bleeding_time(bt);
            }
            if p!().bleeding_time < 3.0 && p!().have_glass {
                p!().set_bleeding_time(3.0);
            }
            if p!().bleeding_time < 0.0 {
                p!().set_bleeding_time(0.0);
                p!().set_bleeding(false);
            }
        }
        if !p!().infected_wound
            && !p!().is_infected
            && (!p!().alcoholic_bandage || !(p!().bandage_life > 0.0))
            && (p!().deep_wound_time > 0.0
                || p!().scratch_time > 0.0
                || p!().cut_time > 0.0
                || p!().stitch_time > 0.0)
        {
            let mut base_chance = 40000i32;
            if !p!().bandaged {
                base_chance -= 10000;
            } else if p!().bandage_life == 0.0 {
                base_chance -= 35000;
            }
            if p!().scratch_time > 0.0 {
                base_chance -= 20000;
            }
            if p!().cut_time > 0.0 {
                base_chance -= 25000;
            }
            if p!().deep_wound_time > 0.0 {
                base_chance -= 30000;
            }
            if p!().have_glass {
                base_chance -= 24000;
            }
            if p!().burn_time > 0.0 {
                base_chance -= 23000;
                if p!().need_burn_wash {
                    base_chance -= 7000;
                }
            }
            // hasDirtyClothing / hasBloodyClothing: nothing worn
            if self.ch.has_worn_clothing {
                return Err(not_ported("BodyPart.DamageUpdate with worn clothing"));
            }
            if base_chance <= 5000 {
                base_chance = 5000;
            }
            if self.world.rand.next(adjust_for_framerate(base_chance)) == 0 {
                p!().infected_wound = true;
            }
        } else if p!().infected_wound {
            let mut reduce = false;
            if p!().alcohol_level > 0.0 {
                p!().alcohol_level -= 2.0E-4f32 * m;
                let l = p!().wound_infection_level - 2.0E-4f32 * m;
                p!().set_wound_infection_level(l);
                if p!().alcohol_level < 0.0 {
                    p!().alcohol_level = 0.0;
                }
                reduce = true;
            }
            if self.ch.reduce_infection_power > 0.0 {
                let l = p!().wound_infection_level - 2.0E-4f32 * m;
                p!().set_wound_infection_level(l);
                self.ch.reduce_infection_power -= 2.0E-4f32 * m;
                if self.ch.reduce_infection_power < 0.0 {
                    self.ch.reduce_infection_power = 0.0;
                }
                reduce = true;
            }
            if p!().garlic_factor > 0.0 {
                let l = p!().wound_infection_level - 2.0E-4f32 * m;
                p!().set_wound_infection_level(l);
                let g = p!().garlic_factor - 8.0E-4f32 * m;
                p!().set_garlic_factor(g);
                reduce = true;
            }
            if !reduce {
                let l = p!().wound_infection_level;
                if p!().is_infected {
                    p!().set_wound_infection_level(l + 2.0E-4f32 * m);
                } else if p!().have_glass {
                    p!().set_wound_infection_level(l + 1.0E-4f32 * m);
                } else {
                    p!().set_wound_infection_level(l + 1.0E-5f32 * m);
                }
            }
        }
        if p!().wound_infection_level > 10.0 {
            p!().set_wound_infection_level(10.0);
        }
        if !p!().infected_wound && p!().alcohol_level > 0.0 {
            p!().alcohol_level -= 2.0E-4f32 * m;
            if p!().alcohol_level < 0.0 {
                p!().alcohol_level = 0.0;
            }
        }
        if p!().infected_wound && p!().bandage_life > 0.0 {
            if p!().alcoholic_bandage {
                let l = p!().wound_infection_level - 6.0E-4f32 * m;
                p!().set_wound_infection_level(l);
            }
            let bl = p!().bandage_life - dbl(2.0E-4);
            p!().set_bandage_life(bl);
        }
        if p!().scratch_time > 0.0 {
            if p!().bandaged {
                let t = p!().scratch_time - dbl(1.5E-4);
                p!().set_scratch_time(t);
                let bl = p!().bandage_life - dbl(8.0E-5);
                p!().set_bandage_life(bl);
                if p!().plantain_factor > 0.0 {
                    let t = p!().scratch_time - dbl(1.0E-4);
                    p!().set_scratch_time(t);
                    let f = p!().plantain_factor - dbl(8.0E-4);
                    p!().set_plantain_factor(f);
                }
            } else {
                let t = p!().scratch_time - dbl(1.0E-5);
                p!().set_scratch_time(t);
            }
            if p!().scratch_time < 0.0 {
                p!().set_scratch_time(0.0);
                p!().get_bandage_xp = true;
                p!().get_stitch_xp = true;
                p!().set_scratched(false)?;
                p!().set_bleeding(false);
                p!().set_bleeding_time(0.0);
                p!().reset_poultice_factors();
            }
        }
        if p!().cut_time > 0.0 {
            if p!().bandaged {
                let t = p!().cut_time - dbl(5.0E-5);
                p!().set_cut_time(t);
                let bl = p!().bandage_life - dbl(1.0E-5);
                p!().set_bandage_life(bl);
                if p!().plantain_factor > 0.0 {
                    let t = p!().cut_time - dbl(5.0E-5);
                    p!().set_cut_time(t);
                    let f = p!().plantain_factor - dbl(8.0E-4);
                    p!().set_plantain_factor(f);
                }
            } else {
                let t = p!().cut_time - dbl(1.0E-6);
                p!().set_cut_time(t);
            }
            if p!().cut_time < 0.0 {
                p!().set_cut_time(0.0);
                p!().get_bandage_xp = true;
                p!().get_stitch_xp = true;
                p!().set_bleeding(false);
                p!().set_bleeding_time(0.0);
                p!().reset_poultice_factors();
            }
        }
        if p!().deep_wound_time > 0.0 {
            if p!().bandaged {
                p!().deep_wound_time -= dbl(2.0E-5);
                let bl = p!().bandage_life - dbl(1.0E-4);
                p!().set_bandage_life(bl);
                if p!().plantain_factor > 0.0 {
                    p!().deep_wound_time -= dbl(7.0E-6);
                    let f = p!().plantain_factor - dbl(8.0E-4);
                    p!().set_plantain_factor(f);
                    if p!().plantain_factor < 0.0 {
                        p!().set_plantain_factor(0.0);
                    }
                }
            } else {
                p!().deep_wound_time -= dbl(2.0E-6);
            }
            if (p!().have_glass || !p!().bandaged) && p!().deep_wound_time < 3.0 {
                p!().deep_wound_time = 3.0;
            }
            if p!().deep_wound_time < 0.0 {
                p!().get_bandage_xp = true;
                p!().get_stitch_xp = true;
                p!().deep_wound_time = 0.0;
                p!().set_deep_wounded(false);
            }
        }
        if p!().stitch_time > 0.0 && p!().stitch_time < 50.0 {
            if p!().bandaged {
                p!().stitch_time += dbl(4.0E-4);
                let bl = p!().bandage_life - dbl(1.0E-4);
                p!().set_bandage_life(bl);
                if !p!().alcoholic_bandage && self.world.rand.next(adjust_for_framerate(80000)) == 0
                {
                    p!().infected_wound = true;
                }
                p!().stitch_time += dbl(1.0E-4);
            } else {
                p!().stitch_time += dbl(2.0E-4);
                if self.world.rand.next(adjust_for_framerate(20000)) == 0 {
                    p!().infected_wound = true;
                }
            }
            if p!().stitch_time > 30.0 {
                p!().get_stitch_xp = true;
            }
            if p!().stitch_time > 50.0 {
                p!().stitch_time = 50.0;
            }
        }
        if p!().fracture_time > 0.0 {
            if p!().splint_factor > 0.0 {
                let sf = f64::from(p!().splint_factor);
                p!().fracture_time -= (5.0E-5 * f64::from(m) * sf) as f32;
            } else {
                p!().fracture_time -= dbl(5.0E-6);
            }
            if p!().comfrey_factor > 0.0 {
                p!().fracture_time -= dbl(5.0E-6);
                let f = p!().comfrey_factor - dbl(5.0E-4);
                p!().set_comfrey_factor(f);
            }
            if p!().fracture_time < 0.0 {
                p!().fracture_time = 0.0;
                p!().get_splint_xp = true;
                p!().reset_poultice_factors();
            }
        }
        if p!().additional_pain > 0.0 {
            p!().additional_pain -= dbl(0.005);
            if p!().additional_pain < 0.0 {
                p!().additional_pain = 0.0;
            }
        }
        // isoPlayer.getFitness() != null && !getFitness().onGoingStiffness(): no exercise is under way
        if p!().stiffness > 0.0 && !self.ch.ongoing_stiffness {
            let s = p!().stiffness - dbl(0.002);
            p!().set_stiffness(s);
            if p!().stiffness < 0.0 {
                p!().set_stiffness(0.0);
            }
        }
        if p!().bandage_life < 0.0 {
            p!().set_bandage_life(0.0);
            p!().get_bandage_xp = true;
        }
        if (p!().wound_infection_level > 0.0 || p!().infected_wound)
            && p!().burn_time <= 0.0
            && p!().fracture_time <= 0.0
            && p!().deep_wound_time <= 0.0
            && p!().scratch_time <= 0.0
            && p!().bite_time <= 0.0
            && p!().cut_time <= 0.0
            && p!().stitch_time <= 0.0
        {
            p!().set_wound_infection_level(0.0);
        }
        if p!().health < 0.0 {
            p!().health = 0.0;
        }
        Ok(())
    }

    /// zombie.characters.IsoGameCharacter.splatBloodFloor: the draws of the three splats. `IsoChunk.addBloodSplat` adds a
    /// floor splat to the chunk's list (and sends it to clients), which nothing in the body simulation reads.
    fn splat_blood_floor(&mut self) {
        if self.ch.current_square.is_some() {
            if (self.ch.health <= 0.0 || self.body.overall_body_health <= 0.0)
                && self.world.rand.next(10) == 0
            {
                self.world.rand.next(20);
            }
            if self.world.rand.next(14) == 0 {
                self.world.rand.next(8);
            }
            if self.world.rand.next(50) == 0 {
                self.world.rand.next(20);
            }
        }
    }

    /// zombie.characters.BodyDamage.BodyDamage.Update
    pub fn update_body(&mut self) -> R {
        self.sync_dead();
        if self.ch.god_mode {
            return Err(not_ported("BodyDamage.Update of a god-mode player"));
        }
        let last_pain = self.stats.get(CharacterStat::Pain);
        let mut n = self.body.get_num_parts_bleeding() * 2;
        n += self.body.get_num_parts_scratched();
        n += self.body.get_num_parts_bitten() * 6;
        if self.body.overall_body_health >= 60.0 && n <= 3 {
            n = 0;
        }
        self.ch.bleeding_level = n as i8;
        if n > 0 {
            let bleed_chance =
                1.0f32 / n as f32 * 200.0 * self.world.game_time.get_inv_multiplier();
            if (self.world.rand.next(bleed_chance as i32) as f32) < bleed_chance * 0.3 {
                self.splat_blood_floor();
            }
            if self.world.rand.next(bleed_chance as i32) == 0 {
                self.splat_blood_floor();
            }
        }
        self.update_thermoregulator()?;
        self.update_dragging_corpse();
        self.update_wetness()?;
        self.update_cold()?;
        self.update_boredom()?;
        self.update_strength();
        self.update_panic_state()?;
        self.update_temperature_state();
        self.update_discomfort();
        self.update_illness();
        if self.body.overall_body_health != 0.0 {
            if !self.body.is_infected {
                for i in 0..body_part_type::MAX {
                    if self.body.body_parts[i].is_infected {
                        self.body.is_infected = true;
                        if self.body.body_parts[i].is_fake_infected {
                            self.body.body_parts[i].is_fake_infected = false;
                            let fever = self.stats.get(CharacterStat::ZombieFever);
                            self.stats.set(CharacterStat::ZombieInfection, fever);
                            self.stats.reset(CharacterStat::ZombieFever);
                            self.set_is_fake_infected(false);
                            self.body.reduce_fake_infection = false;
                        }
                    }
                }
                if self.body.is_infected
                    && self.body.infection_time < 0.0
                    && self
                        .world
                        .sandbox
                        .get_int("ZombieLore.Mortality")
                        .unwrap_or(0)
                        != 7
                {
                    self.body.infection_time = self.get_current_time_for_infection();
                    self.body.infection_mortality_duration = self.pick_mortality_duration();
                }
            }
            if !self.body.is_infected && !self.body.is_fake_infected {
                for i in 0..body_part_type::MAX {
                    if self.body.body_parts[i].is_fake_infected {
                        self.set_is_fake_infected(true);
                        break;
                    }
                }
            }
            if self.body.is_fake_infected
                && !self.body.reduce_fake_infection
                && self.ch.reduce_infection_power == 0.0
            {
                let v = self.body.infection_growth_rate * self.mult_f();
                self.stats.add(CharacterStat::ZombieFever, v);
                if self.stats.is_at_maximum(CharacterStat::ZombieFever) {
                    self.body.reduce_fake_infection = true;
                }
            }
            let v = self.body.drunk_reduction_value * self.mult_f();
            self.stats.remove(CharacterStat::Intoxication, v);
            let m = self.mult_f();
            let mut health_to_add = 0.0f32;
            if self.body.health_from_food_timer > 0.0 {
                health_to_add += self.body.health_from_food * m;
                self.body.health_from_food_timer -= 1.0 * m;
            }
            let mut reduced = 0;
            let (hungry, sick, thirst) = (
                self.lvl(MoodleType::Hungry),
                self.lvl(MoodleType::Sick),
                self.lvl(MoodleType::Thirst),
            );
            if hungry == 2 || sick == 2 || thirst == 2 {
                reduced = 1;
            }
            if hungry == 3 || sick == 3 || thirst == 3 {
                reduced = 2;
            }
            if hungry == 4 || thirst == 4 {
                reduced = 3;
            }
            if self.ch.asleep {
                reduced = -1;
            }
            match reduced {
                0 => health_to_add += self.body.standard_health_addition * m,
                1 => health_to_add += self.body.reduced_health_addition * m,
                2 => health_to_add += self.body.severly_reduced_health_addition * m,
                3 => health_to_add += 0.0,
                _ => {}
            }
            if self.ch.asleep {
                health_to_add += self.body.sleeping_health_addition * m;
                if hungry == 4 || thirst == 4 {
                    health_to_add = 0.0;
                }
            }
            self.add_general_health(health_to_add);
            health_to_add = 0.0;
            let mut poison_damage = 0.0f32;
            let mut hungry_damage;
            let mut sick_damage;
            let mut bleeding_damage;
            let mut thirst_damage = 0.0f32;
            let heavy_load_damage;
            let poison = self.stats.get(CharacterStat::Poison);
            if poison > 0.0 {
                if poison > 10.0 && self.lvl(MoodleType::Sick) >= 1 {
                    poison_damage = 0.0035f32 * java::math_min_f32(poison / 10.0, 3.0) * m;
                    health_to_add += poison_damage;
                }
                let mut well_fed = 0.0f32;
                if self.lvl(MoodleType::FoodEaten) > 0 {
                    well_fed = 1.5E-4f32 * self.lvl(MoodleType::FoodEaten) as f32;
                }
                let dec = (f64::from(well_fed)
                    + self.world.globals.field("poisonLevelDecrease") * f64::from(m))
                    as f32;
                self.stats.remove(CharacterStat::Poison, dec);
                let round = java::math_round_f32(self.stats.get(CharacterStat::Poison) / 10.0);
                let v = self.body.infection_growth_rate * (2.0 + round as f32) * m;
                self.stats.add(CharacterStat::FoodSickness, v);
            }
            let severe = self.body.health_reduction_from_severe_bad_moodles;
            if self.lvl(MoodleType::Hungry) == 4 {
                hungry_damage = severe / 50.0 * m;
                health_to_add += hungry_damage;
            }
            if self.lvl(MoodleType::Sick) == 4 {
                if self.stats.get(CharacterStat::FoodSickness)
                    > self.stats.get(CharacterStat::ZombieInfection)
                {
                    sick_damage = severe * m;
                    health_to_add += sick_damage;
                } else if self.world.sandbox_double("WoundInfectionFactor") > 0.0
                    && self.get_general_wound_infection_level()
                        > self.stats.get(CharacterStat::ZombieInfection)
                {
                    sick_damage = severe * m;
                    health_to_add += sick_damage;
                }
            }
            if self.lvl(MoodleType::Bleeding) == 4 {
                bleeding_damage = severe * m;
                health_to_add += bleeding_damage;
            }
            if self.lvl(MoodleType::Thirst) == 4 {
                thirst_damage = severe / 10.0 * m;
                health_to_add += thirst_damage;
            }
            let heavy = self.lvl(MoodleType::HeavyLoad);
            let seated = self
                .body
                .thermoregulator
                .as_ref()
                .is_some_and(|t| t.metabolic_target == Metabolics::SeatedResting.get_met());
            if heavy > 2
                && self.ch.vehicle_inside_temperature.is_none()
                && !self.ch.asleep
                && !self.ch.sitting
                && !seated
                && self.body.overall_body_health > 75.0
                && {
                    // Rand.PerThirtiethOfASecond(10)
                    let m30 = self.thirty_fps();
                    self.world.rand.next_float(0.0, 10.0 * m30) < m30
                }
            {
                heavy_load_damage = severe / ((5 - heavy) as f32 / 10.0) * m;
                health_to_add += heavy_load_damage;
                self.add_back_muscle_strain(heavy_load_damage / 2.0);
            }
            self.reduce_general_health(health_to_add);
            // LuaEventManager.triggerEvent("OnPlayerGetDamage", ...): no listener in the server's Lua
            let _ = (poison_damage, thirst_damage);
            if self.ch.pain_effect > 0.0 {
                let t = self.thirty_fps();
                self.stats.remove(CharacterStat::Pain, 0.023333333f32 * t);
                self.ch.pain_effect -= t;
            } else {
                self.ch.pain_delta = 0.0;
                health_to_add = 0.0;
                let sev = self.injury_severity();
                for i in 0..body_part_type::MAX {
                    health_to_add += self.body.body_parts[i].get_pain(sev)
                        * body_part_type::get_pain_modifyer(i as i32);
                }
                health_to_add -= self.body.pain_reduction;
                let pain = self.stats.get(CharacterStat::Pain);
                if health_to_add > pain {
                    self.stats
                        .add(CharacterStat::Pain, (health_to_add - pain) / 500.0);
                } else {
                    self.stats.set(CharacterStat::Pain, health_to_add);
                }
            }
            self.body.pain_reduction -= 0.005 * self.mult_f();
            if self.body.pain_reduction < 0.0 {
                self.body.pain_reduction = 0.0;
            }
            if self.body.is_infected {
                let mortality = self
                    .world
                    .sandbox
                    .get_int("ZombieLore.Mortality")
                    .unwrap_or(0);
                if mortality == 1 {
                    self.reduce_general_health(110.0);
                    self.stats.set(
                        CharacterStat::ZombieInfection,
                        CharacterStat::ZombieInfection.maximum(),
                    );
                } else if mortality != 7 {
                    let world_age_hours = self.get_current_time_for_infection();
                    if self.body.infection_mortality_duration < 0.0 {
                        self.body.infection_mortality_duration = self.pick_mortality_duration();
                    }
                    self.body.infection_time =
                        check_hours_f32(self.body.infection_time, world_age_hours);
                    hungry_damage = java::math_min_f32(
                        (world_age_hours - self.body.infection_time)
                            / self.body.infection_mortality_duration,
                        1.0,
                    );
                    self.stats
                        .set(CharacterStat::ZombieInfection, hungry_damage * 100.0);
                    if hungry_damage == 1.0 {
                        self.reduce_general_health(110.0);
                    } else {
                        hungry_damage *= hungry_damage;
                        hungry_damage *= hungry_damage;
                        sick_damage = (1.0 - hungry_damage) * 100.0;
                        bleeding_damage = self.body.overall_body_health - sick_damage;
                        if bleeding_damage > 0.0 && sick_damage <= 99.0 {
                            self.reduce_general_health(bleeding_damage);
                        }
                    }
                }
            }
            for i in 0..body_part_type::MAX {
                self.damage_update(i)?;
            }
            self.calculate_overall_health();
            if self.body.overall_body_health <= 0.0 {
                if self.body.is_on_fire {
                    return Err(not_ported("BodyDamage.Update: burnt to death"));
                }
                self.body.burnt_to_death = false;
            }
            if self.body.reduce_fake_infection && self.body.overall_body_health > 0.0 {
                let v = self.body.infection_growth_rate * self.mult_f() * 2.0;
                self.stats.remove(CharacterStat::ZombieFever, v);
            }
            if self.ch.reduce_infection_power > 0.0 && self.body.overall_body_health > 0.0 {
                let v = self.body.infection_growth_rate * self.mult_f();
                self.stats.remove(CharacterStat::ZombieFever, v);
                self.ch.reduce_infection_power -= v;
                if self.ch.reduce_infection_power < 0.0 {
                    self.ch.reduce_infection_power = 0.0;
                }
            }
            if self.stats.get(CharacterStat::ZombieFever) <= 0.0 {
                for i in 0..body_part_type::MAX {
                    self.body.body_parts[i].is_fake_infected = false;
                }
                self.set_is_fake_infected(false);
                self.stats.reset(CharacterStat::ZombieFever);
                self.body.reduce_fake_infection = false;
            }
            if last_pain == self.stats.get(CharacterStat::Pain) {
                let t = self.thirty_fps();
                self.stats.remove(CharacterStat::Pain, 0.25 * t);
            }
        }
        Ok(())
    }

    /// zombie.characters.BodyDamage.BodyDamage.setIsFakeInfected: also marks the first body part
    fn set_is_fake_infected(&mut self, fake: bool) {
        self.body.is_fake_infected = fake;
        self.body.body_parts[0].is_fake_infected = fake;
    }

    /// zombie.characters.IsoGameCharacter.addBackMuscleStrain
    fn add_back_muscle_strain(&mut self, mut painfactor: f32) {
        let factor = self.world.sandbox_double("MuscleStrainFactor");
        if factor > 0.0 {
            painfactor *= 2.5;
            painfactor = (f64::from(painfactor) * factor) as f32;
            for i in [body_part_type::TORSO_UPPER, body_part_type::TORSO_LOWER] {
                let p = &mut self.body.body_parts[i];
                p.stiffness = pz_math::clamp(p.stiffness + painfactor, 0.0, 100.0);
            }
        }
    }
}

/// zombie.GameTime.checkHours (float, float)
fn check_hours_f32(hours: f32, world_age_hours: f32) -> f32 {
    if hours < 0.0 {
        world_age_hours
    } else {
        // GameTime.minHours (float): PZMath.min
        pz_math::min(hours, world_age_hours)
    }
}
