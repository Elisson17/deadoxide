//! zombie.erosion.categories.NaturePlants: the wild flowers and ground plants.
use super::erosion_category::{
    clear_cat_mod_data, clerp, current_bloom, current_season, set_cat_mod_data, update_obj,
    CategoryBase, ErosionCategory,
};
use crate::java;
use crate::zombie::erosion::erosion_data::{Category, CategoryData, Chunk, Square};
use crate::zombie::erosion::erosion_env::{at, Env, Pos};
use crate::zombie::erosion::obj::erosion_obj::ErosionObj;
use crate::zombie::erosion::obj::erosion_obj_sprites::ErosionObjSprites;
use crate::zombie::erosion::season::erosion_ice_queen::ErosionIceQueen;
use crate::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;

const SOIL_REF: [&[i32]; 12] = [
    &[17, 17, 17, 17, 17, 17, 17, 17, 17, 1, 2, 8, 8],
    &[11, 12, 1, 2, 8, 1, 2, 8, 1, 2, 8, 1, 2, 8, 1, 2, 8],
    &[11, 12, 11, 12, 11, 12, 11, 12, 15, 16, 18, 19],
    &[22, 22, 22, 22, 22, 22, 22, 22, 22, 3, 4, 14],
    &[15, 16, 3, 4, 14, 3, 4, 14, 3, 4, 14, 3, 4, 14],
    &[11, 12, 15, 16, 15, 16, 15, 16, 15, 16, 21],
    &[13, 13, 13, 13, 13, 13, 13, 13, 13, 5, 6, 24],
    &[18, 19, 5, 6, 24, 5, 6, 24, 5, 6, 24, 5, 6, 24],
    &[18, 19, 18, 19, 18, 19, 18, 19, 20, 21],
    &[7, 7, 7, 7, 7, 7, 7, 7, 7, 9, 10, 23],
    &[19, 20, 9, 10, 23, 9, 10, 23, 9, 10, 23, 9, 10, 23],
    &[15, 16, 18, 19, 20, 19, 20, 19, 20],
];

/// zombie.erosion.categories.NaturePlants.PlantInit
struct PlantInit {
    name: &'static str,
    has_flower: bool,
    bloomstart: f32,
    bloomend: f32,
}

const fn plant(name: &'static str, has_flower: bool, bloomstart: f32, bloomend: f32) -> PlantInit {
    PlantInit {
        name,
        has_flower,
        bloomstart,
        bloomend,
    }
}

const PLANTS: [PlantInit; 24] = [
    plant("Butterfly Weed", true, 0.05, 0.25),
    plant("Butterfly Weed", true, 0.05, 0.25),
    plant("Swamp Sunflower", true, 0.2, 0.45),
    plant("Swamp Sunflower", true, 0.2, 0.45),
    plant("Purple Coneflower", true, 0.1, 0.35),
    plant("Purple Coneflower", true, 0.1, 0.35),
    plant("Joe-Pye Weed", true, 0.8, 1.0),
    plant("Blazing Star", true, 0.25, 0.65),
    plant("Wild Bergamot", true, 0.45, 0.6),
    plant("Wild Bergamot", true, 0.45, 0.6),
    plant("White Beard-tongue", true, 0.2, 0.65),
    plant("White Beard-tongue", true, 0.2, 0.65),
    plant("Ironweed", true, 0.75, 0.85),
    plant("White Baneberry", true, 0.4, 0.8),
    plant("Wild Columbine", true, 0.85, 1.0),
    plant("Wild Columbine", true, 0.85, 1.0),
    plant("Jack-in-the-pulpit", false, 0.0, 0.0),
    plant("Wild Ginger", true, 0.1, 0.9),
    plant("Wild Ginger", true, 0.1, 0.9),
    plant("Wild Geranium", true, 0.65, 0.9),
    plant("Alumroot", true, 0.35, 0.75),
    plant("Wild Blue Phlox", true, 0.15, 0.55),
    plant("Polemonium Reptans", true, 0.4, 0.6),
    plant("Foamflower", true, 0.45, 1.0),
];

/// zombie.erosion.categories.NaturePlants
pub struct NaturePlants {
    base: CategoryBase,
    spawn_chance: [i32; 100],
    objs: Vec<ErosionObj>,
}

impl Default for NaturePlants {
    fn default() -> Self {
        NaturePlants {
            base: CategoryBase::default(),
            spawn_chance: [0; 100],
            objs: Vec::new(),
        }
    }
}

impl NaturePlants {
    /// The objects of the square below `i` whose sprite is a ground cover tile, taken off the square (the
    /// `while (--i > 0)` loops of replaceExistingObject).
    fn remove_ground_cover_below(env: &mut Env, pos: Pos, mut i: i32) -> Result<(), String> {
        loop {
            i -= 1;
            if i <= 0 {
                return Ok(());
            }
            if let Some(name) = env.object_sprite_name(pos, i as usize)? {
                if name.starts_with("vegetation_groundcover_01_") {
                    env.map
                        .remove_tile_object_erosion_no_recalc(pos, i as usize, env.sm);
                }
            }
        }
    }
}

