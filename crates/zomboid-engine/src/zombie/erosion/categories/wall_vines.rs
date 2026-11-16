//! zombie.erosion.categories.WallVines: vines that grow up the outside of walls.
use std::collections::HashMap;

use super::erosion_category::{set_cat_mod_data, valid_wall, CategoryBase, ErosionCategory};
use crate::zombie::erosion::erosion_data::{Category, CategoryData, Chunk, Square};
use crate::zombie::erosion::erosion_env::{at, Env, Pos};
use crate::zombie::erosion::obj::erosion_obj_overlay::ErosionObjOverlay;
use crate::zombie::erosion::obj::erosion_obj_overlay_sprites::ErosionObjOverlaySprites;
use crate::zombie::erosion::season::erosion_ice_queen::ErosionIceQueen;
use crate::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;

/// zombie.erosion.categories.WallVines
pub struct WallVines {
    base: CategoryBase,
    objs: Vec<ErosionObjOverlay>,
    objs_ref: [[i32; 2]; 3],
    sprite_to_obj: HashMap<String, i32>,
    sprite_to_stage: HashMap<String, i32>,
    spawn_chance: [i32; 100],
}

impl Default for WallVines {
    fn default() -> Self {
        WallVines {
            base: CategoryBase::default(),
            objs: Vec::new(),
            objs_ref: [[0; 2]; 3],
            sprite_to_obj: HashMap::new(),
            sprite_to_stage: HashMap::new(),
            spawn_chance: [0; 100],
        }
    }
}

impl ErosionCategory for WallVines {
    fn base(&self) -> &CategoryBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut CategoryBase {
        &mut self.base
    }

    fn kind(&self) -> Category {
        Category::WallVines
    }

    /// zombie.erosion.categories.WallVines.replaceExistingObject: a vine sprite attached to an object of the map
    /// becomes this category's data and is taken off the object (the original removes the entry at the index twice:
    /// the vine and the sprite that moved into its place).
    fn replace_existing_object(
        &mut self,
        env: &mut Env,
        pos: Pos,
        ed: &mut Square,
        _chunk: &Chunk,
        _is_exterior: bool,
        _has_wall: bool,
    ) -> Result<bool, String> {
        let objs_size = env.sq(pos)?.objects.len() as i32;
        let mut i = objs_size - 1;
        while i >= 1 {
            let idx = i as usize;
            let attached: Vec<_> = env.sq(pos)?.objects[idx]
                .attached_anim_sprite
                .clone()
                .unwrap_or_default();
            for (j, spr) in attached.iter().enumerate() {
                let Some(name) = env.sm.sprite(*spr).name.clone() else {
                    continue;
                };
                if name.starts_with("f_wallvines_1_") && self.sprite_to_obj.contains_key(&name) {
                    let di = set_cat_mod_data(&self.base, Category::WallVines, ed);
                    let d = &mut ed.regions[di];
                    d.game_obj = self.sprite_to_obj[&name];
                    d.stage = self.sprite_to_stage[&name];
                    d.max_stage = 2;
                    d.spawn_time = 0;
                    if let Some(list) = env.sq_mut(pos)?.objects[idx].attached_anim_sprite.as_mut()
                    {
                        list.remove(j);
                        if j < list.len() {
                            list.remove(j);
                        }
                    }
                    return Ok(true);
                }
            }
            i -= 1;
        }
        Ok(false)
    }

    /// zombie.erosion.categories.WallVines.validateSpawn
    fn validate_spawn(
        &mut self,
        env: &mut Env,
        pos: Pos,
        ed: &mut Square,
        _chunk: &Chunk,
        is_exterior: bool,
        _has_wall: bool,
        _is_respawn: bool,
    ) -> Result<bool, String> {
        if !is_exterior {
            return Ok(false);
        }
        let e_value = ed.noise_main_int;
        let spawn_chance = at(&self.spawn_chance, e_value)?;
        if spawn_chance == 0 {
            return Ok(false);
        }
        let (x, y) = (pos.0, pos.1);
        if ed.rand(env.rands, x, y, 101) >= spawn_chance {
            return Ok(false);
        }
        let north = valid_wall(env, Some(pos), true, true)?;
        let west = valid_wall(env, Some(pos), false, true)?;
        let dir: usize = if north.is_some() && west.is_some() {
            0
        } else if north.is_some() {
            1
        } else if west.is_some() {
            2
        } else {
            return Ok(false);
        };
        let di = set_cat_mod_data(&self.base, Category::WallVines, ed);
        let refs = self.objs_ref[dir];
        let pick = ed.rand(env.rands, x, y, refs.len() as i32);
        let game_obj = at(&refs, pick)?;
        let d = &mut ed.regions[di];
        d.game_obj = game_obj;
        d.max_stage = if e_value > 65 {
            3
        } else if e_value > 60 {
            2
        } else if e_value > 55 {
            1
        } else {
            0
        };
        d.stage = 0;
        d.spawn_time = 100 - e_value;
        if d.max_stage == 3 {
            let top_pos = (pos.0, pos.1, pos.2 + 1);
            if env
                .map
                .get_grid_square(top_pos.0, top_pos.1, top_pos.2)
                .is_some()
            {
                let obj = valid_wall(env, Some(top_pos), dir == 1, true)?;
                let game_obj_overlay = self.objs.get(d.game_obj as usize);
                match (obj, game_obj_overlay) {
                    (Some(_), Some(g)) => {
                        let mut top = CategoryData::alloc(Category::WallVines, 0, 0);
                        top.game_obj = at(&refs, pick)?;
                        top.max_stage = if e_value > 75 {
                            2
                        } else if e_value > 70 {
                            1
                        } else {
                            0
                        };
                        top.stage = 0;
                        top.spawn_time = d.spawn_time
                            + (g.cycle_time as f32 / (d.max_stage as f32 + 1.0) * 4.0) as i32;
                        d.has_top = Some(Box::new(top));
                    }
                    _ => d.max_stage = 2,
                }
            } else {
                d.max_stage = 2;
            }
        }
        Ok(true)
    }

