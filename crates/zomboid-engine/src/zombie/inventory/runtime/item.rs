//! zombie.inventory.InventoryItem and the item classes `Item.InstanceItem` makes of the scripts the port models:
//! ComboItem (`ItemType.NORMAL`), DrainableComboItem, Food and Literature.
use crate::se::krka::kahlua::j2se::kahlua_table_impl::{KahluaTable, LuaValue};
use crate::zombie::inventory::inventory_item::ItemType;
use crate::zombie::inventory::item_picker_roll::firearm::ammo_item_key;
use crate::zombie::inventory::item_picker_roll::{
    script_bool, script_complex, script_float, script_int, script_str, script_weight_capacity,
    unported,
};
use crate::zombie::scripting::item::Item;
use crate::zombie::scripting::objects::resource_location::ResourceLocation;

/// An item of `World::items`.
pub type ItemId = usize;
/// A container of `World::containers`.
pub type ContainerId = usize;

/// `StringUtils.moduleDotType(module, type)`: a type that holds a '.' is already a full type.
pub fn module_dot_type(module: &str, type_name: Option<&str>) -> Option<String> {
    type_name.map(|t| {
        if t.contains('.') {
            t.to_string()
        } else {
            format!("{module}.{t}")
        }
    })
}

/// zombie.inventory.types.Food: the fields that class adds to `InventoryItem`.
#[derive(Clone, Debug)]
pub struct FoodData {
    pub bad_cold: bool,
    pub good_hot: bool,
    pub heat: f32,
    pub end_change: f32,
    pub hung_change: f32,
    pub rotten: bool,
    pub dangerous_uncooked: bool,
    pub last_cook_minute: i32,
    pub thirst_change: f32,
    pub poison: bool,
    pub replace_on_cooked: Option<Vec<String>>,
    pub base_hunger: f32,
    pub is_spice: bool,
    pub is_tainted: bool,
    pub poison_detection_level: i32,
    pub poison_level_for_recipe: i32,
    pub use_for_poison: i32,
    pub poison_power: i32,
    pub food_type: Option<String>,
    pub remove_negative_effect_on_cooked: bool,
    pub on_cooked: Option<String>,
    pub flu_reduction: i32,
    pub food_sickness_change: i32,
    pub pain_reduction: f32,
    pub herbalist_type: Option<String>,
    pub carbohydrates: f32,
    pub lipids: f32,
    pub proteins: f32,
    pub calories: f32,
    pub packaged: bool,
    pub freezing_time: f32,
    pub frozen: bool,
    pub can_be_frozen: bool,
    pub last_frozen_update: f32,
    pub replace_on_rotten: Option<String>,
    pub rotten_time: f32,
    pub compost_time: f32,
    pub on_eat: Option<String>,
    pub bad_in_microwave: bool,
    pub cooked_in_microwave: bool,
    pub temperature_time_accum: f32,
    pub chef: Option<String>,
    pub spices: Option<Vec<String>>,
    pub milk_qty: i32,
    pub milk_type: Option<String>,
}

/// zombie.inventory.types.DrainableComboItem: the fields that class adds to `InventoryItem`.
#[derive(Clone, Debug)]
pub struct DrainableData {
    pub use_while_equiped: bool,
    pub use_while_unequiped: bool,
    pub ticks_per_equip_use: i32,
    pub use_delta: f32,
    pub ticks: f32,
    pub replace_on_deplete: Option<String>,
    pub replace_on_deplete_full_type: Option<String>,
    pub replace_on_cooked: Option<Vec<String>>,
    pub on_cooked: Option<String>,
    pub can_consolidate: bool,
    pub weight_empty: f32,
    pub on_eat: Option<String>,
    pub last_update_minutes: i32,
    pub heat: f32,
    pub last_cook_minute: i32,
    pub towel_wetness_accumulator: f32,
}

/// zombie.inventory.types.Literature: the fields that class adds to `InventoryItem`.
#[derive(Clone, Debug)]
pub struct LiteratureData {
    pub already_read: bool,
    pub number_of_pages: i32,
    pub book_name: String,
    pub lvl_skill_trained: i32,
    pub num_levels_trained: i32,
    pub skill_trained: String,
    pub already_read_pages: i32,
    pub can_be_write: bool,
    pub page_to_write: i32,
    /// `customPages` (a HashMap of page number 1.. to text): the texts in page order
    pub custom_pages: Option<Vec<String>>,
    pub locked_by: Option<String>,
    pub learned_recipes: Option<Vec<String>>,
}

