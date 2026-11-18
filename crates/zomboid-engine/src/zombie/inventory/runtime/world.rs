//! What item and container operations read from the rest of the game, and the arenas that hold them.
use std::collections::HashMap;

use super::container::Container;
use super::item::{ItemId, Kind, LiveItem};
use crate::java::Random;
use crate::zombie::core::random::rand_abstract::RandAbstract;
use crate::zombie::game_time::GameTime;
use crate::zombie::inventory::inventory_item::ItemType;
use crate::zombie::inventory::item_picker_roll::{
    script_actual_weight, script_complex, script_float, script_item_type, script_str, unported,
};
use crate::zombie::sandbox_options::SandboxOptions;
use crate::zombie::scripting::item::{Item, Value};
use crate::zombie::scripting::script_manager::{BucketIndex, ScriptManager};

/// `ScriptManager.instance`'s items with the lookups `ScriptManager.FindItem`/`getItem` use.
pub struct Scripts<'a> {
    pub manager: &'a ScriptManager,
    index: BucketIndex,
    by_full: HashMap<String, usize>,
}

impl<'a> Scripts<'a> {
    pub fn new(manager: &'a ScriptManager) -> Self {
        Scripts {
            manager,
            index: manager.item_index(),
            by_full: (manager.items.iter().enumerate())
                .map(|(i, item)| (item.full_name(), i))
                .collect(),
        }
    }

    /// ScriptManager.FindItem(name): the index of the item script, `None` for the original's null.
    pub fn find_item(&self, name: &str) -> Option<usize> {
        let full = self.manager.find_item(&self.index, name)?;
        self.by_full.get(&full).copied()
    }

    pub fn item(&self, index: usize) -> &'a Item {
        &self.manager.items[index]
    }

    /// `Item.obsolete`
    pub fn is_obsolete(&self, index: usize) -> bool {
        matches!(self.item(index).get("obsolete"), Some(Value::Bool(true)))
    }
}

/// The items and containers of a scenario and everything outside them that their operations read.
pub struct World<'a> {
    pub scripts: &'a Scripts<'a>,
    /// `Rand` (`RandStandard.INSTANCE`): a seeded `java.util.Random` here.
    pub rand: RandAbstract<Random>,
    pub game_time: GameTime,
    pub sandbox: SandboxOptions,
    /// `(float)ZomboidGlobals.equippedOrWornEncumbranceMultiplier`
    pub equipped_multiplier: f32,
    pub items: Vec<LiveItem>,
    pub containers: Vec<Container>,
    /// `IsoWorld.instance.currentCell.processItems`
    pub process_items: Vec<ItemId>,
    /// `IsoWorld.instance.currentCell.processItemsRemove`
    pub process_items_remove: Vec<ItemId>,
}

impl<'a> World<'a> {
    pub fn new(
        scripts: &'a Scripts<'a>,
        sandbox: SandboxOptions,
        equipped_multiplier: f64,
        seed: i64,
    ) -> Self {
        World {
            scripts,
            rand: RandAbstract::new(Random::new(seed)),
            game_time: GameTime::default(),
            sandbox,
            equipped_multiplier: equipped_multiplier as f32,
            items: Vec::new(),
            containers: Vec::new(),
            process_items: Vec::new(),
            process_items_remove: Vec::new(),
        }
    }

    /// The item class a script makes (`Item.itemType`): `Normal` unless the script names another.
    pub fn script_class(&self, script: usize) -> Result<ItemType, String> {
        script_item_type(self.scripts.item(script))
    }

