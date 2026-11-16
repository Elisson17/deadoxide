//! zombie.globalObjects.SGlobalObjects, SGlobalObjectSystem (with its base GlobalObjectSystem), GlobalObject and
//! SGlobalObject: the read side. The original's systems all point at one shared `GlobalObjectLookup`; here the
//! registry owns it and a system is addressed by its index.
use super::global_object_lookup::{GlobalObjectLookup, MetaGridBounds, ObjectRef};
use crate::java_nio::ByteBuffer;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTable;

/// `SliceY.SliceBuffer`: what a file load reads of the file (the original never reads more).
const SLICE_BUFFER_SIZE: usize = 10_485_760;

/// zombie.globalObjects.GlobalObject (the saved part): a position and the object's mod data table.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobalObject {
    pub system: usize,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub mod_data: KahluaTable,
}

/// zombie.globalObjects.SGlobalObjectSystem
#[derive(Clone, Debug, PartialEq)]
pub struct SGlobalObjectSystem {
    pub name: String,
    pub mod_data: KahluaTable,
    pub objects: Vec<GlobalObject>,
    /// `loadedWorldVersion()`: -1 until a load finished.
    pub loaded_world_version: i32,
}

/// zombie.globalObjects.SGlobalObjects: the registered systems and the shared lookup.
pub struct SGlobalObjects {
    pub systems: Vec<SGlobalObjectSystem>,
    lookup: GlobalObjectLookup,
}

/// zombie.util.StringUtils.containsDoubleDot
fn contains_double_dot(s: &str) -> bool {
    s.contains("..") || s.contains("\u{0}.\u{0}.")
}

impl SGlobalObjects {
    /// A registry with the lookup `GlobalObjectLookup.init(metaGrid)` made.
    pub fn new(bounds: MetaGridBounds) -> Self {
        SGlobalObjects {
            systems: Vec::new(),
            lookup: GlobalObjectLookup::new(bounds),
        }
    }

    /// `getSystemByName`
    pub fn get_system_by_name(&self, name: &str) -> Option<usize> {
        self.systems.iter().position(|s| s.name == name)
    }

    /// `newSystem` (the `GlobalObjectSystem` constructor check on the name included): the new system's index.
    pub fn new_system(&mut self, name: &str) -> Result<usize, String> {
        if self.get_system_by_name(name).is_some() {
            return Err(
                "java.lang.IllegalStateException: system with that name already exists".to_string(),
            );
        }
        if contains_double_dot(name) {
            return Err(format!(
                "java.lang.IllegalStateException: Name of GlobalObjectSystem invalid: {name}"
            ));
        }
        self.systems.push(SGlobalObjectSystem {
            name: name.to_string(),
            mod_data: KahluaTable::default(),
            objects: Vec::new(),
            loaded_world_version: -1,
        });
        Ok(self.systems.len() - 1)
    }

    fn locate(&self) -> impl Fn(ObjectRef) -> (i32, i32, i32) + '_ {
        move |o| {
            let obj = &self.systems[o.system].objects[o.index];
            (obj.x, obj.y, obj.z)
        }
    }

    /// `GlobalObjectSystem.getObjectAt(x, y, z)`: the object's index in the system's list.
    pub fn get_object_at(&self, system: usize, x: i32, y: i32, z: i32) -> Option<usize> {
        self.lookup
            .get_object_at(system, x, y, z, &self.locate())
            .map(|o| o.index)
    }

    /// `GlobalObjectSystem.newObject`: throws for a square that already has an object of the system, else adds
    /// the object to the system and the lookup.
    pub fn new_object(&mut self, system: usize, x: i32, y: i32, z: i32) -> Result<usize, String> {
        if self.get_object_at(system, x, y, z).is_some() {
            return Err(format!(
                "java.lang.IllegalStateException: already an object at {x},{y},{z}"
            ));
        }
        let sys = &mut self.systems[system];
        sys.objects.push(GlobalObject {
            system,
            x,
            y,
            z,
            mod_data: KahluaTable::default(),
        });
        let index = sys.objects.len() - 1;
        self.lookup.add_object(ObjectRef { system, index }, x, y)?;
        Ok(index)
    }

    /// `GlobalObjectSystem.hasObjectsInChunk`
    pub fn has_objects_in_chunk(&self, system: usize, wx: i32, wy: i32) -> bool {
        self.lookup.has_objects_in_chunk(system, wx, wy)
    }

    /// `GlobalObjectSystem.getObjectsInChunk`: object indices in lookup order.
    pub fn get_objects_in_chunk(&self, system: usize, wx: i32, wy: i32) -> Vec<usize> {
        self.lookup
            .get_objects_in_chunk(system, wx, wy)
            .into_iter()
            .map(|o| o.index)
            .collect()
    }

    /// `GlobalObjectSystem.getObjectsAdjacentTo`: the 3 x 3 squares around (x, y) on level z, row by row.
    pub fn get_objects_adjacent_to(&self, system: usize, x: i32, y: i32, z: i32) -> Vec<usize> {
        let mut list = Vec::new();
        for dy in -1..=1_i32 {
            for dx in -1..=1_i32 {
                if let Some(i) =
                    self.get_object_at(system, x.wrapping_add(dx), y.wrapping_add(dy), z)
                {
                    list.push(i);
                }
            }
        }
        list
    }

    /// `SGlobalObjectSystem.load(ByteBuffer, int)`: the optional mod data table, then the objects; a failure leaves
    /// what was read before it, and `loadedWorldVersion` unchanged.
    pub fn load(
        &mut self,
        system: usize,
        bb: &mut ByteBuffer,
        world_version: i32,
    ) -> Result<(), String> {
        if bb.get()? != 0 {
            self.systems[system].mod_data.load_into(bb, world_version)?;
        }
        let count = bb.get_int()?;
        for _ in 0..count {
            let x = bb.get_int()?;
            let y = bb.get_int()?;
            let z = i32::from(bb.get()?);
            let index = self.new_object(system, x, y, z)?;
            // SGlobalObject.load
            if bb.get()? != 0 {
                self.systems[system].objects[index]
                    .mod_data
                    .load_into(bb, world_version)?;
            }
        }
        self.systems[system].loaded_world_version = world_version;
        Ok(())
    }

    /// `SGlobalObjectSystem.load()` over the bytes of `gos_<name>.bin` (`None` is a missing file): the "GLOS" magic, a
    /// world version of at most 249, then `load(ByteBuffer, int)`. The original logs a failure and keeps what was
    /// loaded; this returns the error text the log would carry.
    pub fn load_file(&mut self, system: usize, data: Option<&[u8]>) -> Result<(), String> {
        let Some(data) = data else {
            return Ok(());
        };
        if data.is_empty() {
            return Err("java.lang.IllegalArgumentException: newLimit < 0: (-1 < 0)".to_string());
        }
        let data = &data[..data.len().min(SLICE_BUFFER_SIZE)];
        let mut bb = ByteBuffer::wrap_slice_buffer(data);
        if bb.get_bytes(4)? != b"GLOS" {
            return Err(
                "java.io.IOException: doesn't appear to be a GlobalObjectSystem file".to_string(),
            );
        }
        let world_version = bb.get_int()?;
        if world_version > 249 {
            return Err(format!(
                "java.io.IOException: file is from a newer version {world_version} of the game"
            ));
        }
        self.load(system, &mut bb, world_version)
    }
}
