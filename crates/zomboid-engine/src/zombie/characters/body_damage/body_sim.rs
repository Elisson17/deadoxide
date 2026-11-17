//! The world and character state the player body simulation (BodyDamage, Nutrition, the stat updates of
//! IsoGameCharacter) reads and writes. The original reads them through `GameTime.getInstance()`,
//! `SandboxOptions.instance`, `Rand` and the `IsoGameCharacter` the body belongs to; the port passes them
//! explicitly, so a headless player needs no cell, square or inventory unless a path asks for one.
use super::super::stats::Stats;
use super::super::traits::character_traits::CharacterTraits;
use super::body_damage::{BodyDamage, Thermoregulator};
use super::nutrition::Nutrition;
use crate::java::{JavaRandom, Random};
use crate::zombie::characters::moodles::moodle::MoodleCtx;
use crate::zombie::characters::moodles::moodles::Moodles;
use crate::zombie::characters::stats::CharacterStat;
use crate::zombie::core::math::pz_math;
use crate::zombie::core::random::rand_abstract::RandAbstract;
use crate::zombie::game_time::GameTime;
use crate::zombie::iso::weather::climate_manager::ClimateManager;
use crate::zombie::iso::weather::temperature;
use crate::zombie::sandbox_options::{SandboxOptions, Value};
use crate::zombie::scripting::objects::moodle_type::MoodleType;
use crate::zombie::scripting::objects::resource_location::ResourceLocation;
use crate::zombie::zomboid_globals::ZomboidGlobals;

impl JavaRandom for Box<dyn JavaRandom> {
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        (**self).next_int_bound(bound)
    }
    fn next_float(&mut self) -> f32 {
        (**self).next_float()
    }
}

/// What a tick of the body reads from the world: `GameTime.getInstance()`, `SandboxOptions.instance` and the
/// standard random generator `Rand` draws from.
pub struct BodyWorld {
    pub game_time: GameTime,
    pub sandbox: SandboxOptions,
    /// `ZomboidGlobals`, as `Load` read them from `defines.lua`
    pub globals: ZomboidGlobals,
    /// `ClimateManager.getInstance()`
    pub climate: ClimateManager,
    /// `IsoWorld.instance.isHydroPowerOn()`
    pub hydro_power_on: bool,
    pub rand: RandAbstract<Box<dyn JavaRandom>>,
}

impl BodyWorld {
    /// A world whose `Rand` is a plain `java.util.Random` of `seed` (the oracle swaps `RandStandard`'s generator for
    /// one the same way).
    pub fn new(
        game_time: GameTime,
        sandbox: SandboxOptions,
        globals: ZomboidGlobals,
        climate: ClimateManager,
        seed: i64,
    ) -> Self {
        BodyWorld {
            game_time,
            sandbox,
            globals,
            climate,
            hydro_power_on: false,
            rand: RandAbstract::new(Box::new(Random::new(seed))),
        }
    }

    /// `SandboxOptions.instance.<boolean option>.getValue()`; an option that is not a boolean is false.
    pub fn sandbox_bool(&self, name: &str) -> bool {
        matches!(self.sandbox.get(name), Some(Value::Boolean(true)))
    }

    /// `SandboxOptions.instance.<enum or integer option>.getValue()`.
    pub fn sandbox_int(&self, name: &str) -> i32 {
        self.sandbox.get_int(name).unwrap_or(0)
    }

    /// `SandboxOptions.instance.<double option>.getValue()`.
    pub fn sandbox_double(&self, name: &str) -> f64 {
        match self.sandbox.get(name) {
            Some(Value::Double(v)) => *v,
            _ => 0.0,
        }
    }
}

