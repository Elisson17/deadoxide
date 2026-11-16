//! The `Food` state the loot rolls read and change (`zombie.inventory.types.Food` and the `InventoryItem` fields
//! under it), and the steps of `ItemPickerJava` that work on it: `setAutoAge`, cooking, `trashItem`,
//! `wearDownItem`, `rotItem`, `multiplyFoodValues`.
use super::*;

/// The fields of a `Food` the roll touches (an `InventoryItem` with `ItemType.FOOD` made by `Item.InstanceItem`).
#[derive(Clone, Debug)]
pub struct FoodState {
    /// `InventoryItem.age` (days).
    pub age: f32,
    pub off_age: i32,
    pub off_age_max: i32,
    pub cooked: bool,
    pub burnt: bool,
    /// `Food.rotten` (`setRotten`; nothing the rolls read depends on it: `isRotten()` compares the age).
    pub rotten: bool,
    pub cooking_time: f32,
    pub minutes_to_cook: f32,
    pub minutes_to_burn: f32,
    pub is_cookable: bool,
    pub can_be_frozen: bool,
    pub dangerous_uncooked: bool,
    pub hung_change: f32,
    pub base_hunger: f32,
    pub thirst_change: f32,
    pub end_change: f32,
    pub boredom_change: f32,
    pub unhappy_change: f32,
    pub stress_change: f32,
    pub fatigue_change: f32,
    pub flu_reduction: i32,
    pub food_sickness_change: i32,
    pub pain_reduction: f32,
    pub calories: f32,
    pub carbohydrates: f32,
    pub proteins: f32,
    pub lipids: f32,
    pub poison_power: i32,
}

impl FoodState {
    /// `Item.InstanceItem` for `ItemType.FOOD` (the `Food` constructor's defaults, then the script's values; the
    /// per-item `setBoredomChange`/`setStressChange`/... after the class branch).
    pub(super) fn from_script(script: &Item) -> Self {
        FoodState {
            age: 0.0,
            off_age: script_int(script, "daysFresh"),
            off_age_max: script_int(script, "daysTotallyRotten"),
            cooked: false,
            burnt: false,
            rotten: false,
            cooking_time: 0.0,
            minutes_to_cook: script_int(script, "minutesToCook") as f32,
            minutes_to_burn: script_int(script, "minutesToBurn") as f32,
            is_cookable: script_bool(script, "isCookable"),
            can_be_frozen: !script_bool(script, "cantBeFrozen"),
            dangerous_uncooked: script_bool(script, "dangerousUncooked"),
            hung_change: script_float(script, "hungerChange") / 100.0f32,
            base_hunger: script_float(script, "hungerChange") / 100.0f32,
            thirst_change: script_float(script, "thirstChange") / 100.0f32,
            end_change: script_float(script, "enduranceChange") / 100.0f32,
            boredom_change: script_float(script, "boredomChange"),
            unhappy_change: script_float(script, "unhappyChange"),
            stress_change: script_float(script, "stressChange") / 100.0f32,
            fatigue_change: script_float(script, "fatigueChange") / 100.0f32,
            flu_reduction: script_int(script, "fluReduction"),
            food_sickness_change: script_int(script, "foodSicknessChange"),
            pain_reduction: script_int(script, "painReduction") as f32,
            calories: script_float(script, "calories"),
            carbohydrates: script_float(script, "carbohydrates"),
            proteins: script_float(script, "proteins"),
            lipids: script_float(script, "lipids"),
            poison_power: script_int(script, "poisonPower"),
        }
    }

    /// `InventoryItem.setCooked(true)`: the cooking time reaches `minutesToCook`.
    fn set_cooked(&mut self) {
        self.cooked = true;
        if self.cooking_time < self.minutes_to_cook {
            self.cooking_time = self.minutes_to_cook;
        }
    }

    /// `InventoryItem.setBurnt(true)`: the cooking time reaches `minutesToBurn`.
    fn set_burnt(&mut self) {
        self.burnt = true;
        if self.cooking_time < self.minutes_to_burn {
            self.cooking_time = self.minutes_to_burn;
        }
    }

    /// `Food.isRotten()` (a fertilized food is neither rotten nor, below, not fresh: eggs only, never rolled).
    fn is_rotten(&self) -> bool {
        self.age >= self.off_age_max as f32
    }

