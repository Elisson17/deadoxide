//! zombie.iso.areas.isoregion: the saved part of IsoRegions, the per-chunk square flags a save keeps in
//! `isoregiondata/datachunk_<x>_<y>.bin` (and the list of known chunks in `RegionHeader.bin`).
//!
//! Ported: `IsoRegions.hash`, `data.DataChunk` (the fields a save holds and `load`), `data.DataRoot` (cells and chunks,
//! `SelectInfo.reset`, `getDataChunk`, `getSquareFlags`), `jobs.JobChunkUpdate` (`addChunkFromFile`,
//! `readChunksPacket`) and `IsoRegionWorker.loadSaveMap`. Not ported: everything derived from the flags (the chunk
//! and world regions, `recalculate`, flood fill, merging: `DataRoot.processDirtyChunks`), the writers, the network
//! packets, the square updates and the debug renderer.
pub mod data;
pub mod iso_region_worker;
pub mod job_chunk_update;

/// zombie.iso.areas.isoregion.IsoRegions.hash
pub fn hash(x: i32, y: i32) -> i32 {
    y.wrapping_shl(16) ^ x
}