/// The parts of the `IsoGameCharacter` (an `IsoPlayer`) the body simulation reads or changes.
#[derive(Clone, Debug, Default)]
pub struct CharCtx {
    pub is_dead: bool,
    pub god_mode: bool,
    /// `Nutrition.update`'s `firstAction` modifier (`getBodyDamage().getThermoregulator()`-less: set by the caller)
    pub first_action_calories_modifier: Option<f32>,
    pub in_swipe_or_climb_state: bool,
    pub running: bool,
    pub sprinting: bool,
    pub player_moving: bool,
    pub asleep: bool,
    pub traits: CharacterTraits,
    /// `Nutrition.setWeight` fell below 35: the character takes the health damage `setWeight` applies.
    pub low_weight_damage_due: bool,
    /// `IsoPlayer.moodleCantSprint`
    pub moodle_cant_sprint: bool,
    /// `getInventory().getCapacityWeight()` and `getMaxWeight()`
    pub capacity_weight: f32,
    pub max_weight: i32,
    /// `getCorpseSicknessRate()`
    pub corpse_sickness_rate: f32,
    /// `getCurrentBuilding() != null`
    pub in_building: bool,
    /// `getCurrentBuilding() != null && getCurrentBuilding().isToxic()`
    pub in_toxic_building: bool,
    /// `getSquare()`: the square the character stands on, as its facts; `None` without one
    pub current_square: Option<SquareFacts>,
    /// whether the `square` field `getAirTemperatureForCharacter` reads is set (`IsoGameCharacter.update` sets it
    /// from the current square; a character that was only placed has none)
    pub square_field_set: bool,
    /// `getVehicle() != null` and its `getInsideTemperature()`
    pub vehicle_inside_temperature: Option<f32>,
    /// `isSitOnGround() || isSittingOnFurniture()`
    pub sitting: bool,
    /// `isResting`
    pub resting: bool,
    /// `isReading()`
    pub reading: bool,
    /// `!characterActions.isEmpty()`
    pub has_actions: bool,
    /// `isWalking()`
    pub walking: bool,
    /// `isCurrentState(SwipeStatePlayer.instance())`
    pub in_swipe_state: bool,
    /// `getCurrentSquare() == getLastSquare()`
    pub last_square_same: bool,
    /// `idleSquareTime`
    pub idle_square_time: f32,
    /// `getPerkLevel(PerkFactory.Perks.Fitness)`
    pub perk_fitness: i32,
    /// `isUnlimitedEndurance()`
    pub unlimited_endurance: bool,
    /// `GameServer.server && (!sleepAllowed || !sleepNeeded)`: the server resets fatigue every tick
    pub sleep_blocked: bool,
    /// `this == IsoPlayer.getInstance()`
    pub is_local_instance: bool,
    /// `IsoPlayer.allPlayersAsleep()` (every living player in `IsoPlayer.players` is asleep)
    pub all_players_asleep: bool,
    /// `timeOfSleep`, `delayToActuallySleep`, `bedType` ("averageBed")
    pub time_of_sleep: f32,
    pub delay_to_actually_sleep: f32,
    pub bed_type: String,
    /// `IsoPlayer.isAllowSprint()`
    pub allow_sprint: bool,
    /// `isSneaking()`
    pub sneaking: bool,
    /// `IsoPlayer.currentSpeed`
    pub current_speed: f32,
    /// `isAttacking()`
    pub attacking: bool,
    /// `getFitness().getCurrentExe() != null`
    pub fitness_exercise: bool,
    /// `IsoGameCharacter.health` (what `setHealth` writes)
    pub health: f32,
    /// `IsoPlayer.moveSpeed`
    pub move_speed: f32,
    /// `IsoGameCharacter.forceWakeUp`
    pub force_wake_up: bool,
    /// `IsoPlayer.getSleepingPillsTaken()`
    pub sleeping_pills_taken: i32,
    /// `getBetaEffect()` and `getBetaDelta()`
    pub beta_effect: f32,
    pub beta_delta: f32,
    /// `isDraggingCorpse()`
    pub dragging_corpse: bool,
    /// `CorpseCount.instance.getCorpseCount(character)`
    pub corpse_count: i32,
    /// `getHoursSurvived()` (`GameTime.getWorldAgeHours()`)
    pub hours_survived: f64,
    /// `getClothingDiscomfortModifier()`
    pub clothing_discomfort_modifier: f32,
    /// `getMaxWeightBase()` (8) and `IsoPlayer.maxWeightDelta` (1.0)
    pub max_weight_base: i32,
    pub max_weight_delta: f32,
    /// `getPerkLevel(PerkFactory.Perks.Strength)`
    pub perk_strength: i32,
    /// `getTimeSinceLastSmoke()`
    pub time_since_last_smoke: f32,
    /// `getNumSurvivorsInVicinity()`
    pub num_survivors_in_vicinity: i32,
    /// `IsSpeaking()` and `callOut`
    pub speaking: bool,
    pub call_out: bool,
    /// `isInCombat()`
    pub currently_in_combat: bool,
    /// `getReduceInfectionPower()`
    pub reduce_infection_power: f32,
    /// `getPainEffect()` and `getPainDelta()`
    pub pain_effect: f32,
    pub pain_delta: f32,
    /// `IsoPlayer.bleedingLevel`
    pub bleeding_level: i8,
    /// `getFitness().onGoingStiffness()`
    pub ongoing_stiffness: bool,
    /// the character wears at least one item (clothing paths are not ported)
    pub has_worn_clothing: bool,
    /// `getBed()` is a tent
    pub bed_is_tent: bool,
}

