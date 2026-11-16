//! zombie.characters.IsoGameCharacter: what a save stores for any character (descriptor, visual, inventory, stats,
//! body, XP, books, recipes, cheats), loaded in the original's order.
use std::collections::{BTreeMap, BTreeSet};

use super::body_damage::body_damage::BodyDamage;
use super::body_damage::body_part::BodySandbox;
use super::player_cheats::PlayerCheats;
use super::stats::Stats;
use super::survivor_desc::SurvivorDesc;
use super::traits::character_traits::CharacterTraits;
use crate::java::JavaRandom;
use crate::java_nio::{read_string, ByteBuffer};
use crate::zombie::core::math::pz_math;
use crate::zombie::core::random::rand_abstract::RandAbstract;
use crate::zombie::core::skinnedmodel::visual::human_visual::HumanVisual;
use crate::zombie::inventory::inventory_item::{not_ported, InventoryItem, ItemFactory};
use crate::zombie::inventory::item_container::ItemContainer;
use crate::zombie::iso::iso_directions::IsoDirections;
use crate::zombie::iso::iso_moving_object::IsoMovingObject;
use crate::zombie::iso::vector2::Vector2;

/// zombie.characters.skills.PerkFactory.Perks: the ids `new Perk(id)` registers (`PerkById`), MAX last.
pub const PERK_IDS: [&str; 46] = [
    "None",
    "Agility",
    "Cooking",
    "Melee",
    "Crafting",
    "Fitness",
    "Strength",
    "Blunt",
    "Axe",
    "Lightfoot",
    "Nimble",
    "Sprinting",
    "Sneak",
    "Woodwork",
    "Aiming",
    "Reloading",
    "Farming",
    "Survivalist",
    "Fishing",
    "Trapping",
    "Passiv",
    "Firearm",
    "PlantScavenging",
    "Doctor",
    "Electricity",
    "Blacksmith",
    "MetalWelding",
    "Melting",
    "Mechanics",
    "Spear",
    "Maintenance",
    "SmallBlade",
    "LongBlade",
    "SmallBlunt",
    "Combat",
    "Tailoring",
    "Tracking",
    "Husbandry",
    "FlintKnapping",
    "Masonry",
    "Pottery",
    "Carving",
    "Butchering",
    "Glassmaking",
    "FarmingCategory",
    "PhysicalCategory",
];

/// zombie.characters.IsoGameCharacter.XP.loadPerk: `Perks.FromString` (unknown ids and MAX are dropped).
fn load_perk(b: &mut ByteBuffer) -> Result<Option<String>, String> {
    let name = read_string(b)?;
    Ok(PERK_IDS.contains(&name.as_str()).then_some(name))
}

/// zombie.characters.IsoGameCharacter.LevelUpLevels
const LEVEL_UP_LEVELS: [i32; 32] = [
    25, 75, 150, 225, 300, 400, 500, 600, 700, 800, 900, 1000, 1200, 1400, 1600, 1800, 2000, 2200,
    2400, 2600, 2800, 3000, 3200, 3400, 3600, 3800, 4000, 4400, 4800, 5200, 5600, 6000,
];

/// zombie.characters.IsoGameCharacter.getXpForLevel with the field initialiser `levelUpMultiplier` 2.5.
pub fn get_xp_for_level(level: i32) -> Result<i32, String> {
    let m = 2.5f32;
    let n = LEVEL_UP_LEVELS.len() as i32;
    if level < n {
        let v = usize::try_from(level)
            .ok()
            .and_then(|i| LEVEL_UP_LEVELS.get(i))
            .ok_or_else(|| {
                format!("java.lang.ArrayIndexOutOfBoundsException: Index {level} out of bounds")
            })?;
        Ok((*v as f32 * m) as i32)
    } else {
        Ok(((LEVEL_UP_LEVELS[31] + (level - n + 1) * 400) as f32 * m) as i32)
    }
}

/// zombie.characters.IsoGameCharacter.XPMultiplier
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XpMultiplier {
    pub multiplier: f32,
    pub min_level: i32,
    pub max_level: i32,
}

