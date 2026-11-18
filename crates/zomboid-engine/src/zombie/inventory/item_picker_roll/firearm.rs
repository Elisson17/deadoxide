//! Firearms and ammunition in the loot rolls: `HandWeapon.randomizeFirearmAsLoot`,
//! `ItemPickerJava.doGunStorageContainer`, the ammunition fields of an `InventoryItem` and what they weigh.
use super::*;

/// What a `HandWeapon` keeps that the rolls read: `containsClip` and `roundChambered`, and its `attachments` (part
/// type and the part's `getWeightModifier()`, in the order the `HashMap` was filled: removals and puts move an
/// entry to the end of its bucket).
#[derive(Clone, Debug, Default)]
pub struct WeaponState {
    pub contains_clip: bool,
    pub round_chambered: bool,
    pub attachments: Vec<(String, f32)>,
}

/// `AmmoType.getItemKey()` of the ammunition types the scripts name (`AmmoType` registers each with its
/// `ItemKey.Normal` constant: `"Base." + id`).
pub(crate) fn ammo_item_key(ammo_type: &str) -> Option<&'static str> {
    Some(match ammo_type {
        "base:bullets_3030" => "Base.3030Bullets",
        "base:bullets_308" => "Base.308Bullets",
        "base:bullets_357" => "Base.Bullets357",
        "base:bullets_38" => "Base.Bullets38",
        "base:bullets_44" => "Base.Bullets44",
        "base:bullets_45" => "Base.Bullets45",
        "base:bullets_556" => "Base.556Bullets",
        "base:bullets_9mm" => "Base.Bullets9mm",
        "base:cap_gun_cap" => "Base.CapGunCap",
        "base:shotgun_shells" => "Base.ShotgunShells",
        _ => return None,
    })
}