    /// `Food.isFresh()`
    fn is_fresh(&self) -> bool {
        self.age < self.off_age as f32
    }

    /// `Food.getHungerChange()`: the hunger change as the food's state modifies it.
    pub fn hunger_change(&self) -> f32 {
        let hung = self.hung_change;
        if hung != 0.0 {
            if self.cooked {
                return hung * 1.3f32;
            }
            let sign = if hung < 0.0 { -1.0f32 } else { 1.0f32 };
            let abs = hung.abs();
            if self.burnt {
                return (abs / 3.0f32).max(0.01f32) * sign;
            }
            if self.age >= self.off_age as f32 && self.age < self.off_age_max as f32 {
                return (abs / 1.3f32).max(0.01f32) * sign;
            }
            if self.age >= self.off_age_max as f32 {
                return (abs / 2.2f32).max(0.01f32) * sign;
            }
        }
        hung
    }

    /// `Food.getThirstChange()`
    pub fn thirst_change_modified(&self) -> f32 {
        if self.burnt {
            self.thirst_change / 5.0f32
        } else if self.cooked {
            self.thirst_change / 2.0f32
        } else {
            self.thirst_change
        }
    }

    /// `Food.getFoodSicknessChange()`
    pub fn food_sickness(&self) -> i32 {
        if self.burnt {
            (self.food_sickness_change as f32 / 3.0f32) as i32
        } else if self.age >= self.off_age as f32 && self.age < self.off_age_max as f32 {
            (self.food_sickness_change as f32 / 1.3f32) as i32
        } else if self.age >= self.off_age_max as f32 {
            (self.food_sickness_change as f32 / 2.2f32) as i32
        } else if self.cooked {
            (self.food_sickness_change as f32 * 1.3f32) as i32
        } else {
            self.food_sickness_change
        }
    }

    /// `Food.multiplyFoodValues(percentage)`
    fn multiply_food_values(&mut self, percentage: f32) {
        self.boredom_change *= percentage;
        self.unhappy_change *= percentage;
        self.hung_change *= percentage;
        self.flu_reduction = (self.flu_reduction as f32 * percentage) as i32;
        self.thirst_change *= percentage;
        self.pain_reduction *= percentage;
        self.food_sickness_change = (self.food_sickness() as f32 * percentage) as i32;
        self.end_change *= percentage;
        self.stress_change *= percentage;
        self.fatigue_change *= percentage;
        self.calories *= percentage;
        self.carbohydrates *= percentage;
        self.proteins *= percentage;
        self.lipids *= percentage;
        self.poison_power = (self.poison_power as f32 * percentage) as i32;
    }

    /// The part of `trashItem` / `trashItemLooted` / `wearDownItem` that scales a food after its age and cooking
    /// were decided: `baseHunger` is the smaller of the item's base hunger and its current hunger change, in
    /// hundredths and negated.
    fn trash_base_hunger(&self) -> f64 {
        let base = (-(self.base_hunger * 100.0f32)) as f64 + 0.1;
        let hunger = (-(self.hunger_change() * 100.0f32)) as f64 + 0.1;
        if hunger < base {
            hunger
        } else {
            base
        }
    }

    /// `food.setCooked(true)` followed by `food.getOnCooked()` for a `SpawnCooked` food (`itemSpawnSanityCheck`):
    /// the only `OnCooked` function the scripts name for those is `RecipeCodeOnCooked.cannedFood`.
    pub(super) fn spawn_cooked(&mut self, on_cooked: Option<&str>) -> Result<(), String> {
        self.set_cooked();
        match on_cooked {
            None | Some("") => {}
            Some("RecipeCodeOnCooked.cannedFood") => {
                // RecipeCodeOnCooked.cannedFood
                let aged = self.age / self.off_age_max as f32;
                self.off_age_max = 1560;
                self.off_age = 730;
                self.age = self.off_age_max as f32 * aged;
            }
            Some(other) => return Err(unported(format!("OnCooked function {other}"))),
        }
        Ok(())
    }
}

