//! zombie.vehicles.BaseVehicle: a car or trailer, as `vehicles.db` stores it.
use std::collections::{BTreeMap, HashMap};

use super::vehicle_part::{PartOwner, VehiclePart};
use crate::java_nio::{read_string, ByteBuffer};
use crate::zombie::core::math::pz_math;
use crate::zombie::inventory::inventory_item::{not_ported, InventoryItem, ItemFactory};
use crate::zombie::iso::iso_moving_object::IsoMovingObject;
use crate::zombie::util::string_utils;

/// zombie.vehicles.VehicleParts: the parts in save order, with the lookups `add` keeps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VehicleParts {
    pub parts: Vec<VehiclePart>,
    /// `partsById`: the first part with each non-blank id.
    pub parts_by_id: HashMap<String, usize>,
    /// The last part with id "Battery".
    pub battery: Option<usize>,
    /// The last part with id "Engine".
    pub engine: Option<usize>,
}

impl VehicleParts {
    /// zombie.vehicles.VehicleParts.add
    pub fn add(&mut self, part: VehiclePart) {
        let i = self.parts.len();
        let id = part.part_id.clone();
        self.parts.push(part);
        if !string_utils::is_null_or_whitespace(Some(&id)) && !self.parts_by_id.contains_key(&id) {
            self.parts_by_id.insert(id.clone(), i);
        }
        if id == "Battery" {
            self.battery = Some(i);
        }
        if id == "Engine" {
            self.engine = Some(i);
        }
    }
}

/// zombie.vehicles.VehicleAlarm, the saved fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VehicleAlarm {
    /// World age in hours.
    pub start_time: f64,
    /// `StringUtils.discardNullOrWhitespace`
    pub chosen_sound: Option<String>,
}

/// zombie.vehicles.BaseVehicle, the saved fields.
#[derive(Clone, Debug, PartialEq)]
pub struct BaseVehicle {
    pub moving: IsoMovingObject,
    /// Clamped to the floor the vehicle stands on: [level, level + 0.995] × 2.44949.
    pub saved_physics_z: f32,
    /// `savedRot` (x, y, z, w)
    pub saved_rot: [f32; 4],
    pub script_name: String,
    pub skin_index: i32,
    pub front_end_durability: i32,
    pub rear_end_durability: i32,
    pub current_front_end_durability: i32,
    pub current_rear_end_durability: i32,
    pub key_id: i32,
    pub key_spawned: i8,
    pub headlights_on: bool,
    pub created: bool,
    pub sound_horn_on: bool,
    pub sound_back_move_on: bool,
    /// `LightbarLightsMode.set`: clamped to [0, 3].
    pub lightbar_lights_mode: i32,
    /// `LightbarSirenMode.set`: clamped to [0, 3]; a broken "lightbar" part sets it to 0.
    pub lightbar_siren_mode: i32,
    pub parts: VehicleParts,
    pub key_is_on_door: bool,
    pub hotwired: bool,
    pub hotwired_broken: bool,
    pub keys_in_ignition: bool,
    pub rust: f32,
    pub color_hue: f32,
    pub color_saturation: f32,
    pub color_value: f32,
    pub mechanical_id: i32,
    pub alarmed: bool,
    pub vehicle_alarm: VehicleAlarm,
    pub siren_start_time: f64,
    /// `setCurrentKey`: also the one item of the ignition switch container.
    pub current_key: Option<InventoryItem>,
    /// `bloodIntensity` (a HashMap in the original; ordered here).
    pub blood_intensity: BTreeMap<String, i8>,
    /// -1 when not towing.
    pub vehicle_towing_id: i32,
    pub tow_attachment_self: Option<String>,
    pub tow_attachment_other: Option<String>,
    pub row_constraint_z_offset: f32,
    pub regulator_speed: f32,
    pub previously_entered: bool,
    pub previously_moved: bool,
    pub do_damage_overlay: bool,
}

