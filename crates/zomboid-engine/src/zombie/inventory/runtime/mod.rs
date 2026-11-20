//! The inventory as the running game holds it: items (`zombie.inventory.InventoryItem` and the classes of
//! `zombie.inventory.types` that `Item.InstanceItem` makes of a script) and the `ItemContainer`s that hold them, with the
//! operations the game runs on them. `inventory_item.rs` is the other view of an item, the state a save stores.
//!
//! Ownership is restructured, not the algorithms: every item and container lives in an arena of `World` and refers to the
//! others by index (`ItemId`, `ContainerId`), where the original holds object references. What the original reads from
//! the rest of the game is a field of `World` (the scripts, the generator, the clock, the sandbox options, the
//! processing list of the cell). Whatever an operation would reach beyond that (a character, a world object, the UI, a
//! sound, the network) is not modelled: an operation that needs it returns `Err("not ported: ...")` instead of a value.
pub mod container;
pub mod food;
pub mod item;
pub mod save;
pub mod update;
pub mod world;

pub use container::{Container, Pred};
pub use item::{ContainerId, ItemId, LiveItem};
pub use world::{Scripts, World};