/// zombie.characters.IsoGameCharacter.XP, the saved fields, with the perk list it fills.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Xp {
    pub total_xp: f32,
    pub level: i32,
    pub lastlevel: i32,
    /// `xpMap` (a HashMap in the original; ordered by perk id here).
    pub xp_map: BTreeMap<String, f32>,
    /// `IsoGameCharacter.perkList` in save order: (perk, level).
    pub perk_list: Vec<(String, i32)>,
    pub xp_map_multiplier: BTreeMap<String, XpMultiplier>,
}

impl Xp {
    /// zombie.characters.IsoGameCharacter.XP.load after `characterTraits.load`. The `LogLevelPerk` events it fires for
    /// changed levels are not modelled.
    pub fn load(b: &mut ByteBuffer) -> Result<Self, String> {
        let mut x = Xp {
            total_xp: b.get_float()?,
            level: b.get_int()?,
            lastlevel: b.get_int()?,
            ..Xp::default()
        };
        for _ in 0..b.get_int()? {
            let perk = load_perk(b)?;
            let xp = b.get_float()?;
            if let Some(p) = perk {
                x.xp_map.insert(p, xp);
            }
        }
        for _ in 0..b.get_int()? {
            let perk = load_perk(b)?;
            let level = b.get_int()?;
            if let Some(p) = perk {
                x.perk_list.push((p, level));
            }
        }
        for _ in 0..b.get_int()? {
            let perk = load_perk(b)?;
            let multiplier = b.get_float()?;
            let min_level = i32::from(b.get()?);
            let max_level = i32::from(b.get()?);
            if let Some(p) = perk {
                x.xp_map_multiplier.insert(
                    p,
                    XpMultiplier {
                        multiplier,
                        min_level,
                        max_level,
                    },
                );
            }
        }
        if x.total_xp > get_xp_for_level(x.level + 1)? as f32 {
            x.total_xp = get_xp_for_level(x.level)? as f32;
        }
        Ok(x)
    }
}

/// zombie.characters.IsoGameCharacter, the saved fields of a living (non-zombie) character.
#[derive(Clone, Debug, PartialEq)]
pub struct IsoGameCharacter {
    pub moving: IsoMovingObject,
    pub descriptor: Option<SurvivorDesc>,
    pub visual: HumanVisual,
    pub inventory: ItemContainer,
    /// `savedInventoryItems` (= `inventory.includingObsoleteItems`): `None` where an item could not be created.
    pub saved_items: Vec<Option<InventoryItem>>,
    pub asleep: bool,
    pub force_wake_up_time: f32,
    pub stats: Stats,
    pub body_damage: BodyDamage,
    pub traits: CharacterTraits,
    pub xp: Xp,
    /// Index into `saved_items` of `leftHandItem`, where the saved int is in range.
    pub left_hand: Option<usize>,
    pub right_hand: Option<usize>,
    pub depress_effect: f32,
    pub depress_first_take_time: f32,
    pub beta_effect: f32,
    pub beta_delta: f32,
    pub pain_effect: f32,
    pub pain_delta: f32,
    pub sleeping_tablet_effect: f32,
    pub sleeping_tablet_delta: f32,
    /// `readBooks`: (full type, pages already read).
    pub read_books: Vec<(String, i32)>,
    pub reduce_infection_power: f32,
    pub known_recipes: Vec<String>,
    pub last_hour_sleeped: i32,
    pub time_since_last_smoke: f32,
    pub beard_grow_timing: f32,
    pub hair_grow_timing: f32,
    /// The cheat flag bytes as saved (unlimited carry, build, health, mechanics, movables, farming, fishing, brush
    /// tool, fast move, instant actions, unlimited endurance, unlimited ammo, know all recipes; those the world version
    /// has). Their setters only change `cheats` where `PlayerCheats.isCheatAllowed` (debug, client or server) and the
    /// role has the capability; in a single-player game they change nothing.
    pub cheat_flags: Vec<bool>,
    pub sneaking: bool,
    pub death_drag_down: bool,
    /// `readLiterature` (a HashMap in the original; ordered here): title → day.
    pub read_literature: BTreeMap<String, i32>,
    pub read_print_media: BTreeSet<String>,
    pub last_animal_pet: i64,
    pub cheats: PlayerCheats,
}