/// The class of an item (what `Item.InstanceItem` made of the script) with the fields only that class has.
#[derive(Clone, Debug)]
pub enum Kind {
    /// zombie.inventory.types.ComboItem
    Combo,
    Drainable(DrainableData),
    Food(Box<FoodData>),
    Literature(LiteratureData),
}

/// zombie.inventory.InventoryItem (and its subclass fields in `kind`).
#[derive(Clone, Debug)]
pub struct LiveItem {
    /// The item script (`scriptItem`): an index into the scripts' items.
    pub script: usize,
    pub item_type: ItemType,
    pub kind: Kind,
    pub container: Option<ContainerId>,
    pub name: String,
    pub original_name: String,
    pub module: String,
    pub type_name: String,
    pub full_type: String,
    pub id: i32,
    pub condition: i32,
    pub condition_max: i32,
    pub uses: i32,
    pub count: i32,
    pub age: f32,
    pub last_aged: f32,
    pub is_cookable: bool,
    pub cooking_time: f32,
    pub minutes_to_cook: f32,
    pub minutes_to_burn: f32,
    pub cooked: bool,
    pub burnt: bool,
    pub off_age: i32,
    pub off_age_max: i32,
    pub weight: f32,
    pub actual_weight: f32,
    pub custom_weight: bool,
    pub replace_on_use: Option<String>,
    pub replace_on_use_full_type: Option<String>,
    pub boredom_change: f32,
    pub unhappy_change: f32,
    pub stress_change: f32,
    pub fatigue_change: f32,
    pub food_sickness_change: i32,
    pub extra_items: Vec<String>,
    pub can_stack: bool,
    pub activated: bool,
    pub can_be_activated: bool,
    pub item_heat: f32,
    pub is_wet: bool,
    pub wet_cooldown: f32,
    /// `itemWhenDry`
    pub item_when_dry: Option<String>,
    pub registry_id: i16,
    pub key_id: i32,
    pub favorite: bool,
    /// `broken` (`setBroken(condition <= 0)`)
    pub broken: bool,
    /// `modelIndex` (-1 until `synchWithVisual` picks one)
    pub model_index: i32,
    /// `ammoType.getItemKey()` of the script's `AmmoType` (the item type of the rounds it holds)
    pub ammo_item: Option<String>,
    pub max_ammo: i32,
    pub current_ammo_count: i32,
    pub is_initialised: bool,
    pub have_been_repaired: i32,
    pub time_multiplier: f32,
    pub last_update_ms: i64,
    /// `itemCapacity`
    pub item_capacity: f32,
    pub custom_color: bool,
    /// `col` (r, g, b, a)
    pub col: [f32; 4],
    pub color_red: f32,
    pub color_green: f32,
    pub color_blue: f32,
    /// `table` (modData): `None` until `getModData()` makes it
    pub mod_data: Option<KahluaTable>,
    pub byte_data: Option<Vec<u8>>,
    pub custom_name: bool,
    pub remote_control_id: i32,
    pub remote_range: i32,
    pub worker: Option<String>,
    pub stash_map: Option<String>,
    pub zombie_infected: bool,
    pub attached_slot: i32,
    pub attached_slot_type: Option<String>,
    pub attached_to_model: Option<String>,
    pub max_capacity: i32,
    pub recorded_media_index: i16,
    pub world_x_rotation: f32,
    pub world_y_rotation: f32,
    pub world_z_rotation: f32,
    pub world_scale: f32,
}

fn text(item: &Item, name: &str) -> Option<String> {
    script_str(item, name).map(str::to_string)
}

