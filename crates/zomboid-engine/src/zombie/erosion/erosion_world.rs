//! zombie.erosion.ErosionWorld: which categories a square may have, and their update.
use super::erosion_data::{Chunk, Square};
use super::erosion_env::{Env, Pos};
use super::erosion_regions::ErosionRegions;
use super::season::erosion_ice_queen::ErosionIceQueen;
use crate::zombie::iso::sprite_details::iso_flag_type::IsoFlagType;
use crate::zombie::iso::sprite_details::iso_object_type::IsoObjectType;

/// zombie.erosion.ErosionWorld (with the static `ErosionRegions` it uses).
#[derive(Default)]
pub struct ErosionWorld {
    pub regions: ErosionRegions,
}

impl ErosionWorld {
    /// zombie.erosion.ErosionWorld.init: `ErosionRegions.init()`.
    pub fn init(&mut self, sm: &mut crate::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager, ice_queen: &mut ErosionIceQueen) -> bool {
        self.regions.init(sm, ice_queen);
        true
    }

    /// zombie.erosion.ErosionWorld.validateSpawn: for every region the square suits, the first category that takes an
    /// existing object or accepts a spawn; without any, the square does nothing.
    pub fn validate_spawn(
        &mut self,
        env: &mut Env,
        pos: Pos,
        sq_erosion_data: &mut Square,
        chunk_data: &Chunk,
    ) -> Result<(), String> {
        let (is_exterior, has_wall, sq_tex_name) = {
            let square = env.sq(pos)?;
            let floor = square.get_floor(env.sm);
            let name = floor
                .and_then(|f| square.objects[f].sprite)
                .and_then(|s| env.sm.sprite(s).name.clone());
            (
                square.has(IsoFlagType::exterior),
                square.has_type(IsoObjectType::wall),
                name,
            )
        };
        let Some(sq_tex_name) = sq_tex_name else {
            sq_erosion_data.do_nothing = true;
            return Ok(());
        };
        let mut has_spawned = false;
        for region in &mut self.regions.regions {
            let matches = region
                .tile_name_match
                .as_ref()
                .is_none_or(|m| sq_tex_name.starts_with(m.as_str()))
                && (!region.check_exterior || region.is_exterior == is_exterior)
                && (!region.has_wall || region.has_wall == has_wall);
            if !matches {
                continue;
            }
            for category in &mut region.categories {
                let mut spawned = category.replace_existing_object(
                    env,
                    pos,
                    sq_erosion_data,
                    chunk_data,
                    is_exterior,
                    has_wall,
                )?;
                if !spawned {
                    spawned = category.validate_spawn(
                        env,
                        pos,
                        sq_erosion_data,
                        chunk_data,
                        is_exterior,
                        has_wall,
                        false,
                    )?;
                }
                if spawned {
                    has_spawned = true;
                    break;
                }
            }
        }
        if !has_spawned {
            sq_erosion_data.do_nothing = true;
        }
        Ok(())
    }

    /// zombie.erosion.ErosionWorld.update: every category data of the square, in order; a data object that a
    /// category takes off the square is not skipped over (`i--`).
    pub fn update(
        &mut self,
        env: &mut Env,
        pos: Pos,
        sq_erosion_data: &mut Square,
        chunk_data: &Chunk,
        e_tick: i32,
    ) -> Result<(), String> {
        let mut i = 0usize;
        while i < sq_erosion_data.regions.len() {
            let mut data = sq_erosion_data.regions[i].clone();
            let category = self
                .regions
                .get_category(data.region_id, data.category_id)?;
            let size = sq_erosion_data.regions.len();
            category.update(env, pos, sq_erosion_data, &mut data, chunk_data, e_tick)?;
            if size > sq_erosion_data.regions.len() {
                // the category took its data off the square; the one that moved into its place is next
            } else {
                sq_erosion_data.regions[i] = data;
                i += 1;
            }
        }
        Ok(())
    }
}
