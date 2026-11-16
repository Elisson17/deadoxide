//! zombie.characters.IsoZombie: construction (descriptor, look, stats, speed) of a zombie, in the original's
//! operation order and with its random draws.
//!
//! What is here is measured against the original run headless (`pz-dump zombie` vs `ZombieProbe`); the rest of the
//! 5,976-line class (states, targeting, hearing, attacks, outfits ...) is not ported yet.
use super::iso_game_character::IsoGameCharacterBase;
use super::survivor_factory::SurvivorFactory;
use crate::java::Random;
use crate::zombie::core::random::rand_abstract::RandAbstract;
use crate::zombie::core::skinnedmodel::population::outfit_rng::OutfitRng;
use crate::zombie::iso::iso_directions::IsoDirections;
use crate::zombie::sandbox_options::SandboxOptions;

/// The `SandboxOptions.instance.lore` values the zombie code reads, as the integers `getValue()` returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lore {
    pub speed: i32,
    pub sprinter_percentage: i32,
    pub strength: i32,
    pub cognition: i32,
    pub door_opening_percentage: i32,
    pub crawl_under_vehicle: i32,
    pub memory: i32,
    pub sight: i32,
    pub hearing: i32,
    pub active_only: i32,
}

impl Lore {
    /// The values `SandboxOptions.instance.lore.<option>.getValue()` returns, by option name ("ZombieLore.Speed").
    pub fn from_sandbox(sandbox: &SandboxOptions) -> Result<Self, String> {
        let get = |name: &str| {
            sandbox
                .get_int(&format!("ZombieLore.{name}"))
                .ok_or_else(|| format!("sandbox option ZombieLore.{name} is not an integer option"))
        };
        Ok(Lore {
            speed: get("Speed")?,
            sprinter_percentage: get("SprinterPercentage")?,
            strength: get("Strength")?,
            cognition: get("Cognition")?,
            door_opening_percentage: get("DoorOpeningPercentage")?,
            crawl_under_vehicle: get("CrawlUnderVehicle")?,
            memory: get("Memory")?,
            sight: get("Sight")?,
            hearing: get("Hearing")?,
            active_only: get("ActiveOnly")?,
        })
    }
}

/// What the original's singletons give the zombie code, made explicit: the global `Rand`, the thread's `OutfitRNG`,
/// `SandboxOptions.instance.lore`, `SurvivorFactory`'s lists and `SurvivorDesc.idCount`.
pub struct ZombieWorld {
    pub rand: RandAbstract<Random>,
    pub outfit_rng: OutfitRng,
    pub lore: Lore,
    pub survivors: SurvivorFactory,
    pub survivor_id_count: i32,
}

/// zombie.characters.IsoZombie, the fields construction sets or reads.
#[derive(Clone, Debug, PartialEq)]
pub struct IsoZombie {
    pub character: IsoGameCharacterBase,
    /// `palette` (final)
    pub palette: i32,
    pub sprite_name: String,
    /// `sprite.def.tintr/tintg/tintb`
    pub tint: [f32; 3],
    pub zombie_id: i32,
    pub hurt_sound: String,
    pub speed_type: i32,
    pub strength: i32,
    pub cognition: i32,
    pub memory: i32,
    pub sight: i32,
    pub hearing: i32,
    pub walk_type: Option<String>,
    pub walk_variant: String,
    /// `speedMod` (IsoGameCharacter, initialiser 1.0): set by the speed routines
    pub speed_mod: f32,
    pub lunger: bool,
    pub running: bool,
    pub crawling: bool,
    pub inactive: bool,
    pub can_crawl_under_vehicle: bool,
}

