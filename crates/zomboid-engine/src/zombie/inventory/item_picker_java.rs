//! zombie.inventory.ItemPickerJava: the loot tables the game builds from the Lua distributions and the container
//! filling that reads them. This stage: `Parse` (the Lua tables `SuburbsDistributions`, `ProceduralDistributions`,
//! `VehicleDistributions`, `NoContainerFillRooms` and `WeaponUpgrades` as the ported Lua VM holds them) and the
//! sandbox loot modifiers. The original's `THashMap`s (`rooms`, `containers`, ...) are `HashMap`s here: their own
//! iteration order is not observable through the class (the oracle prints them sorted by key).
use std::collections::{HashMap, HashSet};

use super::inventory_item_factory::get_item;
use crate::java;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::object::{cast_string, rstr, Object, TableRef, Throwable, R};
use crate::zombie::game_window::ClientStartup;
use crate::zombie::sandbox_options::{SandboxOptions, Value};
use crate::zombie::util::string_utils;

/// zombie.inventory.ItemPickerJava.ItemPickerItem
#[derive(Clone, Debug, PartialEq)]
pub struct ItemPickerItem {
    pub item_name: String,
    pub chance: f32,
}

/// zombie.inventory.ItemPickerJava.ProceduralItem
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProceduralItem {
    pub name: Option<String>,
    pub min: i32,
    pub max: i32,
    pub force_for_items: Option<Vec<String>>,
    pub force_for_zones: Option<Vec<String>>,
    pub force_for_tiles: Option<Vec<String>>,
    pub force_for_rooms: Option<Vec<String>>,
    pub weight_chance: i32,
}

/// zombie.inventory.ItemPickerJava.ItemPickerContainer
#[derive(Clone, Debug, PartialEq)]
pub struct ItemPickerContainer {
    pub items: Vec<ItemPickerItem>,
    /// `float rolls` (assigned from an int)
    pub rolls: f32,
    pub no_auto_age: bool,
    pub is_shop: bool,
    pub fill_rand: i32,
    pub max_map: i32,
    pub stash_chance: i32,
    pub junk: Option<Box<ItemPickerContainer>>,
    pub bags: Option<Box<ItemPickerContainer>>,
    pub procedural: bool,
    pub dont_spawn_ammo: bool,
    pub gun_storage: bool,
    pub ignore_zombie_density: bool,
    pub cook_food: bool,
    pub can_burn: bool,
    pub is_trash: bool,
    pub is_worn: bool,
    pub is_rotten: bool,
    pub only_one: bool,
    pub default_inventory_loot: bool,
    pub procedural_items: Option<Vec<ProceduralItem>>,
}

impl Default for ItemPickerContainer {
    fn default() -> Self {
        ItemPickerContainer {
            items: Vec::new(),
            rolls: 0.0,
            no_auto_age: false,
            is_shop: false,
            fill_rand: 0,
            max_map: 0,
            stash_chance: 0,
            junk: None,
            bags: None,
            procedural: false,
            dont_spawn_ammo: false,
            gun_storage: false,
            ignore_zombie_density: false,
            cook_food: false,
            can_burn: false,
            is_trash: false,
            is_worn: false,
            is_rotten: false,
            only_one: false,
            default_inventory_loot: true,
            procedural_items: None,
        }
    }
}

/// zombie.inventory.ItemPickerJava.ItemPickerRoom
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemPickerRoom {
    pub containers: HashMap<String, ItemPickerContainer>,
    pub fill_rand: i32,
    pub is_shop: bool,
    pub specific_id: Option<String>,
    pub profession_chance: i32,
    pub outfit: Option<String>,
    pub outfit_female: Option<String>,
    pub outfit_male: Option<String>,
    pub outfit_chance: Option<String>,
    pub vehicle: Option<String>,
    pub vehicles: Option<Vec<String>>,
    pub vehicle_chance: Option<String>,
    pub vehicle_distribution: Option<String>,
    pub vehicle_skin: Option<i32>,
    pub female_chance: Option<String>,
    pub room_types: Option<String>,
    pub zone_requires: Option<String>,
    pub zone_disallows: Option<String>,
    pub container_chance: Option<String>,
    pub female_odds: Option<String>,
    pub bag_type: Option<String>,
    pub bag_table: Option<String>,
    pub profession_chance_int: i32,
}

