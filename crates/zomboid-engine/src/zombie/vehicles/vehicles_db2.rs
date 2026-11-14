//! zombie.vehicles.VehiclesDB2: the save's vehicle store (`vehicles.db`, one row per vehicle with its chunk, position
//! and the bytes `BaseVehicle.save` wrote). Reading only; the port does not write saves.
use super::base_vehicle::BaseVehicle;
use crate::java_nio::ByteBuffer;
use crate::zombie::inventory::inventory_item::ItemFactory;

/// zombie.iso.IsoObject.factoryVehicle: the class id `IsoMovingObject.save` writes for a vehicle.
pub const FACTORY_VEHICLE_CLASS_ID: i8 = 33;

/// zombie.vehicles.VehiclesDB2.VehicleBuffer: one row of the store.
#[derive(Clone, Debug, PartialEq)]
pub struct VehicleBuffer {
    pub id: i32,
    pub wx: i32,
    pub wy: i32,
    pub x: f32,
    pub y: f32,
    pub world_version: i32,
    pub in_meta: bool,
    pub bb: Vec<u8>,
}

impl VehicleBuffer {
    /// zombie.vehicles.VehiclesDB2.QueueLoadChunk.vehicleLoaded: `new BaseVehicle(cell)`, then `load` from the bytes
    /// after the serialise flag and class id. On an error the original logs it and deletes the row from the store.
    pub fn load_vehicle(&self, factory: &dyn ItemFactory) -> Result<BaseVehicle, String> {
        let mut b = ByteBuffer::wrap(&self.bb);
        b.set_position(2)?;
        BaseVehicle::load(&mut b, self.world_version, factory)
    }
}

/// zombie.vehicles.VehiclesDB2.SQLStore, the reading part.
#[cfg(not(target_arch = "wasm32"))]
pub struct SqlStore {
    conn: rusqlite::Connection,
}

#[cfg(not(target_arch = "wasm32"))]
impl SqlStore {
    /// Opens `vehicles.db` read-only (the original opens it for writing).
    pub fn open(db: &std::path::Path) -> Result<Self, String> {
        let conn = rusqlite::Connection::open_with_flags(
            db,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| format!("{}: {e}", db.display()))?;
        Ok(SqlStore { conn })
    }

    /// zombie.vehicles.VehiclesDB2.SQLStore.loadChunk (int, int, consumer): the rows of chunk (wx, wy) in the order
    /// SQLite returns them, keeping those whose first byte (serialise) is set and whose class id is a vehicle's.
    pub fn load_chunk(&self, wx: i32, wy: i32) -> Result<Vec<VehicleBuffer>, String> {
        let mut st = self
            .conn
            .prepare(
                "SELECT id, x, y, data, worldversion, inMeta FROM vehicles WHERE wx=? AND wy=?",
            )
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map([wx, wy], |r| {
                Ok(VehicleBuffer {
                    id: r.get(0)?,
                    wx,
                    wy,
                    // ResultSet.getFloat of a REAL column
                    x: r.get::<_, f64>(1)? as f32,
                    y: r.get::<_, f64>(2)? as f32,
                    bb: r.get(3)?,
                    world_version: r.get(4)?,
                    // ResultSet.getBoolean: NULL is false
                    in_meta: r.get::<_, Option<i64>>(5)?.unwrap_or(0) != 0,
                })
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            let v = row.map_err(|e| e.to_string())?;
            let serialise = v.bb.first().is_some_and(|&s| s != 0);
            let class_id = v.bb.get(1).map(|&c| c as i8);
            if class_id == Some(FACTORY_VEHICLE_CLASS_ID) && serialise {
                out.push(v);
            }
        }
        Ok(out)
    }

    /// The chunks that hold vehicles, ordered (for dumps and for loading a region).
    pub fn chunks(&self) -> Result<Vec<(i32, i32)>, String> {
        let mut st = self
            .conn
            .prepare("SELECT DISTINCT wx, wy FROM vehicles ORDER BY wx, wy")
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| e.to_string())?;
        rows.map(|r| r.map_err(|e| e.to_string())).collect()
    }
}