impl LiveItem {
    /// zombie.scripting.objects.Item.InstanceItem for `item_type`s Normal, Drainable, Food and Literature: the class
    /// constructor, the class branch's setters and the setters after it, in the original's order. Every other class
    /// (and a script with a component, `OnCreate` or an ammo type) is `Err("not ported: ...")`.
    pub fn instance(script: usize, item: &Item, item_type: ItemType) -> Result<LiveItem, String> {
        let ammo_item = match script_complex(item, "ammotype") {
            None => None,
            Some(v) => Some(
                ammo_item_key(crate::java::trim(v))
                    .ok_or_else(|| unported(format!("AmmoType {v} ({})", item.full_name())))?
                    .to_string(),
            ),
        };
        if !matches!(
            item_type,
            ItemType::Normal | ItemType::Drainable | ItemType::Food | ItemType::Literature
        ) {
            return Err(unported(format!("item class {item_type:?}")));
        }
        let display = match item.get("displayName") {
            Some(crate::zombie::scripting::item::Value::Str(Some(s))) => s.clone(),
            _ => String::new(),
        };
        let module = item.module.clone();
        let mut it = LiveItem {
            script,
            item_type,
            kind: Kind::Combo,
            container: None,
            name: display.clone(),
            original_name: display,
            module: module.clone(),
            type_name: item.name.clone(),
            full_type: format!("{module}.{}", item.name),
            id: 0,
            condition: 10,
            condition_max: 10,
            uses: 1,
            count: 1,
            age: 0.0,
            last_aged: -1.0,
            is_cookable: false,
            cooking_time: 0.0,
            minutes_to_cook: 60.0,
            minutes_to_burn: 120.0,
            cooked: false,
            burnt: false,
            off_age: 1_000_000_000,
            off_age_max: 1_000_000_000,
            weight: 1.0,
            actual_weight: 1.0,
            custom_weight: false,
            replace_on_use: None,
            replace_on_use_full_type: None,
            boredom_change: 0.0,
            unhappy_change: 0.0,
            stress_change: 0.0,
            fatigue_change: 0.0,
            food_sickness_change: 0,
            extra_items: Vec::new(),
            can_stack: false,
            activated: false,
            can_be_activated: false,
            item_heat: 1.0,
            is_wet: false,
            wet_cooldown: -1.0,
            item_when_dry: None,
            registry_id: -1,
            key_id: -1,
            favorite: false,
            broken: false,
            model_index: -1,
            ammo_item: None,
            max_ammo: 0,
            current_ammo_count: 0,
            is_initialised: false,
            have_been_repaired: 0,
            time_multiplier: 0.0,
            last_update_ms: -1,
            item_capacity: -1.0,
            custom_color: false,
            col: [1.0; 4],
            color_red: 1.0,
            color_green: 1.0,
            color_blue: 1.0,
            mod_data: None,
            byte_data: None,
            custom_name: false,
            remote_control_id: -1,
            remote_range: 0,
            worker: None,
            stash_map: None,
            zombie_infected: false,
            attached_slot: -1,
            attached_slot_type: None,
            attached_to_model: None,
            max_capacity: -1,
            recorded_media_index: -1,
            world_x_rotation: 0.0,
            world_y_rotation: 0.0,
            world_z_rotation: -1.0,
            world_scale: 1.0,
        };
        match item_type {
            ItemType::Food => {
                // the Food constructor, then the class branch of Item.InstanceItem
                let hunger = script_float(item, "hungerChange");
                let replace_on_cooked = script_complex(item, "replaceoncooked").map(|v| {
                    crate::java::split_char(crate::java::trim(v), ';')
                        .into_iter()
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                });
                it.kind = Kind::Food(Box::new(FoodData {
                    bad_cold: script_bool(item, "badCold"),
                    good_hot: script_bool(item, "goodHot"),
                    heat: 1.0,
                    end_change: script_float(item, "enduranceChange") / 100.0f32,
                    hung_change: hunger / 100.0f32,
                    rotten: false,
                    dangerous_uncooked: script_bool(item, "dangerousUncooked"),
                    last_cook_minute: 0,
                    thirst_change: script_float(item, "thirstChange") / 100.0f32,
                    poison: script_bool(item, "poison"),
                    replace_on_cooked,
                    base_hunger: hunger / 100.0f32,
                    is_spice: script_bool(item, "spice"),
                    is_tainted: false,
                    poison_detection_level: -1,
                    poison_level_for_recipe: script_int(item, "poisonDetectionLevel"),
                    use_for_poison: script_int(item, "useForPoison"),
                    poison_power: script_int(item, "poisonPower"),
                    food_type: text(item, "foodType"),
                    remove_negative_effect_on_cooked: script_bool(
                        item,
                        "removeNegativeEffectOnCooked",
                    ),
                    on_cooked: text(item, "onCooked"),
                    flu_reduction: script_int(item, "fluReduction"),
                    food_sickness_change: script_int(item, "foodSicknessChange"),
                    pain_reduction: script_int(item, "painReduction") as f32,
                    herbalist_type: text(item, "herbalistType"),
                    carbohydrates: script_float(item, "carbohydrates"),
                    lipids: script_float(item, "lipids"),
                    proteins: script_float(item, "proteins"),
                    calories: script_float(item, "calories"),
                    packaged: script_bool(item, "packaged"),
                    freezing_time: 0.0,
                    frozen: false,
                    can_be_frozen: !script_bool(item, "cantBeFrozen"),
                    last_frozen_update: -1.0,
                    replace_on_rotten: text(item, "replaceOnRotten"),
                    rotten_time: 0.0,
                    compost_time: 0.0,
                    on_eat: text(item, "onEat"),
                    bad_in_microwave: script_bool(item, "badInMicrowave"),
                    cooked_in_microwave: false,
                    temperature_time_accum: 0.0,
                    chef: None,
                    spices: None,
                    milk_qty: 0,
                    milk_type: None,
                }));
                it.off_age = script_int(item, "daysFresh");
                it.off_age_max = script_int(item, "daysTotallyRotten");
                it.is_cookable = script_bool(item, "isCookable");
                it.minutes_to_cook = script_int(item, "minutesToCook") as f32;
                it.minutes_to_burn = script_int(item, "minutesToBurn") as f32;
                it.set_replace_on_use(text(item, "replaceOnUse"));
                it.food_sickness_change = script_int(item, "foodSicknessChange");
            }
            ItemType::Literature => {
                it.kind = Kind::Literature(LiteratureData {
                    already_read: false,
                    number_of_pages: script_int(item, "numberOfPages"),
                    book_name: it.name.clone(),
                    lvl_skill_trained: script_int(item, "lvlSkillTrained"),
                    num_levels_trained: script_int(item, "numLevelsTrained"),
                    skill_trained: text(item, "skillTrained").unwrap_or_else(|| "None".into()),
                    already_read_pages: 0,
                    can_be_write: script_bool(item, "canBeWrite"),
                    page_to_write: script_int(item, "pageToWrite"),
                    custom_pages: None,
                    locked_by: None,
                    learned_recipes: None,
                });
                it.set_replace_on_use(text(item, "replaceOnUse"));
            }
            ItemType::Drainable => {
                let replace = text(item, "replaceOnDeplete");
                let full = module_dot_type(&module, replace.as_deref());
                it.kind = Kind::Drainable(DrainableData {
                    use_while_equiped: script_bool(item, "useWhileEquipped"),
                    use_while_unequiped: script_bool(item, "useWhileUnequipped"),
                    ticks_per_equip_use: script_int(item, "ticksPerEquipUse"),
                    use_delta: script_float(item, "useDelta"),
                    ticks: 0.0,
                    replace_on_deplete: replace,
                    replace_on_deplete_full_type: full,
                    replace_on_cooked: script_complex(item, "replaceoncooked").map(|v| {
                        crate::java::split_char(crate::java::trim(v), ';')
                            .into_iter()
                            .map(str::to_string)
                            .collect::<Vec<_>>()
                    }),
                    on_cooked: text(item, "onCooked"),
                    can_consolidate: !script_bool(item, "cantBeConsolided"),
                    weight_empty: script_float(item, "weightEmpty"),
                    on_eat: text(item, "onEat"),
                    last_update_minutes: -1,
                    heat: 1.0,
                    last_cook_minute: 0,
                    towel_wetness_accumulator: 0.0,
                });
                it.is_cookable = script_bool(item, "isCookable");
                it.minutes_to_cook = script_int(item, "minutesToCook") as f32;
            }
            _ => {}
        }
        // the setters after the class branch
        it.condition_max = script_int(item, "conditionMax");
        it.set_condition_no_sound(it.condition_max);
        let (aw, _) = script_weight_capacity(item);
        it.set_actual_weight(aw);
        it.set_weight(aw);
        it.boredom_change = script_float(item, "boredomChange");
        it.stress_change = script_float(item, "stressChange") / 100.0f32;
        it.food_sickness_change = script_int(item, "foodSicknessChange");
        it.unhappy_change = script_float(item, "unhappyChange");
        it.can_stack = script_bool(item, "canStack");
        it.count = script_int(item, "count");
        it.fatigue_change = script_float(item, "fatigueChange") / 100.0f32;
        it.set_replace_on_use(text(item, "replaceOnUse"));
        it.can_be_activated = script_bool(item, "activatedItem");
        it.ammo_item = ammo_item;
        it.max_ammo = script_int(item, "maxAmmo");
        it.is_wet = script_bool(item, "isWet");
        it.wet_cooldown = script_float(item, "wetCooldown");
        it.item_when_dry = text(item, "itemWhenDry");
        // `if (colorRed < 255 || ...) item.setColor(new Color(r / 255.0F, g / 255.0F, b / 255.0F))` (alpha 1, no clamping)
        let (cr, cg, cb) = (
            script_int(item, "colorRed"),
            script_int(item, "colorGreen"),
            script_int(item, "colorBlue"),
        );
        if cr < 255 || cg < 255 || cb < 255 {
            it.col = [
                cr as f32 / 255.0f32,
                cg as f32 / 255.0f32,
                cb as f32 / 255.0f32,
                1.0,
            ];
        }
        it.color_red = cr as f32 / 255.0f32;
        it.color_green = cg as f32 / 255.0f32;
        it.color_blue = cb as f32 / 255.0f32;
        it.remote_range = script_int(item, "remoteRange");
        it.item_capacity = script_int(item, "itemCapacity") as f32;
        it.max_capacity = script_int(item, "maxCapacity");
        // `item.copyModData(this.defaultModData)` (the parameters `DoParam` has no branch for: a number when the value
        // parses as a double, else the text), then `item.setTooltip(this.tooltip)`: `getModData().rawset("Tooltip", tooltip)`
        let mut table = KahluaTable::default();
        for (k, v) in &item.unknown {
            let value = match crate::java::parse_double(crate::java::trim(v)) {
                Some(d) => LuaValue::Number(d),
                None => LuaValue::String(v.clone()),
            };
            table.rawset(LuaValue::String(crate::java::trim(k).to_string()), value);
        }
        if let Some(t) = text(item, "tooltip") {
            table.rawset(LuaValue::String("Tooltip".into()), LuaValue::String(t));
        }
        it.mod_data = Some(table);
        Ok(it)
    }

