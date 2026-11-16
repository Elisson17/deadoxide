//! What the erosion classes reach through singletons in the original, made explicit: `IsoWorld.instance.currentCell`
//! (here the server's loaded chunks), `IsoSpriteManager.instance`, the tile property aliases, the season of
//! `ErosionMain.getInstance().getSeasons()` and the `ErosionData.Square.rands` table.
use super::season::erosion_season::ErosionSeason;
use crate::zombie::core::tile_property_alias_map::TilePropertyAliasMap;
use crate::zombie::iso::iso_grid_square::IsoGridSquare;
use crate::zombie::iso::server_map::ServerMap;
use crate::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;

/// The coordinates of a square: x, y, z.
pub type Pos = (i32, i32, i32);

/// The state the category, object and world code runs against.
pub struct Env<'a> {
    pub map: &'a mut ServerMap,
    pub sm: &'a mut IsoSpriteManager,
    pub alias: &'a TilePropertyAliasMap,
    pub season: &'a ErosionSeason,
    pub rands: &'a [i32],
}

/// `ArrayIndexOutOfBoundsException` of a Java array read, as the message the JVM gives.
pub fn aioobe(index: i32, len: usize) -> String {
    format!(
        "java.lang.ArrayIndexOutOfBoundsException: Index {index} out of bounds for length {len}"
    )
}

/// `array[index]` of a Java int array.
pub fn at(array: &[i32], index: i32) -> Result<i32, String> {
    if index < 0 {
        return Err(aioobe(index, array.len()));
    }
    array
        .get(index as usize)
        .copied()
        .ok_or_else(|| aioobe(index, array.len()))
}

impl Env<'_> {
    /// The square at `pos`; the erosion code is only called for squares of loaded chunks.
    pub fn sq(&self, pos: Pos) -> Result<&IsoGridSquare, String> {
        self.map
            .get_grid_square(pos.0, pos.1, pos.2)
            .ok_or_else(|| {
                format!(
                    "java.lang.NullPointerException: no square at {},{},{}",
                    pos.0, pos.1, pos.2
                )
            })
    }

    pub fn sq_mut(&mut self, pos: Pos) -> Result<&mut IsoGridSquare, String> {
        self.map
            .get_grid_square_mut(pos.0, pos.1, pos.2)
            .ok_or_else(|| {
                format!(
                    "java.lang.NullPointerException: no square at {},{},{}",
                    pos.0, pos.1, pos.2
                )
            })
    }

    /// `obj.getSprite().getName()` of the object at `index` on the square at `pos`: `None` when the object has no
    /// sprite or the sprite no name (the tests of the original are `spr != null && spr.getName() != null`).
    pub fn object_sprite_name(&self, pos: Pos, index: usize) -> Result<Option<String>, String> {
        let sq = self.sq(pos)?;
        let o = sq.objects.get(index).ok_or_else(|| {
            format!(
                "java.lang.IndexOutOfBoundsException: Index {index} out of bounds for length {}",
                sq.objects.len()
            )
        })?;
        Ok(o.sprite.and_then(|s| self.sm.sprite(s).name.clone()))
    }
}
