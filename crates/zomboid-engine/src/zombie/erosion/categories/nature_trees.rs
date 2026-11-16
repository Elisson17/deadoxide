//! zombie.erosion.categories.NatureTrees: the trees of the natural tiles.
use super::erosion_category::{
    clear_cat_mod_data, clerp, current_season, set_cat_mod_data, update_obj, CategoryBase,
    ErosionCategory,
};
use crate::zombie::core::math::pz_math;
use crate::zombie::erosion::erosion_data::{Category, CategoryData, Chunk, Square};
use crate::zombie::erosion::erosion_env::{at, Env, Pos};
use crate::zombie::erosion::obj::erosion_obj::ErosionObj;
use crate::zombie::erosion::obj::erosion_obj_sprites::ErosionObjSprites;
use crate::zombie::erosion::season::erosion_ice_queen::ErosionIceQueen;
use crate::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;

const SOIL_REF: [&[i32]; 12] = [
    &[2, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5],
    &[1, 1, 2, 2, 2, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 5, 5, 5, 5],
    &[2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 3, 3, 4, 4, 4, 5],
    &[1, 7, 7, 7, 9, 9, 9, 9, 9, 9, 9],
    &[2, 2, 1, 1, 1, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 9, 9, 9, 9],
    &[1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 7, 7, 7, 9],
    &[1, 2, 8, 8, 8, 6, 6, 6, 6, 6, 6, 6, 6],
    &[1, 1, 2, 2, 3, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 6, 6, 6, 6, 6],
    &[1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 3, 3, 8, 8, 8, 6],
    &[3, 10, 10, 10, 11, 11, 11, 11, 11, 11, 11],
    &[1, 1, 3, 3, 3, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 11, 11, 11, 11],
    &[1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 10, 10, 10, 11],
];

/// zombie.erosion.categories.NatureTrees.TreeInit
struct TreeInit {
    name: &'static str,
    tileset_name: String,
    jumbo_tileset_name: String,
    jumbo_xl_tileset_name: String,
    jumbo_xxl_tileset_name: String,
    evergreen: bool,
}

impl TreeInit {
    fn new(name: &'static str, tileset_name: &str, seasonal: bool) -> Self {
        TreeInit {
            name,
            tileset_name: tileset_name.to_string(),
            jumbo_tileset_name: tileset_name.replace("_1", "JUMBO_1"),
            jumbo_xl_tileset_name: tileset_name.replace("_1", "JUMBOXL_1"),
            jumbo_xxl_tileset_name: tileset_name.replace("_1", "JUMBOXXL_1"),
            evergreen: seasonal,
        }
    }
}

/// zombie.erosion.categories.NatureTrees
pub struct NatureTrees {
    base: CategoryBase,
    trees: Vec<TreeInit>,
    spawn_chance: [i32; 100],
    objs: Vec<ErosionObj>,
}

impl Default for NatureTrees {
    fn default() -> Self {
        NatureTrees {
            base: CategoryBase::default(),
            trees: vec![
                TreeInit::new("American Holly", "e_americanholly_1", true),
                TreeInit::new("Canadian Hemlock", "e_canadianhemlock_1", true),
                TreeInit::new("Virginia Pine", "e_virginiapine_1", true),
                TreeInit::new("Riverbirch", "e_riverbirch_1", false),
                TreeInit::new("Cockspur Hawthorn", "e_cockspurhawthorn_1", false),
                TreeInit::new("Dogwood", "e_dogwood_1", false),
                TreeInit::new("Carolina Silverbell", "e_carolinasilverbell_1", false),
                TreeInit::new("Yellowwood", "e_yellowwood_1", false),
                TreeInit::new("Eastern Redbud", "e_easternredbud_1", false),
                TreeInit::new("Redmaple", "e_redmaple_1", false),
                TreeInit::new("American Linden", "e_americanlinden_1", false),
            ],
            spawn_chance: [0; 100],
            objs: Vec::new(),
        }
    }
}

impl NatureTrees {
    /// The soil row of a square: its soil, or a random row when the soil is outside the table.
    fn soil_row(env: &Env, ed: &Square, x: i32, y: i32) -> &'static [i32] {
        let mut soil = ed.soil;
        if soil < 0 || soil >= SOIL_REF.len() as i32 {
            soil = ed.rand(env.rands, x, y, SOIL_REF.len() as i32);
        }
        SOIL_REF[soil as usize]
    }
}

impl ErosionCategory for NatureTrees {
    fn base(&self) -> &CategoryBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut CategoryBase {
        &mut self.base
    }

    fn kind(&self) -> Category {
        Category::NatureTrees
    }

