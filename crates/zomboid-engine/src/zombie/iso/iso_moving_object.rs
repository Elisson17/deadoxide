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
        })
    }
}