    /// zombie.erosion.categories.WallVines.update
    // `ed` and `chunk` only go down the recursion into the top square's data, as in the original
    #[allow(clippy::only_used_in_recursion)]
    fn update(
        &mut self,
        env: &mut Env,
        pos: Pos,
        ed: &mut Square,
        data: &mut CategoryData,
        chunk: &Chunk,
        e_tick: i32,
    ) -> Result<(), String> {
        if e_tick >= data.spawn_time && !data.do_nothing {
            if data.game_obj >= 0 && (data.game_obj as usize) < self.objs.len() {
                let game_obj = &self.objs[data.game_obj as usize];
                let max_stage = data.max_stage;
                let mut stage = ((e_tick - data.spawn_time) as f32
                    / (game_obj.cycle_time as f32 / (max_stage as f32 + 1.0)))
                    .floor() as i32;
                if stage < data.stage {
                    stage = data.stage;
                }
                if stage > max_stage {
                    stage = max_stage;
                }
                if stage > game_obj.stages {
                    stage = game_obj.stages;
                }
                if stage == 3 && data.has_top.as_ref().is_some_and(|t| t.spawn_time > e_tick) {
                    stage = 2;
                }
                let disp_season = env.season.get_season();
                if stage != data.stage || data.disp_season != disp_season {
                    let north = valid_wall(env, Some(pos), true, true)?;
                    let west = valid_wall(env, Some(pos), false, true)?;
                    let object = if north.is_some() {
                        north
                    } else if west.is_some() {
                        west
                    } else {
                        None
                    };
                    data.disp_season = disp_season;
                    if let Some(object) = object {
                        let cur_id = data.cur_id;
                        let sm = &mut *env.sm;
                        let sq = env
                            .map
                            .get_grid_square_mut(object.x, object.y, object.z)
                            .ok_or_else(|| {
                                "java.lang.NullPointerException: no square".to_string()
                            })?;
                        let id = game_obj.set_overlay(
                            sm,
                            &mut sq.objects[object.index],
                            cur_id,
                            stage,
                            disp_season,
                            0.0,
                        );
                        if id >= 0 {
                            data.cur_id = id;
                        }
                    } else {
                        data.do_nothing = true;
                    }
                    if stage == 3 && data.has_top.is_some() {
                        let top_pos = (pos.0, pos.1, pos.2 + 1);
                        if env
                            .map
                            .get_grid_square(top_pos.0, top_pos.1, top_pos.2)
                            .is_some()
                        {
                            if let Some(top) = data.has_top.as_deref_mut() {
                                self.update(env, top_pos, ed, top, chunk, e_tick)?;
                            }
                        }
                    }
                }
            } else {
                data.do_nothing = true;
            }
        }
        Ok(())
    }

    /// zombie.erosion.categories.WallVines.init
    fn init(&mut self, _sm: &mut IsoSpriteManager, _ice_queen: &mut ErosionIceQueen) {
        for i in 0..100 {
            self.spawn_chance[i] = if i >= 50 { 100 } else { 0 };
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
        let seasons = [5, 2, 4, 1];
        let dir = [2usize, 2, 1, 1, 0, 0];
        let mut count = [0usize; 3];
        for (i, d) in dir.iter().enumerate() {
            let mut vinespr = ErosionObjOverlaySprites::new(4, "WallVines");
            for stage in 0..=3 {
                for season in 0..=2 {
                    let id = season * 24 + stage * 6 + i as i32;
                    vinespr.set_sprite(
                        stage,
                        &format!("f_wallvines_1_{id}"),
                        seasons[season as usize],
                    );
                    if season == 2 {
                        vinespr.set_sprite(
                            stage,
                            &format!("f_wallvines_1_{id}"),
                            seasons[season as usize + 1],
                        );
                    }
                    self.sprite_to_obj
                        .insert(format!("f_wallvines_1_{id}"), self.objs.len() as i32);
                    self.sprite_to_stage
                        .insert(format!("f_wallvines_1_{id}"), stage);
                }
            }
            self.objs.push(ErosionObjOverlay::new(vinespr, 60, false));
            self.objs_ref[*d][count[*d]] = self.objs.len() as i32 - 1;
            count[*d] += 1;
        }
    }
}
