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
    /// `actualWeight` of the instance (a drainable's `updateWeight` changes it).
    pub actual_weight: f32,
    /// `getDisplayName().equals(getFullType())`: the translation is missing, the weight reads as 0.
    pub zero_weight: bool,
    /// What `getFluidContainer().getAmount()` can be: a lower and an upper bound (equal when known); `None` without
    /// a FluidContainer component.
    pub fluid_amount: Option<(f32, f32)>,
}

/// An ItemContainer: the type, the capacity set on it, its items and the square it was made for (`sourceGrid`).
#[derive(Clone, Debug)]
pub struct LootContainer {
    pub type_name: String,
    pub capacity: i32,
    pub items: Vec<LootItem>,
    pub square: Option<LootSquare>,
}

impl LootContainer {
    pub fn new(type_name: &str, capacity: i32, square: Option<LootSquare>) -> Self {
        LootContainer {
            type_name: type_name.to_string(),
            capacity,
            items: Vec::new(),
            square,
        }
    }
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

    /// The item's contribution to `ItemContainer.getContentsWeight` (`getUnequippedWeight`): a lower and an upper
    /// bound.
    fn unequipped_weight(&self, it: &LootItem) -> Result<(f32, f32), String> {
        let actual = self.actual_weight(it)?;
        let (lo, hi) = it.fluid_amount.unwrap_or((0.0, 0.0));
        Ok((actual + lo, actual + hi))
    }

    /// InventoryItem.getActualWeight and its Food override.
    fn actual_weight(&self, it: &LootItem) -> Result<f32, String> {
        let script = self.script(it.script)?;
        if it.class == ItemType::Weapon {
            // HandWeapon.getActualWeight: the script's weight plus attached parts (none)
            return Ok(script_actual_weight(script));
        }
        let base = if it.zero_weight {
            0.0
        } else {
            it.actual_weight.max(0.0)
        };
        if it.class != ItemType::Food {
            return Ok(base);
        }
        // Food.getActualWeight for a fresh item (no extra items, not custom weight, not cooked or burnt)
        let hung_change = script_float(script, "hungerChange") / 100.0f32;
        let thirst_change = script_float(script, "thirstChange") / 100.0f32;
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

    /// ItemContainer.hasRoomFor(null, weightVal): `floatingPointCorrection(contents) + weightVal <= capacity`. A
    /// FluidContainer item's contents weight is only bounded here; the answer must not depend on where in the
    /// bounds it lies.
    fn has_room_for(&self, container: &LootContainer, weight_val: f32) -> Result<bool, String> {
        let (mut lo, mut hi) = (0.0f32, 0.0f32);
        for it in &container.items {
            let (a, b) = self.unequipped_weight(it)?;
            lo += a;
            hi += b;
        }
        let cap = container.capacity.min(100) as f32;
        let a = floating_point_correction(lo) + weight_val <= cap;
        let b = floating_point_correction(hi) + weight_val <= cap;
        if a != b {
            return Err(unported(
                "hasRoomFor depends on the amount of a fluid container (only bounded)",
            ));
        }
        Ok(a)
    }

    /// ItemPickerJava.tryAddItemToContainer: `Ok(Some(index))` of the new item in the container.
    fn try_add_item_to_container(
        &mut self,
        container: &mut LootContainer,
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
        if !self.has_room_for(container, total_weight)? {
            return Ok(None);
        }
        // `container.getContainingItem()` is null: no bag checks
        self.spawn_item(item_type, container)
    }

    /// ItemSpawner.spawnItem(String, ItemContainer): `InventoryItemFactory.CreateItem`, `isItemAllowed`, `AddItem`.
    fn spawn_item(
        &mut self,
        item_type: &str,
        container: &mut LootContainer,
    ) -> Result<Option<usize>, String> {
        let Some(item) = self.create_item(item_type)? else {
            return Ok(None);
        };
        // isItemAllowed: no accepted category or function, no parent object
        // ItemContainer.AddItem refuses an item whose id the container holds (and returns the held one)
        if container.items.iter().any(|i| i.id == item.id) {
            return Err(unported("a spawned item id equal to one in the container"));
        }
        container.items.push(item);
        Ok(Some(container.items.len() - 1))
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
        ) {
            return Err(unported(format!(
                "item class {class:?} ({})",
                script.full_name()
            )));
        }
        for c in &script.components {
            if c != "FluidContainer" && c != "Durability" {
                return Err(unported(format!("component {c} ({})", script.full_name())));
            }
        }
        if script.fluid_blocks > 1 {
            return Err(unported("two FluidContainer blocks"));
        }
        if class == ItemType::Weapon
            && (script_bool(script, "ranged")
                || script_str(script, "magazineType").is_some_and(|m| !m.is_empty())
                || script_str(script, "ammoBox").is_some_and(|m| !m.is_empty()))
        {
            return Err(unported(format!("firearm {}", script.full_name())));
        }
        for key in ["ammotype", "headcondition", "headconditionmax"] {
            if script_complex(script, key).is_some() {
                return Err(unported(format!(
                    "script parameter {key} ({})",
                    script.full_name()
                )));
            }
        }
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
            actual_weight: if script_weight < 0.0 {
                0.0
            } else {
                script_weight
            },
            zero_weight: false,
            fluid_amount: None,
        };
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