/// What the simulation reads of an `IsoGridSquare`: `isInARoom()`, `haveRoof` and `getZ()`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SquareFacts {
    pub in_room: bool,
    pub have_roof: bool,
    pub z: i32,
}

impl CharCtx {
    /// zombie.iso.weather.Temperature.getWindChillAmountForPlayer
    pub fn wind_chill_amount(&self, w: &BodyWorld) -> f32 {
        // `player.getSquare()` is the `square` field, which only `IsoGameCharacter.update` sets
        let in_room = self.square_field_set && self.current_square.is_some_and(|s| s.in_room);
        let blocked = self.vehicle_inside_temperature.is_some() || in_room;
        let air = self.air_temperature_for_character(w, true);
        temperature::get_wind_chill_amount_for_player(blocked, air, w.climate.get_temperature())
    }

    /// zombie.iso.weather.ClimateManager.getAirTemperatureForCharacter: the climate's temperature unless the
    /// character's `square` field is set.
    pub fn air_temperature_for_character(&self, w: &BodyWorld, do_wind_chill: bool) -> f32 {
        match (self.square_field_set, self.current_square) {
            (true, Some(sq)) => w.climate.get_air_temperature_for_square(
                sq.in_room,
                sq.z,
                w.hydro_power_on,
                self.vehicle_inside_temperature,
                do_wind_chill,
            ),
            _ => w.climate.get_temperature(),
        }
    }

    /// `IsoGameCharacter.hasTrait(CharacterTrait)` of the base trait `name`.
    pub fn has_trait(&self, name: &str) -> bool {
        ResourceLocation::of(name).is_ok_and(|id| self.traits.get(&id))
    }

    /// `getCharacterTraits().add(CharacterTrait)` of the base trait `name`.
    pub fn add_trait(&mut self, name: &str) {
        if let Ok(id) = ResourceLocation::of(name) {
            self.traits.add(id);
        }
    }

    /// `getCharacterTraits().remove(CharacterTrait)` of the base trait `name`.
    pub fn remove_trait(&mut self, name: &str) {
        if let Ok(id) = ResourceLocation::of(name) {
            self.traits.remove(id);
        }
    }
}

/// The state of one player's body between ticks: what the original keeps in the `IsoPlayer`'s `Stats`,
/// `Nutrition` and character flags, and the world it ticks in.
pub struct PlayerBody {
    pub stats: Stats,
    pub body: BodyDamage,
    pub moodles: Moodles,
    pub nutrition: Nutrition,
    pub ch: CharCtx,
    pub world: BodyWorld,
}

