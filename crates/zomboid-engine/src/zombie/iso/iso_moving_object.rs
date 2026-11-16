//! zombie.iso.IsoMovingObject: the position, facing and modData a save stores for anything that moves (characters,
//! corpses, vehicles).
use crate::java_nio::ByteBuffer;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTable;

/// zombie.iso.IsoMovingObject, the saved fields.
#[derive(Clone, Debug, PartialEq)]
pub struct IsoMovingObject {
    pub offset_x: f32,
    pub offset_y: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// The saved int; `setForwardIsoDirection(IsoDirections.fromIndex(dir))` keeps `dir & 7`.
    pub dir: i32,
    pub table: Option<KahluaTable>,
    /// `width` (field initialiser 0.24); characters set their own.
    pub width: f32,
    /// `weight` (field initialiser 1.0).
    pub weight: f32,
    /// `stateEventDelayTimer`: counted down by `GameTime.getMultiplier()` in `update`.
    pub state_event_delay_timer: f32,
}

impl IsoMovingObject {
    /// zombie.iso.IsoMovingObject.load. `IsoWorld.saveoffsetx/saveoffsety` are 0 outside the old world-shift code.
    pub fn load(b: &mut ByteBuffer, world_version: i32) -> Result<Self, String> {
        let offset_x = b.get_float()?;
        let offset_y = b.get_float()?;
        let x = b.get_float()?;
        let y = b.get_float()?;
        let z = b.get_float()?;
        let dir = b.get_int()?;
        let table = if b.get()? != 0 {
            Some(KahluaTable::load(b, world_version)?)
        } else {
            None
        };
        Ok(IsoMovingObject {
            offset_x,
            offset_y,
            x,
            y,
            z,
            dir,
            table,
            width: 0.24,
            weight: 1.0,
            state_event_delay_timer: 0.0,
        })
    }

    /// zombie.iso.IsoMovingObject(cell, x, y, z) as a character's constructor calls it: the position and the field
    /// initialisers; the world registration (`addMovingObject`), sprite and ids are not modelled.
    pub fn at(x: f32, y: f32, z: f32) -> Self {
        IsoMovingObject {
            offset_x: 0.0,
            offset_y: 0.0,
            x,
            y,
            z,
            dir: 0,
            table: None,
            width: 0.24,
            weight: 1.0,
            state_event_delay_timer: 0.0,
        }
    }
}