    /// zombie.inventory.InventoryItemFactory.CreateItem(itemType): `InstanceItem`, then the factory's own steps (the
    /// random id; a drainable starts full). `Ok(None)` is the original's null (no such script). A script that
    /// `InstanceItem` would give a class, component, `OnCreate` or ammo type this port does not model is an `Err`.
    pub fn create_item(&mut self, item_type: &str) -> Result<Option<ItemId>, String> {
        let Some(si) = self.scripts.find_item(item_type) else {
            return Ok(None);
        };
        let script = self.scripts.item(si);
        let class = self.script_class(si)?;
        if script.has_components() {
            return Err(unported(format!("component ({})", script.full_name())));
        }
        if script_str(script, "luaCreate").is_some() {
            return Err(unported(format!("OnCreate ({})", script.full_name())));
        }
        let mut item = LiveItem::instance(si, script, class)?;
        // InstanceItem: `OutfitRNG.setSeed(Rand.Next(Integer.MAX_VALUE))`, then `item.synchWithVisual()`
        self.rand.next(i32::MAX);
        self.synch_with_visual(&mut item, script)?;
        // the factory: `item.id = Rand.Next(2146250223) + 1233423`
        item.id = self.rand.next(2_146_250_223).wrapping_add(1_233_423);
        let id = self.items.len();
        self.items.push(item);
        // `if (item instanceof Drainable) item.setCurrentUses((int)(item.getMaxUses() * useDelta))`, useDelta 1
        if matches!(self.items[id].kind, Kind::Drainable(_)) {
            let uses = (self.items[id].max_uses() as f32 * 1.0f32) as i32;
            self.set_current_uses(id, uses);
        }
        Ok(Some(id))
    }

    /// InventoryItem.synchWithVisual for the classes modelled here (not a HandWeapon, Clothing or InventoryContainer): a
    /// fresh item whose script lists `StaticModelsByIndex` or `WorldStaticModelsByIndex` picks its model index (the
    /// larger of the two lists), and the icon of that index replaces its texture. The texture is not modelled; a script
    /// whose `IconsForTexture` is too short for the index makes the original throw, which is an `Err` here.
    fn synch_with_visual(&mut self, item: &mut LiveItem, script: &Item) -> Result<(), String> {
        let list = |key: &str| -> Option<usize> {
            let v = script_complex(script, key)?;
            let n = crate::java::split_char(v, ';').len();
            (n > 0).then_some(n)
        };
        let (a, b) = (
            list("staticmodelsbyindex"),
            list("worldstaticmodelsbyindex"),
        );
        if (a.is_some() || b.is_some()) && item.model_index == -1 {
            let max_index = match (a, b) {
                (Some(a), Some(b)) => a.max(b),
                (Some(a), None) => a,
                (None, b) => b.unwrap_or(0),
            };
            item.model_index = self.rand.next(max_index as i32);
        }
        if item.model_index != -1 {
            if let Some(icons) = script_complex(script, "iconsfortexture") {
                let n = crate::java::split_char(icons, ';').len();
                if n > 0 && item.model_index as usize >= n {
                    return Err(unported(format!(
                        "IndexOutOfBoundsException in synchWithVisual ({})",
                        script.full_name()
                    )));
                }
            }
        }
        Ok(())
    }

    /// InventoryItem.setCurrentUses and the DrainableComboItem override (which updates the weight).
    pub fn set_current_uses(&mut self, id: ItemId, uses: i32) {
        self.items[id].uses = uses;
        if matches!(self.items[id].kind, Kind::Drainable(_)) {
            self.update_weight(id);
        }
    }