impl LootRoller<'_> {
    /// `InventoryItem.getContentsWeight` (base) as a lower and an upper bound: the ammunition the item holds
    /// (`scriptAmmo.getActualWeight() * getCurrentAmmoCount()`), else its fluid.
    fn base_contents_weight(&self, it: &LootItem) -> Result<(f32, f32), String> {
        if let Some(key) = &it.ammo_item {
            if let Some(si) = self.startup.find_item(key) {
                let w = script_actual_weight(self.script(si)?);
                let v = w * it.current_ammo as f32;
                return Ok((v, v));
            }
        }
        Ok(it.fluid_amount.unwrap_or((0.0, 0.0)))
    }

    /// `InventoryItem.getContentsWeight` of any item but a bag: for a `HandWeapon` the chambered round and the
    /// clip, then the base.
    pub(super) fn item_contents_weight(&self, it: &LootItem) -> Result<(f32, f32), String> {
        let Some(w) = &it.weapon else {
            return self.base_contents_weight(it);
        };
        let script = self.script(it.script)?;
        let mut contents = 0.0f32;
        if script_bool(script, "haveChamber") && w.round_chambered {
            if let Some(key) = &it.ammo_item {
                if let Some(si) = self.startup.find_item(key) {
                    contents += script_actual_weight(self.script(si)?);
                }
            }
        }
        if w.contains_clip {
            if let Some(m) = script_str(script, "magazineType").filter(|m| !m.trim().is_empty()) {
                if let Some(si) = self.startup.find_item(m) {
                    contents += script_actual_weight(self.script(si)?);
                }
            }
        }
        let (blo, bhi) = self.base_contents_weight(it)?;
        Ok((contents + blo, contents + bhi))
    }

    /// `HandWeapon.setRoundChambered(true)`: only a weapon with a chamber holds one.
    fn set_round_chambered(&self, it: &mut LootItem, script: &Item) {
        if let Some(w) = it.weapon.as_mut() {
            w.round_chambered = script_bool(script, "haveChamber");
        }
    }

    /// HandWeapon.randomizeFirearmAsLoot for the weapon at `idx` of the container `path` leads to in `root`.
    pub(super) fn randomize_firearm_as_loot(
        &mut self,
        root: &mut LootContainer,
        path: &[usize],
        idx: usize,
    ) -> Result<(), String> {
        let script = self.script(container_at(root, path).items[idx].script)?;
        let magazine = script_str(script, "magazineType").filter(|m| !m.is_empty());
        let have_chamber = script_bool(script, "haveChamber");
        let max_ammo = script_int(script, "maxAmmo");
        let mut ammo_box_chance = 30;
        if self.reload_difficulty > 1 && magazine.is_some() && self.rand.next(100) < 90 {
            if self.rand.next_bool(3) {
                let clip = self.spawn_item(root, path, magazine.unwrap_or_default())?;
                let clip_max = clip.map(|c| container_at(root, path).items[c].max_ammo);
                if self.rand.next_bool(5) {
                    // `clip.getMaxAmmo()` of a clip that was refused is a NullPointerException
                    let (Some(c), Some(m)) = (clip, clip_max) else {
                        return Err(clothing::NPE.to_string());
                    };
                    let v = self.rand.next_range(1, m);
                    container_at_mut(root, path).items[c].current_ammo = v;
                }
                if !self.rand.next_bool(5) {
                    let (Some(c), Some(m)) = (clip, clip_max) else {
                        return Err(clothing::NPE.to_string());
                    };
                    container_at_mut(root, path).items[c].current_ammo = m;
                }
            } else {
                if magazine.is_some_and(|m| !m.trim().is_empty()) {
                    // setContainsClip(true): `usesExternalMagazine()`, a magazine type
                    if let Some(w) = container_at_mut(root, path).items[idx].weapon.as_mut() {
                        w.contains_clip = true;
                    }
                }
                if self.rand.next_bool(6) {
                    let v = self.rand.next_range(1, max_ammo);
                    container_at_mut(root, path).items[idx].current_ammo = v;
                } else {
                    ammo_box_chance = self.rand.next_range(60, 100);
                }
            }
            // `this.haveChamber() & Rand.NextBool(5)`: no short circuit
            let chamber_roll = self.rand.next_bool(5);
            if have_chamber & chamber_roll {
                self.set_round_chambered(&mut container_at_mut(root, path).items[idx], script);
            }
        }
        if self.reload_difficulty == 1 || (magazine.is_none() && self.rand.next(100) < 30) {
            let v = self.rand.next_range(1, max_ammo);
            let w = &mut container_at_mut(root, path).items[idx];
            w.current_ammo = v;
            if have_chamber {
                self.set_round_chambered(w, script);
            }
        }
        if let Some(ammo_box) = script_str(script, "ammoBox").filter(|b| !b.is_empty()) {
            if self.rand.next(100) < ammo_box_chance {
                self.spawn_item(root, path, ammo_box)?;
            } else if self.rand.next(100) < 50 {
                // spawnItems(getAmmoType().getItemKey(), Rand.Next(1, 5), container): ItemContainer.AddItems
                let key = container_at(root, path).items[idx]
                    .ammo_item
                    .clone()
                    .ok_or_else(|| clothing::NPE.to_string())?;
                let count = self.rand.next_range(1, 5);
                for _ in 0..count {
                    self.add_item_by_type(root, path, &key)?;
                }
            }
        }
        Ok(())
    }

    /// ItemContainer.AddItem(String): the item is created and added (no `isItemAllowed`, no id test).
    pub(super) fn add_item_by_type(
        &mut self,
        root: &mut LootContainer,
        path: &[usize],
        item_type: &str,
    ) -> Result<(), String> {
        let Some(si) = self.startup.find_item(item_type) else {
            return Ok(());
        };
        if self.startup.is_obsolete(si) {
            return Ok(());
        }
        let Some(item) = self.create_item(item_type)? else {
            return Ok(());
        };
        container_at_mut(root, path).items.push(item);
        Ok(())
    }

    /// ItemPickerJava.doGunStorageContainer for the weapon at `idx` of the container `path` leads to.
    pub(super) fn do_gun_storage_container(
        &mut self,
        dont_spawn_ammo: bool,
        root: &mut LootContainer,
        path: &[usize],
        idx: usize,
    ) -> Result<(), String> {
        let script = self.script(container_at(root, path).items[idx].script)?;
        let magazine = script_str(script, "magazineType").filter(|m| !m.is_empty());
        let ammo_box = script_str(script, "ammoBox").filter(|m| !m.is_empty());
        let clip_count = if magazine.is_some() {
            let hi = self.rand.next_range(2, 4);
            self.rand.next_range(1, hi)
        } else {
            0
        };
        let box_count = if ammo_box.is_some() {
            let hi = self.rand.next_range(1, 3);
            self.rand.next_range(0, hi)
        } else {
            0
        };
        for _ in 0..clip_count {
            let mt = magazine.unwrap_or_default();
            let Some(mut clip) = self.create_item(mt)? else {
                return Err(clothing::NPE.to_string());
            };
            if !dont_spawn_ammo {
                clip.current_ammo = clip.max_ammo;
            }
            self.spawn_created_item(root, path, clip)?;
        }
        for _ in 0..box_count {
            let bt = ammo_box.unwrap_or_default();
            let Some(b) = self.create_item(bt)? else {
                return Err(clothing::NPE.to_string());
            };
            self.spawn_created_item(root, path, b)?;
        }
        if !dont_spawn_ammo {
            let w = &mut container_at_mut(root, path).items[idx];
            if let Some(ws) = w.weapon.as_mut() {
                ws.contains_clip = magazine.is_some();
            }
            w.current_ammo = w.max_ammo;
        }
        Ok(())
    }
}