    /// HandWeapon.randomizeFirearmAsLoot for a weapon with no magazine type, no ammo box and no chamber.
    fn randomize_firearm_as_loot(&mut self, script: &Item) -> Result<(), String> {
        // `reloadDifficulty > 1 && !isNullOrEmpty(magazineType) && ...`: no magazine type
        if self.reload_difficulty == 1 || self.rand.next(100) < 30 {
            let max_ammo = script_int(script, "maxAmmo");
            self.rand.next_range(1, max_ammo);
            // `if (haveChamber()) setRoundChambered(true)`: the round weighs only with an ammo type (gated out)
        }
        // `getContainer() != null && !isNullOrEmpty(getAmmoBox())`: no ammo box (the class gate)
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
        let _ = (wet_clothes, dirty_clothes);
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
                    let Some(idx) = self.try_add_item_to_container(container, &item_name)? else {
                        return Ok(());
                    };
                    let mut spawn = container.items[idx].clone();
                    let script = self.script(spawn.script)?;
                    // ItemConfigurator.ConfigureItem: the scripts have no item config
                    if !is_shop {
                        // checkStashItem(spawnItem, containerDist): a MapItem never gets here
                        self.check_stash_item(&spawn)?;
                    }
                    // the freezer: Food.isFreezing needs `isPowered()`, false without a parent object
                    if script_str(script, "recordedMediaCat").is_some() {
                        return Err(unported("recorded media"));
                    }
                    // setAutoAge: state only
                    if !is_trash
                        && self
                            .picker
                            .weapon_upgrade_map
                            .contains_key(&spawn.type_name)
                    {
                        return Err(unported("DoWeaponUpgrade"));
                    }
                    let drainable = spawn.class == ItemType::Drainable;
                    if drainable
                        && has_tag(script, "LessFull")
                        && !is_shop
                        && !is_trash
                        && self.rand.next(100) < 80
                    {
                        self.randomize_uses(&mut spawn)?;
                    } else if drainable && !is_shop && !is_trash && self.rand.next(100) < 40 {
                        self.randomize_uses(&mut spawn)?;
                    }
                    if is_empty_fluid_container && script.contains_component("FluidContainer") {
                        spawn.fluid_amount = Some((0.0, 0.0));
                    }
                    let weapon = spawn.class == ItemType::Weapon;
                    let has_sharpness = script_complex(script, "sharpness").is_some();
                    if !is_shop
                        && !is_trash
                        && (has_tag(script, "ShowCondition") || weapon || has_sharpness)
                        && self.rand.next(100) < 40
                    {
                        // InventoryItem.randomizeGeneralCondition: randomizeCondition (a HandWeapon without a
                        // `physicsObject` skips it), no head condition, randomizeSharpness
                        let condition_max = script_int(script, "conditionMax");
                        if condition_max <= 0 {
                            return Err(unported("randomizeGeneralCondition with ConditionMax 0"));
                        }
                        if !weapon || script_str(script, "physicsObject").is_some() {
                            // setCondition(con, false) keeps 0..=conditionMax
                            spawn.condition = 1
                                .max(self.rand.next(condition_max + 1))
                                .clamp(0, condition_max);
                        }
                        if has_sharpness {
                            // getMaxSharpness: condition / conditionMax; setSharpness clamps to 0..=max (and 1)
                            let max_sharpness = spawn.condition as f32 / condition_max as f32;
                            let v = self.rand.next_float(0.0, max_sharpness);
                            let v = v.min(max_sharpness).clamp(0.0, 1.0);
                            spawn.sharpness = Some(v);
                        }
                    }
                    if !is_trash && weapon {
                        if dist.gun_storage {
                            return Err(unported("doGunStorageContainer"));
                        } else if !dist.dont_spawn_ammo && self.rand.next(100) < 90 {
                            self.randomize_firearm_as_loot(script)?;
                        }
                    }
                    if spawn.class == ItemType::Food
                        && script_bool(script, "isCookable")
                        && (dist.cook_food || dist.can_burn)
                        && script_complex(script, "replaceoncooked").is_none()
                    {
                        return Err(unported("cooking spawned food"));
                    }
                    if dist.is_trash || dist.is_worn || dist.is_rotten {
                        return Err(unported("trashItem / wearDownItem / rotItem"));
                    }
                    for tag in [
                        "Regional",
                        "KeyRing",
                        "CarKey",
                        "BuildingKey",
                        "SpawnCooked",
                        "ApplyOwnerName",
                        "MonogramOwnerName",
                        "AlwaysHasStuff",
                        "NeverEmpty",
                        "BagsFillException",
                    ] {
                        if has_tag(script, tag) {
                            return Err(unported(format!("item tag {tag} ({})", spawn.full_type)));
                        }
                    }
                    if script_str(script, "spawnWith").is_some_and(|s| !s.is_empty())
                        && !is_trash
                        && !dist.is_worn
                    {
                        return Err(unported("spawnWith"));
                    }
                    if has_tag(script, "SpawnFullUnlessLaundry") && !laundry {
                        if spawn.class == ItemType::Food {
                            return Err(unported("Food.setCurrentUses (consumeHunger)"));
                        }
                        if drainable {
                            spawn.uses = Self::drainable_max_uses(script);
                            self.drainable_update_weight(&mut spawn)?;
                        } else {
                            spawn.uses = 1;
                        }
                    }
                    if drainable && spawn.uses <= 0 {
                        return Err(unported("itemSpawnSanityCheck of a depleted drainable"));
                    }
                    container.items[idx] = spawn;
                    if dist.only_one {
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }

    /// InventoryItem.getActualWeight of an item of the container (what `weight` prints).
    pub fn item_actual_weight(&self, it: &LootItem) -> Result<f32, String> {
        self.actual_weight(it)
    }

    /// InventoryItem.getCurrentUses and its Food override (`(int)Math.abs(hungChange * 100)` once the food has a
    /// base hunger).
    pub fn item_current_uses(&self, it: &LootItem) -> Result<i32, String> {
        if it.class == ItemType::Food {
            let base_hunger = script_float(self.script(it.script)?, "hungerChange") / 100.0f32;
            if base_hunger != 0.0 {
                return Ok((base_hunger * 100.0f32).abs() as i32);
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
        let max = it.condition as f32 / script_int(self.script(it.script)?, "conditionMax") as f32;
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