impl PlayerBody {
    /// A player with the constructors' defaults (`new Stats()`, `new Nutrition(player)`).
    pub fn new(world: BodyWorld) -> Self {
        PlayerBody {
            stats: Stats::default(),
            body: BodyDamage::new(true),
            moodles: Moodles::new(),
            nutrition: Nutrition::new(),
            ch: CharCtx::default(),
            world,
        }
    }

    /// `IsoPlayer.update`'s `nutrition.update()`, with the thermoregulator's energy multiplier.
    pub fn update_nutrition(&mut self) {
        let energy = self
            .body
            .thermoregulator
            .as_ref()
            .map(|t| t.energy_multiplier);
        self.nutrition.update(&mut self.ch, &self.world, energy);
    }

    /// `IsoGameCharacter.update`'s `moodles.Update()`.
    pub fn update_moodles(&mut self) -> Result<(), String> {
        let mut c = MoodleCtx {
            stats: &self.stats,
            body: &self.body,
            ch: &mut self.ch,
            world: &self.world,
        };
        self.moodles.update(&mut c)
    }

    fn mult(&self) -> f64 {
        f64::from(self.world.game_time.get_multiplier())
    }

    fn dmpd(&self) -> f64 {
        f64::from(self.world.game_time.get_delta_minutes_per_day())
    }

    /// zombie.SandboxOptions.getStatsDecreaseMultiplier
    fn stats_decrease_multiplier(&self) -> f64 {
        match self.world.sandbox_int("StatsDecrease") {
            1 => 2.0,
            2 => 1.6,
            4 => 0.8,
            5 => 0.65,
            _ => 1.0,
        }
    }

    fn thermo_multiplier(&self, pick: fn(&Thermoregulator) -> f64) -> f64 {
        self.body.thermoregulator.as_ref().map_or(1.0, pick)
    }

    /// zombie.characters.IsoGameCharacter.calculateStats (the Lua hook "CalculateStats" is taken as unhooked).
    pub fn calculate_stats(&mut self) -> Result<(), String> {
        if self.ch.sleep_blocked {
            self.stats.reset(CharacterStat::Fatigue);
        }
        self.update_endurance();
        // updateTripping
        if self.stats.tripping {
            self.stats.tripping_rot_angle += 0.06;
        }
        self.update_thirst();
        self.update_stress()?;
        self.update_stats_wake_state();
        self.update_morale();
        self.update_fitness();
        Ok(())
    }

    /// IsoGameCharacter.updateEndurance
    fn update_endurance(&mut self) {
        self.stats.last_endurance = self.stats.get(CharacterStat::Endurance);
        if self.ch.unlimited_endurance {
            self.stats.reset(CharacterStat::Endurance);
        }
    }

    /// IsoGameCharacter.updateThirst (`autoDrink` finds no water source in the empty inventory of a headless
    /// character)
    fn update_thirst(&mut self) {
        let mut trait_mod = 1.0f32;
        if self.ch.has_trait("HighThirst") {
            trait_mod *= 2.0;
        }
        if self.ch.has_trait("LowThirst") {
            trait_mod *= 0.5;
        }
        let g = &self.world.globals;
        let delta = if self.ch.asleep {
            g.field("thirstSleepingIncrease")
                * self.stats_decrease_multiplier()
                * self.mult()
                * self.dmpd()
                * f64::from(trait_mod)
        } else {
            let running = if self.ch.is_local_instance && self.ch.running {
                1.2
            } else {
                1.0
            };
            g.field("thirstIncrease")
                * self.stats_decrease_multiplier()
                * self.mult()
                * running
                * self.dmpd()
                * f64::from(trait_mod)
                * self.thermo_multiplier(|t| t.fluids_multiplier)
        };
        self.stats.add(CharacterStat::Thirst, delta as f32);
    }

