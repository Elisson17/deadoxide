//! zombie.iso.areas.DesignationZone: player-drawn zones (animal pens) as `map_meta.bin` stores them.
use crate::java_nio::{read_string, ByteBuffer};
use crate::zombie::inventory::inventory_item::not_ported;

/// The fields of one saved designation zone.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DesignationZone {
    pub id: f64,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub h: i32,
    pub w: i32,
    pub zone_type: String,
    pub name: String,
    pub hour_last_seen: i32,
}

impl DesignationZone {
    /// zombie.iso.areas.DesignationZone.load: reads one zone; only "AnimalZone" becomes a zone, any other type is
    /// read and dropped (`null`). An animal zone is `not ported:` here: the original builds a `DesignationZoneAnimal`
    /// (a `Rand` draw for the id it then overwrites, registration in the zone lists, `check()`).
    pub fn load(b: &mut ByteBuffer, _world_version: i32) -> Result<Option<Self>, String> {
        let z = DesignationZone {
            id: b.get_double()?,
            x: b.get_int()?,
            y: b.get_int()?,
            z: b.get_int()?,
            h: b.get_int()?,
            w: b.get_int()?,
            zone_type: read_string(b)?,
            name: read_string(b)?,
            hour_last_seen: b.get_int()?,
        };
        if z.zone_type == "AnimalZone" {
            return Err(not_ported("DesignationZoneAnimal from map_meta.bin"));
        }
        Ok(None)
    }
}
