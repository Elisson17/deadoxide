//! What time and use do to items: `InventoryItem.update` and the overrides of `Food`, `DrainableComboItem` and
//! `Literature`, `InventoryItem.Use`, `DrainableComboItem.Use`, `ItemUser.UseItem`, `Food.destroyThisItem`.
//!
//! The original runs single-player here (`GameServer.server` and `GameClient.client` false): the time multiplier is
//! `GameTime.getMultiplier()`, nothing is sent to a client, and nothing is rendered. Whatever an operation would reach
//! beyond the arenas of `World` (a Lua function, a character, a world item, a sound) is `Err("not ported ...")`.
use super::item::{DrainableData, ItemId, Kind};
use super::world::World;
use crate::zombie::inventory::item_picker_roll::{script_bool, script_int, unported};

/// The names of the cooked dishes that restart their ageing (`Food.update`).
const RESTART_AGE_ON_COOKED: [&str; 10] = [
    "RicePot",
    "PastaPot",
    "RicePan",
    "PastaPan",
    "WaterPotRice",
    "WaterPotPasta",
    "WaterSaucepanRice",
    "WaterSaucepanPasta",
    "RiceBowl",
    "PastaBowl",
];

impl World<'_> {
    /// The DrainableComboItem data of an item; a programming error when the item is another class.
    fn drainable(&self, id: ItemId) -> &DrainableData {
        match &self.items[id].kind {
            Kind::Drainable(d) => d,
            _ => panic!("item {id} is not a DrainableComboItem"),
        }
    }

    fn drainable_mut(&mut self, id: ItemId) -> &mut DrainableData {
        match &mut self.items[id].kind {
            Kind::Drainable(d) => d,
            _ => panic!("item {id} is not a DrainableComboItem"),
        }
    }

    /// InventoryItem.calculateTimeMultiplier with `GameServer.server` false: `GameTime.getMultiplier()`.
    fn calculate_time_multiplier(&mut self, id: ItemId) {
        self.items[id].time_multiplier = self.game_time.get_multiplier();
    }

    /// `InventoryItem.update()` of the item's class: `Literature` does nothing, `Food` and `DrainableComboItem` have
    /// their own and do not call the base one, every other class (ComboItem) runs the base one.
    pub fn update_item(&mut self, id: ItemId) -> Result<(), String> {
        match &self.items[id].kind {
            Kind::Literature(_) => Ok(()),
            Kind::Food(_) => self.food_update(id),
            Kind::Drainable(_) => self.drainable_update(id),
            Kind::Combo => self.base_update(id),
        }
    }

    /// InventoryItem.getOutermostContainer(): `ItemContainer.getTemperature` of it, 1.0 for none (`Food.updateTemperature`).
    fn outermost_temperature(&self, id: ItemId) -> Result<f32, String> {
        match self.outermost_container(id) {
            Some(c) => self.container_temperature(c),
            None => Ok(1.0f32),
        }
    }

    /// `container.items.remove(item); item.container = null`, as `ItemUser.RemoveItem` and the depletion code do it
    /// (no `OnBeforeRemoveFromContainer`).
    fn drop_from_container(&mut self, id: ItemId) {
        if let Some(c) = self.items[id].container {
            if let Some(p) = self.containers[c].items.iter().position(|&i| i == id) {
                self.containers[c].items.remove(p);
            }
            self.items[id].container = None;
        }
    }

    /// InventoryItem.update (the base class: `ComboItem` has no override).
    fn base_update(&mut self, id: ItemId) -> Result<(), String> {
        self.calculate_time_multiplier(id);
        if self.items[id].is_wet {
            let tm = self.items[id].time_multiplier;
            self.items[id].wet_cooldown -= 1.0f32 * tm;
            if self.items[id].wet_cooldown <= 0.0f32 {
                let Some(dry_type) = self.items[id].item_when_dry.clone() else {
                    return Err("java.lang.NullPointerException: itemWhenDry".to_string());
                };
                let dry = self.create_item(&dry_type)?;
                if self.items[id].favorite && dry.is_none() {
                    return Err("java.lang.NullPointerException: dryItem".to_string());
                }
                if let Some(d) = dry {
                    self.items[d].favorite = self.items[id].favorite;
                }
                // `worldItem == null`: the container branch
                if let Some(c) = self.items[id].container {
                    if let Some(d) = dry {
                        self.add_item(c, d)?;
                    }
                    self.remove_item(c, id)?;
                }
                self.items[id].is_wet = false;
                self.add_to_process_items_remove(id);
                // LuaEventManager.triggerEvent("OnContainerUpdate"): no handler in the booted server's Lua
            }
        }
        // `hasComponent(FluidContainer)`: a script with a component is never made here
        if self.items[id].container.is_none() && self.items[id].item_heat != 1.0f32 {
            let tm = self.items[id].time_multiplier;
            let it = &mut self.items[id];
            if it.item_heat > 1.0f32 {
                it.item_heat -= 0.001f32 * tm;
                if it.item_heat < 1.0f32 {
                    it.item_heat = 1.0f32;
                }
            }
            if it.item_heat < 1.0f32 {
                it.item_heat += 0.001f32 * tm;
                if it.item_heat > 1.0f32 {
                    it.item_heat = 1.0f32;
                }
            }
        }
        // handleWorldItemInRain: `getWorldItem() != null` is false
        Ok(())
    }

    /// Food.updateTemperature
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn food_update_temperature(&mut self, id: ItemId) -> Result<(), String> {
        let tm = self.items[id].time_multiplier;
        let accum = self.food(id).temperature_time_accum + tm / 1.6f32;
        self.food_mut(id).temperature_time_accum = accum;
        if !(accum < 10.0f32) {
            // `isFertilized() && heat < 0.5`: a Food is never fertilized here
            let temp = self.outermost_temperature(id)?;
            let f = self.food_mut(id);
            if f.heat > temp {
                f.heat -= 0.001f32 * f.temperature_time_accum;
                if f.heat < 0.2f32.max(temp) {
                    f.heat = 0.2f32.max(temp);
                }
            }
            if f.heat < temp {
                f.heat += temp / 1000.0f32 * f.temperature_time_accum;
                if f.heat > 3.0f32.min(temp) {
                    f.heat = 3.0f32.min(temp);
                }
            }
            f.temperature_time_accum = 0.0f32;
        }
        Ok(())
    }

    /// InventoryItem.copyConditionStatesFrom for the items modelled here (no clothing visual, no attribute component,
    /// no mod data): the condition, the repairs, the favourite flag, and a drainable's use fraction.
    pub fn copy_condition_states_from(&mut self, dst: ItemId, src: ItemId) -> Result<(), String> {
        // setConditionFrom
        let (cond, cond_max, repaired) = {
            let o = &self.items[src];
            (o.condition, o.condition_max, o.have_been_repaired)
        };
        if self.items[dst].condition_max == cond_max {
            self.set_condition(dst, cond)?;
        } else {
            let perc = cond as f32 / cond_max as f32;
            let v = (self.items[dst].condition_max as f32 * perc) as i32;
            self.set_condition(dst, v)?;
            self.items[dst].have_been_repaired = repaired;
        }
        self.items[dst].have_been_repaired = repaired;
        self.items[dst].favorite = self.items[src].favorite;
        // copyBloodLevelFrom: `setBloodLevel` does nothing for these classes
        if matches!(self.items[dst].kind, Kind::Drainable(_))
            && matches!(self.items[src].kind, Kind::Drainable(_))
        {
            let f = self.current_uses_float(src);
            self.set_current_uses_float(dst, f);
        }
        Ok(())
    }

    /// Food.destroyThisItem for an item in a container (a world item is not modelled).
    fn destroy_this_item(&mut self, id: ItemId) -> Result<(), String> {
        if let Some(c) = self.items[id].container {
            self.remove_item(c, id)?;
            self.add_to_process_items_remove(id);
        }
        // LuaEventManager.triggerEvent("OnContainerUpdate"): no handler in the booted server's Lua
        Ok(())
    }

    /// InventoryItemFactory.CreateItem(String itemType, Food food): a new item of the type that takes the food's nutrition
    /// values; the original's null for anything that is not a Food (the item was made, and its draws are spent).
    fn create_item_like_food(
        &mut self,
        item_type: &str,
        food: ItemId,
    ) -> Result<Option<ItemId>, String> {
        let Some(new) = self.create_item(item_type)? else {
            return Ok(None);
        };
        if !matches!(self.items[new].kind, Kind::Food(_)) {
            return Ok(None);
        }
        let (base_hunger, hung, calories, carbs, proteins, lipids) = {
            let f = self.food(food);
            (
                f.base_hunger,
                f.hung_change,
                f.calories,
                f.carbohydrates,
                f.proteins,
                f.lipids,
            )
        };
        let (boredom, unhappy) = (
            self.items[food].boredom_change,
            self.items[food].unhappy_change,
        );
        self.items[new].boredom_change = boredom;
        self.items[new].unhappy_change = unhappy;
        let f = self.food_mut(new);
        f.base_hunger = base_hunger;
        f.hung_change = hung;
        f.carbohydrates = carbs;
        f.lipids = lipids;
        f.proteins = proteins;
        f.calories = calories;
        Ok(Some(new))
    }

    /// Food.updateRotting
    fn food_update_rotting(&mut self, id: ItemId, outermost: Option<usize>) -> Result<(), String> {
        let _ = outermost;
        if f64::from(self.items[id].off_age_max) == 1.0e9 {
            return Ok(());
        }
        // `!GameClient.client`
        if let Some(rot) = self
            .food(id)
            .replace_on_rotten
            .clone()
            .filter(|s| !s.is_empty())
        {
            self.update_age(id, false)?;
            if self.food_is_rotten(id) {
                let module = self.items[id].module.clone();
                let Some(new) = self.create_item_like_food(&format!("{module}.{rot}"), id)? else {
                    // DebugType.General.warn("ReplaceOnRotten = ... doesn't exist for ...")
                    self.destroy_this_item(id)?;
                    return Ok(());
                };
                self.items[new].age = self.items[id].age;
                self.copy_condition_states_from(new, id)?;
                // `worldInvObj == null`: the container branch
                if let Some(c) = self.items[id].container {
                    self.add_item(c, new)?;
                }
                self.destroy_this_item(id)?;
                return Ok(());
            }
        }
        let removal = self.sandbox_int("DaysForRottenFoodRemoval")?;
        if removal >= 0 {
            // `outermostContainer.parent instanceof IsoCompost`: a container here has no parent object
            self.update_age(id, false)?;
            if self.items[id].age > (self.items[id].off_age_max + removal) as f32 {
                self.destroy_this_item(id)?;
            }
        }
        Ok(())
    }

    /// Food.update
    fn food_update(&mut self, id: ItemId) -> Result<(), String> {
        self.calculate_time_multiplier(id);
        if self.has_tag(id, "base:alreadycooked") {
            self.set_cooked(id, true);
        }
        self.food_update_temperature(id)?;
        // checkEggHatch(null): `!isFertilized()` returns at once
        let outermost = self.outermost_container(id);
        if let Some(oc) = outermost {
            // `if (GameServer.server) updateAge(false)`
            if self.items[id].is_cookable && !self.food(id).frozen {
                if self.food(id).heat > 1.6f32 {
                    // setFertilized(false)
                    let current_cook_minute = self.game_time.get_minutes();
                    if current_cook_minute != self.food(id).last_cook_minute {
                        self.food_mut(id).last_cook_minute = current_cook_minute;
                        let mut dt = self.food(id).heat / 1.5f32;
                        if self.container_temperature(oc)? <= 1.6f32 {
                            dt *= 0.05f32;
                        }
                        self.items[id].cooking_time += dt;
                        // shouldPlayCookingSound: the container has no parent object
                        let (ctime, mtc, mtb) = {
                            let it = &self.items[id];
                            (it.cooking_time, it.minutes_to_cook, it.minutes_to_burn)
                        };
                        if self.food(id).is_tainted && ctime > mtc.min(10.0f32) {
                            self.food_mut(id).is_tainted = false;
                        }
                        let (cooked, burnt) = (self.items[id].cooked, self.items[id].burnt);
                        if !cooked && !burnt && (ctime > mtc || ctime > mtb) {
                            if self.food(id).replace_on_cooked.is_some() && !self.food_is_rotten(id)
                            {
                                let list =
                                    self.food(id).replace_on_cooked.clone().unwrap_or_default();
                                for name in &list {
                                    if let Some(new) = self.add_item_type(oc, name)? {
                                        self.copy_condition_states_from(new, id)?;
                                        let micro = self.containers[oc].type_name == "microwave";
                                        if micro && self.food(new).bad_in_microwave {
                                            self.items[new].unhappy_change = 5.0f32;
                                            self.items[new].boredom_change = 5.0f32;
                                            self.food_mut(new).cooked_in_microwave = true;
                                        }
                                    }
                                }
                                self.remove_item(oc, id)?;
                                self.add_to_process_items_remove(id);
                                return Ok(());
                            }
                            self.set_cooked(id, true);
                            if script_bool(
                                self.scripts.item(self.items[id].script),
                                "removeUnhappinessWhenCooked",
                            ) {
                                self.items[id].unhappy_change = 0.0f32;
                            }
                            if RESTART_AGE_ON_COOKED.contains(&self.items[id].type_name.as_str()) {
                                self.items[id].age = 0.0f32;
                                self.items[id].off_age = 1;
                                self.items[id].off_age_max = 2;
                            }
                            if self.food(id).remove_negative_effect_on_cooked {
                                if self.food(id).thirst_change > 0.0f32 {
                                    self.food_mut(id).thirst_change = 0.0f32;
                                }
                                if self.items[id].unhappy_change > 0.0f32 {
                                    self.items[id].unhappy_change = 0.0f32;
                                }
                                if self.items[id].boredom_change > 0.0f32 {
                                    self.items[id].boredom_change = 0.0f32;
                                }
                            }
                            if self
                                .food(id)
                                .on_cooked
                                .as_deref()
                                .is_some_and(|s| !s.is_empty())
                            {
                                return Err(unported(format!(
                                    "Lua OnCooked {}",
                                    self.food(id).on_cooked.clone().unwrap_or_default()
                                )));
                            }
                            if self.food(id).bad_in_microwave
                                && self.containers[oc].type_name == "microwave"
                            {
                                self.items[id].unhappy_change = 5.0f32;
                                self.items[id].boredom_change = 5.0f32;
                                self.food_mut(id).cooked_in_microwave = true;
                            }
                            // the chef's cooking experience: `chef` is set by Lua only
                        }
                        if self.items[id].cooking_time > self.items[id].minutes_to_burn {
                            self.items[id].burnt = true;
                            self.set_cooked(id, false);
                        }
                        // the fire of a burnt item on a powered stove needs `container.getParent()`
                    }
                }
            } else if self.food(id).is_tainted
                && self.food(id).heat > 1.6f32
                && !self.food(id).frozen
            {
                let current_cook_minute = self.game_time.get_minutes();
                if current_cook_minute != self.food(id).last_cook_minute {
                    self.food_mut(id).last_cook_minute = current_cook_minute;
                    let mut dtx = 1.0f32;
                    if self.container_temperature(oc)? <= 1.6f32 {
                        dtx *= 0.2f32;
                    }
                    self.items[id].cooking_time += dtx;
                    if self.items[id].cooking_time > 10.0f32 {
                        self.food_mut(id).is_tainted = false;
                    }
                }
            }
        }
        self.food_update_rotting(id, outermost)
    }

    /// DrainableComboItem.update
    fn drainable_update(&mut self, id: ItemId) -> Result<(), String> {
        self.calculate_time_multiplier(id);
        let tm = self.items[id].time_multiplier;
        if let Some(oc) = self.outermost_container(id) {
            let temp = self.container_temperature(oc)?;
            let Kind::Drainable(d) = &mut self.items[id].kind else {
                return Ok(());
            };
            if d.heat > temp {
                d.heat -= 0.001f32 * tm;
                if d.heat < 0.2f32.max(temp) {
                    d.heat = 0.2f32.max(temp);
                }
            }
            if d.heat < temp {
                d.heat += temp / 1000.0f32 * tm;
                if d.heat > 3.0f32.min(temp) {
                    d.heat = 3.0f32.min(temp);
                }
            }
            let heat = d.heat;
            if self.items[id].is_cookable && heat > 1.6f32 {
                let current_cook_minute = self.game_time.get_minutes();
                if current_cook_minute != self.drainable(id).last_cook_minute {
                    self.drainable_mut(id).last_cook_minute = current_cook_minute;
                    let mut dt = heat / 1.5f32;
                    if self.container_temperature(oc)? <= 1.6f32 {
                        dt *= 0.05f32;
                    }
                    let mut time_to_cook = self.items[id].cooking_time;
                    if time_to_cook < 1.0f32 {
                        time_to_cook = 10.0f32;
                    }
                    time_to_cook += dt;
                    if !self.items[id].cooked && time_to_cook > self.items[id].minutes_to_cook {
                        self.set_cooked(id, true);
                        if let Some(list) = self.drainable(id).replace_on_cooked.clone() {
                            let container = self.items[id].container;
                            let Some(c) = container else {
                                return Err("java.lang.NullPointerException: container".to_string());
                            };
                            for name in &list {
                                if let Some(new) = self.add_item_type(c, name)? {
                                    if matches!(self.items[new].kind, Kind::Drainable(_)) {
                                        let u = self.current_uses(id);
                                        self.set_current_uses_virtual(new, u);
                                    }
                                    self.copy_condition_states_from(new, id)?;
                                }
                            }
                            self.remove_item(c, id)?;
                            self.add_to_process_items_remove(id);
                            return Ok(());
                        }
                        if self.drainable(id).on_cooked.is_some() {
                            return Err(unported(format!(
                                "Lua OnCooked {}",
                                self.drainable(id).on_cooked.clone().unwrap_or_default()
                            )));
                        }
                    }
                    if self.items[id].cooking_time > self.items[id].minutes_to_burn {
                        self.items[id].burnt = true;
                        self.set_cooked(id, false);
                    }
                }
            }
        }
        if self.items[id].container.is_none() && self.drainable(id).heat != 1.0f32 {
            let d = self.drainable_mut(id);
            if d.heat > 1.0f32 {
                d.heat -= 0.001f32 * tm;
                if d.heat < 1.0f32 {
                    d.heat = 1.0f32;
                }
            }
            if d.heat < 1.0f32 {
                d.heat += 0.001f32 * tm;
                if d.heat > 1.0f32 {
                    d.heat = 1.0f32;
                }
            }
        }
        let (use_equiped, use_unequiped, ticks_per) = {
            let d = self.drainable(id);
            (
                d.use_while_equiped,
                d.use_while_unequiped,
                d.ticks_per_equip_use,
            )
        };
        if use_equiped && self.items[id].uses > 0 {
            // no container here has a player as its parent: `p == null`
            if self.items[id].can_be_activated && self.items[id].activated {
                return Err(unported("playDeactivateSound of an activated item"));
            }
        }
        let activated_ok = {
            let it = &self.items[id];
            !it.can_be_activated || it.activated
        };
        if use_unequiped && self.items[id].uses > 0 && activated_ok {
            let ticks = self.drainable(id).ticks + tm;
            self.drainable_mut(id).ticks = ticks;
            while self.drainable(id).ticks >= ticks_per as f32 {
                let t = self.drainable(id).ticks - ticks_per as f32;
                self.drainable_mut(id).ticks = t;
                if self.items[id].uses > 0 {
                    self.drainable_use(id)?;
                }
            }
        }
        let script = self.scripts.item(self.items[id].script);
        let keep_on_deplete = !script_bool(script, "disappearOnUse");
        let replace = self.drainable(id).replace_on_deplete.clone();
        if self.current_uses(id) <= 0
            && replace.is_none()
            && !keep_on_deplete
            && self.items[id].container.is_some()
        {
            self.drop_from_container(id);
        }
        if self.current_uses(id) <= 0 && replace.is_some() {
            let full = self
                .drainable(id)
                .replace_on_deplete_full_type
                .clone()
                .unwrap_or_default();
            if let Some(c) = self.items[id].container {
                let Some(new) = self.add_item_type(c, &full)? else {
                    return Err("java.lang.NullPointerException: replaceOnDeplete item".to_string());
                };
                self.copy_condition_states_from(new, id)?;
                self.remove_item(c, id)?;
            }
        }
        // updateTowelWetness: no rain, no world item
        self.drainable_mut(id).towel_wetness_accumulator = 0.0f32;
        Ok(())
    }

    /// DrainableComboItem.Use(false, false, false) (no world item)
    pub fn drainable_use(&mut self, id: ItemId) -> Result<(), String> {
        self.items[id].uses -= 1;
        if self.items[id].uses <= 0 {
            let script = self.scripts.item(self.items[id].script);
            let disappear = script_bool(script, "disappearOnUse");
            if self.drainable(id).replace_on_deplete.is_some() {
                let full = self
                    .drainable(id)
                    .replace_on_deplete_full_type
                    .clone()
                    .unwrap_or_default();
                if let Some(c) = self.items[id].container {
                    if let Some(new) = self.add_item_type(c, &full)? {
                        self.copy_condition_states_from(new, id)?;
                        self.remove_item(c, id)?;
                    }
                }
            } else {
                if !disappear {
                    // isKeepOnDeplete: returns before updateWeight
                    return Ok(());
                }
                if self.items[id].container.is_some() {
                    self.drop_from_container(id);
                }
            }
        }
        self.update_weight(id);
        Ok(())
    }

    /// InventoryItem.Use() and the DrainableComboItem override.
    pub fn use_item(&mut self, id: ItemId) -> Result<(), String> {
        if matches!(self.items[id].kind, Kind::Drainable(_)) {
            return self.drainable_use(id);
        }
        let disappear = script_bool(self.scripts.item(self.items[id].script), "disappearOnUse");
        if disappear {
            let uses = self.current_uses(id);
            self.set_current_uses_virtual(id, uses - 1);
            if let (Some(replace), Some(c)) = (
                self.items[id].replace_on_use.clone(),
                self.items[id].container,
            ) {
                let s = if replace.contains('.') {
                    replace
                } else {
                    format!("{}.{}", self.items[id].module, replace)
                };
                if let Some(new) = self.add_item_type(c, &s)? {
                    // the colour fields of the new item are the old one's (1.0 for both)
                    self.items[new].model_index = self.items[id].model_index;
                    self.copy_condition_states_from(new, id)?;
                    self.items[new].favorite = self.items[id].favorite;
                }
            }
            if self.current_uses(id) <= 0 {
                // `isKeepOnDeplete()` is `!disappearOnUse`, false inside this branch
                self.drop_from_container(id);
            }
        }
        Ok(())
    }

    /// ItemUser.CreateItem(fullType, result): `Count` new items of the script.
    fn item_user_create_items(&mut self, full_type: &str) -> Result<Vec<ItemId>, String> {
        let mut result = Vec::new();
        let Some(si) = self.scripts.find_item(full_type) else {
            // DebugType.General.warn("ERROR: ItemUses.CreateItem: can't find ...")
            return Ok(result);
        };
        let count = script_int(self.scripts.item(si), "count");
        for _ in 0..count {
            match self.create_item(full_type)? {
                Some(i) => result.push(i),
                None => return Ok(result),
            }
        }
        Ok(result)
    }

    /// ItemUser.UseItem(item, bCrafting, bInContainer, consumes, keep, destroy) for an item with no world item: the number
    /// of uses taken.
    pub fn item_user_use_item(
        &mut self,
        id: ItemId,
        crafting: bool,
        in_container: bool,
        consumes: i32,
        keep: bool,
        destroy: bool,
    ) -> Result<i32, String> {
        let script = self.scripts.item(self.items[id].script);
        let disappear = script_bool(script, "disappearOnUse");
        if !disappear && !crafting && !destroy {
            return Ok(0);
        }
        let consumed = self.current_uses(id).min(consumes);
        if !keep {
            let u = self.current_uses(id);
            self.set_current_uses_virtual(id, u - consumed);
        }
        if let Some(replace) = self.items[id].replace_on_use.clone() {
            if !in_container && !crafting && !destroy {
                let full = if replace.contains('.') {
                    replace
                } else {
                    format!("{}.{}", self.items[id].module, replace)
                };
                for new in self.item_user_create_items(&full)? {
                    self.item_user_add_item(id, new)?;
                    self.copy_condition_states_from(new, id)?;
                }
            }
        }
        let replace_on_deplete = match &self.items[id].kind {
            Kind::Drainable(d) => d.replace_on_deplete.clone().filter(|s| !s.is_empty()),
            _ => None,
        };
        if let Some(replace) = replace_on_deplete {
            if self.current_uses(id) <= 0 {
                let full = if replace.contains('.') {
                    replace
                } else {
                    format!("{}.{}", self.items[id].module, replace)
                };
                for new in self.item_user_create_items(&full)? {
                    self.item_user_add_item(id, new)?;
                    self.copy_condition_states_from(new, id)?;
                }
            }
        }
        // `destroy ? RemoveItem : (uses <= 0 && !isKeepOnDeplete() ? RemoveItem : nothing)`
        if destroy || (self.current_uses(id) <= 0 && disappear) {
            self.drop_from_container(id);
        }
        Ok(consumed)
    }

    /// ItemUser.AddItem(existingItem, newItem) for an existing item with no world item.
    fn item_user_add_item(&mut self, existing: ItemId, new: ItemId) -> Result<(), String> {
        if let Some(c) = self.items[existing].container {
            self.add_item(c, new)?;
        }
        Ok(())
    }
}
