//! zombie.erosion.categories.NatureBush: the bushes of the natural tiles.
use super::erosion_category::{
    current_bloom, current_season, set_cat_mod_data, update_obj, CategoryBase, ErosionCategory,
};
use crate::java;
use crate::zombie::erosion::erosion_data::{Category, CategoryData, Chunk, Square};
use crate::zombie::erosion::erosion_env::{at, Env, Pos};
use crate::zombie::erosion::obj::erosion_obj::ErosionObj;
use crate::zombie::erosion::obj::erosion_obj_sprites::ErosionObjSprites;
use crate::zombie::erosion::season::erosion_ice_queen::ErosionIceQueen;
use crate::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;

const SOIL_REF: [&[i32]; 12] = [
    &[11, 11, 12, 13],
    &[5, 5, 7, 8, 11, 11, 12, 13, 11, 11, 12, 13],
    &[5, 5, 7, 8, 5, 5, 7, 8, 11, 11, 12, 13],
    &[1, 1, 4, 5],
    &[5, 5, 7, 8, 1, 1, 4, 5, 1, 1, 4, 5],
    &[5, 5, 7, 8, 5, 5, 7, 8, 1, 1, 4, 5],
    &[9, 10, 14, 15],
    &[5, 5, 7, 8, 9, 10, 14, 15, 9, 10, 14, 15],
    &[5, 5, 7, 8, 5, 5, 7, 8, 9, 10, 14, 15],
    &[2, 3, 16, 16],
    &[5, 5, 7, 8, 2, 3, 16, 16, 2, 3, 16, 16],
    &[5, 5, 7, 8, 5, 5, 7, 8, 2, 3, 16, 16],
];

/// zombie.erosion.categories.NatureBush.BushInit
struct BushInit {
    name: &'static str,
    bloomstart: f32,
    bloomend: f32,
    has_flower: bool,
}

const BUSH: [BushInit; 16] = [
    BushInit {
        name: "Spicebush",
        bloomstart: 0.05,
        bloomend: 0.35,
        has_flower: false,
    },
    BushInit {
        name: "Ninebark",
        bloomstart: 0.65,
        bloomend: 0.75,
        has_flower: true,
    },
    BushInit {
        name: "Ninebark",
        bloomstart: 0.65,
        bloomend: 0.75,
        has_flower: true,
    },
    BushInit {
        name: "Blueberry",
        bloomstart: 0.4,
        bloomend: 0.5,
        has_flower: true,
    },
    BushInit {
        name: "Blackberry",
        bloomstart: 0.4,
        bloomend: 0.5,
        has_flower: true,
    },
    BushInit {
        name: "Piedmont azalea",
        bloomstart: 0.0,
        bloomend: 0.15,
        has_flower: true,
    },
    BushInit {
        name: "Piedmont azalea",
        bloomstart: 0.0,
        bloomend: 0.15,
        has_flower: true,
    },
    BushInit {
        name: "Arrowwood viburnum",
        bloomstart: 0.3,
        bloomend: 0.8,
        has_flower: true,
    },
    BushInit {
        name: "Red chokeberry",
        bloomstart: 0.9,
        bloomend: 1.0,
        has_flower: true,
    },
    BushInit {
        name: "Red chokeberry",
        bloomstart: 0.9,
        bloomend: 1.0,
        has_flower: true,
    },
    BushInit {
        name: "Beautyberry",
        bloomstart: 0.7,
        bloomend: 0.85,
        has_flower: true,
    },
    BushInit {
        name: "New jersey tea",
        bloomstart: 0.4,
        bloomend: 0.8,
        has_flower: true,
    },
    BushInit {
        name: "New jersey tea",
        bloomstart: 0.4,
        bloomend: 0.8,
        has_flower: true,
    },
    BushInit {
        name: "Wild hydrangea",
        bloomstart: 0.2,
        bloomend: 0.35,
        has_flower: true,
    },
    BushInit {
        name: "Wild hydrangea",
        bloomstart: 0.2,
        bloomend: 0.35,
        has_flower: true,
    },
    BushInit {
        name: "Shrubby St. John's wort",
        bloomstart: 0.35,
        bloomend: 0.75,
        has_flower: true,
    },
];

