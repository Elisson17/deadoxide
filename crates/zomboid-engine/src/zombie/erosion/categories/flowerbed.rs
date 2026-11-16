//! zombie.erosion.categories.Flowerbed: the flower beds of the map, which change with the season.
use super::erosion_category::{
    clear_cat_mod_data, set_cat_mod_data, update_obj, CategoryBase, ErosionCategory,
};
use crate::java;
use crate::zombie::erosion::erosion_data::{Category, CategoryData, Chunk, Square};
use crate::zombie::erosion::erosion_env::{Env, Pos};
use crate::zombie::erosion::obj::erosion_obj::ErosionObj;
use crate::zombie::erosion::obj::erosion_obj_sprites::ErosionObjSprites;
use crate::zombie::erosion::season::erosion_ice_queen::ErosionIceQueen;
use crate::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;

const TILE_ID: [i32; 12] = [16, 17, 18, 19, 20, 21, 22, 23, 28, 29, 30, 31];

/// zombie.erosion.categories.Flowerbed
#[derive(Default)]
pub struct Flowerbed {
    base: CategoryBase,
    objs: Vec<ErosionObj>,
}

fn parse(digits: &str) -> Result<i32, String> {
    java::parse_int(digits)
        .ok_or_else(|| format!("java.lang.NumberFormatException: For input string: \"{digits}\""))
}

impl ErosionCategory for Flowerbed {
    fn base(&self) -> &CategoryBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut CategoryBase {
        &mut self.base
    }

    fn kind(&self) -> Category {
        Category::Flowerbed
    }

    /// zombie.erosion.categories.Flowerbed.replaceExistingObject: a flower bed tile of the map gets this
    /// category's data and its object's name.
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
        while i >= 0 {
            let idx = i as usize;
            if let Some(name) = env.object_sprite_name(pos, idx)? {
                if name.starts_with("f_flowerbed_1") {
                    let mut id = parse(&name.replace("f_flowerbed_1_", ""))?;
                    if id <= 23 {
                        if id >= 12 {
                            id -= 12;
                        }
                        let di = set_cat_mod_data(&self.base, Category::Flowerbed, ed);
                        let d = &mut ed.regions[di];
                        d.has_spawned = true;
                        d.game_obj = id;
                        d.disp_season = -1;
                        let obj_name = self
                            .objs
                            .get(id as usize)
                            .ok_or_else(|| format!("java.lang.IndexOutOfBoundsException: Index {id}"))?
                            .name
                            .clone();
                        env.sq_mut(pos)?.objects[idx].name = Some(obj_name);
                        return Ok(true);
                    }
                }
                if name.starts_with("vegetation_ornamental_01") {
                    let id = parse(&name.replace("vegetation_ornamental_01_", ""))?;
                    for (j, tile) in TILE_ID.iter().enumerate() {
                        if *tile == id {
                            let di = set_cat_mod_data(&self.base, Category::Flowerbed, ed);
                            let d = &mut ed.regions[di];
                            d.has_spawned = true;
                            d.game_obj = j as i32;
                            d.disp_season = -1;
                            let obj_name = self.objs[j].name.clone();
                            env.sq_mut(pos)?.objects[idx].name = Some(obj_name);
                            return Ok(true);
                        }
                    }
                }
            }
            i -= 1;
        }
        Ok(false)
    }

    /// zombie.erosion.categories.Flowerbed.validateSpawn: nothing spawns.
    fn validate_spawn(
        &mut self,
        _env: &mut Env,
        _pos: Pos,
        _ed: &mut Square,
        _chunk: &Chunk,
        _is_exterior: bool,
        _has_wall: bool,
        _is_respawn: bool,
    ) -> Result<bool, String> {
        Ok(false)
    }

    /// zombie.erosion.categories.Flowerbed.update: the bed shows the season; in winter (5) it turns into the plain
    /// ground tile `blends_natural_01_64` and loses its data.
    fn update(
        &mut self,
        env: &mut Env,
        pos: Pos,
        ed: &mut Square,
        data: &mut CategoryData,
        _chunk: &Chunk,
        _e_tick: i32,
    ) -> Result<(), String> {
        if !data.do_nothing {
            if data.game_obj >= 0 && (data.game_obj as usize) < self.objs.len() {
                let game_obj = &mut self.objs[data.game_obj as usize];
                let disp_season = env.season.get_season();
                if disp_season == 5 {
                    if let Some(index) = game_obj.get_object(env.sq(pos)?) {
                        let sprite = env.sm.get_sprite("blends_natural_01_64");
                        let obj = &mut env.sq_mut(pos)?.objects[index];
                        obj.sprite = Some(sprite);
                        obj.name = None;
                    }
                    clear_cat_mod_data(&self.base, ed);
                } else {
                    update_obj(
                        &self.base, env, ed, data, pos, game_obj, false, 0, disp_season, false,
                    )?;
                }
            } else {
                clear_cat_mod_data(&self.base, ed);
            }
        }
        Ok(())
    }

    /// zombie.erosion.categories.Flowerbed.init
    fn init(&mut self, _sm: &mut IsoSpriteManager, _ice_queen: &mut ErosionIceQueen) {
        for tile in TILE_ID {
            let mut obj_spr = ErosionObjSprites::new(1, "Flowerbed", false, false, false);
            obj_spr.set_base(0, &format!("vegetation_ornamental_01_{tile}"), 1);
            obj_spr.set_base(0, &format!("vegetation_ornamental_01_{tile}"), 2);
            obj_spr.set_base(0, &format!("vegetation_ornamental_01_{}", tile + 16), 4);
            self.objs.push(ErosionObj::new(obj_spr, 30, 0.0, 0.0, false));
        }
    }
}
