//! zombie.iso.areas.NonPvpZone: a rectangle where players cannot hurt each other.
use crate::java_nio::{read_string, ByteBuffer};

/// zombie.iso.areas.NonPvpZone, the saved fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NonPvpZone {
    pub x: i32,
    pub y: i32,
    pub x2: i32,
    pub y2: i32,
    pub size: i32,
    pub title: String,
}

impl NonPvpZone {
    /// zombie.iso.areas.NonPvpZone.load
    pub fn load(b: &mut ByteBuffer, _world_version: i32) -> Result<Self, String> {
        Ok(NonPvpZone {
            x: b.get_int()?,
            y: b.get_int()?,
            x2: b.get_int()?,
            y2: b.get_int()?,
            size: b.get_int()?,
            title: read_string(b)?,
        })
    }
}
