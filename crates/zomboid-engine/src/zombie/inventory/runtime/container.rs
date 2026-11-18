//! zombie.inventory.ItemContainer: a container with no parent object (the `parent` of the original is null: no character,
//! dead body, stove or world object, no vehicle part and no containing item), which is what the scripts' own bags and the
//! probes' boxes are. The operations are the original's, over the arenas of `World`.
use super::item::{ContainerId, ItemId, Kind};
use super::world::World;
use crate::java;
use crate::zombie::inventory::item_picker_roll::floating_point_correction;
use crate::zombie::scripting::objects::resource_location::ResourceLocation;

/// zombie.inventory.ItemContainer: the fields its operations here read or change.
#[derive(Clone, Debug)]
pub struct Container {
    pub type_name: String,
    pub id: i32,
    pub capacity: i32,
    pub items: Vec<ItemId>,
    pub explored: bool,
    pub has_been_looted: bool,
    pub custom_temperature: f32,
    pub only_accept_category: Option<String>,
    pub age_factor: f32,
    pub cooking_factor: f32,
}

/// What a query of `ItemContainer` matches an item against.
#[derive(Clone, Debug)]
pub enum Pred {
    /// `TypePredicate`: `compareType(type, item)`
    Type(String),
    /// `CategoryPredicate`: `item.getCategory().equals(category)`
    Category(String),
    /// `TagPredicate`: `item.hasTag(itemTag)`
    Tag(ResourceLocation),
    /// `item instanceof Food`
    Food,
}

/// ItemContainer.compareType (String, String)
fn compare_type_str(type1: &str, type2: &str) -> bool {
    if type1.contains('/') {
        let Some(p) = type1.find(type2) else {
            return false;
        };
        let b = type1.as_bytes();
        let ch_before = if p > 0 { b[p - 1] } else { 0 };
        let end = p + type2.len();
        let ch_after = if end < b.len() { b[end] } else { 0 };
        (ch_before == 0 && ch_after == b'/')
            || (ch_before == b'/' && ch_after == 0)
            || (ch_before == b'/' && ch_after == b'/')
    } else {
        type1 == type2
    }
}