/// zombie.characters.IsoGameCharacter as its constructor leaves it, for the fields a zombie's construction and idle
/// update read or write (the saved fields are `IsoGameCharacter`; a zombie keeps no inventory, stats or body damage
/// there: its `bodyDamage`, `moodles` and `xp` are null). Everything else the original's 17,000 lines hold (animation
/// player, action context, worn items, sound emitter ...) is not here.
#[derive(Clone, Debug, PartialEq)]
pub struct IsoGameCharacterBase {
    pub moving: IsoMovingObject,
    /// `patienceMin` 20, `patienceMax` 150 (field initialisers) and `patience`, drawn in the constructor.
    pub patience_min: i32,
    pub patience_max: i32,
    pub patience: i32,
    pub health: f32,
    /// `forwardDirection`
    pub forward_direction: Vector2,
    pub descriptor: Option<SurvivorDesc>,
    /// the character's own `humanVisual` (`new HumanVisual(this)`: cleared)
    pub human_visual: HumanVisual,
    /// `characterGender == CharacterGender.FEMALE`
    pub female: bool,
    /// `turnDeltaNormal` 1.0
    pub turn_delta_normal: f32,
}

impl IsoGameCharacterBase {
    /// zombie.characters.IsoGameCharacter(cell, x, y, z), the part that sets these fields: `patience =
    /// Rand.Next(patienceMin, patienceMax)`, then the position (`x + 0.5F`, `y + 0.5F`, `z`) and the default
    /// `offsetX/offsetY` 0. (The constructor's other work, the emitter, inventory, state machine and file watcher,
    /// draws nothing and is not modelled.)
    pub fn new<R: JavaRandom>(rand: &mut RandAbstract<R>, x: f32, y: f32, z: f32) -> Self {
        let mut moving = IsoMovingObject::at(x, y, z);
        let patience_min = 20;
        let patience_max = 150;
        let patience = rand.next_range(patience_min, patience_max);
        moving.x = x + 0.5;
        moving.y = y + 0.5;
        moving.z = z;
        IsoGameCharacterBase {
            moving,
            patience_min,
            patience_max,
            patience,
            health: 0.0,
            forward_direction: Vector2::default(),
            descriptor: None,
            human_visual: HumanVisual::default(),
            female: true,
            turn_delta_normal: 1.0,
        }
    }

    /// zombie.characters.IsoGameCharacter.setForwardIsoDirection: the moving object's direction, then
    /// `setForwardDirectionFromIsoDirection`. `Err` is the IllegalStateException of a zero-length direction.
    pub fn set_forward_iso_direction(&mut self, d: IsoDirections) -> Result<(), String> {
        self.moving.dir = d as i32;
        // IsoMovingObject.getVectorFromDirection(vec, dir): the direction's vector, normalized
        let (x, y) = d.to_vector();
        self.set_forward_direction(x, y)
    }

    /// zombie.characters.IsoGameCharacter.setForwardDirection(float, float): the vector normalized in place, the
    /// iso direction from the (unnormalized) arguments.
    pub fn set_forward_direction(&mut self, x: f32, y: f32) -> Result<(), String> {
        self.forward_direction.x = x;
        self.forward_direction.y = y;
        let length = self.forward_direction.normalize();
        self.moving.dir = IsoDirections::from_angle_xy(x, y) as i32;
        if pz_math::equal(length, 0.0, 1.0e-7) {
            return Err("Forward Direction cannot be zero length vector.".into());
        }
        Ok(())
    }

    /// `getForwardIsoDirection`
    pub fn forward_iso_direction(&self) -> IsoDirections {
        IsoDirections::from_index(self.moving.dir)
    }

    /// zombie.characters.IsoGameCharacter.setFemale: the character's gender and its descriptor's.
    pub fn set_female(&mut self, female: bool) {
        self.female = female;
        if let Some(d) = self.descriptor.as_mut() {
            d.female = female;
        }
    }
}

