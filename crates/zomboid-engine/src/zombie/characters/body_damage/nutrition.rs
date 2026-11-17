//! zombie.characters.BodyDamage.Nutrition: calories, macronutrients and body weight, as a save stores them.
use super::body_sim::{BodyWorld, CharCtx};
use crate::java_nio::ByteBuffer;
use crate::zombie::inventory::inventory_item::not_ported;

/// zombie.characters.BodyDamage.Nutrition: the saved fields and what `update` keeps between ticks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Nutrition {
    pub calories: f32,
    pub proteins: f32,
    pub lipids: f32,
    pub carbohydrates: f32,
    pub weight: f64,
    pub updated_weight: i32,
    pub calories_max: f32,
    pub calories_min: f32,
    pub inc_weight: bool,
    pub inc_weight_lot: bool,
    pub dec_weight: bool,
}

impl Nutrition {
    /// zombie.characters.BodyDamage.Nutrition.load through the clamping setters. A weight below 35 also damages the
    /// character (`setWeight`), which is not ported.
    pub fn load(b: &mut ByteBuffer) -> Result<Self, String> {
        let clamp = |v: f32, lo: f32, hi: f32| {
            let v = if v < lo { lo } else { v };
            if v > hi {
                hi
            } else {
                v
            }
        };
        let calories = clamp(b.get_float()?, -2200.0, 3700.0);
        let proteins = clamp(b.get_float()?, -500.0, 1000.0);
        let lipids = clamp(b.get_float()?, -500.0, 1000.0);
        let carbohydrates = clamp(b.get_float()?, -500.0, 1000.0);
        let weight = f64::from(b.get_float()?);
        if weight < 35.0 {
            return Err(not_ported("Nutrition.setWeight below 35: health damage"));
        }
        Ok(Nutrition {
            calories,
            proteins,
            lipids,
            carbohydrates,
            weight,
            ..Nutrition::new()
        })
    }
}

impl Default for Nutrition {
    fn default() -> Self {
        Self::new()
    }
}

impl Nutrition {
    /// `new Nutrition(parent)`: `setWeight(80.0)` and `setCalories(800.0F)` over the field initialisers.
    pub fn new() -> Self {
        Nutrition {
            calories: 800.0,
            proteins: 0.0,
            lipids: 0.0,
            carbohydrates: 0.0,
            weight: 80.0,
            updated_weight: 0,
            calories_max: 0.0,
            calories_min: 0.0,
            inc_weight: false,
            inc_weight_lot: false,
            dec_weight: false,
        }
    }

    /// `setCarbohydrates`
    pub fn set_carbohydrates(&mut self, carbohydrates: f32) {
        self.carbohydrates = carbohydrates.clamp_java(-500.0, 1000.0);
    }

    /// `setProteins`
    pub fn set_proteins(&mut self, proteins: f32) {
        self.proteins = proteins.clamp_java(-500.0, 1000.0);
    }

    /// `setCalories`
    pub fn set_calories(&mut self, calories: f32) {
        self.calories = calories.clamp_java(-2200.0, 3700.0);
    }

    /// `setLipids`
    pub fn set_lipids(&mut self, lipids: f32) {
        self.lipids = lipids.clamp_java(-500.0, 1000.0);
    }

    /// `setWeight`: below 35 the weight is 35 and the character is damaged by
    /// `getHealthReductionFromSevereBadMoodles() * getMultiplier()`; the damage is not applied here, the answer is
    /// true when it is due (the caller runs `ReduceGeneralHealth` and fires `OnPlayerGetDamage "LOWWEIGHT"`).
    #[must_use]
    pub fn set_weight(&mut self, weight: f64) -> bool {
        let mut low = false;
        let mut weight = weight;
        if weight < 35.0 {
            weight = 35.0;
            low = true;
        }
        self.weight = weight;
        low
    }

    /// `Nutrition.update`
    pub fn update(&mut self, c: &mut CharCtx, w: &BodyWorld, energy_multiplier: Option<f64>) {
        if !w.sandbox_bool("Nutrition") || c.is_dead || c.god_mode {
            return;
        }
        let secs = w.game_time.get_game_world_seconds_since_last_update();
        self.set_carbohydrates(self.carbohydrates - 0.0035f32 * secs);
        self.set_lipids(self.lipids - 0.00113f32 * secs);
        self.set_proteins(self.proteins - 8.6E-4f32 * secs);
        self.update_calories(c, w, energy_multiplier);
        self.update_weight(c, w);
    }