/// zombie.erosion.categories.NatureBush
pub struct NatureBush {
    base: CategoryBase,
    objs: Vec<ErosionObj>,
    spawn_chance: [i32; 100],
}

impl Default for NatureBush {
    fn default() -> Self {
        NatureBush {
            base: CategoryBase::default(),
            objs: Vec::new(),
            spawn_chance: [0; 100],
        }
    }
}

impl ErosionCategory for NatureBush {
    fn base(&self) -> &CategoryBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut CategoryBase {
        &mut self.base
    }

    fn kind(&self) -> Category {
        Category::NatureBush
    }

    /// zombie.erosion.categories.NatureBush.replaceExistingObject: the foliage and bush tiles of the map become bushes
    /// of this category and are taken off the square.
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
        let mut replaced = false;
        let (x, y) = (pos.0, pos.1);
        let mut i = objs_size - 1;
        while i >= 1 {
            let idx = i as usize;
            if let Some(name) = env.object_sprite_name(pos, idx)? {
                if name.starts_with("vegetation_foliage") {
                    let mut soil = ed.soil;
                    if soil < 0 || soil >= SOIL_REF.len() as i32 {
                        soil = ed.rand(env.rands, x, y, SOIL_REF.len() as i32);
                    }
                    let soil_ref = SOIL_REF[soil as usize];
                    let e_value = ed.noise_main_int;
                    let di = set_cat_mod_data(&self.base, Category::NatureBush, ed);
                    let game_obj =
                        at(soil_ref, ed.rand(env.rands, x, y, soil_ref.len() as i32))? - 1;
                    let d = &mut ed.regions[di];
                    d.game_obj = game_obj;
                    d.max_stage = (e_value as f32 / 60.0).floor() as i32;
                    d.stage = d.max_stage;
                    d.spawn_time = 0;
                    env.map.remove_tile_object(pos, idx, env.sm, env.alias);
                    replaced = true;
                }
                if name.starts_with("f_bushes_1_") {
                    Self::replace_bush(&self.base, env, pos, idx, ed, &name)?;
                    replaced = true;
                }
            }
            i -= 1;
        }
        Ok(replaced)
    }

    /// zombie.erosion.categories.NatureBush.validateSpawn
    fn validate_spawn(
        &mut self,
        env: &mut Env,
        pos: Pos,
        ed: &mut Square,
        _chunk: &Chunk,
        _is_exterior: bool,
        has_wall: bool,
        _is_respawn: bool,
    ) -> Result<bool, String> {
        if env.sq(pos)?.objects.len() as i32 > if has_wall { 2 } else { 1 } {
            return Ok(false);
        }
        if ed.soil >= 0 && (ed.soil as usize) < SOIL_REF.len() {
            let soil_ref = SOIL_REF[ed.soil as usize];
            let e_value = ed.noise_main_int;
            let (x, y) = (pos.0, pos.1);
            let rand_value = ed.rand(env.rands, x, y, 101);
            if rand_value < at(&self.spawn_chance, e_value)? {
                let di = set_cat_mod_data(&self.base, Category::NatureBush, ed);
                let game_obj = at(soil_ref, ed.rand(env.rands, x, y, soil_ref.len() as i32))? - 1;
                let d = &mut ed.regions[di];
                d.game_obj = game_obj;
                d.max_stage = (e_value as f32 / 60.0).floor() as i32;
                d.stage = 0;
                d.spawn_time = 100 - e_value;
                return Ok(true);
            }
            Ok(false)
        } else {
            Ok(false)
        }
    }

    /// zombie.erosion.categories.NatureBush.update
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
                if stage < data.stage {
                    stage = data.stage;
                }
                if stage > max_stage {
                    stage = max_stage;
                }
                let disp_season = current_season(&self.base, env, ed.magic_num);
                let bloom = current_bloom(env, ed.magic_num, game_obj);
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
                    bloom,
                )?;
            } else {
                data.do_nothing = true;
            }
        }
        Ok(())
    }

    /// zombie.erosion.categories.NatureBush.init
    fn init(&mut self, sm: &mut IsoSpriteManager, ice_queen: &mut ErosionIceQueen) {
        use super::erosion_category::clerp;
        for i in 0..100 {
            if (45..60).contains(&i) {
                self.spawn_chance[i as usize] = clerp((i - 45) as f32 / 15.0, 0.0, 20.0) as i32;
            }
            if (60..90).contains(&i) {
                self.spawn_chance[i as usize] = clerp((i - 60) as f32 / 30.0, 20.0, 0.0) as i32;
            }
        }
        let sd = &mut self.base.season_disp;
        sd[5].season1 = 0;
        sd[5].season2 = 0;
        sd[5].split = false;
        sd[1].season1 = 1;
        sd[1].season2 = 0;
        sd[1].split = false;
        sd[2].season1 = 2;
        sd[2].season2 = 2;
        sd[2].split = true;
        sd[4].season1 = 4;
        sd[4].season2 = 0;
        sd[4].split = true;
        for id in 1..=BUSH.len() as i32 {
            let i = id - 1;
            let trunk = i - (i as f32 / 8.0).floor() as i32 * 8;
            let b = &BUSH[i as usize];
            let mut obj_spr = ErosionObjSprites::new(2, b.name, true, b.has_flower, true);
            let base_id = trunk;
            let snow_id = base_id + 16;
            let spring_id = snow_id + 16;
            let autumn_id = spring_id + 16;
            let summer_id = 64 + i;
            let bloom_id = summer_id + 16;
            obj_spr.set_base(0, &format!("f_bushes_1_{base_id}"), 0);
            obj_spr.set_base(1, &format!("f_bushes_1_{}", base_id + 8), 0);
            ice_queen.add_sprite(
                sm,
                &format!("f_bushes_1_{base_id}"),
                &format!("f_bushes_1_{snow_id}"),
            );
            ice_queen.add_sprite(
                sm,
                &format!("f_bushes_1_{}", base_id + 8),
                &format!("f_bushes_1_{}", snow_id + 8),
            );
            obj_spr.set_child_sprite(0, &format!("f_bushes_1_{spring_id}"), 1);
            obj_spr.set_child_sprite(1, &format!("f_bushes_1_{}", spring_id + 8), 1);
            obj_spr.set_child_sprite(0, &format!("f_bushes_1_{autumn_id}"), 4);
            obj_spr.set_child_sprite(1, &format!("f_bushes_1_{}", autumn_id + 8), 4);
            obj_spr.set_child_sprite(0, &format!("f_bushes_1_{summer_id}"), 2);
            obj_spr.set_child_sprite(1, &format!("f_bushes_1_{}", summer_id + 32), 2);
            if b.has_flower {
                obj_spr.set_flower(0, &format!("f_bushes_1_{bloom_id}"));
                obj_spr.set_flower(1, &format!("f_bushes_1_{}", bloom_id + 32));
            }
            let bloomstart = if b.has_flower { b.bloomstart } else { 0.0 };
            let bloomend = if b.has_flower { b.bloomend } else { 0.0 };
            self.objs
                .push(ErosionObj::new(obj_spr, 60, bloomstart, bloomend, true));
        }
    }
}

impl NatureBush {
    /// The `f_bushes_1_` branch of replaceExistingObject: the bush's id gives the object (`id % 16`).
    fn replace_bush(
        base: &CategoryBase,
        env: &mut Env,
        pos: Pos,
        idx: usize,
        ed: &mut Square,
        name: &str,
    ) -> Result<(), String> {
        let digits = name.replace("f_bushes_1_", "");
        let id = java::parse_int(&digits).ok_or_else(|| {
            format!("java.lang.NumberFormatException: For input string: \"{digits}\"")
        })?;
        let di = set_cat_mod_data(base, Category::NatureBush, ed);
        let d = &mut ed.regions[di];
        d.game_obj = id % 16;
        d.max_stage = 1;
        d.stage = d.max_stage;
        d.spawn_time = 0;
        env.map.remove_tile_object(pos, idx, env.sm, env.alias);
        Ok(())
    }
}