impl LootRoller<'_> {
    /// `SandboxOptions.fridgeFactor` as `Food.getFridgeFactor` reads it.
    fn fridge_factor(&self) -> Result<f32, String> {
        let v = self
            .sandbox
            .get_int("FridgeFactor")
            .ok_or_else(|| unported("sandbox option FridgeFactor"))?;
        Ok(match v {
            1 => 0.4f32,
            2 => 0.3f32,
            4 => 0.1f32,
            5 => 0.03f32,
            6 => 0.0f32,
            _ => 0.2f32,
        })
    }

    /// `SandboxOptions.foodRotSpeed` as `Food.getFoodRotSpeed` reads it.
    fn food_rot_speed(&self) -> Result<f32, String> {
        let v = self
            .sandbox
            .get_int("FoodRotSpeed")
            .ok_or_else(|| unported("sandbox option FoodRotSpeed"))?;
        Ok(match v {
            1 => 1.7f32,
            2 => 1.4f32,
            4 => 0.7f32,
            5 => 0.4f32,
            _ => 1.0f32,
        })
    }

    /// `GameTime.getWorldAgeDaysSinceBegin` of the standalone `GameTime`.
    pub(super) fn world_age_days(&self) -> Result<f64, String> {
        let since_apo = self
            .sandbox
            .get_int("TimeSinceApo")
            .ok_or_else(|| unported("sandbox option TimeSinceApo"))?;
        Ok(
            crate::zombie::game_time::GameTime::default().get_world_age_hours() / 24.0
                + ((since_apo - 1) * 30) as f64,
        )
    }

    /// `InventoryItem.setAutoAge` for an item whose outermost container has the type `outermost` (only a `Food`
    /// does anything: its age from the world age, the food rot speed and a fridge or freezer).
    pub(super) fn item_set_auto_age(
        &self,
        it: &mut LootItem,
        outermost: &str,
    ) -> Result<(), String> {
        let Some(food) = it.food.as_mut() else {
            return Ok(());
        };
        // Food.setAutoAge
        let world_age_days = self.world_age_days()? as f32;
        let mut age_increase = world_age_days;
        let in_fridge = outermost == "fridge";
        let in_freezer = outermost == "freezer";
        if in_fridge || in_freezer {
            let elec = self
                .sandbox
                .get_int("ElecShutModifier")
                .ok_or_else(|| unported("sandbox option ElecShutModifier"))?;
            if elec > -1 {
                let days_with_electricity = (elec as f32).min(world_age_days);
                if !in_fridge && food.can_be_frozen {
                    let days_frozen = days_with_electricity;
                    if world_age_days > days_with_electricity {
                        return Err(unported(
                            "Food.setAutoAge in a freezer after the electricity shut off",
                        ));
                    }
                    age_increase = world_age_days - days_frozen;
                    age_increase += days_frozen * 0.0f32;
                } else {
                    age_increase = world_age_days - days_with_electricity;
                    age_increase += days_with_electricity * self.fridge_factor()?;
                }
            }
        }
        food.age = age_increase * self.food_rot_speed()?;
        Ok(())
    }

    /// The cooking of a spawned food in `doRollItemInternal`: `Rand.Next(100) < 25` burns it where the container
    /// distribution allows (`canBurn`), else it is cooked and aged again.
    pub(super) fn cook_spawned_food(
        &mut self,
        it: &mut LootItem,
        can_burn: bool,
        outermost: &str,
    ) -> Result<(), String> {
        let burn = {
            let Some(food) = it.food.as_ref() else {
                return Ok(());
            };
            can_burn && food.minutes_to_burn > 0.0 && self.rand.next(100) < 25
        };
        if burn {
            if let Some(food) = it.food.as_mut() {
                food.set_burnt();
            }
        } else {
            if let Some(food) = it.food.as_mut() {
                food.set_cooked();
            }
            self.item_set_auto_age(it, outermost)?;
        }
        Ok(())
    }

    /// `trashItem`'s and `wearDownItem`'s shared test: an item that shows its condition, or a `HandWeapon` without a
    /// physics object.
    fn condition_item(script: &Item, class: ItemType) -> bool {
        has_tag(script, "ShowCondition")
            || (class == ItemType::Weapon && script_str(script, "physicsObject").is_none())
    }

    /// `InventoryItem.setSharpness(value)`: capped at `getMaxSharpness()` and 0..=1.
    fn set_sharpness(&self, it: &mut LootItem, value: f32) -> Result<(), String> {
        if it.sharpness.is_some() {
            let max = self.max_sharpness(it)?;
            let mut v = value;
            if v > max {
                v = max;
            }
            v = v.clamp(0.0, 1.0);
            it.sharpness = Some(v);
        }
        Ok(())
    }

    /// `InventoryItem.setHeadCondition(value)`: kept between 0 and `getHeadConditionMax()`.
    fn set_head_condition(&self, it: &mut LootItem, value: i32) -> Result<(), String> {
        if it.head_condition.is_some() {
            let mut v = value.max(0);
            let max = self.head_condition_max(it)?;
            if v > max {
                v = max;
            }
            if v <= 0 {
                return Err(unported(
                    "a head condition of 0 (setCondition(0) with a sound)",
                ));
            }
            it.head_condition = Some(v);
        }
        Ok(())
    }

    /// `InventoryItem.setCondition(value, false)`: kept between 0 and `ConditionMax`.
    fn set_condition_no_sound(&self, it: &mut LootItem, value: i32) -> Result<(), String> {
        let max = script_int(self.script(it.script)?, "conditionMax");
        it.condition = value.max(0).min(max);
        Ok(())
    }

    /// The head condition, sharpness and drainable part shared by `trashItem` (`looted` false) and
    /// `wearDownItem`; each draws as the original does.
    fn degrade_tools(
        &mut self,
        it: &mut LootItem,
        script: &Item,
        wear_down: bool,
    ) -> Result<(), String> {
        if it.head_condition.is_some() {
            if wear_down || self.rand.next_bool(2) {
                let max = self.head_condition_max(it)?;
                let v = self.rand.next_range(1, max);
                self.set_head_condition(it, v)?;
            } else {
                self.set_head_condition(it, 1)?;
            }
        }
        if it.sharpness.is_some() {
            if wear_down || self.rand.next_bool(2) {
                let max = self.max_sharpness(it)?;
                let v = self.rand.next_float(0.0, max);
                self.set_sharpness(it, v)?;
            } else {
                self.set_sharpness(it, 0.0)?;
            }
        }
        if it.class == ItemType::Drainable {
            let limit = if wear_down { 75 } else { 90 };
            if self.rand.next(100) < limit {
                it.uses = 1;
                self.drainable_update_weight(it)?;
            } else {
                self.randomize_uses(it)?;
            }
        }
        let _ = script;
        Ok(())
    }

    /// The scaling of a food's values by its hunger (the end of `trashItem` and `wearDownItem`): a food worth at
    /// least 4 (hundredths of hunger) loses more than one worth 2 or 3.
    fn scale_by_hunger(&mut self, food: &mut FoodState, base_hunger: f64, wear_down: bool) {
        if base_hunger >= 4.0 {
            let roll = self.rand.next(8);
            if wear_down {
                if roll <= 2 {
                    food.multiply_food_values(0.75f32);
                } else if roll <= 4 {
                    food.multiply_food_values(0.5f32);
                } else {
                    food.multiply_food_values(0.25f32);
                }
            } else if roll == 0 {
                food.multiply_food_values(0.75f32);
            } else if roll <= 2 {
                food.multiply_food_values(0.5f32);
            } else {
                food.multiply_food_values(0.25f32);
            }
        } else if base_hunger >= 2.0 {
            food.multiply_food_values(0.5f32);
        }
    }

    /// ItemPickerJava.trashItem
    pub(super) fn trash_item(&mut self, it: &mut LootItem) -> Result<(), String> {
        let script = self.script(it.script)?;
        if Self::condition_item(script, it.class) {
            // `if (Rand.Next(100) < 95) setConditionNoSound(1) else setConditionNoSound(1)`
            self.rand.next(100);
            self.set_condition_no_sound(it, 1)?;
        }
        self.degrade_tools(it, script, false)?;
        if it.class == ItemType::Food
            && (!script_bool(script, "cannedFood") || !script_bool(script, "cantEat"))
        {
            let vermin = has_tag(script, "Vermin");
            let canned = script_bool(script, "cannedFood");
            let replace_on_cooked = script_complex(script, "replaceoncooked").is_some();
            let mut food = it
                .food
                .take()
                .ok_or_else(|| unported("a food without state"))?;
            let mut burnt = false;
            let mut gross = false;
            if !vermin
                && food.is_cookable
                && !canned
                && !replace_on_cooked
                && self.rand.next(100) < 75
            {
                if self.rand.next(100) < 50 {
                    food.set_cooked();
                } else {
                    burnt = true;
                    food.set_burnt();
                }
            }
            if !food.is_rotten() && food.off_age_max < 1_000_000_000 && self.rand.next(100) < 95 {
                food.rotten = true;
                food.age = food.off_age_max as f32;
                gross = true;
            } else if food.is_fresh() && food.off_age < 1_000_000_000 && self.rand.next(100) < 95 {
                food.age = food.off_age as f32;
                gross = true;
            }
            if gross && self.rand.next(2) == 0 {
                gross = false;
            }
            if food.dangerous_uncooked && !food.cooked {
                gross = true;
            }
            if vermin {
                gross = true;
            }
            let base_hunger = food.trash_base_hunger();
            if !burnt && !gross && self.rand.next(100) != 0 {
                self.scale_by_hunger(&mut food, base_hunger, false);
            }
            it.food = Some(food);
        }
        if it.class == ItemType::Clothing {
            self.clothing_randomize_condition(it, 25, 95, 10, 75)?;
        }
        Ok(())
    }

    /// ItemPickerJava.wearDownItem
    pub(super) fn wear_down_item(&mut self, it: &mut LootItem) -> Result<(), String> {
        let script = self.script(it.script)?;
        if Self::condition_item(script, it.class) {
            let max = script_int(script, "conditionMax");
            if self.rand.next(100) < 25 {
                self.set_condition_no_sound(it, 1)?;
            } else {
                let mut roll1 = self.rand.next(max);
                let roll2 = self.rand.next(max);
                let roll3 = self.rand.next(max);
                if roll2 < roll1 {
                    roll1 = roll2;
                }
                if roll3 < roll1 {
                    roll1 = roll3;
                }
                self.set_condition_no_sound(it, roll1 + 1)?;
            }
        }
        self.degrade_tools(it, script, true)?;
        if it.class == ItemType::Food
            && (!script_bool(script, "cannedFood") || !script_bool(script, "cantEat"))
        {
            let vermin = has_tag(script, "Vermin");
            let replace_on_cooked = script_complex(script, "replaceoncooked").is_some();
            let mut food = it
                .food
                .take()
                .ok_or_else(|| unported("a food without state"))?;
            let mut burnt = false;
            let mut gross = false;
            if !vermin && food.is_cookable && !replace_on_cooked && self.rand.next(100) < 50 {
                if self.rand.next(100) < 50 {
                    food.set_cooked();
                } else {
                    burnt = true;
                    food.set_burnt();
                }
            }
            if !food.is_rotten() && food.off_age_max < 1_000_000_000 && self.rand.next(100) < 75 {
                food.rotten = true;
                food.age = food.off_age_max as f32;
                gross = true;
            } else if food.is_fresh() && food.off_age < 1_000_000_000 && self.rand.next(100) < 75 {
                food.age = food.off_age as f32;
                gross = true;
            }
            if gross && self.rand.next(2) == 0 {
                gross = false;
            }
            if food.dangerous_uncooked && !food.cooked {
                gross = true;
            }
            if vermin {
                gross = true;
            }
            let base_hunger = food.trash_base_hunger();
            if !burnt && !gross && self.rand.next(100) < 75 {
                self.scale_by_hunger(&mut food, base_hunger, true);
            }
            it.food = Some(food);
        }
        if it.class == ItemType::Clothing {
            self.clothing_randomize_condition(it, 0, 25, 1, 25)?;
        }
        Ok(())
    }

    /// ItemPickerJava.rotItem
    pub(super) fn rot_item(&mut self, it: &mut LootItem) -> Result<(), String> {
        let script = self.script(it.script)?;
        if it.class == ItemType::Food
            && (!script_bool(script, "cannedFood") || !script_bool(script, "cantEat"))
        {
            let food = it
                .food
                .as_mut()
                .ok_or_else(|| unported("a food without state"))?;
            if food.off_age_max < 1_000_000_000 {
                if food.is_rotten() {
                    return Ok(());
                }
                if self.rand.next(100) < 75 {
                    food.rotten = true;
                    food.age = food.off_age_max as f32;
                } else if food.is_fresh() && self.rand.next(100) < 95 {
                    food.age = food.off_age as f32;
                }
            }
        }
        if it.class == ItemType::Clothing {
            self.clothing_randomize_condition(it, 0, 75, 1, 25)?;
        }
        Ok(())
    }
}
