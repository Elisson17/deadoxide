//! zombie.inventory.types.Food: what it reports as its state changes it, its use counts, ageing and freezing, over the
//! arenas of `World`. A Food with no container is not modelled apart from what it reports (nothing here holds one in the
//! world or in a bag).
use super::container::lerp;
use super::item::{FoodData, ItemId, Kind};
use super::world::World;
use crate::zombie::inventory::item_picker_roll::{script_complex, script_str, unported};

/// zombie.GameTime.checkHours (float, float): a negative stamp becomes the world age, any other is capped by it.
pub fn check_hours(hours: f32, world_age_hours: f32) -> f32 {
    if hours < 0.0f32 {
        world_age_hours
    } else {
        // GameTime.minHours -> PZMath.min
        if hours < world_age_hours {
            hours
        } else {
            world_age_hours
        }
    }
}

impl World<'_> {
    /// The Food data of an item; a programming error when the item is another class.
    fn food(&self, id: ItemId) -> &FoodData {
        match &self.items[id].kind {
            Kind::Food(f) => f,
            _ => panic!("item {id} is not a Food"),
        }
    }

    fn food_mut(&mut self, id: ItemId) -> &mut FoodData {
        match &mut self.items[id].kind {
            Kind::Food(f) => f,
            _ => panic!("item {id} is not a Food"),
        }
    }

    /// Food.setHeat
    pub fn set_food_heat(&mut self, id: ItemId, heat: f32) {
        self.food_mut(id).heat = heat;
    }

    /// Food.isFrozen
    pub fn food_is_frozen(&self, id: ItemId) -> bool {
        self.food(id).frozen
    }

    /// InventoryItem.getCurrentUses and the Food override: `getBaseHunger() == 0 ? super : (int)Math.abs(hungChange * 100)`.
    pub fn current_uses(&self, id: ItemId) -> i32 {
        match &self.items[id].kind {
            Kind::Food(f) if f.base_hunger != 0.0f32 => (f.hung_change * 100.0f32).abs() as i32,
            _ => self.items[id].uses,
        }
    }

    /// InventoryItem.getMaxUses and the overrides of Food and DrainableComboItem.
    pub fn max_uses(&self, id: ItemId) -> i32 {
        match &self.items[id].kind {
            Kind::Food(f) => {
                if f.base_hunger == 0.0f32 {
                    1
                } else {
                    (f.base_hunger * 100.0f32).abs() as i32
                }
            }
            _ => self.items[id].max_uses(),
        }
    }

    /// InventoryItem.getCurrentUsesFloat and the overrides.
    pub fn current_uses_float(&self, id: ItemId) -> f32 {
        match &self.items[id].kind {
            Kind::Food(f) if f.base_hunger != 0.0f32 => f.hung_change.abs(),
            Kind::Food(_) => self.items[id].uses as f32 / 1.0f32,
            _ => self.items[id].current_uses_float(),
        }
    }

    /// InventoryItem.setCurrentUses with the overrides of Food (a food with a base hunger consumes hunger instead) and
    /// DrainableComboItem (the weight follows).
    pub fn set_current_uses_virtual(&mut self, id: ItemId, uses: i32) {
        match &self.items[id].kind {
            Kind::Food(f) => {
                let uses = uses.max(0);
                if f.base_hunger == 0.0f32 {
                    self.items[id].uses = uses;
                } else {
                    let cur = self.current_uses(id);
                    self.consume_hunger(id, (cur - uses) as f32 / 100.0f32);
                }
            }
            Kind::Drainable(_) => self.set_current_uses(id, uses),
            _ => self.items[id].uses = uses,
        }
    }

    /// InventoryItem.setCurrentUsesFloat (`useDelta` stays its default 0.03125 for every class but DrainableComboItem, which
    /// has its own and updates the weight): `Math.round(clamp(newUses, 0, 1) / useDelta)`.
    pub fn set_current_uses_float(&mut self, id: ItemId, new_uses: f32) {
        let new_uses = new_uses.clamp(0.0f32, 1.0f32);
        let use_delta = match &self.items[id].kind {
            Kind::Drainable(d) => d.use_delta,
            _ => 0.03125f32,
        };
        // Math.round(float) is floor(x + 1/2) computed exactly
        let q = f64::from(new_uses / use_delta);
        self.items[id].uses = (q + 0.5).floor() as i32;
        if matches!(self.items[id].kind, Kind::Drainable(_)) {
            self.update_weight(id);
        }
    }

    /// Food.consumeHunger
    pub fn consume_hunger(&mut self, id: ItemId, real_used_hunger: f32) {
        let percentage_used = (real_used_hunger / self.food(id).hung_change).abs();
        self.multiply_food_values(id, 1.0f32 - percentage_used);
    }

    /// Food.multiplyFoodValues
    pub fn multiply_food_values(&mut self, id: ItemId, p: f32) {
        // the arguments are read through the getters (which depend on age, cooking and burning) before any is stored
        let sickness = self.food_sickness_change(id);
        let poison = self.food(id).poison_power;
        let it = &mut self.items[id];
        it.boredom_change *= p;
        it.unhappy_change *= p;
        it.stress_change *= p;
        it.fatigue_change *= p;
        let Kind::Food(f) = &mut it.kind else {
            return;
        };
        f.hung_change *= p;
        f.flu_reduction = (f.flu_reduction as f32 * p) as i32;
        f.thirst_change *= p;
        f.pain_reduction *= p;
        f.food_sickness_change = (sickness as f32 * p) as i32;
        f.end_change *= p;
        f.calories *= p;
        f.carbohydrates *= p;
        f.proteins *= p;
        f.lipids *= p;
        f.poison_power = (poison as f32 * p) as i32;
    }

    /// Food.getFoodSicknessChange
    pub fn food_sickness_change(&self, id: ItemId) -> i32 {
        let it = &self.items[id];
        let v = self.food(id).food_sickness_change;
        if it.burnt {
            (v as f32 / 3.0f32) as i32
        } else if it.age >= it.off_age as f32 && it.age < it.off_age_max as f32 {
            (v as f32 / 1.3f32) as i32
        } else if it.age >= it.off_age_max as f32 {
            (v as f32 / 2.2f32) as i32
        } else if it.cooked {
            (v as f32 * 1.3f32) as i32
        } else {
            v
        }
    }

    /// Food.getHungerChange
    pub fn food_hunger_change(&self, id: ItemId) -> f32 {
        let it = &self.items[id];
        let hung = self.food(id).hung_change;
        if hung != 0.0f32 {
            if it.cooked {
                return hung * 1.3f32;
            }
            let sign = if hung < 0.0f32 { -1.0f32 } else { 1.0f32 };
            let abs = hung.abs();
            if it.burnt {
                return (abs / 3.0f32).max(0.01f32) * sign;
            }
            if it.age >= it.off_age as f32 && it.age < it.off_age_max as f32 {
                return (abs / 1.3f32).max(0.01f32) * sign;
            }
            if it.age >= it.off_age_max as f32 {
                return (abs / 2.2f32).max(0.01f32) * sign;
            }
        }
        hung
    }

    /// `"Icecream".equals(getType()) || hasTag(ItemTag.GOOD_FROZEN)`
    fn good_frozen(&self, id: ItemId) -> bool {
        self.items[id].type_name == "Icecream" || self.has_tag(id, "base:goodfrozen")
    }

    /// Food.getUnhappyChange (a Food is never fertilized here)
    pub fn food_unhappy_change(&self, id: ItemId) -> f32 {
        let it = &self.items[id];
        let f = self.food(id);
        let mut result = it.unhappy_change;
        if f.frozen && !self.good_frozen(id) {
            result += 30.0f32;
        }
        if it.burnt {
            result += 20.0f32;
        }
        if it.age >= it.off_age as f32 && it.age < it.off_age_max as f32 {
            result += 10.0f32;
        }
        if it.age >= it.off_age_max as f32 {
            result += 20.0f32;
        }
        if f.bad_cold && it.is_cookable && it.cooked && f.heat < 1.3f32 {
            result += 2.0f32;
        }
        if f.good_hot && it.is_cookable && it.cooked && f.heat > 1.3f32 {
            result -= 2.0f32;
        }
        result
    }

    /// Food.getBoredomChange (a Food is never fertilized here)
    pub fn food_boredom_change(&self, id: ItemId) -> f32 {
        let it = &self.items[id];
        let mut result = it.boredom_change;
        if self.food(id).frozen && !self.good_frozen(id) {
            result += 30.0f32;
        }
        if it.burnt {
            result += 20.0f32;
        }
        if it.age >= it.off_age as f32 && it.age < it.off_age_max as f32 {
            result += 10.0f32;
        }
        if it.age >= it.off_age_max as f32 {
            result += 20.0f32;
        }
        result
    }

    /// Food.getStressChange (a Food is never fertilized here)
    pub fn food_stress_change(&self, id: ItemId) -> f32 {
        let it = &self.items[id];
        if it.burnt {
            it.stress_change / 4.0f32
        } else if it.age >= it.off_age as f32 && it.age < it.off_age_max as f32 {
            it.stress_change / 1.3f32
        } else if it.age >= it.off_age_max as f32 {
            it.stress_change / 2.0f32
        } else if it.cooked {
            it.stress_change * 1.3f32
        } else {
            it.stress_change
        }
    }

    /// Food.getEnduranceChange
    pub fn food_endurance_change(&self, id: ItemId) -> f32 {
        let it = &self.items[id];
        let end = self.food(id).end_change;
        if it.burnt {
            end / 3.0f32
        } else if it.age >= it.off_age as f32 && it.age < it.off_age_max as f32 {
            end / 2.0f32
        } else if it.cooked {
            end * 2.0f32
        } else {
            end
        }
    }

    /// Food.isRotten (a Food is never fertilized here)
    pub fn food_is_rotten(&self, id: ItemId) -> bool {
        let it = &self.items[id];
        it.age >= it.off_age_max as f32
    }

    /// Food.isFresh (a Food is never fertilized here)
    pub fn food_is_fresh(&self, id: ItemId) -> bool {
        let it = &self.items[id];
        it.age < it.off_age as f32
    }

    /// Food.getFridgeFactor
    fn fridge_factor(&self) -> Result<f32, String> {
        Ok(match self.sandbox_int("FridgeFactor")? {
            1 => 0.4f32,
            2 => 0.3f32,
            4 => 0.1f32,
            5 => 0.03f32,
            6 => 0.0f32,
            _ => 0.2f32,
        })
    }

    /// Food.getFoodRotSpeed
    fn food_rot_speed(&self) -> Result<f32, String> {
        Ok(match self.sandbox_int("FoodRotSpeed")? {
            1 => 1.7f32,
            2 => 1.4f32,
            4 => 0.7f32,
            5 => 0.4f32,
            _ => 1.0f32,
        })
    }

    /// Food.setFreezingTime
    pub fn set_freezing_time(&mut self, id: ItemId, mut freezing_time: f32) {
        let f = self.food_mut(id);
        if freezing_time >= 100.0f32 {
            f.frozen = true;
            freezing_time = 100.0f32;
        } else if freezing_time <= 0.0f32 {
            freezing_time = 0.0f32;
            f.frozen = false;
        }
        f.freezing_time = freezing_time;
    }

    /// Food.isFreezing: `canBeFrozen && freezingTime < 100 && isInFreezer(outermost) ? outermost.isPowered() : false`.
    /// `ItemContainer.isPowered` needs a parent object, which these containers do not have: no item here freezes.
    fn food_is_freezing(&self) -> bool {
        false
    }

    /// Food.isThawing: `!canBeFrozen || freezingTime <= 0` is false, else `!isInFreezer ? true : !isPowered()`, and nothing
    /// here is powered.
    fn food_is_thawing(&self, id: ItemId) -> bool {
        let f = self.food(id);
        f.can_be_frozen && f.freezing_time > 0.0f32
    }

    /// InventoryItem.getOutermostContainer: the container unless it is the floor (a container here holds no container).
    pub fn outermost_container(&self, id: ItemId) -> Option<usize> {
        let c = self.items[id].container?;
        if self.containers[c].type_name == "floor" {
            return None;
        }
        Some(c)
    }

    /// Food.updateFreezing
    fn update_freezing(
        &mut self,
        id: ItemId,
        outermost: Option<usize>,
        world_age_hours: f32,
    ) -> Result<(), String> {
        let last = check_hours(self.food(id).last_frozen_update, world_age_hours);
        self.food_mut(id).last_frozen_update = last;
        if world_age_hours > last {
            let elapsed_hours = world_age_hours - last;
            if self.food_is_freezing() {
                return Err(unported("a powered freezer"));
            }
            if self.food_is_thawing(id) {
                let mut local_hours_to_thaw = 1.5f32;
                // `isInFridge(outermostContainer) && outermostContainer.isPowered()`: never powered here
                if let Some(o) = outermost {
                    if self.container_temperature(o)? > 1.0f32 {
                        local_hours_to_thaw /= 6.0f32;
                    }
                }
                let t =
                    self.food(id).freezing_time - elapsed_hours / local_hours_to_thaw * 100.0f32;
                self.set_freezing_time(id, t);
            }
            self.food_mut(id).last_frozen_update = world_age_hours;
        }
        Ok(())
    }

    /// Food.updateAge(boolean) (the flag only sends the stats to clients, which a probe without a server never does).
    pub fn update_age(&mut self, id: ItemId, _send_stats: bool) -> Result<(), String> {
        let elapsed_hours = self.game_time.get_world_age_hours() as f32;
        let outermost = self.outermost_container(id);
        self.update_freezing(id, outermost, elapsed_hours)?;
        self.items[id].last_aged = check_hours(self.items[id].last_aged, elapsed_hours);
        let last_aged = self.items[id].last_aged;
        if elapsed_hours > last_aged {
            let mut age_increase = f64::from(elapsed_hours - last_aged);
            if let Some(o) = outermost {
                let temp = self.container_temperature(o)?;
                if self.food(id).heat != temp {
                    if age_increase < f64::from(0.33333334f32) {
                        if !self.process_items.contains(&id) {
                            let heat = self.food(id).heat;
                            let h = lerp(heat, temp, age_increase as f32 / 0.33333334f32);
                            self.food_mut(id).heat = h;
                            self.add_to_process_items(id);
                        }
                    } else {
                        self.food_mut(id).heat = temp;
                    }
                }
            }
            if self.food(id).frozen {
                age_increase *= 0.0;
            } else if outermost.is_some_and(|o| self.is_fridge(o) || self.is_freezer(o)) {
                // `outermostContainer.getSourceGrid() != null && ...haveElectricity()`: these containers stand on no square
                let elec_shut = self.sandbox_int("ElecShutModifier")?;
                if elec_shut > -1 && last_aged < (elec_shut * 24) as f32 {
                    let hours_with_electricity = ((elec_shut * 24) as f32).min(elapsed_hours);
                    age_increase =
                        f64::from((hours_with_electricity - last_aged) * self.fridge_factor()?);
                    if elapsed_hours > (elec_shut * 24) as f32 {
                        age_increase += f64::from(elapsed_hours - (elec_shut * 24) as f32);
                    }
                }
            }
            // `fresh`/`rotten` before and after only decide whether the Lua event OnContainerUpdate fires (no handler is
            // listening in the booted server's Lua)
            let rot = self.food_rot_speed()?;
            let it = &mut self.items[id];
            it.age = (f64::from(it.age) + age_increase * f64::from(rot) / 24.0) as f32;
            it.last_aged = elapsed_hours;
        }
        Ok(())
    }

    /// InventoryItem.setCooked(boolean)
    pub fn set_cooked(&mut self, id: ItemId, cooked: bool) {
        let it = &mut self.items[id];
        it.cooked = cooked;
        if cooked && it.cooking_time < it.minutes_to_cook {
            it.cooking_time = it.minutes_to_cook;
        }
    }

    /// InventoryItem.setBurnt(boolean)
    pub fn set_burnt(&mut self, id: ItemId, burnt: bool) {
        let it = &mut self.items[id];
        it.burnt = burnt;
        if burnt && it.cooking_time < it.minutes_to_burn {
            it.cooking_time = it.minutes_to_burn;
        }
    }

    /// InventoryItem.setCondition(condition, false) and `setBroken`, which calls `onBreak` (a Lua function named by the
    /// script's `OnBreak`; not ported, so a script with one is an `Err`).
    pub fn set_condition(&mut self, id: ItemId, condition: i32) -> Result<(), String> {
        self.items[id].set_condition_no_sound(condition);
        if self.items[id].broken {
            let script = self.scripts.item(self.items[id].script);
            if script_str(script, "onBreak").is_some()
                || script_complex(script, "onbreak").is_some()
            {
                return Err(unported(format!("OnBreak ({})", script.full_name())));
            }
        }
        Ok(())
    }
}