    /// IsoGameCharacter.updateStress (the world's sounds are none: `getStressFromSounds` is 0)
    fn update_stress(&mut self) -> Result<(), String> {
        let g = &self.world.globals;
        let bite = (g.field("stressFromBiteOrScratch") * self.mult() * self.dmpd()) as f32;
        if !self.ch.has_trait("Deaf") {
            let sounds = 0.0f32;
            self.stats.add(
                CharacterStat::Stress,
                (f64::from(sounds) * g.field("stressFromSoundsMultiplier")) as f32,
            );
        }
        if self.body.get_num_parts_bitten() > 0 {
            self.stats.add(CharacterStat::Stress, bite);
        }
        if self.body.get_num_parts_scratched() > 0 {
            self.stats.add(CharacterStat::Stress, bite);
        }
        if self.body.is_infected || self.body.is_fake_infected {
            self.stats.add(CharacterStat::Stress, bite);
        }
        if self.ch.has_trait("Hemophobic") {
            return Err("not ported: IsoGameCharacter.getTotalBlood (Hemophobic stress)".into());
        }
        let anger = (g.field("angerDecrease") * self.mult() * self.dmpd()) as f32;
        self.stats.remove(CharacterStat::Anger, anger);
        Ok(())
    }

    /// IsoGameCharacter.updateStats_WakeState
    fn update_stats_wake_state(&mut self) {
        if self.ch.asleep {
            self.update_stats_sleeping();
        } else {
            self.update_stats_awake();
        }
    }

    /// zombie.SandboxOptions.getEnduranceRegenMultiplier
    fn endurance_regen_multiplier(&self) -> f64 {
        match self.world.sandbox_int("EndRegen") {
            1 => 1.8,
            2 => 1.3,
            4 => 0.7,
            5 => 0.4,
            _ => 1.0,
        }
    }

    /// zombie.characters.IsoGameCharacter.getRecoveryMod
    fn get_recovery_mod(&self) -> f32 {
        let mut modv = match self.ch.perk_fitness {
            0 => 0.7,
            1 => 0.8,
            2 => 0.9,
            3 => 1.0,
            4 => 1.1,
            5 => 1.2,
            6 => 1.3,
            7 => 1.4,
            8 => 1.5,
            9 => 1.55,
            10 => 1.6,
            _ => 0.0,
        };
        if self.ch.has_trait("Obese") {
            modv *= 0.4;
        }
        if self.ch.has_trait("Overweight") {
            modv *= 0.7;
        }
        if self.ch.has_trait("Very Underweight") {
            modv *= 0.7;
        }
        if self.ch.has_trait("Emaciated") {
            modv *= 0.3;
        }
        let lipids = self.nutrition.lipids;
        if lipids < -1500.0 {
            modv *= 0.2;
        } else if lipids < -1000.0 {
            modv *= 0.5;
        }
        let proteins = self.nutrition.proteins;
        if proteins < -1500.0 {
            modv *= 0.2;
        } else if proteins < -1000.0 {
            modv *= 0.5;
        }
        modv
    }

