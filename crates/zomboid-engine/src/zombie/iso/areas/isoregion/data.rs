//! zombie.iso.areas.isoregion.data: DataChunk, DataCell and DataRoot, the part a save fills.
use std::collections::HashMap;

use super::hash;
use crate::java_nio::ByteBuffer;
use crate::zombie::core::math::pz_math;

/// zombie.iso.areas.isoregion.data.DataChunk, the saved fields: the square flags of the 8 x 8 squares of each active
/// level (`squareFlags`, `activeZLayers`, `highestZ`).
#[derive(Clone, Debug, PartialEq)]
pub struct DataChunk {
    pub chunk_x: i32,
    pub chunk_y: i32,
    pub hash_id: i32,
    pub highest_z: i32,
    pub active_z_layers: [bool; 32],
    pub square_flags: Option<Vec<u8>>,
}

impl DataChunk {
    /// The constructor: no levels, no flags.
    fn new(chunk_x: i32, chunk_y: i32, chunk_id: i32) -> Self {
        DataChunk {
            chunk_x,
            chunk_y,
            hash_id: if chunk_id < 0 {
                hash(chunk_x, chunk_y)
            } else {
                chunk_id
            },
            highest_z: 0,
            active_z_layers: [false; 32],
            square_flags: None,
        }
    }

    /// `getCoord1D`
    fn coord_1d(x: i32, y: i32, z: i32) -> i32 {
        z.wrapping_mul(64).wrapping_add(y * 8).wrapping_add(x)
    }

    /// `ensureSquareArray`: the flags array grows to hold level `zlayer` (new bytes 0).
    fn ensure_square_array(&mut self, zlayer: i32) {
        let size = ((zlayer + 1) * 8 * 8) as usize;
        if self.square_flags.as_ref().is_none_or(|f| f.len() < size) {
            let mut new = vec![0u8; size];
            if let Some(old) = &self.square_flags {
                new[..old.len()].copy_from_slice(old);
            }
            self.square_flags = Some(new);
        }
    }

    /// `ensureSquares`: activates a level, filling it with floor flags on level 0 and 0 above.
    fn ensure_squares(&mut self, zlayer: i32) {
        if (0..32).contains(&zlayer) && !self.active_z_layers[zlayer as usize] {
            self.ensure_square_array(zlayer);
            self.active_z_layers[zlayer as usize] = true;
            if zlayer > self.highest_z {
                self.highest_z = zlayer;
            }
            if let Some(flags) = self.square_flags.as_mut() {
                for y in 0..8 {
                    for x in 0..8 {
                        flags[Self::coord_1d(x, y, zlayer) as usize] =
                            if zlayer == 0 { 16 } else { 0 };
                    }
                }
            }
        }
    }

    /// `getSquare(x, y, z, true)`: -1 for a level that is not active or a chunk without flags. A level outside 0..32
    /// is the original's `ArrayIndexOutOfBoundsException`.
    pub fn get_square(&self, x: i32, y: i32, z: i32) -> Result<i8, String> {
        let Some(flags) = &self.square_flags else {
            return Ok(-1);
        };
        let active = usize::try_from(z)
            .ok()
            .and_then(|z| self.active_z_layers.get(z))
            .ok_or_else(|| {
                format!("java.lang.ArrayIndexOutOfBoundsException: Index {z} out of bounds for length 32")
            })?;
        if *active {
            Ok(flags[Self::coord_1d(x, y, z) as usize] as i8)
        } else {
            Ok(-1)
        }
    }

    /// `DataChunk.load(ByteBuffer, int, boolean)`: an optional length, the highest level, a byte count, then that
    /// many flag bytes. The original catches and logs every exception, keeping what was set before it.
    pub fn load(&mut self, bb: &mut ByteBuffer, _world_version: i32, read_length: bool) {
        let _ = (|| -> Result<(), String> {
            if read_length {
                bb.get_int()?;
            }
            self.highest_z = bb.get_int()?;
            // ensureSquares ignores every level from 32 up: starting at 31 is the same
            let mut z = self.highest_z.min(31);
            while z >= 0 {
                self.ensure_squares(z);
                z -= 1;
            }
            let max_bytes = bb.get_int()?;
            for i in 0..max_bytes.max(0) as usize {
                let b = bb.get()?;
                let flags = self
                    .square_flags
                    .as_mut()
                    .ok_or("java.lang.NullPointerException")?;
                *flags.get_mut(i).ok_or_else(|| {
                    format!(
                        "java.lang.ArrayIndexOutOfBoundsException: Index {i} out of bounds for length"
                    )
                })? = b as u8;
            }
            Ok(())
        })();
    }
}

/// zombie.iso.areas.isoregion.data.DataRoot: cells by id, each with its chunks by id.
#[derive(Clone, Debug, Default)]
pub struct DataRoot {
    cells: HashMap<i32, HashMap<i32, DataChunk>>,
}

impl DataRoot {
    /// `select.reset(x, y, z, true, false)`, the chunk it leaves selected: the cell and the chunk are created when
    /// missing. Coordinates divide with Java's truncation (a negative one lands in cell/chunk 0).
    pub fn select_chunk_creating(&mut self, x: i32, y: i32) -> &mut DataChunk {
        let chunkx = x / 8;
        let chunky = y / 8;
        let cell_id = hash(x / 256, y / 256);
        let chunk_id = hash(chunkx, chunky);
        self.cells
            .entry(cell_id)
            .or_default()
            .entry(chunk_id)
            .or_insert_with(|| DataChunk::new(chunkx, chunky, chunk_id))
    }

    /// `getDataChunk(chunkx, chunky)`
    pub fn get_data_chunk(&self, chunkx: i32, chunky: i32) -> Option<&DataChunk> {
        self.cells
            .get(&hash(chunkx / 32, chunky / 32))?
            .get(&hash(chunkx, chunky))
    }

    /// `getAllChunks`, sorted by (chunk x, chunk y) for printing.
    pub fn all_chunks_sorted(&self) -> Vec<&DataChunk> {
        let mut v: Vec<&DataChunk> = self.cells.values().flat_map(|c| c.values()).collect();
        v.sort_by_key(|c| (c.chunk_x, c.chunk_y));
        v
    }

    /// `getSquareFlags(x, y, z)`: `selectInternal.reset(x, y, z, false)` then its `square` (-1 when there is none).
    pub fn get_square_flags(&self, x: i32, y: i32, z: i32) -> Result<i8, String> {
        let chunk_square_x = pz_math::coordmodulo(x, 8);
        let chunk_square_y = pz_math::coordmodulo(y, 8);
        let cell_id = hash(x / 256, y / 256);
        let chunk_id = hash(x / 8, y / 8);
        match self.cells.get(&cell_id).and_then(|c| c.get(&chunk_id)) {
            Some(chunk) => chunk.get_square(chunk_square_x, chunk_square_y, z),
            None => Ok(-1),
        }
    }
}
