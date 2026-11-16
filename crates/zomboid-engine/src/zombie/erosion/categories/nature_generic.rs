//! zombie.erosion.categories.NatureGeneric: the grass, the generic plants and the ferns the map already has.
use super::erosion_category::{
    clerp, current_season, set_cat_mod_data, update_obj, CategoryBase, ErosionCategory,
};
use crate::zombie::core::properties::iso_property_type as pt;
use crate::zombie::erosion::erosion_data::{Category, CategoryData, Chunk, Square};
use crate::zombie::erosion::erosion_env::{Env, Pos};
use crate::zombie::erosion::obj::erosion_obj::ErosionObj;
use crate::zombie::erosion::obj::erosion_obj_sprites::ErosionObjSprites;
use crate::zombie::erosion::season::erosion_ice_queen::ErosionIceQueen;
use crate::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;

/// zombie.erosion.categories.NatureGeneric
#[derive(Default)]
pub struct NatureGeneric {
    base: CategoryBase,
    objs: Vec<ErosionObj>,
    spawn_chance: Vec<i32>,
}

/// zombie.erosion.categories.NatureGeneric.toInt
fn to_int(ch: char) -> i32 {
    match ch {
        '0'..='9' => ch as i32 - '0' as i32,
        _ => 0,
    }
}

impl NatureGeneric {
    /// zombie.erosion.categories.NatureGeneric.getGroundGrassType: 1, 2 or 3 for the grass floor tiles of the
    /// natural tileset rows 2, 4 and 6 (columns 0 and 5 to 7), else 0.
    fn get_ground_grass_type(env: &Env, pos: Pos) -> Result<i32, String> {
        let sq = env.sq(pos)?;
        let Some(floor) = sq.get_floor(env.sm) else {
            return Ok(0);
        };
        let name = sq.objects[floor]
            .sprite
            .and_then(|s| env.sm.sprite(s).name.as_deref());
        if let Some(name) = name {
            if name.starts_with("blends_natural_01_") {
                let chars: Vec<char> = name.chars().collect();
                let mut id = 0i32;
                for x in 18..chars.len() {
                    id = id.wrapping_add(to_int(chars[x]));
                    if x < chars.len() - 1 {
                        id = id.wrapping_mul(10);
                    }
                }
                let row = id / 8;
                let col = id % 8;
                if row == 2 && (col == 0 || col >= 5) {
                    return Ok(1);
                }
                if row == 4 && (col == 0 || col >= 5) {
                    return Ok(2);
                }
                if row == 6 && (col == 0 || col >= 5) {
                    return Ok(3);
                }
            }
        }
        Ok(0)
    }
}

impl ErosionCategory for NatureGeneric {
    fn base(&self) -> &CategoryBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut CategoryBase {
        &mut self.base
    }

    fn kind(&self) -> Category {
        Category::NatureGeneric
    }