/// zombie.inventory.ItemPickerJava.ItemPickerUpgradeWeapons
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemPickerUpgradeWeapons {
    pub name: String,
    pub upgrades: Vec<String>,
}

/// zombie.inventory.ItemPickerJava.VehicleDistribution
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VehicleDistribution {
    pub normal: Option<ItemPickerRoom>,
    pub specific: Vec<ItemPickerRoom>,
}

/// The loot modifiers `InitSandboxLootSettings` sets (floats, in the order of the original's fields).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LootModifiers {
    pub other: f32,
    pub food: f32,
    pub canned_food: f32,
    pub weapon: f32,
    pub ranged_weapon: f32,
    pub ammo: f32,
    pub literature: f32,
    pub survival_gears: f32,
    pub medical: f32,
    pub bag: f32,
    pub mechanics: f32,
    pub clothing: f32,
    pub container: f32,
    pub key: f32,
    pub key_d100: f32,
    pub media: f32,
    pub memento: f32,
    pub cookware: f32,
    pub material: f32,
    pub farming: f32,
    pub tool: f32,
    pub skill_book: f32,
    pub recipe_resource: f32,
}

/// zombie.inventory.ItemPickerJava's static state.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemPickerJava {
    pub modifiers: LootModifiers,
    /// `zombieDensityCap`
    pub zombie_density_cap: f32,
    pub no_container_fill_rooms: Vec<String>,
    pub weapon_upgrades: Vec<ItemPickerUpgradeWeapons>,
    /// `WeaponUpgradeMap`: the name to its index in `weapon_upgrades` (the original maps to the same object).
    pub weapon_upgrade_map: HashMap<String, usize>,
    pub rooms: HashMap<String, ItemPickerRoom>,
    pub containers: HashMap<String, ItemPickerContainer>,
    pub procedural_distributions: HashMap<String, ItemPickerContainer>,
    pub vehicle_distributions: HashMap<String, VehicleDistribution>,
    /// `addedInvalidAlready` (filled only with Core.debug on)
    pub added_invalid_already: Vec<String>,
    /// The item scripts `Parse` marked `setCanSpawnAsLoot(true)` (indexes into the scripts' items).
    pub can_spawn_as_loot: HashSet<usize>,
}

impl Default for ItemPickerJava {
    fn default() -> Self {
        ItemPickerJava {
            modifiers: LootModifiers::default(),
            zombie_density_cap: 8.0,
            no_container_fill_rooms: Vec::new(),
            weapon_upgrades: Vec::new(),
            weapon_upgrade_map: HashMap::new(),
            rooms: HashMap::new(),
            containers: HashMap::new(),
            procedural_distributions: HashMap::new(),
            vehicle_distributions: HashMap::new(),
            added_invalid_already: Vec::new(),
            can_spawn_as_loot: HashSet::new(),
        }
    }
}

/// `(KahluaTableImpl) value`: a table, else the ClassCastException.
fn cast_table(o: &Object) -> R<TableRef> {
    match o {
        Object::Table(t) => Ok(t.clone()),
        _ => Err(Throwable::class_cast(
            o,
            "se.krka.kahlua.j2se.KahluaTableImpl",
        )),
    }
}

/// `(String) value`: the string or null (nil), else the ClassCastException.
fn cast_str(o: &Object) -> R<Option<String>> {
    Ok(cast_string(o)?.map(|s| rstr(&s)))
}

/// `table.delegate.containsKey(key)`
fn contains_key(t: &TableRef, key: &str) -> bool {
    !t.borrow().get_direct(&Object::str(key)).is_nil()
}

/// `table.delegate.get(Double key)`
fn get_number_key(t: &TableRef, key: i32) -> Object {
    t.borrow().get_direct(&Object::Number(f64::from(key)))
}

