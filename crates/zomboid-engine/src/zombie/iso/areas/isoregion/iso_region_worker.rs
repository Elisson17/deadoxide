//! zombie.iso.areas.isoregion.IsoRegionWorker.loadSaveMap and the part of the worker thread that applies the loaded
//! jobs (`JobChunkUpdate.readChunksPacket` into the worker's `rootBuffer`).
use super::data::DataRoot;
use super::hash;
use super::job_chunk_update::{JobChunkUpdate, FILE_BUFFER_SIZE};
use crate::java_nio::ByteBuffer;

/// zombie.iso.areas.isoregion.IsoRegionWorker, the load side.
#[derive(Debug, Default)]
pub struct IsoRegionWorker {
    pub root_buffer: DataRoot,
    pub discovered_chunks: Vec<i32>,
    pub thread_discovered_chunks: Vec<i32>,
    /// The queued `ChunkUpdate` jobs, in order.
    jobs: Vec<JobChunkUpdate>,
    /// The static `byteBuffer` the chunk files are read into (zeros when the probe starts a load).
    file_buffer: Vec<u8>,
}

impl IsoRegionWorker {
    pub fn new() -> Self {
        IsoRegionWorker {
            file_buffer: vec![0; FILE_BUFFER_SIZE],
            ..Default::default()
        }
    }

    /// `loadSaveMap`: the header's list of known chunks, then every `datachunk_*.bin` (`chunk_files`, in the order the
    /// directory is listed) packed into jobs. A failure is the exception the method logs: it stops the load and
    /// leaves the jobs queued so far. Returns it.
    pub fn load_save_map(
        &mut self,
        header: Option<&[u8]>,
        chunk_files: &[Vec<u8>],
    ) -> Result<(), String> {
        let mut header_known_chunks: Vec<i32> = Vec::new();
        let mut has_read_header = false;
        if let Some(header) = header {
            let mut input = ByteBuffer::wrap(header);
            has_read_header = true;
            let _world_vers = input.get_int()?;
            let size = input.get_int()?;
            for _ in 0..size.max(0) {
                header_known_chunks.push(input.get_int()?);
            }
        }
        let mut j = JobChunkUpdate::default();
        let mut loaded_stuff = false;
        for file in chunk_files {
            // bb.clear(); len = inStream.read(bb.array()); bb.limit(len)
            if file.is_empty() {
                return Err(
                    "java.lang.IllegalArgumentException: newLimit < 0: (-1 < 0)".to_string()
                );
            }
            let len = file.len().min(FILE_BUFFER_SIZE);
            self.file_buffer[..len].copy_from_slice(&file[..len]);
            let mut bb = ByteBuffer::wrap(&self.file_buffer[..len]);
            let _l = bb.get_int()?;
            let _vers = bb.get_int()?;
            let x = bb.get_int()?;
            let y = bb.get_int()?;
            let hashid = hash(x, y);
            if !self.discovered_chunks.contains(&hashid) {
                self.discovered_chunks.push(hashid);
            }
            if let Some(i) = header_known_chunks.iter().position(|&h| h == hashid) {
                header_known_chunks.remove(i);
            }
            if !j.can_add_chunk() {
                self.jobs.push(std::mem::take(&mut j));
            }
            j.add_chunk_from_file(&self.file_buffer, len)?;
            loaded_stuff = true;
        }
        if j.chunk_count > 0 {
            self.jobs.push(j);
        }
        let _ = loaded_stuff;
        if has_read_header && !header_known_chunks.is_empty() {
            return Err(format!(
                "IsoRegion: {} previously discovered chunks have not been loaded.",
                header_known_chunks.len()
            ));
        }
        Ok(())
    }

    /// What the worker thread does with the queued `ChunkUpdate` jobs: `readChunksPacket` into `rootBuffer`.
    pub fn apply_jobs(&mut self) {
        for job in std::mem::take(&mut self.jobs) {
            job.read_chunks_packet(&mut self.root_buffer, &mut self.thread_discovered_chunks);
        }
    }
}