impl ErosionCategory for NaturePlants {
    fn base(&self) -> &CategoryBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut CategoryBase {
        &mut self.base
    }

    fn kind(&self) -> Category {
        Category::NaturePlants
    }

    /// zombie.erosion.categories.NaturePlants.replaceExistingObject: the `d_plants_1_` tiles and the ground cover
    /// tiles of the map become plants of this category and are taken off the square without a recalculation.
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
        let (x, y) = (pos.0, pos.1);
        let mut i = objs_size - 1;
        while i >= 1 {
            let idx = i as usize;
            if let Some(name) = env.object_sprite_name(pos, idx)? {
                if name.starts_with("d_plants_1_") {
                    let digits = name.replace("d_plants_1_", "");
                    let id = java::parse_int(&digits).ok_or_else(|| {
                        format!("java.lang.NumberFormatException: For input string: \"{digits}\"")
                    })?;
                    let di = set_cat_mod_data(&self.base, Category::NaturePlants, ed);
                    let d = &mut ed.regions[di];
                    d.game_obj = if id < 32 {
                        id % 8
                    } else if id < 48 {
                        id % 8 + 8
                    } else {
                        id % 8 + 16
                    };
                    d.stage = 0;
                    d.spawn_time = 0;
                    env.map
                        .remove_tile_object_erosion_no_recalc(pos, idx, env.sm);
                    return Ok(true);
                }
                let first = name == "vegetation_groundcover_01_16"
                    || name == "vegetation_groundcover_01_17";
                let second = (18..=23).any(|n| name == format!("vegetation_groundcover_01_{n}"));
                if first || second {
                    let di = set_cat_mod_data(&self.base, Category::NaturePlants, ed);
                    let game_obj = if first {
                        21
                    } else {
                        ed.rand(env.rands, x, y, PLANTS.len() as i32)
                    };
                    let d = &mut ed.regions[di];
                    d.game_obj = game_obj;
                    d.stage = 0;
                    d.spawn_time = 0;
                    env.map
                        .remove_tile_object_erosion_no_recalc(pos, idx, env.sm);
                    Self::remove_ground_cover_below(env, pos, i)?;
                    return Ok(true);
                }
            }
            i -= 1;
        }
        Ok(false)
    }

    /// zombie.erosion.categories.NaturePlants.validateSpawn
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
            if ed.rand(env.rands, x, y, 101) < at(&self.spawn_chance, e_value)? {
                let di = set_cat_mod_data(&self.base, Category::NaturePlants, ed);
                let game_obj = at(soil_ref, ed.rand(env.rands, x, y, soil_ref.len() as i32))? - 1;
                let d = &mut ed.regions[di];
                d.game_obj = game_obj;
                d.stage = 0;
                d.spawn_time = 100 - e_value;
                return Ok(true);
            }
            Ok(false)
        } else {
            Ok(false)
        }
    }

    /// zombie.erosion.categories.NaturePlants.update
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
                let disp_season = current_season(&self.base, env, ed.magic_num);
                let bloom = current_bloom(env, ed.magic_num, game_obj);
                update_obj(
                    &self.base, env, ed, data, pos, game_obj, false, 0, disp_season, bloom,
                )?;
            } else {
                clear_cat_mod_data(&self.base, ed);
            }
        }
        Ok(())
    }

    /// zombie.erosion.categories.NaturePlants.init
    fn init(&mut self, _sm: &mut IsoSpriteManager, _ice_queen: &mut ErosionIceQueen) {
        for i in 0..100 {
            if (20..50).contains(&i) {
                self.spawn_chance[i as usize] = clerp((i - 20) as f32 / 30.0, 0.0, 8.0) as i32;
            } else if (50..80).contains(&i) {
                self.spawn_chance[i as usize] = clerp((i - 50) as f32 / 30.0, 8.0, 0.0) as i32;
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
        let spring_spr: Vec<String> = (0..=7).map(|i| format!("d_plants_1_{i}")).collect();
        let autumn_spr: Vec<String> = (8..=15).map(|i| format!("d_plants_1_{i}")).collect();
        let mut offset = 16;
        for (ix, plant) in PLANTS.iter().enumerate() {
            let ix = ix as i32;
            if ix >= 8 {
                offset = 24;
            }
            if ix >= 16 {
                offset = 32;
            }
            let mut obj_spr = ErosionObjSprites::new(1, plant.name, false, plant.has_flower, false);
            obj_spr.set_base_list(0, &spring_spr, 1);
            obj_spr.set_base_list(0, &autumn_spr, 4);
            obj_spr.set_base(0, &format!("d_plants_1_{}", offset + ix), 2);
            obj_spr.set_flower(0, &format!("d_plants_1_{}", offset + ix + 8));
            let bloomstart = if plant.has_flower { plant.bloomstart } else { 0.0 };
            let bloomend = if plant.has_flower { plant.bloomend } else { 0.0 };
            self.objs
                .push(ErosionObj::new(obj_spr, 30, bloomstart, bloomend, false));
        }
    }
}