impl ItemPickerJava {
    /// zombie.inventory.ItemPickerJava.Parse: the distributions of `env` (LuaManager.env, after the distribution
    /// merge events) and the item scripts of `startup` (ScriptManager.instance).
    pub fn parse(&mut self, env: &TableRef, startup: &ClientStartup) -> R<()> {
        self.rooms.clear();
        self.no_container_fill_rooms.clear();
        self.weapon_upgrade_map.clear();
        self.weapon_upgrades.clear();
        self.containers.clear();
        self.added_invalid_already.clear();
        let no_container_fill_rooms =
            match KahluaTableImpl::rawget(env, &Object::str("NoContainerFillRooms"))? {
                Object::Nil => return Err(Throwable::npe("NoContainerFillRooms")),
                o => cast_table(&o)?,
            };
        for (k, _) in KahluaTableImpl::iterate_snapshot(&no_container_fill_rooms) {
            self.no_container_fill_rooms.push(k.java_to_string());
        }
        let weapon_upgrades = match KahluaTableImpl::rawget(env, &Object::str("WeaponUpgrades"))? {
            Object::Nil => return Err(Throwable::npe("WeaponUpgrades")),
            o => cast_table(&o)?,
        };
        for (k, v) in KahluaTableImpl::iterate_snapshot(&weapon_upgrades) {
            let name = k.java_to_string();
            let mut w = ItemPickerUpgradeWeapons {
                name: name.clone(),
                upgrades: Vec::new(),
            };
            let upgrades = cast_table(&v)?;
            for (_, up) in KahluaTableImpl::iterate_snapshot(&upgrades) {
                w.upgrades.push(up.java_to_string());
            }
            self.weapon_upgrade_map
                .insert(name, self.weapon_upgrades.len());
            self.weapon_upgrades.push(w);
        }
        self.parse_suburbs_distributions(env, startup)?;
        self.parse_vehicle_distributions(env, startup)?;
        self.parse_procedural_distributions(env, startup)?;
        Ok(())
    }

    /// ItemPickerJava.ParseSuburbsDistributions
    fn parse_suburbs_distributions(&mut self, env: &TableRef, startup: &ClientStartup) -> R<()> {
        let table = match KahluaTableImpl::rawget(env, &Object::str("SuburbsDistributions"))? {
            Object::Nil => return Err(Throwable::npe("SuburbsDistributions")),
            o => cast_table(&o)?,
        };
        for (k, v) in KahluaTableImpl::iterate_snapshot(&table) {
            let key = k.java_to_string();
            let containers = cast_table(&v)?;
            if contains_key(&containers, "rolls") {
                let c = self.extract_containers_from_lua(&containers, startup)?;
                self.containers.insert(key, c);
            } else {
                let mut room = ItemPickerRoom::default();
                for (ck, cv) in KahluaTableImpl::iterate_snapshot(&containers) {
                    let container_name = ck.java_to_string();
                    let name = container_name.as_str();
                    if let Object::Number(d) = &cv {
                        room.fill_rand = *d as i32;
                    } else if name == "isShop" {
                        room.is_shop = match &cv {
                            Object::Boolean(b) => *b,
                            o => return Err(Throwable::class_cast(o, "java.lang.Boolean")),
                        };
                    } else if name == "professionChance" {
                        // `((Double) value).intValue()`: a Double value took the first branch, so this is a cast error
                        return Err(Throwable::class_cast(&cv, "java.lang.Double"));
                    } else if name == "outfit" {
                        room.outfit = cast_str(&cv)?;
                    } else if name == "outfitFemale" {
                        room.outfit_female = cast_str(&cv)?;
                    } else if name == "outfitMale" {
                        room.outfit_male = cast_str(&cv)?;
                    } else if name == "outfitChance" {
                        room.outfit_chance = cast_str(&cv)?;
                    } else if name == "vehicle" {
                        room.vehicle = cast_str(&cv)?;
                    } else if name == "vehicles" {
                        // the original splits the key (`containerName`), not the value
                        room.vehicles = Some(
                            java::split_char(&container_name, ';')
                                .into_iter()
                                .map(str::to_string)
                                .collect(),
                        );
                    } else if name == "vehicleChance" {
                        room.vehicle_chance = cast_str(&cv)?;
                    } else if name == "vehicleDistribution" {
                        room.vehicle_distribution = cast_str(&cv)?;
                    } else if name == "vehicleSkin" {
                        // `(Integer) value`: a Lua number took the first branch
                        return Err(Throwable::class_cast(&cv, "java.lang.Integer"));
                    } else if name == "femaleChance" {
                        room.female_chance = cast_str(&cv)?;
                    } else if name == "roomTypes" {
                        room.room_types = cast_str(&cv)?;
                    } else if name == "zoneRequires" {
                        room.zone_requires = cast_str(&cv)?;
                    } else if name == "zoneDisallows" {
                        room.zone_disallows = cast_str(&cv)?;
                    } else if name == "containerChance" {
                        room.container_chance = cast_str(&cv)?;
                    } else if name == "femaleOdds" {
                        room.female_odds = cast_str(&cv)?;
                    } else if name == "bagType" {
                        room.bag_type = cast_str(&cv)?;
                    } else if name == "bagTable" {
                        room.bag_table = cast_str(&cv)?;
                    } else {
                        // `con = (KahluaTableImpl) value` is caught and logged; `con.delegate` is then a NullPointerException
                        let Ok(con) = cast_table(&cv) else {
                            return Err(Throwable::npe("con"));
                        };
                        if contains_key(&con, "procedural")
                            || (!container_name.is_empty()
                                && contains_key(&con, "rolls")
                                && contains_key(&con, "items"))
                        {
                            let c = self.extract_containers_from_lua(&con, startup)?;
                            room.containers.insert(container_name, c);
                        }
                        // else: DebugType.ItemPicker.error("ERROR: SuburbsDistributions[...] is broken")
                    }
                }
                self.rooms.insert(key, room);
            }
        }
        Ok(())
    }