    /// zombie.erosion.categories.NatureGeneric.replaceExistingObject: a grass or generic tile that one of this
    /// category's objects owns (the grass overlays of the map are mapped to the erosion grass first) gives the
    /// square its data; the object is only named, it stays on the square.
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
            let spr0 = env.sq(pos)?.objects[idx].sprite;
            if let Some(mut spr) = spr0 {
                if env.sm.sprite(spr).name.is_some() {
                    let name0 = env.sm.sprite(spr).name.clone().unwrap_or_default();
                    if name0.starts_with("blends_grassoverlays_01") {
                        let tsi = env.sm.sprite(spr).tile_sheet_index;
                        let mapped = if (0..=21).contains(&tsi) {
                            Some(format!("e_newgrass_1_{}", 24 + tsi))
                        } else if (22..=45).contains(&tsi) {
                            Some(format!("e_newgrass_1_{}", 48 + tsi - 22))
                        } else if (48..=69).contains(&tsi) {
                            Some(format!("e_newgrass_1_{}", 72 + tsi - 48))
                        } else {
                            None
                        };
                        if let Some(m) = mapped {
                            if let Some(r) = env.sm.named_map.get(&m) {
                                spr = *r;
                            }
                        }
                    }
                    let spr_name = env.sm.sprite(spr).name.clone().unwrap_or_default();
                    if spr_name.starts_with("e_newgrass_1_") || spr_name.starts_with("d_generic_1_")
                    {
                        let found = self
                            .objs
                            .iter()
                            .position(|o| o.get_entry(&spr_name).is_some());
                        if let Some(found) = found {
                            let cname = env
                                .sm
                                .sprite(spr)
                                .properties
                                .get(env.alias, pt::CUSTOM_NAME)
                                .map(str::to_string);
                            if cname.is_none()
                                || cname
                                    .as_deref()
                                    .is_some_and(|c| !c.contains("Twig") && !c.contains("Branch"))
                            {
                                let entry = self.objs[found].get_entry(&spr_name);
                                let di = set_cat_mod_data(&self.base, Category::NatureGeneric, ed);
                                let d = &mut ed.regions[di];
                                d.game_obj = found as i32;
                                d.stage = entry.map_or(0, |e| e.stage);
                                d.spawn_time = 0;
                                d.disp_season = -1;
                                env.sq_mut(pos)?.objects[idx].name =
                                    Some(self.objs[found].name.clone());
                                d.has_spawned = true;
                                if spr_name.starts_with("e_newgrass_1_") {
                                    d.max_stage = 2;
                                } else {
                                    d.not_grass = true;
                                    d.max_stage = 1;
                                }
                                return Ok(true);
                            }
                        }
                    }
                }
            }
            i -= 1;
        }
        Ok(false)
    }

    /// zombie.erosion.categories.NatureGeneric.validateSpawn: nothing spawns.
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

    /// zombie.erosion.categories.NatureGeneric.update
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
            if data.game_obj >= 0 && (data.game_obj as usize) < self.objs.len() {
                let game_obj = &mut self.objs[data.game_obj as usize];
                let max_stage = data.max_stage;
                let mut stage = ((e_tick - data.spawn_time) as f32
                    / (game_obj.cycle_time as f32 / (max_stage as f32 + 1.0)))
                    .floor() as i32;
                if stage > max_stage {
                    stage = max_stage;
                }
                if stage >= game_obj.stages {
                    stage = game_obj.stages - 1;
                }
                if data.stage == data.max_stage {
                    stage = data.max_stage;
                }
                let mut disp_season = 0;
                if !data.not_grass {
                    disp_season = current_season(&self.base, env, ed.magic_num);
                    let ground_type = Self::get_ground_grass_type(env, pos)?;
                    if ground_type == 2 {
                        disp_season = disp_season.max(3);
                    } else if ground_type == 3 {
                        disp_season = disp_season.max(4);
                    }
                }
                update_obj(
                    &self.base,
                    env,
                    ed,
                    data,
                    pos,
                    game_obj,
                    false,
                    stage,
                    disp_season,
                    false,
                )?;
            } else {
                data.do_nothing = true;
            }
        }
        Ok(())
    }

    /// zombie.erosion.categories.NatureGeneric.init
    fn init(&mut self, sm: &mut IsoSpriteManager, ice_queen: &mut ErosionIceQueen) {
        self.spawn_chance = (0..100)
            .map(|i| clerp(i as f32 / 100.0, 0.0, 99.0) as i32)
            .collect();
        let sd = &mut self.base.season_disp;
        sd[5].season1 = 5;
        sd[5].season2 = 0;
        sd[5].split = false;
        sd[1].season1 = 1;
        sd[1].season2 = 0;
        sd[1].split = false;
        sd[2].season1 = 2;
        sd[2].season2 = 3;
        sd[2].split = true;
        sd[4].season1 = 4;
        sd[4].season2 = 5;
        sd[4].split = true;
        let snames = [1, 2, 3, 4, 5];
        let stages = [2, 1, 0];
        for grassx in 0..=5 {
            let mut obj_spr = ErosionObjSprites::new(3, "Grass", false, false, false);
            for (season, sname) in snames.iter().enumerate() {
                for (stage, st) in stages.iter().enumerate() {
                    let sheetid = season as i32 * 24 + stage as i32 * 8 + grassx;
                    obj_spr.set_base(*st, &format!("e_newgrass_1_{sheetid}"), *sname);
                }
            }
            self.objs
                .push(ErosionObj::new(obj_spr, 60, 0.0, 0.0, false));
        }
        for genericx in 0..=15 {
            let mut obj_spr = ErosionObjSprites::new(2, "Generic", false, false, false);
            for stage in 0..=1 {
                let sheetid = stage * 16 + genericx;
                obj_spr.set_base(stage, &format!("d_generic_1_{sheetid}"), 0);
            }
            self.objs.push(ErosionObj::new(obj_spr, 60, 0.0, 0.0, true));
        }
        for fernx in 0..=7 {
            let mut obj_spr = ErosionObjSprites::new(2, "Fern", true, false, false);
            for stage in 0..=1 {
                let sheetid = 48 + stage * 32 + fernx;
                obj_spr.set_base(stage, &format!("d_generic_1_{sheetid}"), 0);
                ice_queen.add_sprite(
                    sm,
                    &format!("d_generic_1_{sheetid}"),
                    &format!("d_generic_1_{}", sheetid + 16),
                );
            }
            self.objs.push(ErosionObj::new(obj_spr, 60, 0.0, 0.0, true));
        }
    }
}
