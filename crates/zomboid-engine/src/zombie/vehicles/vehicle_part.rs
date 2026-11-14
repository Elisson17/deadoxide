//! zombie.vehicles.VehiclePart: one part of a vehicle (door, tire, seat, engine, trunk ...) with its installed item,
//! container and part-specific state, as a save stores it; and the small part classes it loads.
use crate::java_nio::{read_string, ByteBuffer};
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTable;
use crate::zombie::inventory::inventory_item::{InventoryItem, ItemFactory};
use crate::zombie::inventory::item_container::ItemContainer;
use crate::zombie::iso::chunk_save::load_entity;
use crate::zombie::radio::devices::device_data::DeviceData;

/// zombie.vehicles.VehicleLight, the saved fields.
#[derive(Clone, Debug, PartialEq)]
pub struct VehicleLight {
    pub active: bool,
    pub offset_x: f32,
    pub offset_y: f32,
    pub intensity: f32,
    pub dist: f32,
    pub focusing: i32,
}

impl VehicleLight {
    /// zombie.vehicles.VehicleLight.load
    pub fn load(b: &mut ByteBuffer) -> Result<Self, String> {
        Ok(VehicleLight {
            active: b.get()? != 0,
            offset_x: b.get_float()?,
            offset_y: b.get_float()?,
            intensity: b.get_float()?,
            dist: b.get_float()?,
            focusing: b.get_int()?,
        })
    }
}

/// zombie.vehicles.VehicleDoor, the saved fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VehicleDoor {
    pub open: bool,
    pub locked: bool,
    pub lock_broken: bool,
}

impl VehicleDoor {
    /// zombie.vehicles.VehicleDoor.load
    pub fn load(b: &mut ByteBuffer) -> Result<Self, String> {
        Ok(VehicleDoor {
            open: b.get()? != 0,
            locked: b.get()? != 0,
            lock_broken: b.get()? != 0,
        })
    }
}

/// zombie.vehicles.VehicleWindow, the saved fields (its condition is the part's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VehicleWindow {
    pub open: bool,
    pub open_delta: f32,
}

/// zombie.vehicles.VehicleEngine, the state `BaseVehicle.load` gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VehicleEngine {
    /// `state == engineStateTypes.Running` (the field initialiser is `Idle`).
    pub running: bool,
    pub quality: i32,
    pub loudness: i32,
    pub power: i32,
}

impl VehicleEngine {
    /// zombie.vehicles.VehicleEngine.load (boolean, int, int, int)
    pub fn load(&mut self, engine_running: bool, loudness: i32, quality: i32, power: i32) {
        if engine_running {
            self.running = true;
        }
        self.loudness = loudness;
        self.quality = quality;
        self.power = power;
    }
}

/// zombie.vehicles.VehiclePart, the saved fields. `scriptPart` is not set while loading (the vehicle's script is
/// applied later), so `getId()` is `partId` here.
#[derive(Clone, Debug, PartialEq)]
pub struct VehiclePart {
    pub part_id: String,
    pub created: bool,
    pub last_updated: f32,
    pub item: Option<InventoryItem>,
    pub container: Option<ItemContainer>,
    pub mod_data: Option<KahluaTable>,
    pub device_data: Option<DeviceData>,
    pub light: Option<VehicleLight>,
    pub door: Option<VehicleDoor>,
    /// Created for the part whose id is "Engine".
    pub engine: Option<VehicleEngine>,
    pub window: Option<VehicleWindow>,
    pub condition: i32,
    pub wheel_friction: f32,
    pub mechanic_skill_installer: i32,
    pub suspension_compression: f32,
    pub suspension_damping: f32,
    /// `loadEntity`: (component id, bytes), kept raw.
    pub components: Option<Vec<(i16, Vec<u8>)>>,
}

/// What `VehiclePart.setCondition` changes on its vehicle while loading.
pub struct PartOwner<'a> {
    /// `BaseVehicle.lightbarSirenMode`
    pub lightbar_siren_mode: &'a mut i32,
    /// `BaseVehicle.doDamageOverlay` (`updateDamageOverlayLater`)
    pub do_damage_overlay: &'a mut bool,
}

impl VehiclePart {
    /// zombie.vehicles.VehiclePart.setCondition, for a vehicle without a driver (as loaded) and without a script
    /// part: the condition clamped to [0, 100], passed on to the installed item (`InventoryItem.setCondition(c, false)`).
    pub fn set_condition(&mut self, condition: i32, owner: &mut PartOwner<'_>) {
        // Math.min(100, c), then Math.max(0, c)
        let condition = condition.clamp(0, 100);
        self.condition = condition;
        if let Some(it) = &mut self.item {
            it.set_condition(condition);
        }
        *owner.do_damage_overlay = true;
        if condition <= 0 && self.part_id == "lightbar" {
            // BaseVehicle.setLightbarSirenMode(0)
            *owner.lightbar_siren_mode = 0;
        }
    }

    /// zombie.vehicles.VehiclePart.load
    pub fn load(
        b: &mut ByteBuffer,
        world_version: i32,
        factory: &dyn ItemFactory,
        owner: &mut PartOwner<'_>,
    ) -> Result<Self, String> {
        let part_id = read_string(b)?;
        let created = b.get()? != 0;
        let last_updated = b.get_float()?;
        let item = if b.get()? != 0 {
            InventoryItem::load_item(b, world_version, factory)?
        } else {
            None
        };
        let container = if b.get()? != 0 {
            // new ItemContainer(), items cleared, id = 0, then load
            let mut c = ItemContainer {
                id: 0,
                ..ItemContainer::default()
            };
            c.load(b, world_version, factory)?;
            Some(c)
        } else {
            None
        };
        let mod_data = if b.get()? != 0 {
            Some(KahluaTable::load(b, world_version)?)
        } else {
            None
        };
        let device_data = if b.get()? != 0 {
            Some(DeviceData::load(b)?)
        } else {
            None
        };
        let light = if b.get()? != 0 {
            Some(VehicleLight::load(b)?)
        } else {
            None
        };
        let door = if b.get()? != 0 {
            Some(VehicleDoor::load(b)?)
        } else {
            None
        };
        let engine = (part_id == "Engine").then_some(VehicleEngine {
            running: false,
            quality: 0,
            loudness: 0,
            power: 0,
        });
        let mut part = VehiclePart {
            part_id,
            created,
            last_updated,
            item,
            container,
            mod_data,
            device_data,
            light,
            door,
            engine,
            window: None,
            condition: 0,
            wheel_friction: 0.0,
            mechanic_skill_installer: 0,
            suspension_compression: 0.0,
            suspension_damping: 0.0,
            components: None,
        };
        if b.get()? != 0 {
            // zombie.vehicles.VehicleWindow.load
            let condition = i32::from(b.get()?);
            part.set_condition(condition, owner);
            let open = b.get()? != 0;
            part.window = Some(VehicleWindow {
                open,
                open_delta: if open { 1.0 } else { 0.0 },
            });
        }
        let condition = b.get_int()?;
        part.set_condition(condition, owner);
        part.wheel_friction = b.get_float()?;
        part.mechanic_skill_installer = b.get_int()?;
        part.suspension_compression = b.get_float()?;
        part.suspension_damping = b.get_float()?;
        if world_version >= 200 && b.get()? != 0 {
            part.components = Some(load_entity(b)?);
        }
        Ok(part)
    }
}