impl BaseVehicle {
    /// zombie.vehicles.BaseVehicle.load (with IsoMovingObject.load first; not a debug save). The random key id and
    /// rust of the constructor are overwritten by the save. Animals in trailers are not ported.
    pub fn load(
        b: &mut ByteBuffer,
        world_version: i32,
        factory: &dyn ItemFactory,
    ) -> Result<Self, String> {
        let moving = IsoMovingObject::load(b, world_version)?;
        let physics_z = b.get_float()?;
        let level = pz_math::fastfloor(moving.z) as f32;
        let saved_physics_z = pz_math::clamp(physics_z, level * 2.44949, (level + 0.995) * 2.44949);
        let saved_rot = [
            b.get_float()?,
            b.get_float()?,
            b.get_float()?,
            b.get_float()?,
        ];
        let script_name = read_string(b)?;
        let skin_index = b.get_int()?;
        let is_engine_running = b.get()? != 0;
        let front_end_durability = b.get_int()?;
        let rear_end_durability = b.get_int()?;
        let current_front_end_durability = b.get_int()?;
        let current_rear_end_durability = b.get_int()?;
        let engine_loudness = b.get_int()?;
        let engine_quality = b.get_int()?.clamp(0, 100);
        let key_id = b.get_int()?;
        let key_spawned = b.get()?;
        let headlights_on = b.get()? != 0;
        let created = b.get()? != 0;
        let sound_horn_on = b.get()? != 0;
        let sound_back_move_on = b.get()? != 0;
        let mode = |v: i8| i32::from(v).clamp(0, 3);
        let lightbar_lights_mode = mode(b.get()?);
        let mut lightbar_siren_mode = mode(b.get()?);
        let part_count = b.get_short()?;
        let mut parts = VehicleParts::default();
        let mut do_damage_overlay = false;
        for _ in 0..part_count {
            let mut owner = PartOwner {
                lightbar_siren_mode: &mut lightbar_siren_mode,
                do_damage_overlay: &mut do_damage_overlay,
            };
            parts.add(VehiclePart::load(b, world_version, factory, &mut owner)?);
        }
        let key_is_on_door = b.get()? != 0;
        let hotwired = b.get()? != 0;
        let hotwired_broken = b.get()? != 0;
        let keys_in_ignition = b.get()? != 0;
        let rust = b.get_float()?;
        let color_hue = b.get_float()?;
        let color_saturation = b.get_float()?;
        let color_value = b.get_float()?;
        let engine_power = b.get_int()?;
        if let Some(e) = parts.engine.and_then(|i| parts.parts[i].engine.as_mut()) {
            e.load(
                is_engine_running,
                engine_loudness,
                engine_quality,
                engine_power,
            );
        }
        let _vehicle_id = b.get_short()?;
        if world_version < 229 {
            let _count = read_string(b)?;
        }
        let mechanical_id = b.get_int()?;
        let alarmed = b.get()? != 0;
        let mut vehicle_alarm = VehicleAlarm::default();
        if world_version >= 229 {
            vehicle_alarm.start_time = b.get_double()?;
            let sound = read_string(b)?;
            vehicle_alarm.chosen_sound =
                (!string_utils::is_null_or_whitespace(Some(&sound))).then_some(sound);
        }
        let siren_start_time = b.get_double()?;
        let current_key = if b.get()? != 0 {
            match InventoryItem::load_item(b, world_version, factory) {
                Ok(k) => k,
                Err(e) if e.starts_with("not ported") => return Err(e),
                // DebugType.General.printException: no key, reading goes on where the throw left the buffer.
                Err(_) => None,
            }
        } else {
            None
        };
        let count = b.get()?;
        let mut blood_intensity = BTreeMap::new();
        for _ in 0..count {
            let id = read_string(b)?;
            blood_intensity.insert(id, b.get()?);
        }
        let mut vehicle_towing_id = -1;
        let mut tow_attachment_self = None;
        let mut tow_attachment_other = None;
        let mut row_constraint_z_offset = 0.0;
        if b.get()? != 0 {
            vehicle_towing_id = b.get_int()?;
            tow_attachment_self = Some(read_string(b)?);
            tow_attachment_other = Some(read_string(b)?);
            row_constraint_z_offset = b.get_float()?;
        }
        let regulator_speed = b.get_float()?;
        let previously_entered = b.get()? != 0;
        let previously_moved = world_version >= 196 && b.get()? != 0;
        if world_version >= 212 {
            let _buffer_size = b.get_int()?;
        }
        if b.get()? != 0 {
            return Err(not_ported("animals in vehicles (IsoAnimal.load)"));
        }
        Ok(BaseVehicle {
            moving,
            saved_physics_z,
            saved_rot,
            script_name,
            skin_index,
            front_end_durability,
            rear_end_durability,
            current_front_end_durability,
            current_rear_end_durability,
            key_id,
            key_spawned,
            headlights_on,
            created,
            sound_horn_on,
            sound_back_move_on,
            lightbar_lights_mode,
            lightbar_siren_mode,
            parts,
            key_is_on_door,
            hotwired,
            hotwired_broken,
            keys_in_ignition,
            rust,
            color_hue,
            color_saturation,
            color_value,
            mechanical_id,
            alarmed,
            vehicle_alarm,
            siren_start_time,
            current_key,
            blood_intensity,
            vehicle_towing_id,
            tow_attachment_self,
            tow_attachment_other,
            row_constraint_z_offset,
            regulator_speed,
            previously_entered,
            previously_moved,
            do_damage_overlay,
        })
    }
}
