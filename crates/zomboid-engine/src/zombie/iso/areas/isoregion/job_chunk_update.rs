//! zombie.iso.areas.isoregion.jobs.JobChunkUpdate: a 64 KiB buffer of chunk blocks. Only the two sides a save load
//! uses are ported: filling it from a chunk file and reading it back into a `DataRoot`.
use super::data::DataRoot;
use super::hash;
use crate::java_nio::ByteBuffer;

/// `ByteBuffer.allocate(65536)`
const BUFFER_SIZE: usize = 65536;

/// The static `ByteBuffer.allocate(2076)` a chunk file is read into (`IsoRegionWorker.byteBuffer`).
pub const FILE_BUFFER_SIZE: usize = 2076;

/// zombie.iso.areas.isoregion.jobs.JobChunkUpdate. A fresh job's buffer is zeros (a pooled job keeps stale bytes; the
/// oracle probe empties the pool).
#[derive(Clone, Debug)]
pub struct JobChunkUpdate {
    buffer: Vec<u8>,
    position: usize,
    pub chunk_count: i32,
    buffer_max_bytes: i32,
}

impl Default for JobChunkUpdate {
    fn default() -> Self {
        JobChunkUpdate {
            buffer: vec![0; BUFFER_SIZE],
            position: 0,
            chunk_count: 0,
            buffer_max_bytes: 0,
        }
    }
}

impl JobChunkUpdate {
    fn put_int(&mut self, v: i32) -> Result<(), String> {
        let end = self.position + 4;
        if end > BUFFER_SIZE {
            return Err("java.nio.BufferOverflowException".to_string());
        }
        self.buffer[self.position..end].copy_from_slice(&v.to_be_bytes());
        self.position = end;
        Ok(())
    }

    /// `canAddChunk`
    pub fn can_add_chunk(&self) -> bool {
        self.position + 2076 < BUFFER_SIZE
    }

    /// `addChunkFromFile(bb)`: `file` is the 2076 byte array of the worker's buffer and `limit` the length read
    /// into it. Failures are the exceptions the original lets through (the whole load stops).
    pub fn add_chunk_from_file(&mut self, file: &[u8], limit: usize) -> Result<bool, String> {
        if self.position + limit >= BUFFER_SIZE {
            return Ok(false);
        }
        let mut bb = ByteBuffer::wrap(&file[..limit]);
        let _num_bytes = bb.get_int()?;
        let version = bb.get_int()?;
        let wx = bb.get_int()?;
        let wy = bb.get_int()?;
        // startBufferBlock
        if self.chunk_count == 0 {
            self.position = 0;
            self.put_int(0)?;
            self.put_int(0)?;
        }
        let buffer_start_pos = self.position;
        self.put_int(0)?;
        self.put_int(version)?;
        self.put_int(wx)?;
        self.put_int(wy)?;
        let length = bb.get_int()?;
        // buffer.put(bb.array(), bb.position(), length): the position is 16 again after the reset
        let src_start = 16usize;
        let src_len = usize::try_from(length)
            .ok()
            .filter(|&l| src_start + l <= file.len())
            .ok_or_else(|| {
                format!(
                    "java.lang.IndexOutOfBoundsException: Range [{src_start}, {src_start} + {length}) out of bounds for length {}",
                    file.len()
                )
            })?;
        if src_len > BUFFER_SIZE - self.position {
            return Err("java.nio.BufferOverflowException".to_string());
        }
        self.buffer[self.position..self.position + src_len]
            .copy_from_slice(&file[src_start..src_start + src_len]);
        self.position += src_len;
        // endBufferBlock
        self.buffer_max_bytes = self.position as i32;
        let max = self.buffer_max_bytes;
        let count = self.chunk_count + 1;
        self.position = buffer_start_pos;
        self.put_int(max - buffer_start_pos as i32)?;
        self.chunk_count = count;
        self.position = 0;
        self.put_int(max)?;
        self.put_int(count)?;
        self.position = max as usize;
        Ok(true)
    }

    /// `readChunksPacket(root, knownChunks)` on a server or single player: each block's chunk is selected (created)
    /// and `DataChunk.load` reads its flags from the buffer where the previous load stopped.
    pub fn read_chunks_packet(&self, root: &mut DataRoot, known_chunks: &mut Vec<i32>) {
        let mut bb = ByteBuffer::wrap(&self.buffer);
        let result = (|| -> Result<(), String> {
            let _total_len = bb.get_int()?;
            let count = bb.get_int()?;
            for _ in 0..count.max(0) {
                let _packlen = bb.get_int()?;
                let world_vers = bb.get_int()?;
                let chunkx = bb.get_int()?;
                let chunky = bb.get_int()?;
                let chunk =
                    root.select_chunk_creating(chunkx.wrapping_mul(8), chunky.wrapping_mul(8));
                let hashid = hash(chunkx, chunky);
                if !known_chunks.contains(&hashid) {
                    known_chunks.push(hashid);
                }
                chunk.load(&mut bb, world_vers, true);
            }
            Ok(())
        })();
        let _ = result;
    }
}