impl IsoZombie {
    /// zombie.characters.IsoZombie(IsoCell cell): `this(cell, null, -1)`, so the descriptor is made by
    /// `SurvivorFactory.CreateSurvivor()` and the palette is `Rand.Next(3) + 1`. `Err` where the original would throw
    /// or where a path is not ported (an `ActiveOnly` other than "both", which needs the climate's dawn and dusk).
    ///
    /// Not modelled: the cell and world registration, the grapple/died listeners and variable callbacks, the sprite
    /// instance's animation fields (`def.animFrameIncrease`, `setFrameSpeedPerFrame`: no draws), `setDefaultState`,
    /// `initializeStates`, the action group, worn and attached items, the aggro list and the FMOD parameters.
    pub fn new(world: &mut ZombieWorld) -> Result<Self, String> {
        // super(cell, 0.0F, 0.0F, 0.0F): IsoGameCharacter(...) draws the patience
        let mut character = IsoGameCharacterBase::new(&mut world.rand, 0.0, 0.0, 0.0);
        character.health = 1.8 + world.rand.next_float(0.0, 0.3);
        character.moving.weight = 0.7;
        let direction = IsoDirections::get_random(&mut world.rand);
        character.set_forward_iso_direction(direction)?;
        character.human_visual.random_blood(&mut world.outfit_rng);
        let descriptor = world
            .survivors
            .create_survivor(&mut world.rand, &mut world.survivor_id_count)?;
        let palette = world.rand.next(3) + 1;
        character.descriptor = Some(descriptor);
        let female = character.descriptor.as_ref().is_some_and(|d| d.female);
        character.set_female(female);
        if let Some(d) = character.descriptor.as_mut() {
            if !d.voice_prefix.contains("Zombie") {
                d.voice_prefix = if d.voice_prefix == "VoiceFemale" {
                    "FemaleZombie"
                } else {
                    "MaleZombie"
                }
                .to_string();
            }
        }
        let mut sprite_name = if character.female { "KateZ" } else { "BobZ" }.to_string();
        if palette != 1 {
            sprite_name.push_str(&palette.to_string());
        }
        let mut z = IsoZombie {
            character,
            palette,
            sprite_name,
            tint: [0.0; 3],
            zombie_id: 0,
            hurt_sound: String::new(),
            speed_type: -1,
            strength: -1,
            cognition: -1,
            memory: -1,
            sight: -1,
            hearing: -1,
            walk_type: None,
            walk_variant: "ZombieWalk".to_string(),
            speed_mod: 1.0,
            lunger: false,
            running: false,
            crawling: false,
            inactive: false,
            can_crawl_under_vehicle: true,
        };
        z.init_sprite_parts_zombie(world);
        for t in &mut z.tint {
            *t = 0.95 + world.rand.next(5) as f32 / 100.0;
        }
        // setDefaultState, setFakeDead(false): no draws
        z.inactive = zombie_inactivity_phase(&world.lore)?;
        z.do_zombie_stats(world)?;
        z.character.moving.width = 0.3;
        let voice = z
            .character
            .descriptor
            .as_ref()
            .map_or("", |d| d.voice_prefix.as_str());
        z.hurt_sound = format!("{voice}Hurt");
        Ok(z)
    }

    /// zombie.characters.IsoZombie.InitSpritePartsZombie: the legs sprite takes the descriptor's torso name (not
    /// modelled), `zombieId = Rand.Next(10000)`.
    fn init_sprite_parts_zombie(&mut self, world: &mut ZombieWorld) {
        self.zombie_id = world.rand.next(10000);
    }

    /// zombie.characters.IsoZombie.DoZombieStats
    pub fn do_zombie_stats(&mut self, world: &mut ZombieWorld) -> Result<(), String> {
        let lore = world.lore;
        let rand = &mut world.rand;
        if lore.cognition == 1 {
            self.cognition = 1;
        }
        if lore.cognition == 4 {
            if rand.next(100) < lore.door_opening_percentage {
                self.cognition = 1;
            } else {
                self.cognition = 0;
            }
        }
        if self.strength == -1 && lore.strength == 1 {
            self.strength = 5;
        }
        if self.strength == -1 && lore.strength == 2 {
            self.strength = 3;
        }
        if self.strength == -1 && lore.strength == 3 {
            self.strength = 1;
        }
        if self.strength == -1 && lore.strength == 4 {
            self.strength = rand.next_range(1, 5);
        }
        let memory_setting = lore.memory;
        let mut random_memory = -1;
        if memory_setting == 5 {
            random_memory = rand.next(4);
        } else if memory_setting == 6 {
            random_memory = rand.next(3) + 1;
        }
        if self.memory == -1 && memory_setting == 1 || random_memory == 0 {
            self.memory = 1250;
        }
        if self.memory == -1 && memory_setting == 2 || random_memory == 1 {
            self.memory = 800;
        }
        if self.memory == -1 && memory_setting == 3 || random_memory == 2 {
            self.memory = 500;
        }
        if self.memory == -1 && memory_setting == 4 || random_memory == 3 {
            self.memory = 25;
        }
        if lore.sight == 4 {
            self.sight = rand.next(3) + 1;
        } else if lore.sight == 5 {
            self.sight = rand.next(2) + 2;
        } else {
            self.sight = lore.sight;
        }
        if lore.hearing == 4 {
            self.hearing = rand.next(3) + 1;
        } else if lore.hearing == 5 {
            self.hearing = rand.next(2) + 2;
        } else {
            self.hearing = lore.hearing;
        }
        self.do_zombie_speed(world);
        self.init_can_crawl_under_vehicle(world);
        Ok(())
    }

    /// zombie.characters.IsoZombie.doZombieSpeed()
    pub fn do_zombie_speed(&mut self, world: &mut ZombieWorld) {
        self.do_zombie_speed_internal(world, self.speed_type);
    }