impl IsoGameCharacter {
    /// zombie.characters.IsoGameCharacter.load (not a zombie, not a debug save). A burning character (`SetOnFire`) is
    /// not ported.
    pub fn load(
        b: &mut ByteBuffer,
        world_version: i32,
        factory: &dyn ItemFactory,
        sb: &BodySandbox,
    ) -> Result<Self, String> {
        let moving = IsoMovingObject::load(b, world_version)?;
        let descriptor = if b.get()? != 0 {
            Some(SurvivorDesc::load(b, world_version)?)
        } else {
            None
        };
        let visual = HumanVisual::load(b)?;
        let mut inventory = ItemContainer::default();
        let saved_items = inventory.load(b, world_version, factory)?;
        let asleep = b.get()? != 0;
        let force_wake_up_time = b.get_float()?;
        let stats = Stats::load(b)?;
        // sleepingPillsTaken of a freshly constructed player
        let body_damage = BodyDamage::load(b, world_version, sb, 0)?;
        let traits = CharacterTraits::load(b)?;
        let xp = Xp::load(b)?;
        let hand = |n: i32| usize::try_from(n).ok().filter(|&i| i < saved_items.len());
        let left_hand = hand(b.get_int()?);
        let right_hand = hand(b.get_int()?);
        if b.get()? != 0 {
            return Err(not_ported("IsoGameCharacter.SetOnFire while loading"));
        }
        let mut c = IsoGameCharacter {
            moving,
            descriptor,
            visual,
            inventory,
            saved_items,
            asleep,
            force_wake_up_time,
            stats,
            body_damage,
            traits,
            xp,
            left_hand,
            right_hand,
            depress_effect: b.get_float()?,
            depress_first_take_time: b.get_float()?,
            beta_effect: b.get_float()?,
            beta_delta: b.get_float()?,
            pain_effect: b.get_float()?,
            pain_delta: b.get_float()?,
            sleeping_tablet_effect: b.get_float()?,
            sleeping_tablet_delta: b.get_float()?,
            read_books: Vec::new(),
            reduce_infection_power: 0.0,
            known_recipes: Vec::new(),
            last_hour_sleeped: 0,
            time_since_last_smoke: 0.0,
            beard_grow_timing: 0.0,
            hair_grow_timing: 0.0,
            cheat_flags: Vec::new(),
            sneaking: false,
            death_drag_down: false,
            read_literature: BTreeMap::new(),
            read_print_media: BTreeSet::new(),
            last_animal_pet: 0,
            cheats: PlayerCheats::default(),
        };
        for _ in 0..b.get_int()? {
            let full_type = read_string(b)?;
            c.read_books.push((full_type, b.get_int()?));
        }
        c.reduce_infection_power = b.get_float()?;
        for _ in 0..b.get_int()? {
            c.known_recipes.push(read_string(b)?);
        }
        c.last_hour_sleeped = b.get_int()?;
        c.time_since_last_smoke = b.get_float()?;
        c.beard_grow_timing = b.get_float()?;
        c.hair_grow_timing = b.get_float()?;
        let mut flags = 6;
        if world_version >= 202 {
            flags += 1;
        }
        if world_version >= 217 {
            flags += 2;
        }
        flags += 2;
        if world_version >= 230 {
            flags += 2;
        }
        for _ in 0..flags {
            c.cheat_flags.push(b.get()? != 0);
        }
        c.sneaking = b.get()? != 0;
        c.death_drag_down = b.get()? != 0;
        for _ in 0..b.get_int()? {
            let title = read_string(b)?;
            let day = b.get_int()?;
            // addReadLiterature(title, day): a title saved once (the map's keys are unique) is always added.
            c.read_literature.entry(title).or_insert(day);
        }
        if world_version >= 222 {
            for _ in 0..b.get_int()? {
                c.read_print_media.insert(read_string(b)?);
            }
        }
        c.last_animal_pet = b.get_long()?;
        if world_version >= 231 {
            c.cheats.load(b)?;
        }
        Ok(c)
    }
}