    /// InventoryItem.setReplaceOnUse
    pub fn set_replace_on_use(&mut self, replace: Option<String>) {
        self.replace_on_use_full_type = module_dot_type(&self.module, replace.as_deref());
        self.replace_on_use = replace;
    }

    /// InventoryItem.setCondition(condition, false) (no sound, the debug cheat off): the condition clamped to
    /// 0..=conditionMax.
    pub fn set_condition_no_sound(&mut self, condition: i32) {
        let condition = condition.max(0).min(self.condition_max);
        self.condition = condition;
        self.broken = condition <= 0;
    }

    /// InventoryItem.setWeight
    pub fn set_weight(&mut self, weight: f32) {
        self.weight = if weight < 0.0 { 0.0 } else { weight };
    }

    /// InventoryItem.setActualWeight
    pub fn set_actual_weight(&mut self, actual_weight: f32) {
        self.actual_weight = if actual_weight < 0.0 {
            0.0
        } else {
            actual_weight
        };
    }

    /// `instanceof Clothing`: none of the classes modelled here.
    pub fn is_clothing(&self) -> bool {
        false
    }

    /// InventoryItem.getCategory (`mainCategory` is never set by a script) and the overrides of the classes.
    pub fn category(&self) -> &'static str {
        match self.kind {
            Kind::Food(_) => "Food",
            Kind::Literature(_) => "Literature",
            _ => "Item",
        }
    }

    /// The simple name of the item's Java class.
    pub fn class_name(&self) -> &'static str {
        match self.kind {
            Kind::Combo => "ComboItem",
            Kind::Drainable(_) => "DrainableComboItem",
            Kind::Food(_) => "Food",
            Kind::Literature(_) => "Literature",
        }
    }

    /// The tags of the item's script as `InventoryItem.getTags()` gives them: a set.
    pub fn tags_of(script: &Item) -> Vec<ResourceLocation> {
        let mut t: Vec<ResourceLocation> = Vec::new();
        for l in &script.tags {
            if !t.contains(l) {
                t.push(l.clone());
            }
        }
        t
    }

    /// InventoryItem.getMaxUses and the DrainableComboItem override: `(int)Math.floor(1.0F / useDelta)`.
    pub fn max_uses(&self) -> i32 {
        match &self.kind {
            Kind::Drainable(d) => f64::from(1.0f32 / d.use_delta).floor() as i32,
            _ => 1,
        }
    }

    /// InventoryItem.getCurrentUsesFloat and the DrainableComboItem override.
    pub fn current_uses_float(&self) -> f32 {
        match &self.kind {
            Kind::Drainable(d) => self.uses as f32 * d.use_delta,
            _ => self.uses as f32 / self.max_uses() as f32,
        }
    }
}