    /// zombie.characters.IsoPlayer.updateStats_Sleeping
    fn update_stats_sleeping(&mut self) {
        let mut end_mult = 2.0f32;
        if self.ch.all_players_asleep {
            end_mult *= self.world.game_time.get_delta_minutes_per_day();
        }
        let g = self.world.globals.clone();
        let end = g.field("imobileEnduranceReduce")
            * self.endurance_regen_multiplier()
            * f64::from(self.get_recovery_mod())
            * f64::from(self.world.game_time.get_multiplier())
            * f64::from(end_mult);
        self.stats.add(CharacterStat::Endurance, end as f32);
        if self.stats.is_above_minimum(CharacterStat::Fatigue) {
            let mut mul = 1.0f32;
            if self.ch.has_trait("Insomniac") {
                mul *= 0.5;
            }
            if self.ch.has_trait("NightOwl") {
                mul *= 1.4;
            }
            let mut bed_quality_multiplier = 1.0f32;
            let bed = self.ch.bed_type.as_str();
            if bed == "averageBedPillow" {
                bed_quality_multiplier = 1.05;
            }
            if bed == "goodBed" {
                bed_quality_multiplier = 1.1;
            } else if bed == "goodBedPillow" {
                bed_quality_multiplier = 1.15;
            } else if bed == "badBed" {
                bed_quality_multiplier = 0.9;
            } else if bed == "badBedPillow" {
                bed_quality_multiplier = 0.95;
            } else if bed == "floor" {
                bed_quality_multiplier = 0.6;
            } else if bed == "floorPillow" {
                bed_quality_multiplier = 0.75;
            }
            let elapsed_hours = 1.0 / self.world.game_time.minutes_per_day / 60.0
                * self.world.game_time.get_multiplier()
                / 2.0;
            self.ch.time_of_sleep += elapsed_hours;
            if self.ch.time_of_sleep > self.ch.delay_to_actually_sleep {
                let mut trait_mul = 1.0f32;
                if self.ch.has_trait("NeedsLessSleep") {
                    trait_mul *= 0.75;
                } else if self.ch.has_trait("NeedsMoreSleep") {
                    trait_mul *= 1.18;
                }
                if self.stats.get(CharacterStat::Fatigue) <= 0.3 {
                    let hours_needed = 7.0 * trait_mul;
                    self.stats.remove(
                        CharacterStat::Fatigue,
                        elapsed_hours / hours_needed * 0.3 * mul * bed_quality_multiplier,
                    );
                } else {
                    let hours_needed = 5.0 * trait_mul;
                    self.stats.remove(
                        CharacterStat::Fatigue,
                        elapsed_hours / hours_needed * 0.7 * mul * bed_quality_multiplier,
                    );
                }
            }
        }
        let sdm = self.stats_decrease_multiplier();
        let mult = self.mult();
        let dmpd = self.dmpd();
        let hm = 1.0f64;
        let food_eaten = self.moodles.get_moodle_level(MoodleType::FoodEaten);
        let hunger = if food_eaten == 0 {
            let hunger_mult = f64::from(self.get_appetite_multiplier());
            g.field("hungerIncreaseWhileAsleep") * sdm * hunger_mult * mult * dmpd * hm
        } else {
            g.field("hungerIncreaseWhenWellFed")
                * sdm
                * g.field("hungerIncreaseWhileAsleep")
                * mult
                * hm
                * dmpd
        };
        self.stats.add(CharacterStat::Hunger, hunger as f32);
    }

    /// IsoGameCharacter.getAppetiteMultiplier
    fn get_appetite_multiplier(&self) -> f32 {
        let mut hunger_mult = 1.0 - self.stats.get(CharacterStat::Hunger);
        if self.ch.has_trait("HeartyAppetite") {
            hunger_mult *= 1.5;
        }
        if self.ch.has_trait("LightEater") {
            hunger_mult *= 0.75;
        }
        hunger_mult
    }

