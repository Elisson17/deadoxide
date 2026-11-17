//! The world and character state the player body simulation (BodyDamage, Nutrition, the stat updates of
//! IsoGameCharacter) reads and writes. The original reads them through `GameTime.getInstance()`,
//! `SandboxOptions.instance`, `Rand` and the `IsoGameCharacter` the body belongs to; the port passes them
//! explicitly, so a headless player needs no cell, square or inventory unless a path asks for one.
use super::super::stats::Stats;
use super::super::traits::character_traits::CharacterTraits;
use super::nutrition::Nutrition;
use crate::java::{JavaRandom, Random};
use crate::zombie::core::random::rand_abstract::RandAbstract;
use crate::zombie::game_time::GameTime;
use crate::zombie::sandbox_options::{SandboxOptions, Value};
use crate::zombie::scripting::objects::resource_location::ResourceLocation;

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
    pub rand: RandAbstract<Box<dyn JavaRandom>>,
}

impl BodyWorld {
    /// A world whose `Rand` is a plain `java.util.Random` of `seed` (the oracle swaps `RandStandard`'s generator for
    /// one the same way).
    pub fn new(game_time: GameTime, sandbox: SandboxOptions, seed: i64) -> Self {
        BodyWorld {
            game_time,
            sandbox,
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
}

impl CharCtx {
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
    pub nutrition: Nutrition,
    pub ch: CharCtx,
    pub world: BodyWorld,
}

impl PlayerBody {
    /// A player with the constructors' defaults (`new Stats()`, `new Nutrition(player)`).
    pub fn new(world: BodyWorld) -> Self {
        PlayerBody {
            stats: Stats::default(),
            nutrition: Nutrition::new(),
            ch: CharCtx::default(),
            world,
        }
    }

    /// `IsoPlayer.update`'s `nutrition.update()`.
    pub fn update_nutrition(&mut self) {
        self.nutrition.update(&mut self.ch, &self.world, None);
    }
}
