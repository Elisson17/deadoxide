//! zombie.erosion.season.ErosionIceQueen: the table of winter (snow) sprites that go with the normal ones.
use crate::java;
use crate::zombie::core::math::pz_math;
use crate::zombie::core::tile_property_alias_map::TilePropertyAliasMap;
use crate::zombie::iso::sprite::iso_sprite_manager::{IsoSpriteManager, SpriteRef};
use crate::zombie::util::string_utils;

/// zombie.erosion.season.ErosionIceQueen.Sprite
#[derive(Clone, Debug)]
struct Sprite {
    sprite: SpriteRef,
    #[allow(dead_code)]
    normal: String,
    winter: String,
}

/// zombie.erosion.season.ErosionIceQueen
#[derive(Clone, Debug, Default)]
pub struct ErosionIceQueen {
    sprites: Vec<Sprite>,
    snow_state: bool,
}

impl ErosionIceQueen {
    /// zombie.erosion.season.ErosionIceQueen constructor: `readTileProperties`.
    pub fn new(sm: &mut IsoSpriteManager, alias: &TilePropertyAliasMap) -> Self {
        let mut q = ErosionIceQueen::default();
        q.read_tile_properties(sm, alias);
        q
    }

    /// zombie.erosion.season.ErosionIceQueen.addSprite: both sprites are looked up (and created when the name is
    /// unknown, as `IsoSpriteManager.getSprite` does) and the normal one is given its name.
    pub fn add_sprite(&mut self, sm: &mut IsoSpriteManager, sprite: &str, winter_sprite: &str) {
        let spr = sm.get_sprite(sprite);
        let _winter = sm.get_sprite(winter_sprite);
        sm.sprite_mut(spr).set_name(Some(sprite.to_string()));
        self.sprites.push(Sprite {
            sprite: spr,
            normal: sprite.to_string(),
            winter: winter_sprite.to_string(),
        });
    }

    /// zombie.erosion.season.ErosionIceQueen.setSnow
    pub fn set_snow(&mut self, sm: &mut IsoSpriteManager, is_snow: bool) {
        if self.snow_state != is_snow {
            self.snow_state = is_snow;
            for s in &self.sprites {
                let winter = sm.named_map.get(&s.winter).copied();
                sm.sprite_mut(s.sprite).set_snow_sprite(winter);
            }
        }
    }

    /// zombie.erosion.season.ErosionIceQueen.isSnow
    pub fn is_snow(&self) -> bool {
        self.snow_state
    }

    /// zombie.erosion.season.ErosionIceQueen.readTileProperties: every sprite with a `SnowTile` property. The
    /// original walks a copy of the HashMap's values (its order is not defined by the source); the sprites are
    /// taken in creation order here, which only decides the order in which missing winter sprites are created.
    fn read_tile_properties(&mut self, sm: &mut IsoSpriteManager, alias: &TilePropertyAliasMap) {
        let mut refs: Vec<SpriteRef> = sm.named_map.values().copied().collect();
        refs.sort_by_key(|r| r.0);
        refs.dedup();
        for r in refs {
            let sprite = sm.sprite(r);
            if sprite.properties.has_str(alias, "SnowTile") {
                let snow_tile = sprite.properties.get(alias, "SnowTile").map(str::to_string);
                let name = sprite.name.clone();
                if let Some(snow_tile) = snow_tile {
                    if !string_utils::is_null_or_whitespace(Some(&snow_tile)) {
                        let snow_tile = Self::fix_tile_name(&snow_tile);
                        // A sprite without a name would be put under the key null by the original; none has a
                        // SnowTile property in the installed tile definitions.
                        if let Some(name) = name {
                            self.add_sprite(sm, &name, &snow_tile);
                        }
                    }
                }
            }
        }
    }

    /// zombie.erosion.season.ErosionIceQueen.fixTileName
    fn fix_tile_name(tile_name: &str) -> String {
        match tile_name.rfind('_') {
            None => tile_name.to_string(),
            Some(p) => {
                let tileset_name = &tile_name[..p];
                let index = pz_math::try_parse_int(Some(java::trim(&tile_name[p + 1..])), -1);
                if index == -1 {
                    tile_name.to_string()
                } else {
                    format!("{tileset_name}_{index}")
                }
            }
        }
    }
}