    /// `Nutrition.updateCalories`
    fn update_calories(&mut self, c: &CharCtx, w: &BodyWorld, energy_multiplier: Option<f64>) {
        let secs = w.game_time.get_game_world_seconds_since_last_update();
        let mut modifier = 1.0f32;
        if let Some(m) = c.first_action_calories_modifier {
            modifier = m;
        }
        if c.in_swipe_or_climb_state {
            modifier = 8.0;
        }
        let mut cold_multi = 1.0f32;
        if let Some(e) = energy_multiplier {
            cold_multi = e as f32;
        }
        let calories_delta = (self.weight / 80.0) as f32;
        if c.running && c.player_moving {
            modifier = 1.0;
            self.set_calories(self.calories - 0.13f32 * modifier * calories_delta * secs);
        } else if c.sprinting && c.player_moving {
            modifier = 1.3;
            self.set_calories(self.calories - 0.13f32 * modifier * calories_delta * secs);
        } else if c.player_moving {
            modifier = 0.6;
            self.set_calories(self.calories - 0.13f32 * modifier * calories_delta * secs);
        } else if c.asleep {
            self.set_calories(
                self.calories - 0.003f32 * modifier * cold_multi * calories_delta * secs,
            );
        } else {
            self.set_calories(
                self.calories - 0.016f32 * modifier * cold_multi * calories_delta * secs,
            );
        }
        if self.calories > self.calories_max {
            self.calories_max = self.calories;
        }
        if self.calories < self.calories_min {
            self.calories_min = self.calories;
        }
    }

    /// `Nutrition.updateWeight`
    fn update_weight(&mut self, c: &mut CharCtx, w: &BodyWorld) {
        let secs = w.game_time.get_game_world_seconds_since_last_update();
        self.inc_weight = false;
        self.inc_weight_lot = false;
        self.dec_weight = false;
        let mut calories_to_gain_weight = 1000.0f32;
        let calories_to_gain_weight_max = 4000.0f32;
        if self.weight < 90.0 && c.has_trait("WeightGain") {
            calories_to_gain_weight = 700.0;
        }
        if self.weight > 70.0 && c.has_trait("WeightLoss") {
            calories_to_gain_weight = 1800.0;
        }
        let calories_diff = ((self.weight - 80.0) * 40.0) as f32;
        calories_to_gain_weight += calories_diff;
        let mut calories_to_lose_weight = ((self.weight - 70.0) * 30.0) as f32;
        if calories_to_lose_weight > 0.0 {
            calories_to_lose_weight = 0.0;
        }
        let weight: f64;
        if self.calories > calories_to_gain_weight {
            self.inc_weight = true;
            let mut delta = self.calories / calories_to_gain_weight_max;
            if delta > 1.0 {
                delta = 1.0;
            }
            let mut real_weight_gain = 1.3E-5f32;
            if self.carbohydrates > 700.0 || self.lipids > 700.0 {
                real_weight_gain *= 3.0;
                self.inc_weight_lot = true;
            } else if self.carbohydrates > 400.0 || self.lipids > 400.0 {
                real_weight_gain *= 2.0;
                self.inc_weight_lot = true;
            }
            weight = self.weight + f64::from(real_weight_gain * delta * secs);
        } else if self.calories < calories_to_lose_weight {
            self.dec_weight = true;
            let mut delta = self.calories.abs() / 2500.0;
            if delta > 1.0 {
                delta = 1.0;
            }
            weight = self.weight - f64::from(8.5E-6f32 * delta * secs);
        } else {
            weight = self.weight;
        }
        if self.set_weight(weight) {
            c.low_weight_damage_due = true;
        }
        self.updated_weight += 1;
        if self.updated_weight >= 2000 {
            self.apply_trait_from_weight(c);
            self.updated_weight = 0;
        }
    }

    /// `Nutrition.applyTraitFromWeight`
    pub fn apply_trait_from_weight(&self, c: &mut CharCtx) {
        for t in [
            "Underweight",
            "Very Underweight",
            "Emaciated",
            "Overweight",
            "Obese",
        ] {
            c.remove_trait(t);
        }
        if self.weight >= 100.0 {
            c.add_trait("Obese");
        }
        if self.weight >= 85.0 && self.weight < 100.0 {
            c.add_trait("Overweight");
        }
        if self.weight > 65.0 && self.weight <= 75.0 {
            c.add_trait("Underweight");
        }
        if self.weight > 50.0 && self.weight <= 65.0 {
            c.add_trait("Very Underweight");
        }
        if self.weight <= 50.0 {
            c.add_trait("Emaciated");
        }
    }
}

/// Java's `PZMath`-style clamp on a float, as the setters spell it with two `if`s.
trait ClampJava {
    fn clamp_java(self, lo: f32, hi: f32) -> f32;
}

impl ClampJava for f32 {
    fn clamp_java(self, lo: f32, hi: f32) -> f32 {
        let mut v = self;
        if v < lo {
            v = lo;
        }
        if v > hi {
            v = hi;
        }
        v
    }
}