    /// ItemPickerJava.ParseVehicleDistributions
    fn parse_vehicle_distributions(&mut self, env: &TableRef, startup: &ClientStartup) -> R<()> {
        self.vehicle_distributions.clear();
        let table = match KahluaTableImpl::rawget(env, &Object::str("VehicleDistributions"))? {
            Object::Nil => None,
            o => Some(cast_table(&o)?),
        };
        let Some(table) = table else {
            return Ok(());
        };
        let Object::Table(table) = KahluaTableImpl::rawget_int(&table, 1)? else {
            return Ok(());
        };
        for (k, v) in KahluaTableImpl::iterate_snapshot(&table) {
            let (Object::String(key), Object::Table(table_entry)) = (&k, &v) else {
                continue;
            };
            let mut vehicle_distribution = VehicleDistribution::default();
            if let Object::Table(normal) =
                KahluaTableImpl::rawget(table_entry, &Object::str("Normal"))?
            {
                let mut room = ItemPickerRoom::default();
                for (ck, cv) in KahluaTableImpl::iterate_snapshot(&normal) {
                    let container_name = ck.java_to_string();
                    if container_name != "specificId" {
                        let c = self.extract_containers_from_lua(&cast_table(&cv)?, startup)?;
                        room.containers.insert(container_name, c);
                    }
                }
                vehicle_distribution.normal = Some(room);
            }
            if let Object::Table(specific) =
                KahluaTableImpl::rawget(table_entry, &Object::str("Specific"))?
            {
                let n = KahluaTableImpl::len(&specific)?;
                for i in 1..=n {
                    let table3 = cast_table(&KahluaTableImpl::rawget_int(&specific, i)?)?;
                    let mut room = ItemPickerRoom::default();
                    for (ck, cv) in KahluaTableImpl::iterate_snapshot(&table3) {
                        let container_name = ck.java_to_string();
                        if container_name == "specificId" {
                            room.specific_id = cast_str(&cv)?;
                        } else {
                            let c = self.extract_containers_from_lua(&cast_table(&cv)?, startup)?;
                            room.containers.insert(container_name, c);
                        }
                    }
                    vehicle_distribution.specific.push(room);
                }
            }
            if vehicle_distribution.normal.is_some() {
                self.vehicle_distributions
                    .insert(rstr(key), vehicle_distribution);
            }
        }
        Ok(())
    }

