//! zombie.erosion.categories.StreetCracks: cracks in the street, with grass growing out of them.
use super::erosion_category::{
    clerp, current_season, set_cat_mod_data, update_obj, CategoryBase, ErosionCategory,
};
use crate::zombie::erosion::erosion_data::{Category, CategoryData, Chunk, Square};
use crate::zombie::erosion::erosion_env::{at, Env, Pos};
use crate::zombie::erosion::obj::erosion_obj::ErosionObj;
use crate::zombie::erosion::obj::erosion_obj_overlay::ErosionObjOverlay;
use crate::zombie::erosion::obj::erosion_obj_overlay_sprites::ErosionObjOverlaySprites;
use crate::zombie::erosion::obj::erosion_obj_sprites::ErosionObjSprites;
use crate::zombie::erosion::season::erosion_ice_queen::ErosionIceQueen;
use crate::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;

/// zombie.erosion.categories.StreetCracks
pub struct StreetCracks {
    base: CategoryBase,
    objs: Vec<ErosionObj>,
    crack_objs: Vec<ErosionObjOverlay>,
    spawn_chance: [i32; 100],
}

impl Default for StreetCracks {
    fn default() -> Self {
        StreetCracks {
            base: CategoryBase::default(),
            objs: Vec::new(),
            crack_objs: Vec::new(),
            spawn_chance: [0; 100],
        }
    }
}

impl ErosionCategory for StreetCracks {
    fn base(&self) -> &CategoryBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut CategoryBase {
        &mut self.base
    }

    fn kind(&self) -> Category {
        Category::StreetCracks
    }

    /// zombie.erosion.categories.StreetCracks.replaceExistingObject: nothing is replaced.
    fn replace_existing_object(
        &mut self,
        _env: &mut Env,
        _pos: Pos,
        _ed: &mut Square,
        _chunk: &Chunk,
        _is_exterior: bool,
        _has_wall: bool,
    ) -> Result<bool, String> {
        Ok(false)
    }

    /// zombie.erosion.categories.StreetCracks.validateSpawn
    fn validate_spawn(
        &mut self,
        env: &mut Env,
        pos: Pos,
        ed: &mut Square,
        _chunk: &Chunk,
        _is_exterior: bool,
        _has_wall: bool,
        _is_respawn: bool,
    ) -> Result<bool, String> {
        let e_value = ed.noise_main_int;
        let spawn_chance = at(&self.spawn_chance, e_value)?;
        if spawn_chance == 0 {
            return Ok(false);
        }
        let (x, y) = (pos.0, pos.1);
        if ed.rand(env.rands, x, y, 101) >= spawn_chance {
            return Ok(false);
        }
        let di = set_cat_mod_data(&self.base, Category::StreetCracks, ed);
        let game_obj = ed.rand(env.rands, x, y, self.crack_objs.len() as i32);
        let magic_num = ed.magic_num;
        let d = &mut ed.regions[di];
        d.game_obj = game_obj;
        d.max_stage = if e_value > 65 {
            2
        } else if e_value > 55 {
            1
        } else {
            0
        };
        d.stage = 0;
        d.spawn_time = 150 - e_value;
        if magic_num > 0.5 {
            d.has_grass = true;
        }
        Ok(true)
    }

    /// zombie.erosion.categories.StreetCracks.update
    fn update(
        &mut self,
        env: &mut Env,
        pos: Pos,
        ed: &mut Square,
        data: &mut CategoryData,
        _chunk: &Chunk,
        e_tick: i32,
    ) -> Result<(), String> {
        if e_tick >= data.spawn_time && !data.do_nothing {
            let floor = env.sq(pos)?.get_floor(env.sm);
            if data.game_obj >= 0
                && (data.game_obj as usize) < self.crack_objs.len()
                && floor.is_some()
            {
                let floor = floor.unwrap_or(0);
                let game_obj = &self.crack_objs[data.game_obj as usize];
                let max_stage = data.max_stage;
                let mut stage = ((e_tick - data.spawn_time) as f32
                    / (game_obj.cycle_time as f32 / (max_stage as f32 + 1.0)))
                    .floor() as i32;
                if stage < data.stage {
                    stage = data.stage;
                }
                if stage >= game_obj.stages {
                    stage = game_obj.stages - 1;
                }
                if stage != data.stage {
                    let cur_id = data.cur_id;
                    let sm = &mut *env.sm;
                    let sq = env
                        .map
                        .get_grid_square_mut(pos.0, pos.1, pos.2)
                        .ok_or_else(|| "java.lang.NullPointerException: no square".to_string())?;
                    let id =
                        game_obj.set_overlay(sm, &mut sq.objects[floor], cur_id, stage, 0, 0.0);
                    if id >= 0 {
                        data.cur_id = id;
                    }
                    data.stage = stage;
                } else if !data.has_grass && stage == game_obj.stages - 1 {
                    data.do_nothing = true;
                }
                if data.has_grass {
                    let grass_obj = self.objs.get_mut(data.game_obj as usize);
                    if let Some(grass_obj) = grass_obj {
                        let disp_season = current_season(&self.base, env, ed.magic_num);
                        update_obj(
                            &self.base,
                            env,
                            ed,
                            data,
                            pos,
                            grass_obj,
                            false,
                            stage,
                            disp_season,
                            false,
                        )?;
                    }
                }
            } else {
                data.do_nothing = true;
            }
        }
        Ok(())
    }

    /// zombie.erosion.categories.StreetCracks.init
    fn init(&mut self, _sm: &mut IsoSpriteManager, _ice_queen: &mut ErosionIceQueen) {
        for i in 0..100 {
            self.spawn_chance[i as usize] = if i >= 40 {
                clerp((i - 40) as f32 / 60.0, 0.0, 60.0) as i32
            } else {
                0
            };
        }
        let sd = &mut self.base.season_disp;
        sd[5].season1 = 5;
        sd[5].season2 = 0;
        sd[5].split = false;
        sd[1].season1 = 1;
        sd[1].season2 = 0;
        sd[1].split = false;
        sd[2].season1 = 2;
        sd[2].season2 = 4;
        sd[2].split = true;
        sd[4].season1 = 4;
        sd[4].season2 = 5;
        sd[4].split = true;
        let seasons = [5, 1, 2, 4];
        for i in 0..=7 {
            let mut crackspr = ErosionObjOverlaySprites::new(3, "StreeCracks");
            let mut grassspr = ErosionObjSprites::new(3, "CrackGrass", false, false, false);
            for stage in 0..=2 {
                for season in 0..=seasons.len() as i32 {
                    let id = season * 24 + stage * 8 + i;
                    if season == 0 {
                        crackspr.set_sprite(stage, &format!("d_streetcracks_1_{id}"), 0);
                    } else {
                        grassspr.set_base(
                            stage,
                            &format!("d_streetcracks_1_{id}"),
                            seasons[(season - 1) as usize],
                        );
                    }
                }
            }
            self.crack_objs
                .push(ErosionObjOverlay::new(crackspr, 60, false));
            self.objs
                .push(ErosionObj::new(grassspr, 60, 0.0, 0.0, false));
        }
    }
}
