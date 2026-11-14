//! zombie.characters.animals.AnimalZoneJunction: a point where two animal zones' polylines meet.
use crate::zombie::iso::iso_meta_grid::AnimalZoneRef;

/// zombie.characters.animals.AnimalZoneJunction. The zones are handles into `IsoMetaGrid.animal_zones`; the other
/// zone is `None` where the save names an id no loaded zone has (`ZoneHandler.getZone` returns null).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimalZoneJunction {
    pub zone_self: AnimalZoneRef,
    pub point_index_self: i32,
    pub zone_other: Option<AnimalZoneRef>,
    pub point_index_other: i32,
    /// zombie.characters.animals.AnimalZoneJunction.distanceFromStart: `zoneSelf.getDistanceOfPointFromStart`.
    pub distance_from_start: f32,
}