impl World<'_> {
    /// zombie.inventory.ItemContainer(String containerName, IsoGridSquare square, IsoObject parent) with a null square and
    /// parent, then `setCapacity(capacity)`.
    pub fn new_container(&mut self, type_name: &str, capacity: i32) -> ContainerId {
        let mut c = Container {
            type_name: type_name.to_string(),
            id: -1,
            capacity: 50,
            items: Vec::new(),
            explored: false,
            has_been_looted: false,
            custom_temperature: 0.0,
            only_accept_category: None,
            age_factor: 1.0,
            cooking_factor: 1.0,
        };
        let id = self.containers.len();
        c.capacity = capacity;
        self.containers.push(c);
        if self.is_fridge(id) {
            self.containers[id].age_factor = 0.02;
            self.containers[id].cooking_factor = 0.0;
        }
        id
    }

    /// ItemContainer.setOnlyAcceptCategory: `StringUtils.discardNullOrWhitespace`.
    pub fn set_only_accept_category(&mut self, c: ContainerId, cat: &str) {
        self.containers[c].only_accept_category = if cat.trim().is_empty() {
            None
        } else {
            Some(cat.to_string())
        };
    }

    /// ItemContainer.isFreezer
    pub fn is_freezer(&self, c: ContainerId) -> bool {
        let t = &self.containers[c].type_name;
        t != "none" && (t == "base:freezer" || t == "freezer")
    }

    /// ItemContainer.isFridge (no parent object: the type alone decides)
    pub fn is_fridge(&self, c: ContainerId) -> bool {
        let t = &self.containers[c].type_name;
        if t == "none" || self.is_freezer(c) {
            return false;
        }
        t == "base:fridge" || t == "fridge"
    }

    /// ItemContainer.getCapacity: no vehicle seat, no vehicle part, no containing item: `min(capacity, 100)`.
    pub fn container_capacity(&self, c: ContainerId) -> i32 {
        self.containers[c].capacity.min(100)
    }

    /// ItemContainer.getEffectiveCapacity(null)
    pub fn effective_capacity(&self, c: ContainerId) -> i32 {
        self.container_capacity(c)
    }

    /// ItemContainer.getTemperature with no parent object: the custom temperature, else 1 (a fridge or freezer without
    /// power), except in the hours of the day the power is shut off.
    pub fn container_temperature(&self, c: ContainerId) -> Result<f32, String> {
        let cont = &self.containers[c];
        if cont.custom_temperature != 0.0 {
            return Ok(cont.custom_temperature);
        }
        // isPowered(): `parent != null && ...` is false; stove, barbecue and fireplace need a parent
        if self.is_fridge_or_freezer_warming(c)? {
            let delta = (self.game_time.time_of_day - 7.0f32) / 6.0f32;
            return Ok(lerp(0.2f32, 1.0f32, delta));
        }
        Ok(1.0)
    }

    /// ItemContainer.isFridgeOrFreezerWarming
    pub fn is_fridge_or_freezer_warming(&self, c: ContainerId) -> Result<bool, String> {
        if !self.is_fridge(c) && !self.is_freezer(c) {
            return Ok(false);
        }
        let world_age_days = self.world_age_days_since_begin()? as i32;
        Ok(world_age_days == self.sandbox_int("ElecShutModifier")?
            && self.game_time.time_of_day < 13.0f32)
    }

    /// GameTime.getWorldAgeDaysSinceBegin: the world age in days plus the days of the TimeSinceApo option.
    pub fn world_age_days_since_begin(&self) -> Result<f64, String> {
        let since_apo = self.sandbox_int("TimeSinceApo")?;
        Ok(self.game_time.get_world_age_hours() / 24.0 + ((since_apo - 1) * 30) as f64)
    }

    /// An integer or enum sandbox option as `SandboxOptions.instance` holds it.
    pub fn sandbox_int(&self, name: &str) -> Result<i32, String> {
        self.sandbox
            .get_int(name)
            .ok_or_else(|| format!("not ported: sandbox option {name}"))
    }

    /// ItemContainer.getContentsWeight
    pub fn container_contents_weight(&self, c: ContainerId) -> f32 {
        let mut total = 0.0f32;
        for &i in &self.containers[c].items {
            total += self.unequipped_weight(i);
        }
        total
    }

    /// ItemContainer.getMaxWeight: the capacity (no parent character).
    pub fn container_max_weight(&self, c: ContainerId) -> f32 {
        self.container_capacity(c) as f32
    }

    /// ItemContainer.getCapacityWeight (no parent character or dead body): the contents weight.
    pub fn capacity_weight(&self, c: ContainerId) -> f32 {
        self.container_contents_weight(c)
    }

    /// ItemContainer.getAvailableWeightCapacity
    pub fn available_weight_capacity(&self, c: ContainerId) -> f32 {
        let occupied = self.capacity_weight(c);
        let max = self.container_max_weight(c);
        max - occupied
    }

    /// ItemContainer.getFreeCapacity(null)
    pub fn free_capacity(&self, c: ContainerId) -> f32 {
        (self.effective_capacity(c) as f32 - floating_point_correction(self.capacity_weight(c)))
            .max(0.0)
    }

    /// ItemContainer.isFull(null)
    pub fn is_full(&self, c: ContainerId) -> bool {
        self.free_capacity(c) <= 0.0
    }

    /// ItemContainer.isItemAllowed for a container with no parent object: the OnlyAcceptCategory; the accept function
    /// of a bag (Lua) is not modelled, and these containers have none.
    pub fn is_item_allowed(&self, c: ContainerId, item: ItemId) -> bool {
        let it = &self.items[item];
        // `item.getType().contains("Corpse") && parent instanceof IsoDeadBody` needs a parent
        if let Some(cat) = &self.containers[c].only_accept_category {
            if !java::equals_ignore_case(cat, self.category(item)) {
                return false;
            }
        }
        // `this.isInvalidClothingRackItem(item)`: the container type clothingrack takes clothing and rack-fitting items
        if self.containers[c].type_name == "clothingrack"
            && !it.is_clothing()
            && !self.has_tag(item, "base:fitsclothingrack")
        {
            return false;
        }
        true
    }

    /// ItemContainer.hasRoomFor(null, item): a container with no parent, no containing item and no world item.
    pub fn has_room_for_item(&self, c: ContainerId, item: ItemId) -> bool {
        if !self.is_item_allowed(c, item) {
            return false;
        }
        self.has_room_for(c, self.unequipped_weight(item))
    }

    /// ItemContainer.hasRoomFor(null, weightVal) = `hasRoomFor(null, weightVal, weightVal)` for a container with no
    /// parent: `floatingPointCorrection(getCapacityWeight()) + weightVal <= getEffectiveCapacity(null)`.
    pub fn has_room_for(&self, c: ContainerId, weight_val: f32) -> bool {
        floating_point_correction(self.capacity_weight(c)) + weight_val
            <= self.effective_capacity(c) as f32
    }

    /// ItemContainer.containsID
    pub fn contains_id(&self, c: ContainerId, id: i32) -> bool {
        self.containers[c]
            .items
            .iter()
            .any(|&i| self.items[i].id == id)
    }

    /// ItemContainer.getItemWithID
    pub fn item_with_id(&self, c: ContainerId, id: i32) -> Option<ItemId> {
        self.containers[c]
            .items
            .iter()
            .copied()
            .find(|&i| self.items[i].id == id)
    }

    /// IsoCell.addToProcessItems
    pub fn add_to_process_items(&mut self, item: ItemId) {
        self.process_items_remove.retain(|&i| i != item);
        if !self.process_items.contains(&item) {
            self.process_items.push(item);
        }
    }

    /// IsoCell.addToProcessItemsRemove
    pub fn add_to_process_items_remove(&mut self, item: ItemId) {
        if !self.process_items_remove.contains(&item) {
            self.process_items_remove.push(item);
        }
    }

    /// ItemContainer.AddItem(String type): a new item of the type is put in the container (a Food takes the container's
    /// temperature). `Ok(None)` is the original's null (no such script, an obsolete one, or the factory's null).
    pub fn add_item_type(
        &mut self,
        c: ContainerId,
        type_name: &str,
    ) -> Result<Option<ItemId>, String> {
        let Some(si) = self.scripts.find_item(type_name) else {
            return Ok(None);
        };
        if self.scripts.is_obsolete(si) {
            return Ok(None);
        }
        let Some(item) = self.create_item(type_name)? else {
            return Ok(None);
        };
        self.items[item].container = Some(c);
        self.containers[c].items.push(item);
        if matches!(self.items[item].kind, Kind::Food(_)) {
            let t = self.container_temperature(c)?;
            self.set_food_heat(item, t);
        }
        self.add_to_process_items(item);
        Ok(Some(item))
    }

    /// ItemContainer.AddItem(InventoryItem): an item whose id the container already holds gives that item back.
    pub fn add_item(&mut self, c: ContainerId, item: ItemId) -> Result<ItemId, String> {
        let id = self.items[item].id;
        if self.contains_id(c, id) {
            // DebugType.General.error("Error, container already has id")
            return Ok(self.item_with_id(c, id).unwrap_or(item));
        }
        if let Some(old) = self.items[item].container {
            self.remove_item(old, item)?;
        }
        self.items[item].container = Some(c);
        self.containers[c].items.push(item);
        self.add_to_process_items(item);
        self.on_added_to_container(item)?;
        Ok(item)
    }

    /// InventoryItem.OnAddedToContainer and the Food override (`updateAge()`).
    fn on_added_to_container(&mut self, item: ItemId) -> Result<(), String> {
        if matches!(self.items[item].kind, Kind::Food(_)) {
            self.update_age(item, false)?;
        }
        Ok(())
    }

    /// InventoryItem.OnBeforeRemoveFromContainer and the Food override (`updateAge()`).
    fn on_before_remove_from_container(&mut self, item: ItemId) -> Result<(), String> {
        if matches!(self.items[item].kind, Kind::Food(_)) {
            self.update_age(item, false)?;
        }
        Ok(())
    }

    /// ItemContainer.Remove(InventoryItem)
    pub fn remove_item(&mut self, c: ContainerId, item: ItemId) -> Result<(), String> {
        let Some(m) = self.containers[c].items.iter().position(|&i| i == item) else {
            return Ok(());
        };
        self.on_before_remove_from_container(item)?;
        self.containers[c].items.remove(m);
        self.items[item].container = None;
        Ok(())
    }

    /// ItemContainer.Remove(String itemTypes): the first item of the type loses a use, or goes when it has one (the
    /// original clears its `container` either way).
    pub fn remove_type(&mut self, c: ContainerId, type_name: &str) -> Result<(), String> {
        for m in 0..self.containers[c].items.len() {
            let item = self.containers[c].items[m];
            if self.items[item].type_name == type_name {
                let uses = self.current_uses(item);
                if uses > 1 {
                    self.set_current_uses_virtual(item, uses - 1);
                } else {
                    self.containers[c].items.retain(|&i| i != item);
                }
                self.items[item].container = None;
                return Ok(());
            }
        }
        Ok(())
    }

    /// ItemContainer.RemoveAll(String itemType, int count)
    pub fn remove_all(&mut self, c: ContainerId, type_name: &str, count: i32) -> Vec<ItemId> {
        let mut to_remove = Vec::new();
        for i in 0..self.containers[c].items.len() {
            let item = self.containers[c].items[i];
            let it = &self.items[item];
            if it.type_name == type_name || it.full_type == type_name {
                self.items[item].container = None;
                to_remove.push(item);
                if to_remove.len() as i32 >= count {
                    break;
                }
            }
        }
        for &r in &to_remove {
            if let Some(p) = self.containers[c].items.iter().position(|&i| i == r) {
                self.containers[c].items.remove(p);
            }
        }
        to_remove
    }

    /// ItemContainer.RemoveOneOf(String, boolean insideInv): no item here is a bag, so `insideInv` finds nothing more.
    pub fn remove_one_of(
        &mut self,
        c: ContainerId,
        string: &str,
    ) -> Result<Option<ItemId>, String> {
        for m in 0..self.containers[c].items.len() {
            let item = self.containers[c].items[m];
            if self.items[item].full_type == string || self.items[item].type_name == string {
                let uses = self.current_uses(item);
                if uses > 1 {
                    self.set_current_uses_virtual(item, uses - 1);
                } else {
                    self.items[item].container = None;
                    self.containers[c].items.retain(|&i| i != item);
                }
                return Ok(Some(item));
            }
        }
        Ok(None)
    }

    /// ItemContainer.compareType (String type, InventoryItem item)
    pub fn compare_type(&self, type_name: &str, item: ItemId) -> bool {
        let it = &self.items[item];
        if !type_name.contains('.') {
            compare_type_str(type_name, &it.type_name)
        } else {
            compare_type_str(type_name, &it.full_type) || compare_type_str(type_name, &it.type_name)
        }
    }

    /// `InventoryItem.hasTag(ItemTag)` for the tag at `location`.
    pub fn has_tag(&self, item: ItemId, location: &str) -> bool {
        match ResourceLocation::of(location) {
            Ok(l) => self.scripts.item(self.items[item].script).tags.contains(&l),
            Err(_) => false,
        }
    }

    /// InventoryItem.getCategory
    pub fn category(&self, item: ItemId) -> &'static str {
        self.items[item].category()
    }

    fn test(&self, p: &Pred, item: ItemId) -> bool {
        match p {
            Pred::Type(t) => self.compare_type(t, item),
            Pred::Category(c) => self.category(item) == c,
            Pred::Tag(t) => self.scripts.item(self.items[item].script).tags.contains(t),
            Pred::Food => matches!(self.items[item].kind, Kind::Food(_)),
        }
    }

    /// ItemContainer.getFirst(predicate)
    pub fn get_first(&self, c: ContainerId, p: &Pred) -> Option<ItemId> {
        self.containers[c]
            .items
            .iter()
            .copied()
            .find(|&i| self.test(p, i))
    }

    /// ItemContainer.getSome(predicate, count, result): the first matches, up to `count`.
    pub fn get_some(&self, c: ContainerId, p: &Pred, count: i32) -> Vec<ItemId> {
        let mut result = Vec::new();
        for &i in &self.containers[c].items {
            if self.test(p, i) {
                result.push(i);
                if result.len() as i32 >= count {
                    break;
                }
            }
        }
        result
    }

    /// ItemContainer.getAll(predicate)
    pub fn get_all(&self, c: ContainerId, p: &Pred) -> Vec<ItemId> {
        self.containers[c]
            .items
            .iter()
            .copied()
            .filter(|&i| self.test(p, i))
            .collect()
    }

    /// ItemContainer.getCount(predicate)
    pub fn get_count(&self, c: ContainerId, p: &Pred) -> i32 {
        self.get_all(c, p).len() as i32
    }

    /// ItemContainer.getBestCondition(predicate): the matching item with the highest condition (the first of equals),
    /// null when its condition is not above zero.
    pub fn get_best_condition(&self, c: ContainerId, p: &Pred) -> Option<ItemId> {
        let all = self.get_all(c, p);
        let mut best = *all.first()?;
        for &i in &all[1..] {
            // ConditionComparator: o1.getCondition() - o2.getCondition()
            if self.items[i]
                .condition
                .wrapping_sub(self.items[best].condition)
                > 0
            {
                best = i;
            }
        }
        if self.items[best].condition <= 0 {
            return None;
        }
        Some(best)
    }

    /// ItemContainer.findItem(String type, boolean doInv, boolean ignoreBroken) (`doInv` finds nothing more: no item here
    /// is a bag).
    pub fn find_item(
        &self,
        c: ContainerId,
        type_name: &str,
        ignore_broken: bool,
    ) -> Option<ItemId> {
        if type_name.contains('/') {
            for variant in java::split_char(type_name, '/') {
                if let Some(i) = self.find_item(c, java::trim(variant), ignore_broken) {
                    return Some(i);
                }
            }
            None
        } else if type_name.contains("Type:") {
            if type_name.contains("Food") {
                self.get_first(c, &Pred::Food)
            } else {
                // `Type:Weapon` and `Type:AlarmClock` match HandWeapon and AlarmClock items, which these containers
                // do not hold in the scenarios (such an item is not modelled)
                None
            }
        } else {
            let t = java::trim(type_name);
            self.containers[c]
                .items
                .iter()
                .copied()
                .find(|&i| self.compare_type(t, i) && (ignore_broken || !self.items[i].broken))
        }
    }

    /// ItemContainer.contains(String type, boolean doInv, boolean ignoreBroken)
    pub fn contains_type(&self, c: ContainerId, type_name: &str, ignore_broken: bool) -> bool {
        self.find_item(c, type_name, ignore_broken).is_some()
    }

    /// ItemContainer.containsWithModule(String moduleType, boolean withDeltaLeft)
    pub fn contains_with_module(
        &self,
        c: ContainerId,
        module_type: &str,
        with_delta_left: bool,
    ) -> bool {
        let mut type_name = module_type.to_string();
        let mut module = "Base".to_string();
        if module_type.contains('.') {
            let parts = java::split_char(module_type, '.');
            module = parts[0].to_string();
            type_name = parts.get(1).copied().unwrap_or("").to_string();
        }
        self.containers[c].items.iter().any(|&i| {
            let it = &self.items[i];
            it.type_name == java::trim(&type_name)
                && module == it.module
                && (!with_delta_left
                    || !matches!(it.kind, Kind::Drainable(_))
                    || self.current_uses(i) > 0)
        })
    }

    /// ItemContainer.getNumberOfItem(String findItem, boolean includeReplaceOnDeplete, boolean insideInv=false)
    pub fn number_of_item(
        &self,
        c: ContainerId,
        find: &str,
        include_replace_on_deplete: bool,
    ) -> i32 {
        let mut result = 0;
        for &i in &self.containers[c].items {
            let it = &self.items[i];
            if it.full_type == find || it.type_name == find {
                result += 1;
            } else if include_replace_on_deplete {
                if let Kind::Drainable(d) = &it.kind {
                    if let (Some(r), Some(full)) =
                        (&d.replace_on_deplete, &d.replace_on_deplete_full_type)
                    {
                        if full == find || r == find {
                            result += 1;
                        }
                    }
                }
            }
        }
        result
    }

    /// ItemContainer.FindAndReturnStack(String type): `InventoryItem.CanStack` is false for every class modelled here (only
    /// Clothing overrides it), so no item stacks.
    pub fn find_and_return_stack(
        &mut self,
        c: ContainerId,
        type_name: &str,
    ) -> Result<Option<ItemId>, String> {
        for m in 0..self.containers[c].items.len() {
            let item = self.containers[c].items[m];
            if self.compare_type(type_name, item) {
                let module = self.items[item].module.clone();
                // `InventoryItemFactory.CreateItem(item.module + "." + type)`: draws the generator like any creation
                self.create_item(&format!("{module}.{type_name}"))?;
            }
        }
        Ok(None)
    }
}

/// GameTime.Lerp
pub fn lerp(start: f32, end: f32, delta: f32) -> f32 {
    let delta = delta.clamp(0.0, 1.0);
    let amount = end - start;
    let result = amount * delta;
    start + result
}