    /// DrainableComboItem.updateWeight
    pub fn update_weight(&mut self, id: ItemId) {
        let script = self.scripts.item(self.items[id].script);
        let (replace, replace_full, weight_empty) = match &self.items[id].kind {
            Kind::Drainable(d) => (
                d.replace_on_deplete.is_some(),
                d.replace_on_deplete_full_type.clone(),
                d.weight_empty,
            ),
            _ => return,
        };
        let uses_float = self.items[id].current_uses_float();
        if replace {
            if uses_float >= 1.0f32 {
                self.items[id].custom_weight = true;
                self.items[id].set_actual_weight(script_actual_weight(script));
                let aw = self.actual_weight(id);
                self.items[id].set_weight(aw);
                return;
            }
            let empty = replace_full
                .as_deref()
                .and_then(|f| self.scripts.find_item(f));
            if let Some(e) = empty {
                let empty_aw = script_actual_weight(self.scripts.item(e));
                self.items[id].custom_weight = true;
                self.items[id].set_actual_weight(
                    (script_actual_weight(script) - empty_aw) * uses_float + empty_aw,
                );
                let aw = self.actual_weight(id);
                self.items[id].set_weight(aw);
            }
        }
        if weight_empty != 0.0 {
            self.items[id].custom_weight = true;
            self.items[id].set_actual_weight(
                (script_actual_weight(script) - weight_empty) * uses_float + weight_empty,
            );
        }
    }

    /// InventoryItem.getActualWeightUnmodded, getActualWeight and the Food override.
    pub fn actual_weight(&self, id: ItemId) -> f32 {
        let it = &self.items[id];
        let unmodded = || {
            if it.name == it.full_type {
                0.0f32
            } else {
                it.actual_weight.max(0.0f32)
            }
        };
        let Kind::Food(f) = &it.kind else {
            return unmodded();
        };
        let script = self.scripts.item(it.script);
        let used_delta = |this: &Self| {
            let _ = this;
            let mut used = 1.0f32;
            let hunger = script_float(script, "hungerChange");
            let thirst = script_float(script, "thirstChange");
            if hunger < 0.0 {
                used = f.hung_change * 100.0f32 / hunger;
            } else if thirst < 0.0 {
                used = self.food_thirst_change(id) * 100.0f32 / thirst;
            }
            used
        };
        if !it.extra_items.is_empty() {
            // haveExtraItems: the extra items' weights are not modelled
            return f32::NAN;
        }
        if it.replace_on_use.is_some() && !it.custom_weight {
            let empty = it
                .replace_on_use_full_type
                .as_deref()
                .and_then(|n| self.scripts.find_item(n));
            if let Some(e) = empty {
                let used = used_delta(self);
                let ew = script_actual_weight(self.scripts.item(e));
                return (script_actual_weight(script) - ew) * used + ew;
            }
        } else if !it.custom_weight {
            return script_actual_weight(script) * used_delta(self);
        }
        unmodded()
    }

    /// Food.getThirstChange
    pub fn food_thirst_change(&self, id: ItemId) -> f32 {
        let it = &self.items[id];
        let Kind::Food(f) = &it.kind else {
            return 0.0;
        };
        if it.burnt {
            f.thirst_change / 5.0f32
        } else if it.cooked {
            f.thirst_change / 2.0f32
        } else {
            f.thirst_change
        }
    }

    /// InventoryItem.getWeight and the Food override.
    pub fn weight(&self, id: ItemId) -> f32 {
        let it = &self.items[id];
        if matches!(it.kind, Kind::Food(_)) && it.replace_on_use.is_some() {
            return self.actual_weight(id);
        }
        it.weight.max(0.0f32)
    }

    /// InventoryItem.getContentsWeight for an item without a fluid container: the rounds it holds.
    pub fn contents_weight(&self, id: ItemId) -> f32 {
        let it = &self.items[id];
        if let Some(key) = &it.ammo_item {
            if let Some(si) = self.scripts.find_item(key) {
                return script_actual_weight(self.scripts.item(si)) * it.current_ammo_count as f32;
            }
        }
        0.0
    }

    /// InventoryItem.getUnequippedWeight
    pub fn unequipped_weight(&self, id: ItemId) -> f32 {
        self.actual_weight(id) + self.contents_weight(id)
    }

    /// InventoryItem.getEquippedWeight
    pub fn equipped_weight(&self, id: ItemId) -> f32 {
        (self.actual_weight(id) + self.contents_weight(id)) * self.equipped_multiplier
    }
}