    /// zombie.erosion.categories.NatureTrees.replaceExistingObject: a tree of the map (the jumbo, the vegetation
    /// and the erosion tilesets) becomes a tree of this category; the erosion tilesets' own tiles are removed
    /// (the category draws its own).
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
                if name.starts_with("jumbo_tree_01") || name.starts_with("vegetation_trees") {
                    let jumbo = name.starts_with("jumbo_tree_01");
                    let soil_ref = Self::soil_row(env, ed, x, y);
                    let e_value = ed.noise_main_int;
                    let di = set_cat_mod_data(&self.base, Category::NatureTrees, ed);
                    let game_obj = at(soil_ref, ed.rand(env.rands, x, y, soil_ref.len() as i32))? - 1;
                    let d = &mut ed.regions[di];
                    d.game_obj = game_obj;
                    let base_stage = if jumbo { 5 } else { 3 };
                    d.max_stage = base_stage + (e_value as f32 / 51.0).floor() as i32 - 1;
                    d.stage = d.max_stage;
                    d.spawn_time = 0;
                    d.disp_season = -1;
                    let erosion_obj = self.objs.get(game_obj as usize).ok_or_else(|| {
                        format!("java.lang.IndexOutOfBoundsException: Index {game_obj}")
                    })?;
                    env.sq_mut(pos)?.objects[idx].name = Some(erosion_obj.name.clone());
                    d.has_spawned = true;
                    return Ok(true);
                }
                for t in 0..self.trees.len() {
                    let tree = &self.trees[t];
                    if name.starts_with(&tree.tileset_name) {
                        let di = set_cat_mod_data(&self.base, Category::NatureTrees, ed);
                        let d = &mut ed.regions[di];
                        d.game_obj = t as i32;
                        d.max_stage = 3;
                        d.stage = d.max_stage;
                        d.spawn_time = 0;
                        env.map.remove_tile_object(pos, idx, env.sm, env.alias);
                        return Ok(true);
                    }
                    if name.starts_with(&tree.jumbo_tileset_name) {
                        let di = set_cat_mod_data(&self.base, Category::NatureTrees, ed);
                        let d = &mut ed.regions[di];
                        d.game_obj = t as i32;
                        d.max_stage = 5;
                        let p = name.rfind('_').map_or(-1, |p| p as i32);
                        let index = pz_math::try_parse_int(Some(&name[(p + 1) as usize..]), 0);
                        d.stage = if index % 2 == 0 { 4 } else { 5 };
                        d.spawn_time = 0;
                        env.map.remove_tile_object(pos, idx, env.sm, env.alias);
                        return Ok(true);
                    }
                    if name.starts_with(&tree.jumbo_xl_tileset_name)
                        || name.starts_with(&tree.jumbo_xxl_tileset_name)
                    {
                        let xl = name.starts_with(&tree.jumbo_xl_tileset_name);
                        let di = set_cat_mod_data(&self.base, Category::NatureTrees, ed);
                        let d = &mut ed.regions[di];
                        d.game_obj = t as i32;
                        d.max_stage = if xl { 6 } else { 7 };
                        d.stage = if xl { 6 } else { 7 };
                        d.spawn_time = 0;
                        env.map.remove_tile_object(pos, idx, env.sm, env.alias);
                        return Ok(true);
                    }
                }
            }
            i -= 1;
        }
        Ok(false)
    }

    /// zombie.erosion.categories.NatureTrees.validateSpawn
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
            let chance = at(&self.spawn_chance, e_value)?;
            let (x, y) = (pos.0, pos.1);
            if chance > 0 && ed.rand(env.rands, x, y, 101) < chance {
                let di = set_cat_mod_data(&self.base, Category::NatureTrees, ed);
                let game_obj = at(soil_ref, ed.rand(env.rands, x, y, soil_ref.len() as i32))? - 1;
                let d = &mut ed.regions[di];
                d.game_obj = game_obj;
                d.max_stage = 2 + (e_value - 50) / 17 - 1;
                d.stage = 0;
                d.spawn_time = 130 - e_value;
                return Ok(true);
            }
            Ok(false)
        } else {
            Ok(false)
        }
    }

    /// zombie.erosion.categories.NatureTrees.update
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
                update_obj(
                    &self.base, env, ed, data, pos, game_obj, true, stage, disp_season, false,
                )?;
            } else {
                clear_cat_mod_data(&self.base, ed);
            }
        }
        Ok(())
    }

    /// zombie.erosion.categories.NatureTrees.init
    fn init(&mut self, sm: &mut IsoSpriteManager, ice_queen: &mut ErosionIceQueen) {
        for i in 0..100 {
            self.spawn_chance[i] = if i >= 50 {
                clerp((i as i32 - 50) as f32 / 50.0, 0.0, 90.0) as i32
            } else {
                0
            };
        }
        let snames = [0, 5, 1, 2, 3, 4];
        let sd = &mut self.base.season_disp;
        sd[5].season1 = 0;
        sd[5].season2 = 0;
        sd[5].split = false;
        sd[1].season1 = 1;
        sd[1].season2 = 0;
        sd[1].split = false;
        sd[2].season1 = 2;
        sd[2].season2 = 3;
        sd[2].split = true;
        sd[4].season1 = 4;
        sd[4].season2 = 0;
        sd[4].split = true;
        let mut base_sprite = String::new();
        for id in 0..self.trees.len() {
            let name = self.trees[id].name;
            let sheet = self.trees[id].tileset_name.clone();
            let seasonal = !self.trees[id].evergreen;
            let mut obj_spr = ErosionObjSprites::new(8, name, true, false, seasonal);
            for stage in 0..8 {
                for season in 0..snames.len() as i32 {
                    let sheetid = match stage {
                        4 => season * 2,
                        5 => season * 2 + 1,
                        6 => season,
                        7 => season,
                        _ => season * 4 + stage,
                    };
                    let repl = match stage {
                        4 | 5 => sheet.replace("_1", "JUMBO_1"),
                        6 => sheet.replace("_1", "JUMBOXL_1"),
                        7 => sheet.replace("_1", "JUMBOXXL_1"),
                        _ => sheet.clone(),
                    };
                    if season == 0 {
                        base_sprite = format!("{repl}_{sheetid}");
                        obj_spr.set_base(stage, &base_sprite, 0);
                    } else if season == 1 {
                        ice_queen.add_sprite(sm, &base_sprite, &format!("{repl}_{sheetid}"));
                    } else if seasonal {
                        obj_spr.set_child_sprite(
                            stage,
                            &format!("{repl}_{sheetid}"),
                            snames[season as usize],
                        );
                    }
                }
            }
            self.objs.push(ErosionObj::new(obj_spr, 60, 0.0, 0.0, true));
        }
    }
}
