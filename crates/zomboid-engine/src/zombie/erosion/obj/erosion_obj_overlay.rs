//! zombie.erosion.obj.ErosionObjOverlay: an overlay sprite put on another object (wall vines, wall and street cracks).
use super::erosion_obj_overlay_sprites::ErosionObjOverlaySprites;
use crate::zombie::iso::iso_object::IsoObject;
use crate::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;

/// zombie.erosion.obj.ErosionObjOverlay
#[derive(Clone, Debug)]
pub struct ErosionObjOverlay {
    sprites: ErosionObjOverlaySprites,
    pub name: String,
    pub stages: i32,
    pub apply_alpha: bool,
    pub cycle_time: i32,
}

impl ErosionObjOverlay {
    /// zombie.erosion.obj.ErosionObjOverlay constructor
    pub fn new(sprites: ErosionObjOverlaySprites, cycle_time: i32, apply_alpha: bool) -> Self {
        ErosionObjOverlay {
            name: sprites.name.clone(),
            stages: sprites.stages,
            sprites,
            apply_alpha,
            cycle_time,
        }
    }

    /// zombie.erosion.obj.ErosionObjOverlay.setOverlay: replaces the overlay `cur_id` with the sprite of `stage` and
    /// `season` and returns its id (the sprite's `id`), or -1 for a stage out of range. The alpha is not used.
    pub fn set_overlay(
        &self,
        sm: &mut IsoSpriteManager,
        obj: &mut IsoObject,
        cur_id: i32,
        stage: i32,
        season: i32,
        _alpha: f32,
    ) -> i32 {
        if stage >= 0 && stage < self.stages {
            if cur_id >= 0 {
                self.remove_overlay(sm, obj, cur_id);
            }
            // The original dereferences the sprite of a season that was never set (NullPointerException).
            let Some(overlay) = self.sprites.get_sprite(sm, stage, season) else {
                return -1;
            };
            obj.attached_anim_sprite
                .get_or_insert_with(Vec::new)
                .push(overlay);
            sm.sprite(overlay).id
        } else {
            -1
        }
    }

    /// zombie.erosion.obj.ErosionObjOverlay.removeOverlay: the first loop takes out every overlay whose parent sprite
    /// has that id, so the second loop, which looks for one more, finds none and the method answers false.
    pub fn remove_overlay(&self, sm: &IsoSpriteManager, obj: &mut IsoObject, id: i32) -> bool {
        let Some(list) = obj.attached_anim_sprite.as_mut() else {
            return false;
        };
        if list.is_empty() {
            return false;
        }
        let mut j = 0;
        while j < list.len() {
            if sm.sprite(list[j]).id == id {
                list.remove(j);
            } else {
                j += 1;
            }
        }
        for i in (0..list.len()).rev() {
            if sm.sprite(list[i]).id == id {
                list.remove(i);
                return true;
            }
        }
        false
    }
}
