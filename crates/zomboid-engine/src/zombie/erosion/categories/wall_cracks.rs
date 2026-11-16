//! zombie.erosion.categories.WallCracks: cracks on the outside of walls.
use super::erosion_category::{set_cat_mod_data, valid_wall, CategoryBase, ErosionCategory};
use crate::zombie::erosion::erosion_data::{Category, CategoryData, Chunk, Square};
use crate::zombie::erosion::erosion_env::{at, Env, Pos};
use crate::zombie::erosion::obj::erosion_obj_overlay::ErosionObjOverlay;
use crate::zombie::erosion::obj::erosion_obj_overlay_sprites::ErosionObjOverlaySprites;
use crate::zombie::erosion::season::erosion_ice_queen::ErosionIceQueen;
use crate::zombie::iso::iso_grid_square::ObjectRef;
use crate::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;

/// zombie.erosion.categories.WallCracks
pub struct WallCracks {
    base: CategoryBase,
    objs: Vec<ErosionObjOverlay>,
    objs_ref: [Vec<i32>; 3],
    bot_ref: [Vec<i32>; 3],
    top_ref: [Vec<i32>; 3],
    spawn_chance: [i32; 100],
}

impl Default for WallCracks {
    fn default() -> Self {
        WallCracks {
            base: CategoryBase::default(),
            objs: Vec::new(),
            objs_ref: Default::default(),
            bot_ref: Default::default(),
            top_ref: Default::default(),
            spawn_chance: [0; 100],
        }
    }
}

/// The wall of the pair `valid_wall` finds without a fence (`fencing` sprites do not get cracks).
fn without_fence(env: &Env, wall: Option<ObjectRef>) -> Result<Option<ObjectRef>, String> {
    if let Some(w) = wall {
        if let Some(name) = env.object_sprite_name((w.x, w.y, w.z), w.index)? {
            if name.starts_with("fencing") {
                return Ok(None);
            }
        }
    }
    Ok(wall)
}

impl ErosionCategory for WallCracks {
    fn base(&self) -> &CategoryBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut CategoryBase {
        &mut self.base
    }

    fn kind(&self) -> Category {
        Category::WallCracks
    }

    /// zombie.erosion.categories.WallCracks.replaceExistingObject: nothing is replaced.
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

    /// zombie.erosion.categories.WallCracks.validateSpawn
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
        let north = valid_wall(env, Some(pos), true, false)?;
        let north = without_fence(env, north)?;
        let west = valid_wall(env, Some(pos), false, false)?;
        let west = without_fence(env, west)?;
        let dir: usize = if north.is_some() && west.is_some() {
            0
        } else if north.is_some() {
            1
        } else if west.is_some() {
            2
        } else {
            return Ok(false);
        };
        let magic_num = ed.magic_num;
        let do_top = e_value < 35 && magic_num > 0.3;
        let di = set_cat_mod_data(&self.base, Category::WallCracks, ed);
        let list = &self.objs_ref[dir];
        let pick = ed.rand(env.rands, x, y, list.len() as i32);
        let game_obj = at(list, pick)?;
        let d = &mut ed.regions[di];
        d.game_obj = game_obj;
        d.alpha = 0.0;
        d.spawn_time = e_value;
        if do_top {
            let top_pos = (pos.0, pos.1, pos.2 + 1);
            if env
                .map
                .get_grid_square(top_pos.0, top_pos.1, top_pos.2)
                .is_some()
            {
                let obj = valid_wall(env, Some(top_pos), dir == 1, false)?;
                if obj.is_some() {
                    let refid = ed.rand(env.rands, x, y, self.bot_ref[dir].len() as i32);
                    let d = &mut ed.regions[di];
                    d.game_obj = at(&self.bot_ref[dir], refid)?;
                    let mut top = CategoryData::alloc(Category::WallCracks, 0, 0);
                    top.game_obj = at(&self.top_ref[dir], refid)?;
                    top.alpha = 0.0;
                    top.spawn_time = d.spawn_time;
                    d.has_top = Some(Box::new(top));
                }
            }
        }
        Ok(true)
    }

    /// zombie.erosion.categories.WallCracks.update: the crack fades in over 100 erosion ticks; the part on the
    /// square above follows through the same method.
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
                let old_alpha = data.alpha;
                let this_alpha = ((e_tick - data.spawn_time) as f32 / 100.0).clamp(0.0, 1.0);
                if this_alpha != old_alpha {
                    let north = valid_wall(env, Some(pos), true, false)?;
                    let west = valid_wall(env, Some(pos), false, false)?;
                    let object = if north.is_some() {
                        north
                    } else if west.is_some() {
                        west
                    } else {
                        None
                    };
                    if let Some(object) = object {
                        let cur_id = data.cur_id;
                        let game_obj = &self.objs[data.game_obj as usize];
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
                            0,
                            0,
                            this_alpha,
                        );
                        if id >= 0 {
                            data.alpha = this_alpha;
                            data.cur_id = id;
                        }
                    } else {
                        data.do_nothing = true;
                    }
                    if data.has_top.is_some() {
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

    /// zombie.erosion.categories.WallCracks.init
    fn init(&mut self, _sm: &mut IsoSpriteManager, _ice_queen: &mut ErosionIceQueen) {
        for i in 0..100 {
            self.spawn_chance[i] = if i <= 50 { 100 } else { 0 };
        }
        let dir = [2usize, 2, 2, 1, 1, 1, 0, 0, 0];
        for (i, d) in dir.iter().enumerate() {
            for row in 0..=7 {
                let id = row * 9 + i as i32;
                let mut crackspr = ErosionObjOverlaySprites::new(1, "WallCracks");
                crackspr.set_sprite(0, &format!("d_wallcracks_1_{id}"), 0);
                self.objs.push(ErosionObjOverlay::new(crackspr, 60, true));
                self.objs_ref[*d].push(self.objs.len() as i32 - 1);
                if row == 0 {
                    self.bot_ref[*d].push(self.objs.len() as i32 - 1);
                } else if row == 1 {
                    self.top_ref[*d].push(self.objs.len() as i32 - 1);
                }
            }
        }
    }
}