    /// zombie.characters.IsoZombie.doZombieSpeedInternal
    fn do_zombie_speed_internal(&mut self, world: &mut ZombieWorld, zombie_speed: i32) {
        let zombie_speed = self.determine_zombie_speed(world, zombie_speed);
        let lore_speed = world.lore.speed;
        if self.crawling {
            self.do_crawler_speed(world, zombie_speed);
        } else if lore_speed == 3 || zombie_speed == 3 {
            self.do_shambler(world);
        } else if world.rand.next(3) != 0 {
            self.do_fake_shambler(world, zombie_speed);
        } else if lore_speed != 2 && zombie_speed != 2 {
            if lore_speed == 1 || zombie_speed == 1 {
                self.do_sprinter(world);
            }
        } else {
            self.do_fast_shambler(world);
        }
    }

    /// zombie.characters.IsoZombie.determineZombieSpeed
    fn determine_zombie_speed(&self, world: &mut ZombieWorld, zombie_speed: i32) -> i32 {
        if zombie_speed != -1 {
            return zombie_speed;
        }
        let mut zombie_speed = world.lore.speed;
        if self.inactive {
            zombie_speed = 3;
        } else if zombie_speed == 4 {
            if world.rand.next(100) < world.lore.sprinter_percentage {
                zombie_speed = 1;
            } else {
                zombie_speed = world.rand.next(2) + 2;
            }
        }
        zombie_speed
    }

    /// `this.speedMod = base; this.speedMod = this.speedMod + Rand.Next(1500) / 10000.0F;`
    fn set_speed_mod(&mut self, world: &mut ZombieWorld, base: f32) {
        self.speed_mod = base;
        self.speed_mod += world.rand.next(1500) as f32 / 10000.0;
    }

    /// zombie.characters.IsoZombie.doCrawlerSpeed (`def.animFrameIncrease *= 0.8F` not modelled)
    fn do_crawler_speed(&mut self, world: &mut ZombieWorld, zombie_speed: i32) {
        self.set_speed_mod(world, 0.3);
        self.do_zombie_speed_internal2(world, zombie_speed);
    }

    /// zombie.characters.IsoZombie.doSprinter (the sprite instance's frame speed is not modelled)
    fn do_sprinter(&mut self, world: &mut ZombieWorld) {
        self.lunger = true;
        self.walk_variant = "ZombieWalk2".to_string();
        self.set_speed_mod(world, 0.85);
        self.speed_type = 1;
        self.do_zombie_speed_internal2(world, self.speed_type);
    }

    /// zombie.characters.IsoZombie.doFastShambler
    fn do_fast_shambler(&mut self, world: &mut ZombieWorld) {
        self.lunger = true;
        self.walk_variant = "ZombieWalk2".to_string();
        self.set_speed_mod(world, 0.85);
        self.speed_type = 2;
        self.do_zombie_speed_internal2(world, self.speed_type);
    }

    /// zombie.characters.IsoZombie.doFakeShambler(int)
    fn do_fake_shambler(&mut self, world: &mut ZombieWorld, zombie_speed: i32) {
        self.set_speed_mod(world, 0.55);
        self.walk_variant = "ZombieWalk1".to_string();
        self.do_zombie_speed_internal2(world, zombie_speed);
    }

    /// zombie.characters.IsoZombie.doShambler
    fn do_shambler(&mut self, world: &mut ZombieWorld) {
        self.set_speed_mod(world, 0.55);
        self.walk_variant = "ZombieWalk1".to_string();
        self.speed_type = 3;
        self.do_zombie_speed_internal2(world, self.speed_type);
    }

    /// zombie.characters.IsoZombie.doZombieSpeedInternal2(int): the walk type, `slow1..3`, `1..5` or `sprint1..5`.
    fn do_zombie_speed_internal2(&mut self, world: &mut ZombieWorld, zombie_speed: i32) {
        let mut walk_type = if zombie_speed == 3 {
            format!("slow{}", world.rand.next(3) + 1)
        } else {
            (world.rand.next(5) + 1).to_string()
        };
        if zombie_speed == 1 {
            self.character.turn_delta_normal = 1.0;
            walk_type = format!("sprint{walk_type}");
        }
        self.walk_type = Some(walk_type);
        self.speed_type = zombie_speed;
    }

    /// zombie.characters.IsoZombie.initCanCrawlUnderVehicle: `Rand.Next(100) < chance` for the option's chance.
    fn init_can_crawl_under_vehicle(&mut self, world: &mut ZombieWorld) {
        let chance = match world.lore.crawl_under_vehicle {
            1 => 0,
            2 => 5,
            3 => 10,
            4 => 25,
            5 => 50,
            6 => 75,
            _ => 100,
        };
        self.can_crawl_under_vehicle = world.rand.next(100) < chance;
    }
}

/// zombie.GameTime.isZombieInactivityPhase: `!isZombieActivityPhase()`. With `ActiveOnly` "both" (1) the zombies are
/// always active; the day and night options need the climate's dawn and dusk, which this port does not feed in yet.
fn zombie_inactivity_phase(lore: &Lore) -> Result<bool, String> {
    if lore.active_only == 1 {
        Ok(false)
    } else {
        Err("GameTime.isZombieInactivityPhase with ActiveOnly other than 1 needs the season's dawn and dusk (not ported here)".into())
    }
}