    /// ItemPickerJava.ParseProceduralDistributions
    fn parse_procedural_distributions(&mut self, env: &TableRef, startup: &ClientStartup) -> R<()> {
        self.procedural_distributions.clear();
        if let Object::Table(t1) =
            KahluaTableImpl::rawget(env, &Object::str("ProceduralDistributions"))?
        {
            if let Object::Table(t2) = KahluaTableImpl::rawget(&t1, &Object::str("list"))? {
                for (k, v) in KahluaTableImpl::iterate_snapshot(&t2) {
                    let name = k.java_to_string();
                    let t3 = cast_table(&v)?;
                    let ipc = self.extract_containers_from_lua(&t3, startup)?;
                    self.procedural_distributions.insert(name, ipc);
                }
            }
        }
        Ok(())
    }

    /// ItemPickerJava.ExtractContainersFromLua
    fn extract_containers_from_lua(
        &mut self,
        con: &TableRef,
        startup: &ClientStartup,
    ) -> R<ItemPickerContainer> {
        let mut c = ItemPickerContainer::default();
        let flag = |name: &str| -> R<Option<bool>> {
            Ok(if contains_key(con, name) {
                Some(KahluaTableImpl::rawget_bool(con, name)?)
            } else {
                None
            })
        };
        if let Some(b) = flag("isShop")? {
            c.is_shop = b;
        }
        if contains_key(con, "procedural") {
            c.procedural = KahluaTableImpl::rawget_bool(con, "procedural")?;
            c.procedural_items = Some(extract_proc_list(con)?);
            return Ok(c);
        }
        if let Some(b) = flag("noAutoAge")? {
            c.no_auto_age = b;
        }
        if contains_key(con, "fillRand") {
            c.fill_rand = KahluaTableImpl::rawget_int_of(con, "fillRand")?;
        }
        if contains_key(con, "maxMap") {
            c.max_map = KahluaTableImpl::rawget_int_of(con, "maxMap")?;
        }
        if contains_key(con, "stashChance") {
            c.stash_chance = KahluaTableImpl::rawget_int_of(con, "stashChance")?;
        }
        if let Some(b) = flag("dontSpawnAmmo")? {
            c.dont_spawn_ammo = b;
        }
        if let Some(b) = flag("gunStorage")? {
            c.gun_storage = b;
        }
        if let Some(b) = flag("ignoreZombieDensity")? {
            c.ignore_zombie_density = b;
        }
        if let Some(b) = flag("cookFood")? {
            c.cook_food = b;
        }
        if let Some(b) = flag("canBurn")? {
            c.can_burn = b;
        }
        if let Some(b) = flag("isTrash")? {
            c.is_trash = b;
        }
        if let Some(b) = flag("isWorn")? {
            c.is_worn = b;
        }
        if let Some(b) = flag("isRotten")? {
            c.is_rotten = b;
        }
        if let Some(b) = flag("onlyOne")? {
            c.only_one = b;
        }
        // `double rolls = (Double) con.delegate.get("rolls")`
        let rolls = match con.borrow().get_direct(&Object::str("rolls")) {
            Object::Number(d) => d,
            Object::Nil => return Err(Throwable::npe("rolls")),
            o => return Err(Throwable::class_cast(&o, "java.lang.Double")),
        };
        if contains_key(con, "junk") {
            let junk = cast_table(&KahluaTableImpl::rawget(con, &Object::str("junk"))?)?;
            c.junk = Some(Box::new(self.extract_containers_from_lua(&junk, startup)?));
        }
        if contains_key(con, "bags") {
            let bags = cast_table(&KahluaTableImpl::rawget(con, &Object::str("bags"))?)?;
            c.bags = Some(Box::new(self.extract_containers_from_lua(&bags, startup)?));
        }
        if contains_key(con, "defaultInventoryLoot") {
            c.default_inventory_loot = KahluaTableImpl::rawget_bool(con, "defaultInventoryLoot")?;
        }
        c.rolls = (rolls as i32) as f32;
        let item_list_t = match con.borrow().get_direct(&Object::str("items")) {
            Object::Nil => return Err(Throwable::npe("items")),
            o => cast_table(&o)?,
        };
        let len = KahluaTableImpl::len(&item_list_t)?;
        let mut i = 0;
        while i < len {
            let name = match get_number_key(&item_list_t, i + 1) {
                Object::String(s) => Some(rstr(&s)),
                _ => None,
            };
            let chance = match get_number_key(&item_list_t, i + 2) {
                Object::Number(d) => Some(d),
                _ => None,
            };
            if let (Some(name), Some(chance)) = (name, chance) {
                let script_item = startup.find_item(&name);
                let item_exists = script_item.is_some() || get_item(startup, &name).is_some();
                let obsolete = script_item.is_some_and(|i| startup.is_obsolete(i));
                if !item_exists || obsolete {
                    // Core.debug: DebugType.ItemPicker.println("ignoring invalid ItemPicker item type ...")
                } else {
                    if let Some(si) = script_item {
                        self.can_spawn_as_loot.insert(si);
                    }
                    c.items.push(ItemPickerItem {
                        item_name: name,
                        chance: chance as f32,
                    });
                }
            }
            i += 2;
        }
        Ok(c)
    }