    /// IsoGameCharacter.updateStats_Awake
    fn update_stats_awake(&mut self) {
        let mult = self.mult();
        let dmpd = self.dmpd();
        let sdm = self.stats_decrease_multiplier();
        let g = self.world.globals.clone();
        self.stats.remove(
            CharacterStat::Stress,
            (g.field("stressReduction") * mult * dmpd) as f32,
        );
        let mut fatiguemod = 1.0 - self.stats.get(CharacterStat::Endurance);
        if fatiguemod < 0.3 {
            fatiguemod = 0.3;
        }
        let mut mul = 1.0f32;
        if self.ch.has_trait("NeedsLessSleep") {
            mul = 0.7;
        }
        if self.ch.has_trait("NeedsMoreSleep") {
            mul = 1.3;
        }
        let mut modv = 1.0f32;
        if self.ch.sitting || self.ch.resting {
            modv = 1.5;
        }
        let fatigue = g.field("fatigueIncrease")
            * sdm
            * f64::from(fatiguemod)
            * mult
            * dmpd
            * f64::from(mul)
            * self.thermo_multiplier(|t| t.fatigue_multiplier)
            / f64::from(modv);
        self.stats.add(CharacterStat::Fatigue, fatigue as f32);
        let hunger_mult = f64::from(self.get_appetite_multiplier());
        let food_eaten = self.moodles.get_moodle_level(MoodleType::FoodEaten);
        // IsoGameCharacter.getHungerMultiplier
        let hm = 1.0f64;
        let exercising = self.ch.running && self.ch.player_moving;
        let hunger = if !exercising && !self.ch.in_swipe_state {
            if food_eaten == 0 {
                g.field("hungerIncrease") * sdm * hunger_mult * mult * dmpd * hm
            } else {
                g.field("hungerIncreaseWhenWellFed") * sdm * mult * dmpd * hm
            }
        } else if food_eaten == 0 {
            g.field("hungerIncreaseWhenExercise") / 3.0 * sdm * hunger_mult * mult * dmpd * hm
        } else {
            g.field("hungerIncreaseWhenExercise") * sdm * hunger_mult * mult * dmpd * hm
        };
        self.stats.add(CharacterStat::Hunger, hunger as f32);
        // updateIdleSquareTime
        if self.ch.last_square_same {
            if self.ch.idle_square_time <= 3600.0 {
                self.ch.idle_square_time += 1.0
                    * self.world.game_time.get_multiplier()
                    * self.world.game_time.get_delta_minutes_per_day();
            }
        } else {
            self.ch.idle_square_time = 0.0;
        }
        let in_combat =
            self.stats.last_very_close_zombies > 0 || self.stats.last_num_chasing_zombies >= 3;
        if in_combat {
            self.stats.reset(CharacterStat::Idleness);
        } else if self.is_currently_idle() && self.ch.current_square.is_some() {
            if self.ch.last_square_same && self.ch.idle_square_time >= 1800.0 {
                let v = f64::from(self.stats.get(CharacterStat::Idleness))
                    + g.field("idleIncreaseRate") * mult * dmpd;
                self.stats.set(CharacterStat::Idleness, v as f32);
            }
            if self.ch.current_square.is_some_and(|s| s.in_room) {
                let v = f64::from(self.stats.get(CharacterStat::Idleness))
                    + g.field("idleIncreaseRate") / 3.0 * mult * dmpd;
                self.stats.set(CharacterStat::Idleness, v as f32);
            }
        } else if !self.ch.sitting {
            let v = f64::from(self.stats.get(CharacterStat::Idleness))
                - g.field("idleDecreaseRate") * mult * dmpd;
            self.stats.set(CharacterStat::Idleness, v as f32);
        }
    }

    /// IsoGameCharacter.isCurrentlyIdle (a player on a server whose network states are unset)
    pub(super) fn is_currently_idle(&self) -> bool {
        let moving_on_foot =
            self.ch.player_moving && (self.ch.walking || self.ch.running || self.ch.sprinting);
        let tired_sitting =
            self.ch.sitting && self.moodles.get_moodle_level(MoodleType::Endurance) >= 1;
        let panicking = self.moodles.get_moodle_level(MoodleType::Panic) > 1;
        let in_combat =
            self.stats.last_very_close_zombies > 0 || self.stats.last_num_chasing_zombies >= 3;
        !(moving_on_foot
            || self.ch.asleep
            || tired_sitting
            || self.ch.reading
            || self.ch.has_actions
            || panicking
            || in_combat)
    }

    /// IsoGameCharacter.updateMorale
    fn update_morale(&mut self) {
        let mut modv = 1.0 - self.stats.get_nicotine_stress() - 0.5;
        modv *= 1.0E-4;
        if modv > 0.0 {
            modv += 0.5;
        }
        self.stats
            .add(CharacterStat::Morale, pz_math::clamp(modv, 0.0, 1.0));
    }

    /// IsoGameCharacter.updateFitness
    fn update_fitness(&mut self) {
        self.stats.set(
            CharacterStat::Fitness,
            self.ch.perk_fitness as f32 / 5.0 - 1.0,
        );
    }
}
