//! zombie.erosion.obj.ErosionObjOverlaySprites: the overlay sprite of each stage and season.
use crate::zombie::iso::sprite::iso_sprite_manager::{IsoSpriteManager, SpriteRef};

/// zombie.erosion.obj.ErosionObjOverlaySprites
#[derive(Clone, Debug)]
pub struct ErosionObjOverlaySprites {
    pub name: String,
    pub stages: i32,
    /// per stage, the sprite name of each of the six seasons (`Stage.seasons` of the original)
    sprites: Vec<[Option<String>; 6]>,
}

impl ErosionObjOverlaySprites {
    /// zombie.erosion.obj.ErosionObjOverlaySprites constructor
    pub fn new(stages: i32, name: &str) -> Self {
        ErosionObjOverlaySprites {
            name: name.to_string(),
            stages,
            sprites: (0..stages).map(|_| Default::default()).collect(),
        }
    }

    /// zombie.erosion.obj.ErosionObjOverlaySprites.getSprite: `ErosionMain.getSpriteManager().getSprite(name)`. `None`
    /// where the original dereferences a season that was never set (NullPointerException).
    pub fn get_sprite(
        &self,
        sm: &mut IsoSpriteManager,
        stage: i32,
        season: i32,
    ) -> Option<SpriteRef> {
        let name = self
            .sprites
            .get(stage as usize)?
            .get(season as usize)?
            .as_ref()?;
        Some(sm.get_sprite(name))
    }

    /// zombie.erosion.obj.ErosionObjOverlaySprites.setSprite
    pub fn set_sprite(&mut self, stage: i32, sprite: &str, season: i32) {
        self.sprites[stage as usize][season as usize] = Some(sprite.to_string());
    }
}