    /// ItemPickerJava.InitSandboxLootSettings
    pub fn init_sandbox_loot_settings(&mut self, so: &SandboxOptions) {
        let f = |name: &str| -> f32 {
            match so.get(name) {
                Some(Value::Double(d)) => *d as f32,
                _ => 0.0,
            }
        };
        let m = &mut self.modifiers;
        m.other = f("OtherLootNew");
        m.food = f("FoodLootNew");
        m.weapon = f("WeaponLootNew");
        m.ranged_weapon = f("RangedWeaponLootNew");
        m.ammo = f("AmmoLootNew");
        m.canned_food = f("CannedFoodLootNew");
        m.literature = f("LiteratureLootNew");
        m.survival_gears = f("SurvivalGearsLootNew");
        m.medical = f("MedicalLootNew");
        m.mechanics = f("MechanicsLootNew");
        m.clothing = f("ClothingLootNew");
        m.container = f("ContainerLootNew");
        m.key = f("KeyLootNew");
        m.key_d100 = f("KeyLootNew");
        m.media = f("MediaLootNew");
        m.memento = f("MementoLootNew");
        m.cookware = f("CookwareLootNew");
        m.material = f("MaterialLootNew");
        m.farming = f("FarmingLootNew");
        m.tool = f("ToolLootNew");
        m.skill_book = f("SkillBookLoot");
        m.recipe_resource = f("RecipeResourceLoot");
    }
}

/// ItemPickerJava.ExtractProcList
fn extract_proc_list(table: &TableRef) -> R<Vec<ProceduralItem>> {
    let mut result = Vec::new();
    let proc_list = match KahluaTableImpl::rawget(table, &Object::str("procList"))? {
        Object::Nil => return Err(Throwable::npe("procList")),
        o => cast_table(&o)?,
    };
    let split = |s: Option<String>| -> Option<Vec<String>> {
        s.filter(|s| !string_utils::is_null_or_whitespace(Some(s)))
            .map(|s| {
                java::split_char(&s, ';')
                    .into_iter()
                    .map(str::to_string)
                    .collect()
            })
    };
    for (_, v) in KahluaTableImpl::iterate_snapshot(&proc_list) {
        let entry = cast_table(&v)?;
        result.push(ProceduralItem {
            name: KahluaTableImpl::rawget_str(&entry, "name")?,
            min: KahluaTableImpl::rawget_int_of(&entry, "min")?,
            max: KahluaTableImpl::rawget_int_of(&entry, "max")?,
            weight_chance: KahluaTableImpl::rawget_int_of(&entry, "weightChance")?,
            force_for_items: split(KahluaTableImpl::rawget_str(&entry, "forceForItems")?),
            force_for_zones: split(KahluaTableImpl::rawget_str(&entry, "forceForZones")?),
            force_for_tiles: split(KahluaTableImpl::rawget_str(&entry, "forceForTiles")?),
            force_for_rooms: split(KahluaTableImpl::rawget_str(&entry, "forceForRooms")?),
        });
    }
    Ok(result)
}
