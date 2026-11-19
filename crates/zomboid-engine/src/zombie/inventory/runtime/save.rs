//! `InventoryItem.save`/`load`, `Food.save`/`load`, `Literature.save`/`load`, `InventoryItem.loadItem`,
//! `ItemContainer.save`/`load` and `CompressIdenticalItems.save`/`load` on the live items of a `World`.
//!
//! Every byte goes through `ByteBufferWriter` (the original's `ByteBuffer.put*`) in the original's order. A save is
//! read back by the same code that reads a save file of the game (the `loadItem` framing, the flag headers); what an
//! item of a class this port does not model would need (a visual, a component, an animal track, a texture) is
//! `Err("not ported ...")`.
use super::container::Container;
use super::item::{ContainerId, ItemId, Kind};
use super::world::World;
use crate::java_nio::{
    pack_float_unit_to_byte, read_string, unpack_byte_to_float_unit, write_string, BitHeaderRead,
    BitHeaderWrite, ByteBuffer, ByteBufferWriter, HeaderSize,
};
use crate::se::krka::kahlua::j2se::kahlua_table_impl::{KahluaTable, LuaValue};
use crate::zombie::inventory::item_picker_roll::unported;

impl World<'_> {
    /// InventoryItem.getModData: the table is made when it is first asked for.
    pub fn mod_data(&mut self, id: ItemId) -> &mut KahluaTable {
        self.items[id]
            .mod_data
            .get_or_insert_with(KahluaTable::default)
    }

    /// InventoryItem.setCustomName(boolean): the flag, and modData `customName` is the item's name.
    pub fn set_custom_name(&mut self, id: ItemId, custom: bool) {
        self.items[id].custom_name = custom;
        let name = self.items[id].name.clone();
        self.mod_data(id).rawset(
            LuaValue::String("customName".into()),
            LuaValue::String(name),
        );
    }

    /// InventoryItem.save (ByteBuffer, boolean) with the class overrides of Food and Literature (each writes
    /// `super.save` first). `id_value` is what goes into the id field: the item's own, or 0 where
    /// `CompressIdenticalItems` compares two items.
    fn save_item_as(
        &self,
        id: ItemId,
        w: &mut ByteBufferWriter,
        id_value: i32,
    ) -> Result<(), String> {
        self.save_base(id, w, id_value)?;
        match &self.items[id].kind {
            Kind::Food(_) => self.save_food(id, w),
            Kind::Literature(_) => self.save_literature(id, w),
            Kind::Combo | Kind::Drainable(_) => Ok(()),
        }
    }

    /// InventoryItem.save (ByteBuffer, boolean) of the item itself.
    pub fn save_item(&self, id: ItemId, w: &mut ByteBufferWriter) -> Result<(), String> {
        self.save_item_as(id, w, self.items[id].id)
    }

    /// InventoryItem.saveWithSize: an int length, then `save`.
    pub fn save_item_with_size(&self, id: ItemId, w: &mut ByteBufferWriter) -> Result<(), String> {
        self.save_item_with_size_as(id, w, self.items[id].id)
    }

    fn save_item_with_size_as(
        &self,
        id: ItemId,
        w: &mut ByteBufferWriter,
        id_value: i32,
    ) -> Result<(), String> {
        let position1 = w.position();
        w.put_int(0);
        let position2 = w.position();
        self.save_item_as(id, w, id_value)?;
        let position3 = w.position();
        w.set_position(position1)?;
        w.put_int((position3 - position2) as i32);
        w.set_position(position3)
    }

    /// zombie.inventory.InventoryItem.save
    fn save_base(&self, id: ItemId, w: &mut ByteBufferWriter, id_value: i32) -> Result<(), String> {
        let it = &self.items[id];
        w.put_short(it.registry_id);
        w.put(-1);
        w.put_int(id_value);
        let mut header = BitHeaderWrite::alloc_write(HeaderSize::Byte, w);
        let uses = self.current_uses(id);
        if uses != 1 {
            header.add_flags(1);
            w.put_int(uses);
        }
        if it.condition != it.condition_max {
            header.add_flags(4);
            w.put(it.condition as i8);
        }
        // `visual != null` (flag 8): only clothing and weapons have one
        if it.custom_color
            && (it.col[0] != 1.0 || it.col[1] != 1.0 || it.col[2] != 1.0 || it.col[3] != 1.0)
        {
            header.add_flags(16);
            for c in it.col {
                w.put(pack_float_unit_to_byte(c));
            }
        }
        if it.item_capacity != -1.0f32 {
            header.add_flags(32);
            w.put_float(it.item_capacity);
        }
        let mut bits = BitHeaderWrite::alloc_write(HeaderSize::Integer, w);
        if let Some(t) = it.mod_data.as_ref().filter(|t| !t.entries.is_empty()) {
            bits.add_flags(1);
            t.save(w);
        }
        if it.activated {
            bits.add_flags(2);
        }
        if it.have_been_repaired != 0 {
            bits.add_flags(4);
            w.put_short(it.have_been_repaired as i16);
        }
        if it.name != it.original_name {
            bits.add_flags(8);
            write_string(w, Some(&it.name));
        }
        if let Some(d) = &it.byte_data {
            bits.add_flags(16);
            w.put_int(d.len() as i32);
            w.put_bytes(d);
        }
        if !it.extra_items.is_empty() {
            bits.add_flags(32);
            w.put_int(it.extra_items.len() as i32);
            for t in &it.extra_items {
                w.put_short(self.scripts.registry_id(t));
            }
        }
        if it.custom_name {
            bits.add_flags(64);
        }
        if it.custom_weight {
            bits.add_flags(128);
            w.put_float(self.actual_weight(id));
        }
        if it.key_id != -1 {
            bits.add_flags(256);
            w.put_int(it.key_id);
        }
        if it.remote_control_id != -1 || it.remote_range != 0 {
            bits.add_flags(1024);
            w.put_int(it.remote_control_id);
            w.put_int(it.remote_range);
        }
        if it.color_red != 1.0 || it.color_green != 1.0 || it.color_blue != 1.0 {
            bits.add_flags(2048);
            w.put(pack_float_unit_to_byte(it.color_red));
            w.put(pack_float_unit_to_byte(it.color_green));
            w.put(pack_float_unit_to_byte(it.color_blue));
        }
        if let Some(s) = &it.worker {
            bits.add_flags(4096);
            write_string(w, Some(s));
        }
        if it.wet_cooldown != -1.0f32 {
            bits.add_flags(8192);
            w.put_float(it.wet_cooldown);
        }
        if it.favorite {
            bits.add_flags(16384);
        }
        if let Some(s) = &it.stash_map {
            bits.add_flags(32768);
            write_string(w, Some(s));
        }
        if it.zombie_infected {
            bits.add_flags(65536);
        }
        if it.current_ammo_count != 0 {
            bits.add_flags(131072);
            w.put_int(it.current_ammo_count);
        }
        if it.attached_slot != -1 {
            bits.add_flags(262144);
            w.put_int(it.attached_slot);
        }
        if let Some(s) = &it.attached_slot_type {
            bits.add_flags(524288);
            write_string(w, Some(s));
        }
        if let Some(s) = &it.attached_to_model {
            bits.add_flags(1048576);
            write_string(w, Some(s));
        }
        if it.max_capacity != -1 {
            bits.add_flags(2097152);
            w.put_int(it.max_capacity);
        }
        if it.recorded_media_index >= 0 {
            bits.add_flags(4194304);
            w.put_short(it.recorded_media_index);
        }
        if it.world_scale != 1.0f32 {
            bits.add_flags(16777216);
            w.put_float(it.world_scale);
        }
        if it.is_initialised {
            bits.add_flags(33554432);
        }
        // `requiresEntitySave()` (the item has components), `animalTracks` and a `texture` that differs from the
        // script's icon (flag 268435456): none exists for the classes modelled here (the booted server makes its items
        // without a texture; the loader refuses a save that carries one)
        if it.model_index > -1 {
            bits.add_flags(536870912);
            w.put_int(it.model_index);
        }
        if it.world_x_rotation != 0.0f32
            || it.world_y_rotation != 0.0f32
            || it.world_z_rotation != -1.0f32
        {
            bits.add_flags(1073741824);
            w.put_float(it.world_x_rotation);
            w.put_float(it.world_y_rotation);
            w.put_float(it.world_z_rotation);
        }
        if !bits.equals(0) {
            header.add_flags(64);
            bits.write(w)?;
        } else {
            w.set_position(bits.start_position())?;
        }
        header.write(w)
    }

    /// zombie.inventory.types.Food.save (after `super.save`)
    fn save_food(&self, id: ItemId, w: &mut ByteBufferWriter) -> Result<(), String> {
        let it = &self.items[id];
        let Kind::Food(f) = &it.kind else {
            return Ok(());
        };
        w.put_float(it.age);
        w.put_float(it.last_aged);
        let mut header = BitHeaderWrite::alloc_write(HeaderSize::Byte, w);
        if f.calories != 0.0 || f.proteins != 0.0 || f.lipids != 0.0 || f.carbohydrates != 0.0 {
            header.add_flags(1);
            w.put_float(f.calories);
            w.put_float(f.proteins);
            w.put_float(f.lipids);
            w.put_float(f.carbohydrates);
        }
        if f.hung_change != 0.0 {
            header.add_flags(2);
            w.put_float(f.hung_change);
        }
        if f.base_hunger != 0.0 {
            header.add_flags(4);
            w.put_float(f.base_hunger);
        }
        if it.unhappy_change != 0.0 {
            header.add_flags(8);
            w.put_float(it.unhappy_change);
        }
        if it.boredom_change != 0.0 {
            header.add_flags(16);
            w.put_float(it.boredom_change);
        }
        if f.thirst_change != 0.0 {
            header.add_flags(32);
            w.put_float(f.thirst_change);
        }
        let mut bits = BitHeaderWrite::alloc_write(HeaderSize::Integer, w);
        if f.heat != 1.0 {
            bits.add_flags(1);
            w.put_float(f.heat);
        }
        if f.last_cook_minute != 0 {
            bits.add_flags(2);
            w.put_int(f.last_cook_minute);
        }
        if it.cooking_time != 0.0 {
            bits.add_flags(4);
            w.put_float(it.cooking_time);
        }
        if it.cooked {
            bits.add_flags(8);
        }
        if it.burnt {
            bits.add_flags(16);
        }
        if it.is_cookable {
            bits.add_flags(32);
        }
        if f.dangerous_uncooked {
            bits.add_flags(64);
        }
        if f.poison_detection_level != -1 {
            bits.add_flags(128);
            w.put(f.poison_detection_level as i8);
        }
        if let Some(spices) = &f.spices {
            bits.add_flags(256);
            w.put(spices.len() as i8);
            for s in spices {
                write_string(w, Some(s));
            }
        }
        if f.poison_power != 0 {
            bits.add_flags(512);
            w.put(f.poison_power as i8);
        }
        if let Some(c) = &f.chef {
            bits.add_flags(1024);
            write_string(w, Some(c));
        }
        if f64::from(it.off_age) != 1.0e9 {
            bits.add_flags(2048);
            w.put_int(it.off_age);
        }
        if f64::from(it.off_age_max) != 1.0e9 {
            bits.add_flags(4096);
            w.put_int(it.off_age_max);
        }
        if f.pain_reduction != 0.0 {
            bits.add_flags(8192);
            w.put_float(f.pain_reduction);
        }
        if f.flu_reduction != 0 {
            bits.add_flags(16384);
            w.put_int(f.flu_reduction);
        }
        if f.food_sickness_change != 0 {
            bits.add_flags(32768);
            w.put_int(f.food_sickness_change);
        }
        if f.poison {
            bits.add_flags(65536);
        }
        if f.use_for_poison != 0 {
            bits.add_flags(131072);
            w.put_short(f.use_for_poison as i16);
        }
        if f.freezing_time != 0.0 {
            bits.add_flags(262144);
            w.put_float(f.freezing_time);
        }
        if f.frozen {
            bits.add_flags(524288);
        }
        if f.last_frozen_update != 0.0 {
            bits.add_flags(1048576);
            w.put_float(f.last_frozen_update);
        }
        if f.rotten_time != 0.0 {
            bits.add_flags(2097152);
            w.put_float(f.rotten_time);
        }
        if f.compost_time != 0.0 {
            bits.add_flags(4194304);
            w.put_float(f.compost_time);
        }
        if f.cooked_in_microwave {
            bits.add_flags(8388608);
        }
        if it.fatigue_change != 0.0 {
            bits.add_flags(16777216);
            w.put_float(it.fatigue_change);
        }
        if f.end_change != 0.0 {
            bits.add_flags(33554432);
            w.put_float(f.end_change);
        }
        if f.milk_qty > 0 {
            bits.add_flags(67108864);
            w.put_int(f.milk_qty);
            write_string(w, f.milk_type.as_deref());
        }
        // `isFertilized()` (flag 134217728): set only by an egg, not modelled
        if it.stress_change != 0.0 {
            bits.add_flags(268435456);
            w.put_float(it.stress_change);
        }
        if !bits.equals(0) {
            header.add_flags(64);
            bits.write(w)?;
        } else {
            w.set_position(bits.start_position())?;
        }
        header.write(w)
    }

    /// zombie.inventory.types.Literature.save (after `super.save`)
    fn save_literature(&self, id: ItemId, w: &mut ByteBufferWriter) -> Result<(), String> {
        let Kind::Literature(l) = &self.items[id].kind else {
            return Ok(());
        };
        let mut bits = BitHeaderWrite::alloc_write(HeaderSize::Short, w);
        let mut number_page_type = 0;
        if l.number_of_pages >= 127 && l.number_of_pages < 32767 {
            number_page_type = 1;
        } else if l.number_of_pages >= 32767 {
            number_page_type = 2;
        }
        if l.number_of_pages != -1 {
            bits.add_flags(1);
            if number_page_type == 1 {
                bits.add_flags(2);
                w.put_short(l.number_of_pages as i16);
            } else if number_page_type == 2 {
                bits.add_flags(4);
                w.put_int(l.number_of_pages);
            } else {
                w.put(l.number_of_pages as i8);
            }
        }
        if l.already_read_pages != 0 {
            bits.add_flags(8);
            if number_page_type == 1 {
                w.put_short(l.already_read_pages as i16);
            } else if number_page_type == 2 {
                w.put_int(l.already_read_pages);
            } else {
                w.put(l.already_read_pages as i8);
            }
        }
        if l.can_be_write {
            bits.add_flags(16);
        }
        if let Some(pages) = l.custom_pages.as_ref().filter(|p| !p.is_empty()) {
            bits.add_flags(32);
            w.put_int(pages.len() as i32);
            for p in pages {
                write_string(w, Some(p));
            }
        }
        if let Some(s) = &l.locked_by {
            bits.add_flags(64);
            write_string(w, Some(s));
        }
        if let Some(recipes) = &l.learned_recipes {
            bits.add_flags(128);
            w.put_short(recipes.len() as i16);
            for r in recipes {
                write_string(w, Some(r));
            }
        }
        bits.write(w)
    }

    /// zombie.inventory.CompressIdenticalItems.areItemsIdentical for items without a container, attributes or
    /// component: the same `byteData`, and the same bytes saved with the id 0.
    fn items_identical(&self, a: ItemId, b: ItemId, bytes_a: &[u8]) -> Result<bool, String> {
        if self.items[a].byte_data != self.items[b].byte_data {
            return Ok(false);
        }
        let mut w = ByteBufferWriter::new();
        // `bb.putInt(0); item1.save(bb)` ... the second item is saved after the first, and only the bytes of
        // the item are compared (the length prefix of the first is not part of the comparison)
        self.save_item_as(b, &mut w, 0)?;
        Ok(w.bytes() == bytes_a)
    }

    /// zombie.inventory.CompressIdenticalItems.save: the items grouped by full type (in the order a type is first met),
    /// a run of identical neighbours saved once with its count and the other ids. Returns `savedItems`.
    pub fn compress_identical_items_save(
        &self,
        w: &mut ByteBufferWriter,
        items: &[ItemId],
    ) -> Result<Vec<ItemId>, String> {
        let mut types: Vec<&str> = Vec::new();
        let mut by_type: Vec<Vec<ItemId>> = Vec::new();
        for &i in items {
            let t = self.items[i].full_type.as_str();
            match types.iter().position(|x| *x == t) {
                Some(k) => by_type[k].push(i),
                None => {
                    types.push(t);
                    by_type.push(vec![i]);
                }
            }
        }
        let mut saved = Vec::new();
        let pos_size = w.position();
        w.put_short(0);
        let mut item_count: i32 = 0;
        for save_items in &by_type {
            let mut m = 0;
            while m < save_items.len() {
                let item = save_items[m];
                saved.push(item);
                let mut identical = 1;
                let start_m = m + 1;
                // setCompareItem: the item saved with the id 0
                let mut cw = ByteBufferWriter::new();
                self.save_item_as(item, &mut cw, 0)?;
                let bytes = cw.bytes().to_vec();
                while m + 1 < save_items.len()
                    && self.items_identical(item, save_items[m + 1], &bytes)?
                {
                    saved.push(save_items[m + 1]);
                    m += 1;
                    identical += 1;
                }
                w.put_int(identical);
                self.save_item_with_size(item, w)?;
                if identical > 1 {
                    for &other in &save_items[start_m..=m] {
                        w.put_int(self.items[other].id);
                    }
                }
                item_count += 1;
                m += 1;
            }
        }
        let pos_current = w.position();
        w.set_position(pos_size)?;
        w.put_short(item_count as i16);
        w.set_position(pos_current)?;
        Ok(saved)
    }

    /// zombie.inventory.ItemContainer.save (ByteBuffer): the type, the explored flag, the items, the looted flag and
    /// the capacity. Returns `savedItems`.
    pub fn container_save(
        &self,
        c: ContainerId,
        w: &mut ByteBufferWriter,
    ) -> Result<Vec<ItemId>, String> {
        let cont: &Container = &self.containers[c];
        write_string(w, Some(&cont.type_name));
        w.put(i8::from(cont.explored));
        let saved = self.compress_identical_items_save(w, &cont.items)?;
        w.put(i8::from(cont.has_been_looted));
        w.put_int(cont.capacity);
        Ok(saved)
    }

    /// zombie.inventory.InventoryItem.loadItem (ByteBuffer, int): the new item, `None` for the original's null (the
    /// script is not in the dictionary, or its `load` threw).
    pub fn load_item(
        &mut self,
        b: &mut ByteBuffer,
        world_version: i32,
    ) -> Result<Option<ItemId>, String> {
        let data_len = b.get_int()?;
        if data_len <= 0 {
            return Err(format!(
                "java.io.IOException: InventoryItem.loadItem() invalid item data length: {data_len}"
            ));
        }
        let position = b.position();
        let end = position + data_len as usize;
        let registry_id = b.get_short()?;
        let _save_type = b.get()?;
        let created = match self.scripts.registry_type(registry_id).map(str::to_string) {
            Some(full_type) => self.create_item(&full_type)?,
            None => None,
        };
        let mut item = created;
        if let Some(id) = created {
            match self.load_item_fields(id, b, world_version) {
                Ok(()) => {}
                Err(e) if e.starts_with("not ported") => return Err(e),
                // ExceptionLogger.logException(e); item = null
                Err(_) => item = None,
            }
        }
        if item.is_some() {
            if b.position() != end {
                b.set_position(end)?;
            }
        } else if b.position() >= end {
            b.set_position(end)?;
        } else {
            b.get_bytes(end - b.position())?;
        }
        Ok(item)
    }

    /// The item's `load` (ByteBuffer, int): `InventoryItem.load` and then the class's own part.
    fn load_item_fields(
        &mut self,
        id: ItemId,
        b: &mut ByteBuffer,
        world_version: i32,
    ) -> Result<(), String> {
        self.load_base(id, b, world_version)?;
        match self.items[id].kind {
            Kind::Food(_) => self.load_food(id, b, world_version),
            Kind::Literature(_) => self.load_literature(id, b, world_version),
            Kind::Combo | Kind::Drainable(_) => Ok(()),
        }
    }

    /// zombie.inventory.InventoryItem.load (ByteBuffer, int)
    fn load_base(
        &mut self,
        id: ItemId,
        b: &mut ByteBuffer,
        world_version: i32,
    ) -> Result<(), String> {
        self.items[id].id = b.get_int()?;
        let header = BitHeaderRead::alloc_read(HeaderSize::Byte, b)?;
        self.set_current_uses_virtual(id, 1);
        {
            let it = &mut self.items[id];
            it.name = it.original_name.clone();
            it.condition = it.condition_max;
            it.custom_color = false;
            it.col = [1.0; 4];
            it.item_capacity = -1.0;
            it.mod_data = None;
            it.activated = false;
            it.have_been_repaired = 0;
            it.custom_name = false;
            it.custom_weight = false;
            it.key_id = -1;
            it.remote_control_id = -1;
            it.remote_range = 0;
            it.color_red = 1.0;
            it.color_green = 1.0;
            it.color_blue = 1.0;
            it.worker = None;
            it.wet_cooldown = -1.0;
            it.favorite = false;
            it.stash_map = None;
            it.zombie_infected = false;
            it.current_ammo_count = 0;
            it.attached_slot = -1;
            it.attached_slot_type = None;
            it.attached_to_model = None;
            it.max_capacity = -1;
            it.recorded_media_index = -1;
            it.world_z_rotation = -1.0;
            it.world_scale = 1.0;
            it.is_initialised = false;
        }
        if !header.equals(0) {
            if world_version >= 220 {
                if header.has_flags(1)? {
                    let uses = b.get_int()?;
                    self.set_current_uses_virtual(id, uses);
                }
            } else {
                if header.has_flags(1)? {
                    let uses = i32::from(b.get_short()?);
                    self.set_current_uses_virtual(id, uses);
                }
                if header.has_flags(2)? {
                    b.get()?;
                }
            }
            if header.has_flags(4)? {
                // setConditionWhileLoading
                let it = &mut self.items[id];
                it.condition = i32::from(b.get()?).clamp(0, it.condition_max);
                it.broken = it.condition <= 0;
            }
            if header.has_flags(8)? {
                return Err(unported("InventoryItem.load ItemVisual"));
            }
            if header.has_flags(16)? {
                let mut c = [0.0f32; 4];
                for v in &mut c {
                    // new Color(r, g, b, a) clamps to [0, 1]; the unpacked bytes already are
                    *v = unpack_byte_to_float_unit(b.get()?).clamp(0.0, 1.0);
                }
                let it = &mut self.items[id];
                it.col = c;
                it.custom_color = true;
            }
            if header.has_flags(32)? {
                self.items[id].item_capacity = b.get_float()?;
            }
            if header.has_flags(64)? {
                self.load_bits(id, b, world_version)?;
            }
        }
        // synchWithVisual
        let script = self.scripts.item(self.items[id].script);
        let mut tmp = self.items[id].clone();
        self.synch_with_visual(&mut tmp, script)?;
        self.items[id] = tmp;
        Ok(())
    }

    /// The part of `InventoryItem.load` under flag 64 of the byte header.
    fn load_bits(
        &mut self,
        id: ItemId,
        b: &mut ByteBuffer,
        world_version: i32,
    ) -> Result<(), String> {
        let bits = BitHeaderRead::alloc_read(HeaderSize::Integer, b)?;
        if bits.has_flags(1)? {
            // `table.load(input, worldVersion)` into the table that is there or a new one
            let t = self.mod_data(id);
            t.load_into(b, world_version)?;
        }
        self.items[id].activated = bits.has_flags(2)?;
        if bits.has_flags(4)? {
            self.items[id].have_been_repaired = i32::from(b.get_short()?);
        }
        if bits.has_flags(8)? {
            self.items[id].name = read_string(b)?;
        }
        if bits.has_flags(16)? {
            let size = b.get_int()?;
            // ByteBuffer.allocate(size): a negative size throws
            if size < 0 {
                return Err(format!(
                    "java.lang.IllegalArgumentException: capacity < 0: ({size})"
                ));
            }
            self.items[id].byte_data = Some(b.get_bytes(size as usize)?.to_vec());
        }
        if bits.has_flags(32)? {
            let n = b.get_int()?;
            if n > 0 {
                let mut list = Vec::new();
                for _ in 0..n {
                    let rid = b.get_short()?;
                    // WorldDictionary.getItemTypeFromID: null for an unknown id
                    list.push(self.scripts.registry_type(rid).map(str::to_string));
                }
                self.items[id].extra_items = list
                    .into_iter()
                    .map(|t| t.ok_or_else(|| unported("extraItems with an unknown registry id")))
                    .collect::<Result<Vec<_>, _>>()?;
            }
        }
        let custom = bits.has_flags(64)?;
        self.set_custom_name(id, custom);
        if bits.has_flags(128)? {
            let weight = b.get_float()?;
            if weight >= 0.0 {
                let it = &mut self.items[id];
                it.set_actual_weight(weight);
                it.set_weight(weight);
                it.custom_weight = true;
            }
        }
        if bits.has_flags(256)? {
            self.items[id].key_id = b.get_int()?;
        }
        if bits.has_flags(1024)? {
            self.items[id].remote_control_id = b.get_int()?;
            self.items[id].remote_range = b.get_int()?;
        }
        if bits.has_flags(2048)? {
            let r = unpack_byte_to_float_unit(b.get()?);
            let g = unpack_byte_to_float_unit(b.get()?);
            let bl = unpack_byte_to_float_unit(b.get()?);
            let it = &mut self.items[id];
            it.color_red = r;
            it.color_green = g;
            it.color_blue = bl;
            // new Color(r, g, b): alpha 1, no clamping
            it.col = [r, g, bl, 1.0];
            it.custom_color = true;
        }
        if bits.has_flags(4096)? {
            self.items[id].worker = Some(read_string(b)?);
        }
        if bits.has_flags(8192)? {
            self.items[id].wet_cooldown = b.get_float()?;
        }
        self.items[id].favorite = bits.has_flags(16384)?;
        if bits.has_flags(32768)? {
            self.items[id].stash_map = Some(read_string(b)?);
        }
        self.items[id].zombie_infected = bits.has_flags(65536)?;
        if bits.has_flags(131072)? {
            self.items[id].current_ammo_count = b.get_int()?;
        }
        if bits.has_flags(262144)? {
            self.items[id].attached_slot = b.get_int()?;
        }
        if bits.has_flags(524288)? {
            self.items[id].attached_slot_type = Some(read_string(b)?);
        }
        if bits.has_flags(1048576)? {
            self.items[id].attached_to_model = Some(read_string(b)?);
        }
        if bits.has_flags(2097152)? {
            self.items[id].max_capacity = b.get_int()?;
        }
        if bits.has_flags(4194304)? {
            return Err(unported("InventoryItem.load recorded media"));
        }
        if world_version < 232 && bits.has_flags(8388608)? {
            self.items[id].world_z_rotation = b.get_int()? as f32;
        }
        if bits.has_flags(16777216)? {
            self.items[id].world_scale = b.get_float()?;
        }
        self.items[id].is_initialised = bits.has_flags(33554432)?;
        if bits.has_flags(67108864)? {
            return Err(unported("InventoryItem.load entity components"));
        }
        if bits.has_flags(134217728)? {
            return Err(unported("AnimalTracks.load"));
        }
        if bits.has_flags(268435456)? {
            return Err(unported("InventoryItem.load texture"));
        }
        if bits.has_flags(536870912)? {
            self.items[id].model_index = b.get_int()?;
        }
        if bits.has_flags(1073741824)? {
            let it = &mut self.items[id];
            if world_version >= 232 {
                it.world_x_rotation = b.get_float()?;
                it.world_y_rotation = b.get_float()?;
                it.world_z_rotation = b.get_float()?;
            } else {
                it.world_y_rotation = b.get_int()? as f32;
                it.world_x_rotation = b.get_int()? as f32;
            }
        }
        Ok(())
    }

    /// zombie.inventory.types.Food.load (after `super.load`)
    fn load_food(
        &mut self,
        id: ItemId,
        b: &mut ByteBuffer,
        _world_version: i32,
    ) -> Result<(), String> {
        {
            let it = &mut self.items[id];
            it.unhappy_change = 0.0;
            it.boredom_change = 0.0;
            it.cooking_time = 0.0;
            it.cooked = false;
            it.burnt = false;
            it.is_cookable = false;
            it.off_age = 1_000_000_000;
            it.off_age_max = 1_000_000_000;
            it.fatigue_change = 0.0;
            it.stress_change = 0.0;
            let Kind::Food(f) = &mut it.kind else {
                return Ok(());
            };
            f.calories = 0.0;
            f.proteins = 0.0;
            f.lipids = 0.0;
            f.carbohydrates = 0.0;
            f.hung_change = 0.0;
            f.base_hunger = 0.0;
            f.thirst_change = 0.0;
            f.heat = 1.0;
            f.last_cook_minute = 0;
            f.dangerous_uncooked = false;
            f.poison_detection_level = -1;
            f.spices = None;
            f.poison_power = 0;
            f.chef = None;
            f.pain_reduction = 0.0;
            f.flu_reduction = 0;
            f.food_sickness_change = 0;
            f.poison = false;
            f.use_for_poison = 0;
            f.freezing_time = 0.0;
            f.frozen = false;
            f.last_frozen_update = 0.0;
            f.rotten_time = 0.0;
            f.compost_time = 0.0;
            f.cooked_in_microwave = false;
            f.end_change = 0.0;
        }
        self.items[id].age = b.get_float()?;
        self.items[id].last_aged = b.get_float()?;
        let header = BitHeaderRead::alloc_read(HeaderSize::Byte, b)?;
        if !header.equals(0) {
            if header.has_flags(1)? {
                let (cal, pro, lip, carb) = (
                    b.get_float()?,
                    b.get_float()?,
                    b.get_float()?,
                    b.get_float()?,
                );
                let f = self.food_mut(id);
                f.calories = cal;
                f.proteins = pro;
                f.lipids = lip;
                f.carbohydrates = carb;
            }
            if header.has_flags(2)? {
                let v = b.get_float()?;
                self.food_mut(id).hung_change = v;
            }
            if header.has_flags(4)? {
                let v = b.get_float()?;
                self.food_mut(id).base_hunger = v;
            }
            if header.has_flags(8)? {
                self.items[id].unhappy_change = b.get_float()?;
            }
            if header.has_flags(16)? {
                self.items[id].boredom_change = b.get_float()?;
            }
            if header.has_flags(32)? {
                let v = b.get_float()?;
                self.food_mut(id).thirst_change = v;
            }
            if header.has_flags(64)? {
                let bits = BitHeaderRead::alloc_read(HeaderSize::Integer, b)?;
                if bits.has_flags(1)? {
                    let v = b.get_float()?;
                    self.food_mut(id).heat = v;
                }
                if bits.has_flags(2)? {
                    let v = b.get_int()?;
                    self.food_mut(id).last_cook_minute = v;
                }
                if bits.has_flags(4)? {
                    self.items[id].cooking_time = b.get_float()?;
                }
                self.items[id].cooked = bits.has_flags(8)?;
                self.items[id].burnt = bits.has_flags(16)?;
                self.items[id].is_cookable = bits.has_flags(32)?;
                let v = bits.has_flags(64)?;
                self.food_mut(id).dangerous_uncooked = v;
                if bits.has_flags(128)? {
                    let v = i32::from(b.get()?);
                    self.food_mut(id).poison_detection_level = v;
                }
                if bits.has_flags(256)? {
                    let size = b.get()?;
                    let mut spices = Vec::new();
                    for _ in 0..size {
                        spices.push(read_string(b)?);
                    }
                    self.food_mut(id).spices = Some(spices);
                }
                if bits.has_flags(512)? {
                    let v = i32::from(b.get()?);
                    self.food_mut(id).poison_power = v;
                }
                if bits.has_flags(1024)? {
                    let v = read_string(b)?;
                    self.food_mut(id).chef = Some(v);
                }
                if bits.has_flags(2048)? {
                    self.items[id].off_age = b.get_int()?;
                }
                if bits.has_flags(4096)? {
                    self.items[id].off_age_max = b.get_int()?;
                }
                if bits.has_flags(8192)? {
                    let v = b.get_float()?;
                    self.food_mut(id).pain_reduction = v;
                }
                if bits.has_flags(16384)? {
                    let v = b.get_int()?;
                    self.food_mut(id).flu_reduction = v;
                }
                if bits.has_flags(32768)? {
                    let v = b.get_int()?;
                    self.food_mut(id).food_sickness_change = v;
                }
                let v = bits.has_flags(65536)?;
                self.food_mut(id).poison = v;
                if bits.has_flags(131072)? {
                    let v = i32::from(b.get_short()?);
                    self.food_mut(id).use_for_poison = v;
                }
                if bits.has_flags(262144)? {
                    let v = b.get_float()?;
                    self.food_mut(id).freezing_time = v;
                }
                // setFrozen(boolean): `this.frozen = frozen` (the sound and the cooked state are not touched)
                let v = bits.has_flags(524288)?;
                self.food_mut(id).frozen = v;
                if bits.has_flags(1048576)? {
                    let v = b.get_float()?;
                    self.food_mut(id).last_frozen_update = v;
                }
                if bits.has_flags(2097152)? {
                    let v = b.get_float()?;
                    self.food_mut(id).rotten_time = v;
                }
                if bits.has_flags(4194304)? {
                    let v = b.get_float()?;
                    self.food_mut(id).compost_time = v;
                }
                let v = bits.has_flags(8388608)?;
                self.food_mut(id).cooked_in_microwave = v;
                if bits.has_flags(16777216)? {
                    self.items[id].fatigue_change = b.get_float()?;
                }
                if bits.has_flags(33554432)? {
                    let v = b.get_float()?;
                    self.food_mut(id).end_change = v;
                }
                if bits.has_flags(67108864)? {
                    let q = b.get_int()?;
                    let t = read_string(b)?;
                    let f = self.food_mut(id);
                    f.milk_qty = q;
                    f.milk_type = Some(t);
                }
                if bits.has_flags(134217728)? {
                    return Err(unported("Food.load fertilized egg"));
                }
                if bits.has_flags(268435456)? {
                    self.items[id].stress_change = b.get_float()?;
                }
            }
        }
        // `GameServer.server && lastAged == -1`: the probe clears the server flag; a real server sets lastAged here
        if self.items[id].full_type == "Base.CorpseAnimal" {
            return Err(unported("Food.initAnimalIcons"));
        }
        Ok(())
    }

    /// zombie.inventory.types.Literature.load (after `super.load`)
    fn load_literature(
        &mut self,
        id: ItemId,
        b: &mut ByteBuffer,
        world_version: i32,
    ) -> Result<(), String> {
        let bits = {
            let Kind::Literature(l) = &mut self.items[id].kind else {
                return Ok(());
            };
            l.number_of_pages = -1;
            l.already_read_pages = 0;
            l.can_be_write = false;
            l.custom_pages = None;
            l.locked_by = None;
            l.learned_recipes = None;
            if world_version >= 226 {
                BitHeaderRead::alloc_read(HeaderSize::Short, b)?
            } else {
                BitHeaderRead::alloc_read(HeaderSize::Byte, b)?
            }
        };
        if !bits.equals(0) {
            let mut number_page_type = 0;
            let mut pages = None;
            let mut read_pages = None;
            if bits.has_flags(1)? {
                if bits.has_flags(2)? {
                    number_page_type = 1;
                    pages = Some(i32::from(b.get_short()?));
                } else if bits.has_flags(4)? {
                    number_page_type = 2;
                    pages = Some(b.get_int()?);
                } else {
                    pages = Some(i32::from(b.get()?));
                }
            }
            if bits.has_flags(8)? {
                read_pages = Some(if number_page_type == 1 {
                    i32::from(b.get_short()?)
                } else if number_page_type == 2 {
                    b.get_int()?
                } else {
                    i32::from(b.get()?)
                });
            }
            let can_write = bits.has_flags(16)?;
            let mut custom_pages = None;
            if bits.has_flags(32)? {
                let size = b.get_int()?;
                if size > 0 {
                    let mut v = Vec::new();
                    for _ in 0..size {
                        v.push(read_string(b)?);
                    }
                    custom_pages = Some(v);
                }
            }
            let mut locked_by = None;
            if bits.has_flags(64)? {
                locked_by = Some(read_string(b)?);
            }
            let mut recipes = None;
            if world_version >= 226 && bits.has_flags(128)? {
                let n = b.get_short()?;
                let mut v = Vec::new();
                for _ in 0..n {
                    v.push(read_string(b)?);
                }
                recipes = Some(v);
            }
            let Kind::Literature(l) = &mut self.items[id].kind else {
                return Ok(());
            };
            if let Some(p) = pages {
                l.number_of_pages = p;
            }
            if let Some(p) = read_pages {
                l.already_read_pages = p;
            }
            l.can_be_write = can_write;
            l.custom_pages = custom_pages;
            l.locked_by = locked_by;
            l.learned_recipes = recipes;
        }
        Ok(())
    }

    /// zombie.inventory.CompressIdenticalItems.load into a container's item list (the container's `items` are replaced)
    /// and zombie.inventory.ItemContainer.load: returns `savedItems` (`None` for an item the original could not make).
    pub fn container_load(
        &mut self,
        c: ContainerId,
        b: &mut ByteBuffer,
        world_version: i32,
    ) -> Result<Vec<Option<ItemId>>, String> {
        self.containers[c].type_name = read_string(b)?;
        self.containers[c].explored = b.get()? != 0;
        let mut items: Vec<ItemId> = Vec::new();
        let mut saved: Vec<Option<ItemId>> = Vec::new();
        let count = b.get_short()?;
        for _ in 0..count {
            let identical = b.get_int()?;
            let item_start = b.position();
            let first = self.load_item(b, world_version)?;
            match first {
                None => {
                    let id_list_bytes = if identical > 1 {
                        (identical - 1) as usize * 4
                    } else {
                        0
                    };
                    b.set_position(b.position() + id_list_bytes)?;
                    for _ in 0..identical {
                        saved.push(None);
                    }
                }
                Some(first) => {
                    let mut item = Some(first);
                    for i in 0..identical {
                        if i > 0 {
                            b.set_position(item_start)?;
                            item = self.load_item(b, world_version)?;
                        }
                        if let Some(it) = item {
                            items.push(it);
                        }
                        saved.push(item);
                    }
                    for i in 1..identical {
                        let id = b.get_int()?;
                        let n = saved.len() - identical as usize + i as usize;
                        if let Some(it) = saved[n] {
                            self.items[it].id = id;
                        }
                    }
                }
            }
        }
        for &i in &items {
            self.items[i].container = Some(c);
        }
        self.containers[c].items = items;
        self.containers[c].has_been_looted = b.get()? != 0;
        self.containers[c].capacity = b.get_int()?;
        Ok(saved)
    }
}
