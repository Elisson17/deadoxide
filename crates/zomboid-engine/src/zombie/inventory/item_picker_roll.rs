//! zombie.inventory.ItemPickerJava, the container filling: `fillContainer`, `fillContainerType`, `rollItem`,
//! `rollProceduralItem`, `doRollItem` with the item creation they cause (`tryAddItemToContainer`,
//! `ItemSpawner.spawnItem`, `InventoryItemFactory.CreateItem`) and `StashSystem.checkStashItem`, as far as a headless
//! run reaches. Every random draw is made in the original's order from one `RandAbstract` (the oracle swaps in a
//! seeded `java.util.Random` and prints the draw counter, as for the climate).
//!
//! What is ported is the path of the item classes `Normal` (ComboItem), `Food`, `Literature`, `Drainable` and
//! `Weapon` without ammunition (HandWeapon), with or without a FluidContainer or Durability component. Anything else the roll would reach (clothing, weapons, bags,
//! keys, radios, maps, media, spawnWith items, trash/wear/rot flags, cooked food, ...) returns `Err("not ported: ...")`
//! instead of a value: a scenario that hits one is not reproduced, never approximated.
use std::collections::{HashMap, HashSet};

use super::inventory_item::ItemType;
use super::item_picker_java::{
    ItemPickerContainer, ItemPickerJava, ItemPickerRoom, ProceduralItem,
};
use crate::java::{self, Random};
use crate::zombie::core::random::rand_abstract::RandAbstract;
use crate::zombie::game_window::ClientStartup;
use crate::zombie::sandbox_options::{SandboxOptions, Value as SoValue};
use crate::zombie::scripting::item::{parse_float, Item, Value};
use crate::zombie::scripting::objects::resource_location::ResourceLocation;

mod clothing;
mod firearm;
mod food;
pub use firearm::WeaponState;
pub use food::FoodState;

/// The generator the rolls draw from (`Rand.rand`, a `RandStandard` in the game; a seeded `java.util.Random` here).
pub type LootRand = RandAbstract<Random>;

fn unported(msg: impl std::fmt::Display) -> String {
    format!("not ported: {msg}")
}

/// zombie.core.stash.StashSystem as `initAllStashes` leaves it: how many stash buildings are possible and which item
/// types the stashes name (`Stash.item`).
#[derive(Clone, Debug, Default)]
pub struct StashInfo {
    pub possible: usize,
    pub items: HashSet<String>,
}

/// A room of an IsoBuilding with the `proceduralSpawnedContainer` map of its RoomDef.
#[derive(Clone, Debug)]
pub struct LootRoom {
    pub name: String,
    pub procedural_spawned: HashMap<String, i32>,
}

/// An IsoBuilding: its rooms, in order.
#[derive(Clone, Debug, Default)]
pub struct LootBuilding {
    pub rooms: Vec<LootRoom>,
}

/// The square a container stands on: its building and room (indexes into `LootRoller::buildings`).
#[derive(Clone, Copy, Debug)]
pub struct LootSquare {
    pub building: usize,
    pub room: usize,
}

/// An InventoryItem as far as the roll reads or changes it.
#[derive(Clone, Debug)]
pub struct LootItem {
    /// Index of the item script.
    pub script: usize,
    pub full_type: String,
    /// `getType()`: the script's name.
    pub type_name: String,
    pub class: ItemType,
    pub id: i32,
    pub uses: i32,
    /// `condition` (`ConditionMax` at creation; `randomizeCondition` lowers it).
    pub condition: i32,
    /// The `Sharpness` attribute (`None` without one).
    pub sharpness: Option<f32>,
    /// The `HeadCondition` attribute (`None` without one) and the `HeadConditionMax` attribute if the script has it.
    pub head_condition: Option<i32>,
    pub head_condition_max: Option<i32>,
    /// `actualWeight` of the instance (a drainable's `updateWeight` changes it).
    pub actual_weight: f32,
    /// `getDisplayName().equals(getFullType())`: the translation is missing, the weight reads as 0.
    pub zero_weight: bool,
    /// What `getFluidContainer().getAmount()` can be: a lower and an upper bound (equal when known); `None` without
    /// a FluidContainer component.
    pub fluid_amount: Option<(f32, f32)>,
    /// `InventoryContainer.container`: the bag's own ItemContainer (`None` for every other class).
    pub inner: Option<Box<LootContainer>>,
    /// `InventoryContainer.weightReduction` (0..=100).
    pub weight_reduction: i32,
    /// The `Food` state (`None` for every other class).
    pub food: Option<FoodState>,
    /// `InventoryItem.maxAmmo` / `currentAmmoCount` and the item key of its `AmmoType` (a clip, a firearm).
    pub max_ammo: i32,
    pub current_ammo: i32,
    pub ammo_item: Option<String>,
    /// A `HandWeapon`'s ammunition state (`None` for every other class).
    pub weapon: Option<WeaponState>,
}

/// An ItemContainer: the type, the capacity set on it, its items and the square it was made for (`sourceGrid`).
#[derive(Clone, Debug)]
pub struct LootContainer {
    pub type_name: String,
    pub capacity: i32,
    pub items: Vec<LootItem>,
    pub square: Option<LootSquare>,
    /// `containingItem != null`: the container of a bag (`getCapacity` caps it at 50, not 100).
    pub in_bag: bool,
    /// `inventoryContainer.getMaxItemSize()` of a bag's container (0 for the others).
    pub max_item_size: f32,
    /// `getAcceptItemFunction()` / `getOnlyAcceptCategory()` of a bag's container.
    pub accept_item_function: Option<String>,
    pub only_accept_category: Option<String>,
    /// `explored` (set by `itemSpawnSanityCheck`).
    pub explored: bool,
}

impl LootContainer {
    pub fn new(type_name: &str, capacity: i32, square: Option<LootSquare>) -> Self {
        LootContainer {
            type_name: type_name.to_string(),
            capacity,
            items: Vec::new(),
            square,
            in_bag: false,
            max_item_size: 0.0,
            accept_item_function: None,
            only_accept_category: None,
            explored: false,
        }
    }
}

/// The container `path` (item indexes from `root`, each a bag) leads to; empty means `root` itself.
fn container_at<'c>(root: &'c LootContainer, path: &[usize]) -> &'c LootContainer {
    let mut c = root;
    for &i in path {
        c = c.items[i]
            .inner
            .as_deref()
            .expect("a path step names a bag");
    }
    c
}

fn container_at_mut<'c>(root: &'c mut LootContainer, path: &[usize]) -> &'c mut LootContainer {
    let mut c = root;
    for &i in path {
        c = c.items[i]
            .inner
            .as_deref_mut()
            .expect("a path step names a bag");
    }
    c
}

/// A `HashMap<String, Integer>` filled with `put`/`clear`, iterated in the original's order.
#[derive(Default)]
struct JMap {
    keys: Vec<String>,
    vals: HashMap<String, i32>,
}

impl JMap {
    fn put(&mut self, k: &str, v: i32) {
        if self.vals.insert(k.to_string(), v).is_none() {
            self.keys.push(k.to_string());
        }
    }
    fn clear(&mut self) {
        self.keys.clear();
        self.vals.clear();
    }
    fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
    /// `map.keySet()` order.
    fn order(&self) -> Vec<&str> {
        let ks: Vec<&str> = self.keys.iter().map(String::as_str).collect();
        java::hash_map_order(&ks)
            .into_iter()
            .map(|i| ks[i])
            .collect()
    }
}

/// ItemContainer.floatingPointCorrection
fn floating_point_correction(val: f32) -> f32 {
    let tmp = val * 100.0f32;
    let t = tmp as i32;
    let r = if tmp - (t as f32) >= 0.5f32 {
        tmp + 1.0f32
    } else {
        tmp
    };
    (r as i32) as f32 / 100.0f32
}

fn script_float(item: &Item, name: &str) -> f32 {
    match item.get(name) {
        Some(Value::F32(v)) => *v,
        _ => 0.0,
    }
}
fn script_int(item: &Item, name: &str) -> i32 {
    match item.get(name) {
        Some(Value::I32(v)) => *v,
        _ => 0,
    }
}
fn script_bool(item: &Item, name: &str) -> bool {
    matches!(item.get(name), Some(Value::Bool(true)))
}
fn script_str<'i>(item: &'i Item, name: &str) -> Option<&'i str> {
    match item.get(name) {
        Some(Value::Str(Some(s))) => Some(s.as_str()),
        _ => None,
    }
}
/// The text of the last `COMPLEX` parameter `key` (lower case) of the script.
fn script_complex<'i>(item: &'i Item, key: &str) -> Option<&'i str> {
    item.unported
        .iter()
        .rev()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}
/// An `Int` attribute from a script value (`AttributeInstance.Int.setValueFromScriptString`: `Integer.parseInt`, a
/// failure leaves the initial value 10), kept inside the bounds of `HeadCondition`/`HeadConditionMax` (0..=1000).
fn script_int_attribute(v: &str) -> i32 {
    java::parse_int(java::trim(v)).unwrap_or(10).clamp(0, 1000)
}

fn has_tag(item: &Item, tag: &str) -> bool {
    match ResourceLocation::of(tag) {
        Ok(t) => item.tags.contains(&t),
        Err(_) => false,
    }
}

/// `Item.itemType`: `ItemType` (default `NORMAL`).
fn script_item_type(item: &Item) -> Result<ItemType, String> {
    match script_complex(item, "itemtype") {
        None => Ok(ItemType::Normal),
        Some(v) => {
            ItemType::from_script(java::trim(v)).ok_or_else(|| unported(format!("ItemType {v}")))
        }
    }
}

/// `Item.actualWeight` and `Item.capacity` after the script's `Weight`/`Capacity` parameters in script order
/// (`Capacity` is capped at 50 minus the weight set so far).
fn script_weight_capacity(item: &Item) -> (f32, i32) {
    let mut weight = 1.0f32;
    let mut capacity = 0i32;
    for (k, v) in &item.unported {
        match k.as_str() {
            "weight" => {
                if let Some(w) = parse_float(v) {
                    weight = if w < 0.0 { 0.0 } else { w };
                }
            }
            "capacity" => {
                if let Some(c) = java::parse_int(java::trim(v)) {
                    capacity = if (c as f32) > 50.0f32 - weight {
                        (50.0f32 - weight) as i32
                    } else {
                        c
                    };
                }
            }
            _ => {}
        }
    }
    (weight, capacity)
}