impl LootRoller<'_> {
    /// `HandWeapon.getActualWeight`: the script's weight plus the weight modifier of each attached part, summed in
    /// the iteration order of the `attachments` `HashMap<String, WeaponPart>`.
    pub(super) fn weapon_actual_weight(&self, it: &LootItem, script: &Item) -> f32 {
        let mut weight = script_actual_weight(script);
        if let Some(w) = &it.weapon {
            let keys: Vec<&str> = w.attachments.iter().map(|(k, _)| k.as_str()).collect();
            for i in java::hash_map_order(&keys) {
                weight += w.attachments[i].1;
            }
        }
        weight
    }

    /// ItemPickerJava.DoWeaponUpgrade for the weapon at `idx` of `container`: `Rand.Next(upgrades.size())` parts,
    /// each picked at random (`PZArrayUtil.pickRandom`), created and attached by `HandWeapon.attachWeaponPart`.
    pub(super) fn do_weapon_upgrade(
        &mut self,
        container: &mut LootContainer,
        idx: usize,
        type_name: &str,
    ) -> Result<(), String> {
        let picker = self.picker;
        let Some(&u) = picker.weapon_upgrade_map.get(type_name) else {
            return Ok(());
        };
        let upgrades = &picker.weapon_upgrades[u].upgrades;
        if upgrades.is_empty() {
            return Ok(());
        }
        let rand_upgrade = self.rand.next(upgrades.len() as i32);
        for _ in 0..rand_upgrade {
            let s = &upgrades[self.rand.next(upgrades.len() as i32) as usize];
            let Some(part) = self.create_item(s)? else {
                continue;
            };
            if part.class != ItemType::WeaponPart {
                // `(WeaponPart)part` is a ClassCastException
                return Err(unported(format!(
                    "DoWeaponUpgrade part {s} is not a WeaponPart"
                )));
            }
            let script = self.script(part.script)?;
            let part_type = script_str(script, "partType").filter(|p| !p.is_empty());
            let modifier = script_float(script, "weightModifier");
            // HandWeapon.attachWeaponPart(null, part, true): an attached part of the same type is detached first
            // (the Lua callbacks of these parts are not set), then `setWeaponPart` clears the type and puts it
            if let (Some(pt), Some(w)) = (part_type, container.items[idx].weapon.as_mut()) {
                w.attachments.retain(|(k, _)| k != pt);
                w.attachments.push((pt.to_string(), modifier));
            }
        }
        Ok(())
    }
}
