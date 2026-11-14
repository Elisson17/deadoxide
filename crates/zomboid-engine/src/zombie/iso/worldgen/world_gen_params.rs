//! zombie.iso.worldgen.WorldGenParams: the world seed and generation bounds a save keeps in `map_worldgen.bin`.
use crate::java;
use crate::java_nio::{read_string, ByteBuffer};

/// zombie.iso.worldgen.WorldGenParams.FILE_MAGIC
const FILE_MAGIC: [u8; 4] = *b"WGEN";

/// zombie.iso.worldgen.WorldGenParams, the saved fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldGenParams {
    pub seed_string: String,
    /// `seedString.hashCode()`
    pub seed: i32,
    pub min_x_cell: i32,
    pub min_y_cell: i32,
    pub max_x_cell: i32,
    pub max_y_cell: i32,
}

impl Default for WorldGenParams {
    /// The field initialisers.
    fn default() -> Self {
        WorldGenParams {
            seed_string: String::new(),
            seed: 0,
            min_x_cell: -250,
            min_y_cell: -250,
            max_x_cell: 250,
            max_y_cell: 250,
        }
    }
}

impl WorldGenParams {
    /// zombie.iso.worldgen.WorldGenParams.setSeedString
    pub fn set_seed_string(&mut self, seed_string: &str) {
        self.seed_string = seed_string.to_string();
        self.seed = java::string_hash_code(seed_string);
    }

    /// zombie.iso.worldgen.WorldGenParams.getSeed (the int seed widened to long).
    pub fn get_seed(&self) -> i64 {
        i64::from(self.seed)
    }

    /// zombie.iso.worldgen.WorldGenParams.load over the bytes of `map_worldgen.bin`: magic, world version, seed text,
    /// cell bounds. The original logs a broken file and keeps the fields it had set; this returns them the same way
    /// along with the error.
    pub fn load(data: &[u8]) -> (Self, Option<String>) {
        let mut p = WorldGenParams::default();
        let mut b = ByteBuffer::wrap(data);
        let r = (|| -> Result<(), String> {
            if b.get_bytes(4)? != FILE_MAGIC {
                return Err("java.io.IOException: does not appear to be map_worldgen.bin".into());
            }
            let _world_version = b.get_int()?;
            let seed = read_string(&mut b)?;
            p.set_seed_string(&seed);
            p.min_x_cell = b.get_int()?;
            p.min_y_cell = b.get_int()?;
            p.max_x_cell = b.get_int()?;
            p.max_y_cell = b.get_int()?;
            Ok(())
        })();
        (p, r.err())
    }
}
