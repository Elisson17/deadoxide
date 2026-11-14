/// zombie.iso.zones.ZoneGeometryType, in ordinal order (the byte a save stores).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ZoneGeometryType {
    /// A plain rectangle (x, y, w, h).
    #[default]
    Invalid,
    Point,
    Polyline,
    Polygon,
}

impl ZoneGeometryType {
    /// `ZoneGeometryType.values()[i]`; the loader maps an out-of-range ordinal to `INVALID` before indexing.
    pub fn from_ordinal(i: i32) -> Self {
        match i {
            1 => ZoneGeometryType::Point,
            2 => ZoneGeometryType::Polyline,
            3 => ZoneGeometryType::Polygon,
            _ => ZoneGeometryType::Invalid,
        }
    }
    pub fn ordinal(self) -> i32 {
        self as i32
    }
    /// The Java constant name (`Enum.name()`).
    pub fn name(self) -> &'static str {
        match self {
            ZoneGeometryType::Invalid => "INVALID",
            ZoneGeometryType::Point => "Point",
            ZoneGeometryType::Polyline => "Polyline",
            ZoneGeometryType::Polygon => "Polygon",
        }
    }
}
