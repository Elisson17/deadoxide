//! zombie.globalObjects: the server side of the global object systems (farming plants, campfires, rain barrels, traps,
//! feeding troughs...) that a save keeps in `gos_<system>.bin`: the systems, their objects and the lookup that finds
//! an object by square or chunk.
//!
//! Ported: `SGlobalObjects` (the registry: `newSystem`, `getSystemByName`), `SGlobalObjectSystem` with its two loaders
//! (`load(ByteBuffer, int)` and the file form `load()`), `SGlobalObject.load`, `GlobalObjectSystem.newObject` /
//! `getObjectAt` / `hasObjectsInChunk` / `getObjectsInChunk` / `getObjectsAdjacentTo`, `GlobalObjectLookup`. Not
//! ported: the writers (`save`), `removeObject`, `Reset`, the Lua-facing keys (`setModDataKeys` and friends only
//! filter what the writer saves) and the client network side (`CGlobalObjects`, `SGlobalObjectNetwork`).
//! `DebugLog` lines the original prints (a location outside the map) are not reproduced.
pub mod global_object_lookup;
pub mod s_global_object_system;

pub use global_object_lookup::{GlobalObjectLookup, MetaGridBounds};
pub use s_global_object_system::{GlobalObject, SGlobalObjectSystem, SGlobalObjects};