/// Item.getActualWeight
fn script_actual_weight(item: &Item) -> f32 {
    let w = script_weight_capacity(item).0;
    if w < 0.0 {
        0.0
    } else {
        w
    }
}

/// The state a roll runs on: the scripts and tables the ported game loaded, a generator and the buildings of the
/// squares containers stand on.
pub struct LootRoller<'a> {
    pub startup: &'a ClientStartup,
    pub picker: &'a ItemPickerJava,
    pub sandbox: &'a SandboxOptions,
    pub stash: &'a StashInfo,
    pub rand: LootRand,
    pub buildings: Vec<LootBuilding>,
    /// `Core.getOptionReloadDifficulty()` (option `reloadDifficulty`, default 2).
    pub reload_difficulty: i32,
}

impl<'a> LootRoller<'a> {
    pub fn new(
        startup: &'a ClientStartup,
        picker: &'a ItemPickerJava,
        sandbox: &'a SandboxOptions,
        stash: &'a StashInfo,
        rand: LootRand,
    ) -> Self {
        LootRoller {
            startup,
            picker,
            sandbox,
            stash,
            rand,
            buildings: Vec::new(),
            reload_difficulty: 2,
        }
    }

    fn script(&self, i: usize) -> Result<&'a Item, String> {
        self.startup
            .item(i)
            .ok_or_else(|| "java.lang.IndexOutOfBoundsException: item script".to_string())
    }

    /// ItemPickerJava.NO_GENERIC_LOOT_CONTAINERS.contains(ContainerType.get(ResourceLocation.of(type))).
    fn no_generic_loot_container(type_name: &str) -> Result<bool, String> {
        const NO_GENERIC: [&str; 42] = [
            "barbecue",
            "barbecuepropane",
            "bin",
            "brazier",
            "campfire",
            "cashregister",
            "clothingdryer",
            "clothingdryerbasic",
            "clothingrack",
            "clothingwasher",
            "coffeemaker",
            "coffin",
            "composter",
            "dishwasher",
            "doghouse",
            "dumpster",
            "fireplace",
            "freezer",
            "fridge",
            "icecream",
            "logs",
            "mannequin",
            "medicine",
            "microwave",
            "newspaper_dispatch",
            "newspaper_herald",
            "newspaper_knews",
            "newspaper_times",
            "plankstash",
            "postbox",
            "shelter",
            "stonefurnace",
            "stove",
            "survivorcrate",
            "tent",
            "toaster",
            "trough",
            "vendinggt",
            "vendingpop",
            "vendingsnack",
            "woodstove",
            "",
        ];
        let loc = ResourceLocation::of(type_name)?;
        Ok(loc.namespace == "base" && NO_GENERIC[..41].contains(&loc.path.as_str()))
    }

    fn room_name(&self, sq: LootSquare) -> &str {
        &self.buildings[sq.building].rooms[sq.room].name
    }

    /// ItemPickerJava.fillContainer (ItemPickInfo.GetPickInfo keeps no state a draw depends on). The
    /// `OnFillContainer` events are not fired: their only handler, LootLog.lua, returns at once without ISLootLog.
    pub fn fill_container(&mut self, container: &mut LootContainer) -> Result<(), String> {
        let Some(sq) = container.square else {
            return Ok(());
        };
        if container.type_name == "inventorymale" || container.type_name == "inventoryfemale" {
            return Err(unported(
                "fillContainer of a zombie inventory (outfit tables, bags)",
            ));
        }
        let picker = self.picker;
        let room_name = self.room_name(sq).to_string();
        let mut room_dist: Option<&ItemPickerRoom> = picker.rooms.get("all");
        if let Some(room_dist2) = picker.rooms.get(&room_name) {
            let mut name = room_name.clone();
            let mut container_dist = room_dist2.containers.get(&container.type_name);
            if container_dist.is_none()
                && room_dist2.containers.contains_key("other")
                && !Self::no_generic_loot_container(&container.type_name)?
            {
                container_dist = room_dist2.containers.get("other");
            }
            if container_dist.is_none() && room_dist2.containers.contains_key("all") {
                container_dist = room_dist2.containers.get("all");
                name = "all".to_string();
            }
            if container_dist.is_none() {
                self.fill_container_type_internal(room_dist, container, &name)?;
            } else {
                if picker.rooms.contains_key(&room_name) {
                    room_dist = picker.rooms.get(&room_name);
                }
                if room_dist.is_some() {
                    self.fill_container_type_internal(room_dist, container, &room_name)?;
                }
            }
        } else {
            self.fill_container_type_internal(room_dist, container, &room_name_or_all(&room_name))?;
        }
        Ok(())
    }

    /// ItemPickerJava.fillContainerType
    pub fn fill_container_type(
        &mut self,
        room_dist: Option<&ItemPickerRoom>,
        container: &mut LootContainer,
        room_name: &str,
    ) -> Result<(), String> {
        self.fill_container_type_internal(room_dist, container, room_name)
    }

    fn fill_container_type_internal(
        &mut self,
        room_dist: Option<&ItemPickerRoom>,
        container: &mut LootContainer,
        room_name: &str,
    ) -> Result<(), String> {
        let do_item_container = !self
            .picker
            .no_container_fill_rooms
            .iter()
            .any(|r| r == room_name);
        let Some(rd) = room_dist else {
            return Err("java.lang.NullPointerException".into());
        };
        if let Some(all) = rd.containers.get("all") {
            self.roll_item_internal(Some(all), container, do_item_container, Some(rd))?;
        }
        let mut container_dist = rd.containers.get(&container.type_name);
        if container_dist.is_none() && !Self::no_generic_loot_container(&container.type_name)? {
            container_dist = rd.containers.get("other");
        }
        if container_dist.is_some() {
            self.roll_item_internal(container_dist, container, do_item_container, Some(rd))?;
        }
        Ok(())
    }

    /// ItemPickerJava.getZombieDensityFactor. Headless there is no player and no meta chunk under a square (the meta
    /// grid is empty), so the density is `Math.min(0, cap) * ZombiePopLootEffect` = 0.
    fn zombie_density_factor(&self) -> f32 {
        0.0
    }

    /// ItemPickerJava.rollItem / rollItemInternal
    fn roll_item_internal(
        &mut self,
        container_dist: Option<&ItemPickerContainer>,
        container: &mut LootContainer,
        do_item_container: bool,
        room_dist: Option<&ItemPickerRoom>,
    ) -> Result<(), String> {
        let Some(dist) = container_dist else {
            return Ok(());
        };
        let zombie_density = self.zombie_density_factor();
        if dist.procedural {
            self.roll_procedural_item_internal(
                dist.procedural_items.as_deref().unwrap_or(&[]),
                container,
                zombie_density,
                room_dist,
            )
        } else {
            if let Some(junk) = dist.junk.as_deref() {
                self.do_roll_item_internal(
                    junk,
                    container,
                    zombie_density,
                    do_item_container,
                    room_dist,
                    true,
                )?;
            }
            self.do_roll_item_internal(
                dist,
                container,
                zombie_density,
                do_item_container,
                room_dist,
                false,
            )
        }
    }

    /// ItemPickerJava.getDistribInHashMap
    fn get_distrib_in_hash_map(&mut self, map: &JMap) -> Option<String> {
        let order = map.order();
        let mut total_weight = 0i32;
        for name in &order {
            total_weight = total_weight.wrapping_add(map.vals[*name]);
        }
        if total_weight == -1 {
            let rand = self.rand.next(map.keys.len() as i32);
            for (index, name) in order.iter().enumerate() {
                if index as i32 == rand {
                    return Some((*name).to_string());
                }
            }
        }
        let rand = self.rand.next(total_weight);
        let mut current_chance = 0i32;
        for name in order {
            current_chance = current_chance.wrapping_add(map.vals[name]);
            if current_chance >= rand {
                return Some(name.to_string());
            }
        }
        None
    }

    /// ItemPickerJava.rollProceduralItem / rollProceduralItemInternal. The headless square has no objects, zones or
    /// room bounds, so `forceForItems`/`forceForTiles`/`forceForZones` find nothing to force (a `forceForItems`
    /// entry still takes its branch, which skips the tile, zone and room checks).
    fn roll_procedural_item_internal(
        &mut self,
        procedural_items: &[ProceduralItem],
        container: &mut LootContainer,
        zombie_density: f32,
        room_dist: Option<&ItemPickerRoom>,
    ) -> Result<(), String> {
        let Some(sq) = container.square else {
            return Ok(());
        };
        let room = true;
        let mut forced = JMap::default();
        let mut normal = JMap::default();
        for pi in procedural_items {
            let name = pi.name.clone().unwrap_or_default();
            let (min, max) = (pi.min, pi.max);
            let weight_chance = if pi.weight_chance <= 0 {
                1
            } else {
                pi.weight_chance
            };
            {
                let spawned = &mut self.buildings[sq.building].rooms[sq.room].procedural_spawned;
                if !spawned.contains_key(&name) {
                    spawned.insert(name.clone(), 0);
                }
            }
            let rooms_number = self.buildings[sq.building].rooms.len();
            if pi.force_for_items.is_some() && room && rooms_number <= 500 {
                // x..x2 and y..y2 of the room's RoomDef are empty: nothing found
            } else if pi.force_for_tiles.is_some() {
                // the square has no objects
            } else if pi.force_for_zones.is_none() {
                if let (Some(rooms), true) = (&pi.force_for_rooms, room) {
                    for want in rooms {
                        // IsoBuilding.getRandomRoom(String): draws only when a room of that name exists
                        let n = self.buildings[sq.building]
                            .rooms
                            .iter()
                            .filter(|r| r.name == *want)
                            .count();
                        if n > 0 {
                            self.rand.next(n as i32);
                            forced.clear();
                            forced.put(&name, -1);
                            break;
                        }
                    }
                }
            } else {
                // IsoMetaGrid.getZonesAt: no meta chunk, no zones
            }
            if pi.force_for_items.is_none()
                && pi.force_for_zones.is_none()
                && pi.force_for_tiles.is_none()
                && pi.force_for_rooms.is_none()
            {
                let spawned = self.buildings[sq.building].rooms[sq.room].procedural_spawned[&name];
                if room && min == 1 && spawned == 0 {
                    forced.put(&name, weight_chance);
                } else if !room || spawned < max {
                    normal.put(&name, weight_chance);
                }
            }
        }
        let to_spawn = if !forced.is_empty() {
            self.get_distrib_in_hash_map(&forced)
        } else if !normal.is_empty() {
            self.get_distrib_in_hash_map(&normal)
        } else {
            None
        };
        if let Some(name) = to_spawn {
            if let Some(dist) = self.picker.procedural_distributions.get(&name) {
                if let Some(junk) = dist.junk.as_deref() {
                    self.do_roll_item_internal(
                        junk,
                        container,
                        zombie_density,
                        true,
                        room_dist,
                        true,
                    )?;
                }
                self.do_roll_item_internal(
                    dist,
                    container,
                    zombie_density,
                    true,
                    room_dist,
                    false,
                )?;
                let spawned = &mut self.buildings[sq.building].rooms[sq.room].procedural_spawned;
                let cur = spawned
                    .get(&name)
                    .copied()
                    .ok_or("java.lang.NullPointerException")?;
                spawned.insert(name, cur + 1);
            }
        }
        Ok(())
    }

    /// ItemPickerJava.getLootType
    fn loot_type(&self, item: &Item) -> Result<&'static str, String> {
        let dc = script_str(item, "displayCategory");
        let is = |c: &str| dc == Some(c);
        let ty = script_item_type(item)?;
        let (_, capacity) = script_weight_capacity(item);
        if item.name == "Generator"
            || item.full_name() == "Base.Generator"
            || has_tag(item, "Generator")
        {
            return Ok("Generator");
        }
        if has_tag(item, "IsMemento") || is("Memento") {
            return Ok("Memento");
        }
        if script_bool(item, "medical") || is("FirstAid") || is("FirstAidWeapon") {
            return Ok("Medical");
        }
        if script_bool(item, "mechanicsItem")
            || is("VehicleMaintenance")
            || is("VehicleMaintenanceWeapon")
        {
            return Ok("Mechanics");
        }
        if is("MaterialWeapon") || is("Material") {
            return Ok("Material");
        }
        if is("GardeningWeapon") || is("Gardening") || has_tag(item, "FarmingLoot") {
            return Ok("Farming");
        }
        if is("ToolWeapon") || is("Tool") {
            return Ok("Tool");
        }
        if is("CookingWeapon") || is("Cooking") {
            return Ok("Cookware");
        }
        if script_bool(item, "survivalGear")
            || is("FishingWeapon")
            || is("Fishing")
            || is("Trapping")
            || is("Camping")
            || is("FireSource")
        {
            return Ok("SurvivalGears");
        }
        if ty != ItemType::Food && !is("Food") {
            let ammo_type = script_complex(item, "ammotype").is_some();
            if !is("Ammo") && !has_tag(item, "AmmoCase") && (ty != ItemType::Normal || !ammo_type) {
                let ranged = script_bool(item, "ranged");
                if ty == ItemType::Weapon && !ranged {
                    Ok("Weapon")
                } else if ty == ItemType::WeaponPart
                    || ty == ItemType::Weapon && ranged
                    || has_tag(item, "FirearmLoot")
                {
                    Ok("RangedWeapon")
                } else if ty == ItemType::Key || ty == ItemType::KeyRing || has_tag(item, "KeyRing")
                {
                    Ok("Key")
                } else if capacity > 0 || ty == ItemType::Container || is("Bag") {
                    Ok("Container")
                } else if is("SkillBook") {
                    Ok("SkillBook")
                } else if is("RecipeResource") {
                    Ok("RecipeResource")
                } else if ty == ItemType::Literature {
                    Ok("Literature")
                } else if ty == ItemType::Clothing {
                    Ok("Clothing")
                } else if script_str(item, "recordedMediaCat").is_some() {
                    Ok("Media")
                } else {
                    Ok("Other")
                }
            } else {
                Ok("Ammo")
            }
        } else if !script_bool(item, "cannedFood")
            && script_int(item, "daysFresh") != 1_000_000_000
            && ty == ItemType::Food
        {
            Ok("Food")
        } else {
            Ok("CannedFood")
        }
    }

    /// ItemPickerJava.doSandboxSettings
    fn do_sandbox_settings(&self, value: i32) -> f32 {
        let f = |name: &str| match self.sandbox.get(name) {
            Some(SoValue::Double(d)) => *d as f32,
            _ => 0.0,
        };
        match value {
            1 => 0.0,
            2 => f("InsaneLootFactor"),
            3 => f("ExtremeLootFactor"),
            4 => f("RareLootFactor"),
            5 => f("NormalLootFactor"),
            6 => f("CommonLootFactor"),
            7 => f("AbundantLootFactor"),
            _ => 0.6,
        }
    }

    /// ItemPickerJava.getLootModifierFromType
    fn loot_modifier_from_type(&self, loot_type: &str) -> f32 {
        let m = &self.picker.modifiers;
        match loot_type {
            "Generator" => {
                self.do_sandbox_settings(self.sandbox.get_int("GeneratorSpawning").unwrap_or(5))
            }
            "Memento" => m.memento,
            "Medical" => m.medical,
            "Mechanics" => m.mechanics,
            "Material" => m.material,
            "Farming" => m.farming,
            "Tool" => m.tool,
            "Cookware" => m.cookware,
            "SurvivalGears" => m.survival_gears,
            "CannedFood" => m.canned_food,
            "Food" => m.food,
            "Ammo" => m.ammo,
            "Weapon" => m.weapon,
            "RangedWeapon" => m.ranged_weapon,
            "Key" => m.key,
            "Container" => m.container,
            "SkillBook" => m.skill_book,
            "RecipeResource" => m.recipe_resource,
            "Literature" => m.literature,
            "Clothing" => m.clothing,
            "Media" => m.media,
            _ => m.other,
        }
    }

    /// SandboxOptions.lootItemRemovalListContains
    fn loot_item_removal_list_contains(&self, item_type: &str) -> bool {
        match self.sandbox.get("LootItemRemovalList") {
            Some(SoValue::Str(s)) => s
                .split(',')
                .map(java::trim)
                .any(|x| !x.is_empty() && x == item_type),
            _ => false,
        }
    }

    /// ItemPickerJava.getLootModifier(String)
    fn loot_modifier(&self, item_name: &str) -> Result<f32, String> {
        let mut name = item_name;
        let mut item = self.startup.find_item(name);
        if item.is_none() && name.ends_with("Empty") {
            name = &name[..name.len() - 5];
            item = self.startup.find_item(name);
        }
        let Some(idx) = item else {
            return Ok(self.picker.modifiers.other);
        };
        let short = name.split_once('.').map_or(name, |p| p.1);
        if !self.loot_item_removal_list_contains(name)
            && !self.loot_item_removal_list_contains(short)
        {
            let lt = self.loot_type(self.script(idx)?)?;
            Ok(self.loot_modifier_from_type(lt))
        } else {
            Ok(0.0)
        }
    }

    /// ItemPickerJava.getActualSpawnChance(item, character = null, container, zombieDensity, isJunk)
    fn actual_spawn_chance(
        &self,
        item_name: &str,
        chance: f32,
        container: &LootContainer,
        zombie_density: f32,
        is_junk: bool,
    ) -> Result<f32, String> {
        let script = self.startup.find_item(item_name);
        let mut loot_modifier = self.loot_modifier(item_name)?;
        if is_junk && loot_modifier > 0.0 {
            loot_modifier = 1.0;
        }
        if loot_modifier == 0.0 {
            return Ok(0.0);
        }
        // getBaseChance -> getBaseChanceMultiplier(character = null, isJunk, scriptItem)
        let mut multiplier = 1.0f32;
        if is_junk {
            multiplier *= 1.4f32;
        }
        if let Some(i) = script {
            if has_tag(self.script(i)?, "MoreWhenNoZombies") {
                let zombies = self.sandbox.get_int("Zombies").unwrap_or(4);
                let pop = match self.sandbox.get("ZombieConfig.PopulationMultiplier") {
                    Some(SoValue::Double(d)) => *d,
                    _ => return Err(unported("ZombieConfig.PopulationMultiplier")),
                };
                if zombies == 6 || pop == 0.0 {
                    multiplier *= 2.0f32;
                }
            }
        }
        let base_chance = chance * multiplier;
        // getAdjustedZombieDensity: 0 for junk and for items that ignore the density; the density itself is 0 here
        let _ = zombie_density;
        let zombie_density = 0.0f32;
        let over_time = if container.square.is_some() {
            // SandboxOptions.getCurrentLootMultiplier(square): 1 - diminished percentage / 100
            1.0f32 - self.diminished_loot_percentage()? as f32 / 100.0f32
        } else {
            1.0f32 - self.diminished_loot_percentage()? as f32 / 100.0f32
        };
        Ok((base_chance * 100.0f32 * loot_modifier + zombie_density) * over_time)
    }

    /// SandboxOptions.getCurrentDiminishedLootPercentage(square). The world age is the standalone `GameTime`'s (9:00,
    /// no night survived); a square's region is "General" (no zones headless), which is not null, so `RuralLooted`
    /// does not apply.
    fn diminished_loot_percentage(&self) -> Result<i32, String> {
        let get = |n: &str| {
            self.sandbox
                .get_int(n)
                .ok_or_else(|| unported(format!("sandbox option {n}")))
        };
        let max_looted = get("MaximumDiminishedLoot")?;
        let days_until_max = get("DaysUntilMaximumDiminishedLoot")?;
        if max_looted <= 0 {
            return Ok(0);
        }
        if days_until_max <= 0 {
            return Ok(max_looted);
        }
        let age_days = crate::zombie::game_time::GameTime::default().get_world_age_hours() / 24.0
            + ((get("TimeSinceApo")? - 1) * 30) as f64;
        let mut days = age_days as i32;
        if days <= 0 {
            days = 1;
        }
        if days > days_until_max {
            days = days_until_max;
        }
        let looted = max_looted * days / days_until_max;
        Ok(looted.clamp(0, 100))
    }

    /// `ItemContainer.getContentsWeight` (`getCapacityWeight` of a container with no parent): the sum of the items'
    /// unequipped weights, as a lower and an upper bound.
    fn contents_weight(&self, c: &LootContainer) -> Result<(f32, f32), String> {
        let (mut lo, mut hi) = (0.0f32, 0.0f32);
        for it in &c.items {
            let (a, b) = self.unequipped_weight(it)?;
            lo += a;
            hi += b;
        }
        Ok((lo, hi))
    }

    /// `InventoryItem.getUnequippedWeight` = `getActualWeight() + getContentsWeight()`, a lower and an upper bound.
    /// A bag's contents weight is its container's contents plus its fluid (`InventoryContainer.getContentsWeight`).
    fn unequipped_weight(&self, it: &LootItem) -> Result<(f32, f32), String> {
        let actual = self.actual_weight(it)?;
        if it.inner.is_some() {
            let (clo, chi) = self.bag_contents_weight(it)?;
            return Ok((actual + clo, actual + chi));
        }
        let (clo, chi) = self.item_contents_weight(it)?;
        Ok((actual + clo, actual + chi))
    }

    /// InventoryItem.getActualWeight and its Food override.
    fn actual_weight(&self, it: &LootItem) -> Result<f32, String> {
        let script = self.script(it.script)?;
        if it.class == ItemType::Weapon {
            return Ok(self.weapon_actual_weight(it, script));
        }
        let base = if it.zero_weight {
            0.0
        } else {
            it.actual_weight.max(0.0)
        };
        if it.class != ItemType::Food {
            return Ok(base);
        }
        // Food.getActualWeight (no extra items, not custom weight): `getHungChange()` and `getThirstChange()` of the state
        let food = it
            .food
            .as_ref()
            .ok_or_else(|| unported("a food without state"))?;
        let hung_change = food.hung_change;
        let thirst_change = food.thirst_change_modified();
        if let Some(replace) = script_str(script, "replaceOnUse") {
            let module = &script.module;
            let full = if replace.contains('.') {
                replace.to_string()
            } else {
                format!("{module}.{replace}")
            };
            if let Some(empty_idx) = self.startup.find_item(&full) {
                let empty = self.script(empty_idx)?;
                let mut used_delta = 1.0f32;
                if script_float(script, "hungerChange") < 0.0 {
                    used_delta = hung_change * 100.0f32 / script_float(script, "hungerChange");
                } else if script_float(script, "thirstChange") < 0.0 {
                    used_delta = thirst_change * 100.0f32 / script_float(script, "thirstChange");
                }
                let ew = script_actual_weight(empty);
                return Ok((script_actual_weight(script) - ew) * used_delta + ew);
            }
        } else {
            let mut used_delta = 1.0f32;
            if script_float(script, "hungerChange") < 0.0 {
                used_delta = hung_change * 100.0f32 / script_float(script, "hungerChange");
            } else if script_float(script, "thirstChange") < 0.0 {
                used_delta = thirst_change * 100.0f32 / script_float(script, "thirstChange");
            }
            return Ok(script_actual_weight(script) * used_delta);
        }
        Ok(base)
    }

    /// ItemContainer.hasRoomFor(null, weightVal) = `hasRoomFor(null, weightVal, weightVal)`: a bag's container refuses
    /// an item heavier than the bag's `MaxItemSize`; then `floatingPointCorrection(contents) + weightVal <=
    /// getEffectiveCapacity(null)` (`getCapacity`: 50 inside a bag, 100 otherwise). A FluidContainer item's contents
    /// weight is only bounded here; the answer must not depend on where in the bounds it lies.
    fn has_room_for(&self, container: &LootContainer, weight_val: f32) -> Result<bool, String> {
        if container.in_bag && container.max_item_size > 0.0 && weight_val > container.max_item_size
        {
            return Ok(false);
        }
        let (lo, hi) = self.contents_weight(container)?;
        let cap = container
            .capacity
            .min(if container.in_bag { 50 } else { 100 }) as f32;
        let a = floating_point_correction(lo) + weight_val <= cap;
        let b = floating_point_correction(hi) + weight_val <= cap;
        if a != b {
            return Err(unported(
                "hasRoomFor depends on the amount of a fluid container (only bounded)",
            ));
        }
        Ok(a)
    }

    /// InventoryItem.getCategory (`mainCategory` is never set).
    fn item_category(class: ItemType) -> &'static str {
        match class {
            ItemType::Food => "Food",
            ItemType::Clothing => "Clothing",
            ItemType::Weapon => "Weapon",
            ItemType::Literature => "Literature",
            ItemType::Container => "Container",
            ItemType::Key => "Key",
            ItemType::KeyRing => "Key Ring",
            ItemType::AlarmClock | ItemType::AlarmClockClothing => "AlarmClock",
            ItemType::WeaponPart => "WeaponPart",
            ItemType::Animal => "Animal",
            _ => "Item",
        }
    }

    /// ItemContainer.isItemAllowed for a container with no parent object: the bag's `OnlyAcceptCategory` and its
    /// `AcceptItemFunction`. The five accept functions of `media/lua/server/Items/AcceptItemFunction.lua` that the
    /// scripts name are evaluated natively (stand-ins for the Lua; the oracle runs the real ones); any other name
    /// is not ported.
    fn is_item_allowed(&self, container: &LootContainer, item: &LootItem) -> Result<bool, String> {
        if let Some(cat) = &container.only_accept_category {
            if !cat.eq_ignore_ascii_case(Self::item_category(item.class)) {
                return Ok(false);
            }
        }
        if let Some(f) = &container.accept_item_function {
            let script = self.script(item.script)?;
            let accepted = match f.as_str() {
                "AcceptItemFunction.AmmoStrap_Bullets" => {
                    has_tag(script, "Ammo") && !has_tag(script, "ShotgunShell")
                }
                "AcceptItemFunction.AmmoStrap_Shells" => has_tag(script, "ShotgunShell"),
                "AcceptItemFunction.KeyRing" => {
                    Self::item_category(item.class) == "Key" || has_tag(script, "FitsKeyRing")
                }
                "AcceptItemFunction.HolsterShoulder" => {
                    has_tag(script, "PistolMagazine") && container.items.len() < 2
                }
                "AcceptItemFunction.Wallet" => {
                    item.class == ItemType::Map
                        || item.class == ItemType::Literature
                        || has_tag(script, "FitsWallet")
                }
                other => return Err(unported(format!("accept function {other}"))),
            };
            if !accepted {
                return Ok(false);
            }
        }
        // isInvalidClothingRackItem
        if container.type_name == "clothingrack"
            && item.class != ItemType::Clothing
            && !has_tag(self.script(item.script)?, "FitsClothingRack")
        {
            return Ok(false);
        }
        Ok(true)
    }

    /// ItemPickerJava.tryAddItemToContainer for the container `path` leads to in `root`: `Ok(Some(index))` of the new
    /// item in that container. A bag's container is also asked to fit the weight in the container that holds the
    /// bag (`getContainingItem().getContainer().hasRoomFor`); no container here is a corpse's.
    fn try_add_item_to_container(
        &mut self,
        root: &mut LootContainer,
        path: &[usize],
        item_type: &str,
    ) -> Result<Option<usize>, String> {
        let Some(si) = self.startup.find_item(item_type) else {
            return Ok(None);
        };
        if self.startup.is_obsolete(si) {
            return Ok(None);
        }
        let script = self.script(si)?;
        let total_weight = script_actual_weight(script) * script_int(script, "count") as f32;
        if !self.has_room_for(container_at(root, path), total_weight)? {
            return Ok(None);
        }
        if let Some((_, outer)) = path.split_last() {
            if !self.has_room_for(container_at(root, outer), total_weight)? {
                return Ok(None);
            }
        }
        self.spawn_item(root, path, item_type)
    }

    /// ItemSpawner.spawnItem(String, ItemContainer) (`fill` true): `InventoryItemFactory.CreateItem`, `isItemAllowed`,
    /// `AddItem`, then a bag with a `containers` distribution is filled by `rollContainerItem` (and its junk).
    fn spawn_item(
        &mut self,
        root: &mut LootContainer,
        path: &[usize],
        item_type: &str,
    ) -> Result<Option<usize>, String> {
        let Some(item) = self.create_item(item_type)? else {
            return Ok(None);
        };
        self.spawn_created_item(root, path, item)
    }

    /// ItemSpawner.spawnItem(InventoryItem, ItemContainer) (`fill` true) for an item that was created already.
    fn spawn_created_item(
        &mut self,
        root: &mut LootContainer,
        path: &[usize],
        item: LootItem,
    ) -> Result<Option<usize>, String> {
        if !self.is_item_allowed(container_at(root, path), &item)? {
            return Ok(None);
        }
        // ItemContainer.AddItem refuses an item whose id the container holds (and returns the held one)
        if container_at(root, path)
            .items
            .iter()
            .any(|i| i.id == item.id)
        {
            return Err(unported("a spawned item id equal to one in the container"));
        }
        let (class, type_name) = (item.class, item.type_name.clone());
        let target = container_at_mut(root, path);
        target.items.push(item);
        let idx = target.items.len() - 1;
        if class == ItemType::Container {
            if let Some(dist) = self.picker.containers.get(&type_name) {
                let mut bag = path.to_vec();
                bag.push(idx);
                // ItemPickerJava.rollContainerItem
                self.roll_container_item_internal(root, &bag, Some(dist), false)?;
                if let Some(junk) = dist.junk.as_deref() {
                    self.roll_container_item_internal(root, &bag, Some(junk), true)?;
                }
            }
        }
        // `item.setAutoAge()` (twice in the original: the second changes nothing)
        let outermost = root.type_name.clone();
        let target = container_at_mut(root, path);
        self.item_set_auto_age(&mut target.items[idx], &outermost)?;
        Ok(Some(idx))
    }

    /// ItemPickerJava.onCreateRegion(item, getSquareRegion(square)) and RecipeCodeHelper.nameNewspaper: a paper of
    /// the region's list (`Rand.Next(list)`), then an issue (`Rand.Next(issues)`; a `NewspaperNew` item takes the
    /// last issue after drawing). A square's region is "General" without zones (headless); the item's new name and
    /// `printMedia` mod data are not compared.
    fn on_create_region(&mut self) {
        // ItemGenerationConstants.REGIONAL_PAPERS["General"]: KNOX_KNEWS (6 issues), KENTUCKY_HERALD (5 issues)
        const GENERAL: [i32; 2] = [6, 5];
        let paper = self.rand.next(GENERAL.len() as i32);
        self.rand.next(GENERAL[paper as usize]);
    }

    /// `ZomboidRadio.getInstance().getRecordedMedia().getRandomFromCategory(cat)` in the standalone state: nothing
    /// registered the media (`RecordedMedia.init` and the Lua `OnInitRecordedMedia` run in a game's world load), so
    /// `categorizedMap.containsKey(cat)` is false for every category and the call returns null without a draw.
    /// UNKNOWN: the tables a running game has (CDs, Retail-VHS and Home-VHS draw from them).
    fn recorded_media_random(_cat: &str) -> Option<i32> {
        None
    }

    /// The `getSpawnWith()` step of `doRollItemInternal` / `rollContainerItemInternal`:
    /// `ItemSpawner.spawnItem(spawnWith, container)` and then `spawnWithItem.copyClothing(spawnItem)`. Headless,
    /// `copyClothing` calls the Lua `copyClothingItem`, which fails on the first `getVisual()` (a `ClothingItem` asset
    /// that is not ready gives no visual) before it changes anything, so it draws nothing and sets nothing the
    /// oracle compares. UNKNOWN: the copied tint/texture/decal/dirt/blood/holes in the running game.
    fn spawn_with(
        &mut self,
        root: &mut LootContainer,
        path: &[usize],
        spawn_with: &str,
    ) -> Result<(), String> {
        self.spawn_item(root, path, spawn_with)?;
        Ok(())
    }

    /// InventoryItemFactory.createItemInternal for the supported classes.
    fn create_item(&mut self, item_type: &str) -> Result<Option<LootItem>, String> {
        let mut name = item_type;
        let mut script_idx = self.startup.find_item(name);
        let mut empty_fluid = false;
        if script_idx.is_none() && name.ends_with("Empty") {
            name = &name[..name.len() - 5];
            script_idx = self.startup.find_item(name);
            if let Some(i) = script_idx {
                let s = self.script(i)?;
                if s.contains_component("Durability") {
                    // logged only
                } else if !s.contains_component("FluidContainer") {
                    script_idx = None;
                } else {
                    empty_fluid = true;
                }
            }
        }
        let Some(si) = script_idx else {
            return Ok(None);
        };
        let script = self.script(si)?;
        let class = script_item_type(script)?;
        if !matches!(
            class,
            ItemType::Normal
                | ItemType::Food
                | ItemType::Literature
                | ItemType::Drainable
                | ItemType::Weapon
                | ItemType::Clothing
                | ItemType::Container
                | ItemType::WeaponPart
        ) {
            return Err(unported(format!(
                "item class {class:?} ({})",
                script.full_name()
            )));
        }
        for c in &script.components {
            if c != "FluidContainer" && c != "Durability" && c != "ContextMenuConfig" {
                return Err(unported(format!("component {c} ({})", script.full_name())));
            }
        }
        if script.fluid_blocks > 1 {
            return Err(unported("two FluidContainer blocks"));
        }
        // Item.InstanceItem `item.setAmmoType(this.ammoType)`: the `AmmoType` the script names
        let ammo_item = match script_complex(script, "ammotype") {
            None => None,
            Some(v) => Some(
                firearm::ammo_item_key(java::trim(v))
                    .ok_or_else(|| unported(format!("AmmoType {v} ({})", script.full_name())))?
                    .to_string(),
            ),
        };
        // Item.InstanceItem: a clothing item picks its palette, then the Clothing constructor draws the colour
        if class == ItemType::Clothing {
            if let Some(p) = script_complex(script, "palettes") {
                let n = java::split_char(p, '/').len() as i32;
                self.rand.next(n);
            }
            for _ in 0..3 {
                self.rand.next(255);
            }
        }
        // Item.InstanceItem: the instance's fields from the script
        let (script_weight, _) = script_weight_capacity(script);
        let mut item = LootItem {
            script: si,
            full_type: script.full_name(),
            type_name: script.name.clone(),
            class,
            id: 0,
            uses: 1,
            condition: script_int(script, "conditionMax"),
            sharpness: script_complex(script, "sharpness")
                .and_then(parse_float)
                .map(|v| v.clamp(0.0, 1.0)),
            head_condition: script_complex(script, "headcondition").map(script_int_attribute),
            head_condition_max: script_complex(script, "headconditionmax")
                .map(script_int_attribute),
            actual_weight: if script_weight < 0.0 {
                0.0
            } else {
                script_weight
            },
            zero_weight: false,
            fluid_amount: None,
            inner: None,
            weight_reduction: 0,
            food: (class == ItemType::Food).then(|| FoodState::from_script(script)),
            max_ammo: script_int(script, "maxAmmo"),
            current_ammo: 0,
            ammo_item,
            weapon: (class == ItemType::Weapon).then(WeaponState::default),
        };
        if class == ItemType::Container {
            // Item.InstanceItem: `new InventoryContainer` (its ItemContainer's type is the script name), then
            // setCapacity, setWeightReduction (clamped to 0..=100) and the container's accept settings
            let (_, capacity) = script_weight_capacity(script);
            let mut inner = LootContainer::new(&script.name, capacity, None);
            inner.in_bag = true;
            inner.max_item_size = script_float(script, "maxItemSize");
            inner.only_accept_category =
                script_str(script, "onlyAcceptCategory").map(str::to_string);
            inner.accept_item_function =
                script_str(script, "acceptItemFunction").map(str::to_string);
            item.inner = Some(Box::new(inner));
            item.weight_reduction = script_int(script, "weightReduction").clamp(0, 100);
        }
        // the display name: Translator.getItemNameFromFullType(fullName), and InventoryItem.getActualWeightUnmodded
        // reads 0 when it equals the full type
        let display = self
            .startup
            .translator
            .borrow_mut()
            .get_item_name_from_full_type(
                &script.full_name(),
                script_str(script, "displayName").map(str::to_string),
            )?;
        item.zero_weight = display == item.full_type;
        // GameEntityFactory.CreateInventoryItemEntity: FluidContainer.readFromScript
        if script.contains_component("FluidContainer") {
            let f = script
                .fluid
                .as_ref()
                .ok_or_else(|| unported("FluidContainer component without script data"))?;
            if f.has_filter {
                return Err(unported(format!(
                    "FluidContainer whitelist/blacklist ({})",
                    script.full_name()
                )));
            }
            let mut amount_hi = 0.0f32;
            let mut exact = true;
            if f.initial_fluids > 0 && self.fluid_initial_amount(f) > 0.0 {
                let initial = self.fluid_initial_amount(f);
                if f.initial_fluids == 1 {
                    amount_hi = initial.min(f.capacity);
                } else if f.initial_fluids_is_random {
                    self.rand.next(f.initial_fluids as i32);
                    amount_hi = initial.min(f.capacity);
                } else {
                    amount_hi = (initial * f.initial_fluids as f32).min(f.capacity);
                }
                exact = false;
            }
            item.fluid_amount = Some((if exact { amount_hi } else { 0.0 }, amount_hi));
        }
        // OutfitRNG.setSeed(Rand.Next(Integer.MAX_VALUE)) around synchWithVisual
        self.rand.next(i32::MAX);
        // InventoryItem.synchWithVisual: a fresh item (`modelIndex == -1`) picks a model index
        let list_len =
            |key: &str| script_complex(script, key).map(|v| java::split_char(v, ';').len() as i32);
        if class == ItemType::Weapon && list_len("weaponspritesbyindex").is_some() {
            self.rand
                .next(list_len("weaponspritesbyindex").unwrap_or(0));
        } else {
            let (st, wo) = (
                list_len("staticmodelsbyindex"),
                list_len("worldstaticmodelsbyindex"),
            );
            if st.is_some() || wo.is_some() {
                let max_index = match (st, wo) {
                    (Some(a), Some(b)) => a.max(b),
                    (Some(a), None) => a,
                    (None, b) => b.unwrap_or(0),
                };
                self.rand.next(max_index);
            }
        }
        // createItemInternal
        if empty_fluid && item.fluid_amount.is_some() {
            item.fluid_amount = Some((0.0, 0.0));
        }
        item.id = self.rand.next(2_146_250_223).wrapping_add(1_233_423);
        if class == ItemType::Drainable {
            let max_uses = Self::drainable_max_uses(script);
            item.uses = (max_uses as f32 * 1.0f32) as i32;
            self.drainable_update_weight(&mut item)?;
        } else if class == ItemType::WeaponPart {
            // `item instanceof Drainable`: WeaponPart.getMaxUses() = floor(1 / useDelta), no weight update
            item.uses = (Self::drainable_max_uses(script) as f32 * 1.0f32) as i32;
        }
        Ok(Some(item))
    }

    /// FluidContainerScript.getInitialAmount
    fn fluid_initial_amount(&mut self, f: &crate::zombie::scripting::item::FluidScript) -> f32 {
        if !f.initial_amount_set {
            f.capacity
        } else if f.initial_amount_min == f.initial_amount_max {
            f.initial_amount_min
        } else {
            self.rand
                .next_float(f.initial_amount_min, f.initial_amount_max)
        }
    }

    /// DrainableComboItem.getMaxUses
    fn drainable_max_uses(script: &Item) -> i32 {
        (1.0f32 / script_float(script, "useDelta")).floor() as i32
    }

    /// DrainableComboItem.updateWeight (after `setCurrentUses`)
    fn drainable_update_weight(&self, it: &mut LootItem) -> Result<(), String> {
        let script = self.script(it.script)?;
        let use_delta = script_float(script, "useDelta");
        let uses_float = it.uses as f32 * use_delta;
        if let Some(replace) = script_str(script, "replaceOnDeplete") {
            if uses_float >= 1.0 {
                it.actual_weight = script_actual_weight(script);
                return Ok(());
            }
            let full = if replace.contains('.') {
                replace.to_string()
            } else {
                format!("{}.{replace}", script.module)
            };
            if let Some(ei) = self.startup.find_item(&full) {
                let ew = script_actual_weight(self.script(ei)?);
                it.actual_weight = (script_actual_weight(script) - ew) * uses_float + ew;
            }
        }
        let weight_empty = script_float(script, "weightEmpty");
        if weight_empty != 0.0 {
            it.actual_weight =
                (script_actual_weight(script) - weight_empty) * uses_float + weight_empty;
        }
        Ok(())
    }

    /// StashSystem.checkStashItem for an item no stash names.
    fn check_stash_item(&mut self, it: &LootItem) -> Result<(), String> {
        if self.stash.possible == 0 {
            return Ok(());
        }
        let mut chance = 60i32;
        match self.sandbox.get_int("AnnotatedMapChance").unwrap_or(4) {
            1 => return Ok(()),
            2 => chance += 15,
            3 => chance += 10,
            5 => chance -= 10,
            6 => chance -= 20,
            _ => {}
        }
        if self.rand.next(100) <= 100 - chance && self.stash.items.contains(&it.full_type) {
            return Err(unported(format!(
                "StashSystem.checkStashItem for {}",
                it.full_type
            )));
        }
        Ok(())
    }

    /// DrainableComboItem.randomizeUses
    fn randomize_uses(&mut self, it: &mut LootItem) -> Result<(), String> {
        let script = self.script(it.script)?;
        let max_uses = Self::drainable_max_uses(script);
        if max_uses != 1 {
            let mut amount = self.rand.next(max_uses) + 1;
            if has_tag(script, "LessFull") {
                amount = amount.min(self.rand.next(max_uses) + 1);
            }
            if amount <= max_uses && amount > 0 {
                it.uses = amount;
                self.drainable_update_weight(it)?;
            }
        }
        Ok(())
    }

    /// ItemPickerJava.doRollItem / doRollItemInternal. The two `randomizeUses` branches are the original's (the
    /// second is reached when the first condition fails after its draw), so identical bodies stay.
    #[allow(clippy::if_same_then_else)]
    fn do_roll_item_internal(
        &mut self,
        dist: &ItemPickerContainer,
        container: &mut LootContainer,
        zombie_density: f32,
        do_item_container: bool,
        room_dist: Option<&ItemPickerRoom>,
        is_junk: bool,
    ) -> Result<(), String> {
        let _ = do_item_container;
        let mut wet_clothes = false;
        let mut dirty_clothes = false;
        let mut is_empty_fluid_container = false;
        if container.type_name == "clothingdryer" && self.rand.next_bool(5) {
            wet_clothes = true;
        }
        if container.type_name == "clothingwasher" {
            if self.rand.next_bool(2) {
                wet_clothes = true;
            } else {
                dirty_clothes = true;
            }
        }
        let is_trash = dist.is_trash;
        let mut is_shop = false;
        let corpse =
            container.type_name == "inventorymale" || container.type_name == "inventoryfemale";
        if !corpse && !is_trash {
            is_shop = dist.is_shop || self.container_is_shop(container);
            if !is_shop {
                if let Some(rd) = room_dist {
                    is_shop = rd.is_shop;
                }
            }
            if !is_shop {
                if let Some(sq) = container.square {
                    // IsoGridSquare.isShop / IsoRoom.isShop
                    is_shop = self
                        .picker
                        .rooms
                        .get(self.room_name(sq))
                        .is_some_and(|r| r.is_shop);
                }
            }
        }
        let laundry =
            container.type_name == "clothingdryer" || container.type_name == "clothingwasher";
        let rolls_multiplier = match self.sandbox.get("RollsMultiplier") {
            Some(SoValue::Double(d)) => *d,
            _ => 1.0,
        };
        let rolls = ((dist.rolls as f64 * rolls_multiplier) as i32).max(1);
        for _ in 0..rolls {
            for item in &dist.items {
                let mut item_name = item.item_name.clone();
                let mut script_item = self.startup.find_item(&item_name);
                if script_item.is_none() && item_name.ends_with("Empty") {
                    item_name = item_name[..item_name.len() - 5].to_string();
                    script_item = self.startup.find_item(&item_name);
                    if let Some(i) = script_item {
                        if !self.script(i)?.contains_component("FluidContainer") {
                            script_item = None;
                        } else {
                            is_empty_fluid_container = true;
                        }
                    }
                }
                let _ = script_item;
                let draw = self.rand.next(10000);
                let chance = self.actual_spawn_chance(
                    &item.item_name,
                    item.chance,
                    container,
                    zombie_density,
                    is_junk,
                )?;
                if (draw as f32) < chance {
                    let Some(idx) = self.try_add_item_to_container(container, &[], &item_name)?
                    else {
                        return Ok(());
                    };
                    let (class, type_name, full_type) = {
                        let it = &container.items[idx];
                        (it.class, it.type_name.clone(), it.full_type.clone())
                    };
                    let script = self.script(container.items[idx].script)?;
                    // ItemConfigurator.ConfigureItem: the scripts have no item config
                    if !is_shop {
                        // checkStashItem(spawnItem, containerDist): a MapItem never gets here
                        self.check_stash_item(&container.items[idx])?;
                    }
                    // the freezer: Food.isFreezing needs `isPowered()`, false without a parent object
                    if let Some(cat) = script_str(script, "recordedMediaCat") {
                        if Self::recorded_media_random(cat).is_some() {
                            return Err(unported("recorded media data"));
                        }
                        // no media: `container.Remove(spawnItem)`; for "Home-VHS" the "Retail-VHS" fallback finds
                        // none either; both end the roll
                        container.items.remove(idx);
                        return Ok(());
                    }
                    if !dist.no_auto_age {
                        let outermost = container.type_name.clone();
                        self.item_set_auto_age(&mut container.items[idx], &outermost)?;
                    }
                    if !is_trash && self.picker.weapon_upgrade_map.contains_key(&type_name) {
                        self.do_weapon_upgrade(container, idx, &type_name)?;
                    }
                    let drainable = class == ItemType::Drainable;
                    if drainable
                        && has_tag(script, "LessFull")
                        && !is_shop
                        && !is_trash
                        && self.rand.next(100) < 80
                    {
                        self.randomize_uses(&mut container.items[idx])?;
                    } else if drainable && !is_shop && !is_trash && self.rand.next(100) < 40 {
                        self.randomize_uses(&mut container.items[idx])?;
                    }
                    if is_empty_fluid_container && script.contains_component("FluidContainer") {
                        container.items[idx].fluid_amount = Some((0.0, 0.0));
                    }
                    let weapon = class == ItemType::Weapon;
                    let has_sharpness = script_complex(script, "sharpness").is_some();
                    if !is_shop
                        && !is_trash
                        && (has_tag(script, "ShowCondition") || weapon || has_sharpness)
                        && self.rand.next(100) < 40
                    {
                        self.randomize_general_condition(&mut container.items[idx])?;
                    }
                    if !is_trash && weapon {
                        if dist.gun_storage {
                            self.do_gun_storage_container(
                                dist.dont_spawn_ammo,
                                container,
                                &[],
                                idx,
                            )?;
                        } else if !dist.dont_spawn_ammo && self.rand.next(100) < 90 {
                            self.randomize_firearm_as_loot(container, &[], idx)?;
                        }
                    }
                    // `container.Remove(spawnItem)` of a NeverEmpty bag that stayed empty is done after the item's
                    // remaining steps (they draw nothing for a bag; a bag with `spawnWith` is not ported)
                    let mut remove_bag = false;
                    if !is_shop && class == ItemType::Container {
                        if let Some(bags) = dist.bags.as_deref() {
                            if !has_tag(script, "BagsFillException") {
                                self.roll_container_item_internal(
                                    container,
                                    &[idx],
                                    Some(bags),
                                    false,
                                )?;
                            } else {
                                remove_bag = self.fill_bag_from_containers(
                                    container,
                                    idx,
                                    &type_name,
                                    do_item_container,
                                )?;
                            }
                        } else {
                            remove_bag = self.fill_bag_from_containers(
                                container,
                                idx,
                                &type_name,
                                do_item_container,
                            )?;
                        }
                    }
                    if class == ItemType::Food
                        && script_bool(script, "isCookable")
                        && (dist.cook_food || dist.can_burn)
                        && script_complex(script, "replaceoncooked").is_none()
                    {
                        let outermost = container.type_name.clone();
                        self.cook_spawned_food(
                            &mut container.items[idx],
                            dist.can_burn,
                            &outermost,
                        )?;
                    }
                    if dist.is_trash {
                        self.trash_item(&mut container.items[idx])?;
                    }
                    if dist.is_worn {
                        self.wear_down_item(&mut container.items[idx])?;
                    }
                    if dist.is_rotten {
                        self.rot_item(&mut container.items[idx])?;
                    }
                    if has_tag(script, "Regional") && container.square.is_some() {
                        self.on_create_region();
                    }
                    if class == ItemType::Clothing && (dirty_clothes || wet_clothes) {
                        if dirty_clothes {
                            self.clothing_randomize_condition(
                                &mut container.items[idx],
                                0,
                                75,
                                1,
                                0,
                            )?;
                        }
                        if wet_clothes {
                            self.clothing_randomize_condition(
                                &mut container.items[idx],
                                100,
                                0,
                                0,
                                0,
                            )?;
                        }
                    }
                    self.reject_unported_tags(script, &full_type)?;
                    if let Some(sw) = script_str(script, "spawnWith").filter(|s| !s.is_empty()) {
                        if !is_trash && !dist.is_worn {
                            self.spawn_with(container, &[], sw)?;
                        }
                    }
                    if has_tag(script, "SpawnFullUnlessLaundry") && !laundry {
                        self.set_current_uses_to_max(&mut container.items[idx])?;
                    }
                    let corpse_container = false;
                    self.item_spawn_sanity_check(container, idx, !is_trash && !corpse_container)?;
                    if remove_bag {
                        if script_str(script, "spawnWith").is_some_and(|s| !s.is_empty()) {
                            return Err(unported("NeverEmpty bag with spawnWith"));
                        }
                        container.items.remove(idx);
                    }
                    if dist.only_one {
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }

    /// The `else if (containers.containsKey(spawnItem.getType()))` branch of `doRollItemInternal` for a bag in the
    /// root container: a fill of its own `containers` distribution one time in `fillRand` (`doItemContainer`
    /// permitting), then its junk; true when `NeverEmpty` has the empty bag removed.
    fn fill_bag_from_containers(
        &mut self,
        container: &mut LootContainer,
        idx: usize,
        type_name: &str,
        do_item_container: bool,
    ) -> Result<bool, String> {
        let Some(dist) = self.picker.containers.get(type_name) else {
            return Ok(false);
        };
        if do_item_container && self.rand.next(dist.fill_rand) == 0 {
            self.roll_container_item_internal(container, &[idx], Some(dist), false)?;
            if let Some(junk) = dist.junk.as_deref() {
                self.roll_container_item_internal(container, &[idx], Some(junk), true)?;
            }
            let script = self.script(container.items[idx].script)?;
            let empty = container.items[idx]
                .inner
                .as_ref()
                .is_some_and(|c| c.items.is_empty());
            if has_tag(script, "NeverEmpty") && empty {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The item tags whose handling in the rolls is not ported (each returns `not ported`).
    fn reject_unported_tags(&self, script: &Item, full_type: &str) -> Result<(), String> {
        for tag in ["KeyRing", "CarKey", "BuildingKey"] {
            if has_tag(script, tag) {
                return Err(unported(format!("item tag {tag} ({full_type})")));
            }
        }
        Ok(())
    }

    /// `spawnItem.setCurrentUses(spawnItem.getMaxUses())` (SpawnFullUnlessLaundry).
    fn set_current_uses_to_max(&self, it: &mut LootItem) -> Result<(), String> {
        let script = self.script(it.script)?;
        if it.class == ItemType::Food {
            return Err(unported("Food.setCurrentUses (consumeHunger)"));
        }
        if it.class == ItemType::Drainable {
            it.uses = Self::drainable_max_uses(script);
            self.drainable_update_weight(it)?;
        } else {
            it.uses = 1;
        }
        Ok(())
    }

    /// InventoryItem.randomizeGeneralCondition: `randomizeCondition` (a HandWeapon only with a `PhysicsObject`),
    /// `randomizeHeadCondition` (not ported: items with a head condition never get here), `randomizeSharpness`.
    fn randomize_general_condition(&mut self, it: &mut LootItem) -> Result<(), String> {
        let script = self.script(it.script)?;
        let weapon = it.class == ItemType::Weapon;
        let condition_max = script_int(script, "conditionMax");
        if condition_max <= 0 {
            return Err(unported("randomizeGeneralCondition with ConditionMax 0"));
        }
        if !weapon || script_str(script, "physicsObject").is_some() {
            // setCondition(con, false) keeps 0..=conditionMax
            it.condition = 1
                .max(self.rand.next(condition_max + 1))
                .clamp(0, condition_max);
        }
        if it.head_condition.is_some() {
            // randomizeHeadCondition: the original sets the item's *condition* again (not the head condition)
            it.condition = 1
                .max(self.rand.next(condition_max + 1))
                .clamp(0, condition_max);
        }
        if script_complex(script, "sharpness").is_some() {
            // randomizeSharpness: setSharpness(Rand.Next(0, getMaxSharpness())) clamps to 0..=max (and 1)
            let max_sharpness = self.max_sharpness(it)?;
            let v = self.rand.next_float(0.0, max_sharpness);
            it.sharpness = Some(v.min(max_sharpness).clamp(0.0, 1.0));
        }
        Ok(())
    }

    /// InventoryItem.getHeadConditionMax: 0 without a head condition; the `HeadConditionMax` attribute, else
    /// `getConditionMax()`.
    fn head_condition_max(&self, it: &LootItem) -> Result<i32, String> {
        if it.head_condition.is_none() {
            return Ok(0);
        }
        Ok(match it.head_condition_max {
            Some(m) => m,
            None => script_int(self.script(it.script)?, "conditionMax"),
        })
    }

    /// InventoryItem.getMaxSharpness: 1 without a sharpness attribute; `headCondition / headConditionMax` for an
    /// item with a head condition, else `condition / conditionMax`.
    fn max_sharpness(&self, it: &LootItem) -> Result<f32, String> {
        if it.sharpness.is_none() {
            return Ok(1.0);
        }
        if let Some(h) = it.head_condition {
            return Ok(h as f32 / self.head_condition_max(it)? as f32);
        }
        Ok(it.condition as f32 / script_int(self.script(it.script)?, "conditionMax") as f32)
    }

    /// ItemPickerJava.itemSpawnSanityCheck(spawnItem[, container]): in a shop a bag is emptied unless it
    /// `AlwaysHasStuff`; a bag is marked explored; a depleted drainable that disappears on use gets one use.
    fn item_spawn_sanity_check(
        &mut self,
        container: &mut LootContainer,
        idx: usize,
        with_container: bool,
    ) -> Result<(), String> {
        let script = self.script(container.items[idx].script)?;
        let shop = with_container && self.container_is_shop(container);
        let it = &mut container.items[idx];
        self.sanity_check_item(it, script, shop)
    }

    fn sanity_check_item(
        &self,
        it: &mut LootItem,
        script: &Item,
        shop: bool,
    ) -> Result<(), String> {
        if let Some(inner) = it.inner.as_deref_mut() {
            if shop && !has_tag(script, "AlwaysHasStuff") {
                inner.items.clear();
            }
            inner.explored = true;
        }
        if has_tag(script, "SpawnCooked") {
            if let Some(food) = it.food.as_mut() {
                food.spawn_cooked(script_str(script, "onCooked"))?;
            }
        }
        // unsealIfNotFull: a fluid container's seal state only
        if it.class == ItemType::Drainable
            && script_bool(script, "disappearOnUse")
            && self.item_current_uses(it)? <= 0
        {
            it.uses = 1;
            self.drainable_update_weight(it)?;
        }
        Ok(())
    }

    /// ItemPickerJava.rollContainerItemInternal(pickInfo, bag, character = null, containerDist, isJunk): the loot of
    /// a bag (the item at `bag` in `root`), rolled into its own container. The two `randomizeUses` branches are the
    /// original's, so identical bodies stay.
    #[allow(clippy::if_same_then_else)]
    fn roll_container_item_internal(
        &mut self,
        root: &mut LootContainer,
        bag: &[usize],
        dist: Option<&ItemPickerContainer>,
        is_junk: bool,
    ) -> Result<(), String> {
        let Some(dist) = dist else {
            return Ok(());
        };
        // ItemPickerJava.getZombieDensityFactor(containerDist, container): no meta chunk, so 0
        let zombie_density = self.zombie_density_factor();
        let rolls_multiplier = match self.sandbox.get("RollsMultiplier") {
            Some(SoValue::Double(d)) => *d,
            _ => 1.0,
        };
        let rolls = ((dist.rolls as f64 * rolls_multiplier) as i32).max(1);
        for _ in 0..rolls {
            for item in &dist.items {
                let mut item_name = item.item_name.clone();
                let mut script_item = self.startup.find_item(&item_name);
                let mut is_empty_fluid_container = false;
                if script_item.is_none() && item_name.ends_with("Empty") {
                    item_name = item_name[..item_name.len() - 5].to_string();
                    script_item = self.startup.find_item(&item_name);
                    if let Some(i) = script_item {
                        if !self.script(i)?.contains_component("FluidContainer") {
                            continue;
                        }
                        is_empty_fluid_container = true;
                    }
                }
                let draw = self.rand.next(10000);
                let chance = {
                    let inner = container_at(root, bag);
                    self.actual_spawn_chance(
                        &item.item_name,
                        item.chance,
                        inner,
                        zombie_density,
                        is_junk,
                    )?
                };
                if (draw as f32) >= chance {
                    continue;
                }
                let Some(idx) = self.try_add_item_to_container(root, bag, &item_name)? else {
                    return Ok(());
                };
                let mut path = bag.to_vec();
                path.push(idx);
                let mut remove_bag = false;
                let (class, type_name, full_type, script_idx) = {
                    let it = &container_at(root, bag).items[idx];
                    (
                        it.class,
                        it.type_name.clone(),
                        it.full_type.clone(),
                        it.script,
                    )
                };
                let script = self.script(script_idx)?;
                // ItemConfigurator.ConfigureItem: no item config; a MapItem is not ported
                self.check_stash_item(&container_at(root, bag).items[idx])?;
                if !dist.is_trash && self.picker.weapon_upgrade_map.contains_key(&type_name) {
                    self.do_weapon_upgrade(container_at_mut(root, bag), idx, &type_name)?;
                }
                let weapon = class == ItemType::Weapon;
                if !dist.is_trash && weapon && !dist.dont_spawn_ammo && self.rand.next(100) < 90 {
                    self.randomize_firearm_as_loot(root, bag, idx)?;
                }
                let has_sharpness = script_complex(script, "sharpness").is_some();
                if !dist.is_trash && (has_tag(script, "ShowCondition") || weapon || has_sharpness) {
                    let target = container_at_mut(root, bag);
                    self.randomize_general_condition(&mut target.items[idx])?;
                }
                // the freezer: a bag's container is never "freezer"
                let drainable = class == ItemType::Drainable;
                if drainable && has_tag(script, "LessFull") && self.rand.next(100) < 80 {
                    let target = container_at_mut(root, bag);
                    self.randomize_uses(&mut target.items[idx])?;
                } else if drainable && self.rand.next(100) < 40 {
                    let target = container_at_mut(root, bag);
                    self.randomize_uses(&mut target.items[idx])?;
                }
                if is_empty_fluid_container && script.contains_component("FluidContainer") {
                    container_at_mut(root, bag).items[idx].fluid_amount = Some((0.0, 0.0));
                }
                if has_tag(script, "BuildingKey") {
                    return Err(unported("BuildingKey in a bag"));
                }
                if let Some(cat) = script_str(script, "recordedMediaCat") {
                    if Self::recorded_media_random(cat).is_some() {
                        return Err(unported("recorded media data"));
                    }
                    // no media: `container.Remove(spawnItem)`; for "Home-VHS" the "Retail-VHS" fallback finds
                    // none either; both end the roll
                    container_at_mut(root, bag).items.remove(idx);
                    return Ok(());
                }
                if class == ItemType::Container {
                    if let Some(bags) = dist.bags.as_deref() {
                        if !has_tag(script, "BagsFillException") {
                            self.roll_container_item_internal(root, &path, Some(bags), false)?;
                        } else {
                            remove_bag = self.fill_nested_bag(root, &path, &type_name)?;
                        }
                    } else {
                        remove_bag = self.fill_nested_bag(root, &path, &type_name)?;
                    }
                }
                // `if (!container.getType().equals("freezer")) setAutoAge()`: a bag's container is never a freezer;
                // the food's outermost container is the one the bag lies in
                let outermost = root.type_name.clone();
                self.item_set_auto_age(&mut container_at_mut(root, bag).items[idx], &outermost)?;
                if dist.is_trash {
                    self.trash_item(&mut container_at_mut(root, bag).items[idx])?;
                }
                if dist.is_worn {
                    self.wear_down_item(&mut container_at_mut(root, bag).items[idx])?;
                }
                if has_tag(script, "Regional") && root.square.is_some() {
                    self.on_create_region();
                }
                self.reject_unported_tags(script, &full_type)?;
                if let Some(sw) = script_str(script, "spawnWith").filter(|s| !s.is_empty()) {
                    if !dist.is_worn && !dist.is_trash {
                        self.spawn_with(root, bag, sw)?;
                    }
                }
                if has_tag(script, "SpawnFullUnlessLaundry") {
                    let target = container_at_mut(root, bag);
                    self.set_current_uses_to_max(&mut target.items[idx])?;
                }
                {
                    let target = container_at_mut(root, bag);
                    self.sanity_check_item(&mut target.items[idx], script, false)?;
                }
                if remove_bag {
                    if script_str(script, "spawnWith").is_some_and(|s| !s.is_empty()) {
                        return Err(unported("NeverEmpty bag with spawnWith"));
                    }
                    container_at_mut(root, bag).items.remove(idx);
                }
                if dist.only_one {
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    /// The `else if (containers.containsKey(spawnItem.getType()))` branch of `rollContainerItemInternal` (no
    /// `fillRand` chance here): the nested bag's own `containers` fill and junk; true when `NeverEmpty` has the
    /// empty bag removed.
    fn fill_nested_bag(
        &mut self,
        root: &mut LootContainer,
        path: &[usize],
        type_name: &str,
    ) -> Result<bool, String> {
        let Some(dist) = self.picker.containers.get(type_name) else {
            return Ok(false);
        };
        self.roll_container_item_internal(root, path, Some(dist), false)?;
        if let Some(junk) = dist.junk.as_deref() {
            self.roll_container_item_internal(root, path, Some(junk), true)?;
        }
        let (parent, last) = path.split_at(path.len() - 1);
        let item = &container_at(root, parent).items[last[0]];
        let script = self.script(item.script)?;
        Ok(
            has_tag(script, "NeverEmpty")
                && item.inner.as_ref().is_some_and(|c| c.items.is_empty()),
        )
    }

    /// The `ITEM` lines of the oracle for the items of `c`: `ITEM <path> <type> id= uses= cond= weight= sharp=` (the
    /// weights and the sharpness as float bits), and for a bag its capacities, weight reduction, item count,
    /// contents and unequipped weight and explored flag, followed by the lines of its own items (path `n.m`).
    pub fn item_lines(
        &self,
        prefix: &str,
        c: &LootContainer,
        out: &mut String,
    ) -> Result<(), String> {
        use std::fmt::Write;
        for (n, i) in c.items.iter().enumerate() {
            let path = if prefix.is_empty() {
                n.to_string()
            } else {
                format!("{prefix}.{n}")
            };
            let weight = self.item_actual_weight(i)?;
            let sharp = self.item_sharpness(i)?;
            let uses = self.item_current_uses(i)?;
            let _ = write!(
                out,
                "ITEM {path} {} id={} uses={uses} cond={} weight={:x} sharp={:x}",
                i.full_type,
                i.id,
                i.condition,
                weight.to_bits(),
                sharp.to_bits()
            );
            if let Some(h) = i.head_condition {
                let _ = write!(out, " head={h} hmax={}", self.head_condition_max(i)?);
            }
            if i.max_ammo > 0 || i.weapon.is_some() {
                let (ulo, uhi) = self.unequipped_weight(i)?;
                if ulo != uhi {
                    return Err(unported("a weapon's weight depends on a fluid amount"));
                }
                let _ = write!(
                    out,
                    " ammo={}/{} uw={:x}",
                    i.current_ammo,
                    i.max_ammo,
                    ulo.to_bits()
                );
                if let Some(w) = &i.weapon {
                    let _ = write!(
                        out,
                        " clip={} ch={}",
                        u8::from(w.contains_clip),
                        u8::from(w.round_chambered)
                    );
                }
            }
            if let Some(f) = &i.food {
                let _ = write!(
                    out,
                    " fd age={:x} off={}/{} ck={}{} ct={:x} hung={:x} thirst={:x} end={:x} bore={:x} unh={:x} str={:x} fat={:x} flu={} sick={} pain={:x} kcal={:x} carb={:x} prot={:x} lip={:x} pois={}",
                    f.age.to_bits(),
                    f.off_age,
                    f.off_age_max,
                    u8::from(f.cooked),
                    u8::from(f.burnt),
                    f.cooking_time.to_bits(),
                    f.hung_change.to_bits(),
                    f.thirst_change.to_bits(),
                    f.end_change.to_bits(),
                    f.boredom_change.to_bits(),
                    f.unhappy_change.to_bits(),
                    f.stress_change.to_bits(),
                    f.fatigue_change.to_bits(),
                    f.flu_reduction,
                    f.food_sickness(),
                    f.pain_reduction.to_bits(),
                    f.calories.to_bits(),
                    f.carbohydrates.to_bits(),
                    f.proteins.to_bits(),
                    f.lipids.to_bits(),
                    f.poison_power
                );
            }
            if let Some(inner) = &i.inner {
                let (clo, chi) = self.bag_contents_weight(i)?;
                let (ulo, uhi) = self.unequipped_weight(i)?;
                if clo != chi || ulo != uhi {
                    return Err(unported("a bag's weight depends on a fluid amount"));
                }
                let mut cap = inner.capacity.min(50);
                if cap as f32 > 50.0f32 - i.actual_weight {
                    cap = (50.0f32 - i.actual_weight) as i32;
                }
                let _ = write!(
                    out,
                    " cap={cap} icap={} red={} nin={} cw={:x} uw={:x} exp={}",
                    inner.capacity.min(50),
                    i.weight_reduction,
                    inner.items.len(),
                    clo.to_bits(),
                    ulo.to_bits(),
                    u8::from(inner.explored)
                );
            }
            out.push('\n');
            if let Some(inner) = &i.inner {
                self.item_lines(&path, inner, out)?;
            }
        }
        Ok(())
    }

    /// `InventoryContainer.getContentsWeight`: its container's contents weight plus its fluid.
    fn bag_contents_weight(&self, it: &LootItem) -> Result<(f32, f32), String> {
        let (flo, fhi) = it.fluid_amount.unwrap_or((0.0, 0.0));
        let Some(inner) = &it.inner else {
            return self.item_contents_weight(it);
        };
        let (mut lo, mut hi) = self.contents_weight(inner)?;
        if it.fluid_amount.is_some() {
            lo += flo;
            hi += fhi;
        }
        Ok((lo, hi))
    }

    /// InventoryItem.getActualWeight of an item of the container (what `weight` prints).
    pub fn item_actual_weight(&self, it: &LootItem) -> Result<f32, String> {
        self.actual_weight(it)
    }

    /// InventoryItem.getCurrentUses and its Food override (`(int)Math.abs(hungChange * 100)` once the food has a
    /// base hunger).
    pub fn item_current_uses(&self, it: &LootItem) -> Result<i32, String> {
        if let Some(food) = &it.food {
            if food.base_hunger != 0.0 {
                return Ok((food.hung_change * 100.0f32).abs() as i32);
            }
        }
        Ok(it.uses)
    }

    /// InventoryItem.getSharpness: the attribute capped at `getMaxSharpness()` (condition over `ConditionMax`), 0
    /// without a `Sharpness` attribute.
    pub fn item_sharpness(&self, it: &LootItem) -> Result<f32, String> {
        let Some(v) = it.sharpness else {
            return Ok(0.0);
        };
        let max = self.max_sharpness(it)?;
        Ok(if v > max { max } else { v })
    }

    /// ItemContainer.isShop for a container on a square (`getParent()` is null).
    fn container_is_shop(&self, container: &LootContainer) -> bool {
        let Some(sq) = container.square else {
            return false;
        };
        let room_name = self.room_name(sq);
        // IsoGridSquare.isShop -> IsoRoom.isShop
        if self.picker.rooms.get(room_name).is_some_and(|r| r.is_shop) {
            return true;
        }
        if let Some(rd) = self.picker.rooms.get(room_name) {
            if rd.is_shop {
                return true;
            }
            let mut cd = rd.containers.get(&container.type_name);
            if cd.is_none() && rd.containers.contains_key("other") {
                cd = rd.containers.get("other");
            }
            if cd.is_none() && rd.containers.contains_key("all") {
                cd = rd.containers.get("all");
            }
            if cd.is_some_and(|c| c.is_shop) {
                return true;
            }
        }
        false
    }
}

fn room_name_or_all(room_name: &str) -> String {
    room_name.to_string()
}
