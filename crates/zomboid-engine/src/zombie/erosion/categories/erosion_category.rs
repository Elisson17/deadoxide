//! zombie.erosion.categories.ErosionCategory: what every category shares (the season each category shows, the data
//! lookups on a square, the wall finder, the object update) and the interface the eight categories implement.
use crate::java_fdlibm;
use crate::zombie::erosion::erosion_data::{Category, CategoryData, Chunk, Square};
use crate::zombie::erosion::erosion_env::{Env, Pos};
use crate::zombie::erosion::obj::erosion_obj::ErosionObj;
use crate::zombie::erosion::season::erosion_ice_queen::ErosionIceQueen;
use crate::zombie::iso::iso_directions::IsoDirections;
use crate::zombie::iso::iso_grid_square::ObjectRef;
use crate::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;
use crate::zombie::iso::sprite_details::iso_flag_type::IsoFlagType as F;

/// zombie.erosion.categories.ErosionCategory.SeasonDisplay
#[derive(Clone, Copy, Debug, Default)]
pub struct SeasonDisplay {
    pub season1: i32,
    pub season2: i32,
    pub split: bool,
}

/// The fields of zombie.erosion.categories.ErosionCategory: the category id, the id of its region and the six
/// season displays.
#[derive(Clone, Debug, Default)]
pub struct CategoryBase {
    pub id: i32,
    pub region: i32,
    pub season_disp: [SeasonDisplay; 6],
}

/// What an erosion category implements (the abstract methods of zombie.erosion.categories.ErosionCategory). Every
/// method that can reach a place where the original throws returns `Err` with the exception's text.
pub trait ErosionCategory {
    fn base(&self) -> &CategoryBase;
    fn base_mut(&mut self) -> &mut CategoryBase;
    /// The class of the data this category keeps on a square (what `allocData` makes).
    fn kind(&self) -> Category;
    fn init(&mut self, sm: &mut IsoSpriteManager, ice_queen: &mut ErosionIceQueen);
    fn replace_existing_object(
        &mut self,
        env: &mut Env,
        pos: Pos,
        ed: &mut Square,
        chunk: &Chunk,
        is_exterior: bool,
        has_wall: bool,
    ) -> Result<bool, String>;
    #[allow(clippy::too_many_arguments)]
    fn validate_spawn(
        &mut self,
        env: &mut Env,
        pos: Pos,
        ed: &mut Square,
        chunk: &Chunk,
        is_exterior: bool,
        has_wall: bool,
        is_respawn: bool,
    ) -> Result<bool, String>;
    fn update(
        &mut self,
        env: &mut Env,
        pos: Pos,
        ed: &mut Square,
        data: &mut CategoryData,
        chunk: &Chunk,
        e_tick: i32,
    ) -> Result<(), String>;

    /// zombie.erosion.categories.ErosionCategory.allocData
    fn alloc_data(&self) -> CategoryData {
        CategoryData::alloc(self.kind(), 0, 0)
    }
}

/// zombie.erosion.categories.ErosionCategory.getCatModData: the index of the category's data on the square.
pub fn get_cat_mod_data(base: &CategoryBase, ed: &Square) -> Option<usize> {
    ed.regions
        .iter()
        .position(|d| d.region_id == base.region && d.category_id == base.id)
}

/// zombie.erosion.categories.ErosionCategory.setCatModData: the category's data on the square, made when absent.
pub fn set_cat_mod_data(base: &CategoryBase, kind: Category, ed: &mut Square) -> usize {
    if let Some(i) = get_cat_mod_data(base, ed) {
        return i;
    }
    ed.regions.push(CategoryData::alloc(kind, base.region, base.id));
    // the original logs "> 5 regions on a square" when there are more than five
    ed.regions.len() - 1
}

/// zombie.erosion.categories.ErosionCategory.clearCatModData: takes the category's data off the square.
pub fn clear_cat_mod_data(base: &CategoryBase, ed: &mut Square) {
    if let Some(i) = get_cat_mod_data(base, ed) {
        ed.regions.remove(i);
    }
}

/// zombie.erosion.categories.ErosionCategory.currentSeason: the season (0 to 5, as the category displays it) for a
/// square with that magic number, from the day of the current season.
pub fn current_season(base: &CategoryBase, env: &Env, magic_num: f32) -> i32 {
    let seasons = env.season;
    let season = seasons.get_season();
    let season_day = seasons.get_season_day();
    let season_days = seasons.get_season_days();
    let season_days_half = season_days / 2.0;
    let half_magic_num = season_days_half * magic_num;
    let cur_season = base.season_disp[season as usize];
    if cur_season.split && season_day >= season_days_half + half_magic_num {
        cur_season.season2
    } else if (!cur_season.split || !(season_day >= half_magic_num))
        && !(season_day >= season_days * magic_num)
    {
        let prev_season = if season == 5 {
            base.season_disp[4]
        } else if season == 1 {
            base.season_disp[5]
        } else if season == 2 {
            base.season_disp[1]
        } else {
            base.season_disp[2]
        };
        if prev_season.split {
            prev_season.season2
        } else {
            prev_season.season1
        }
    } else {
        cur_season.season1
    }
}

/// zombie.erosion.categories.ErosionCategory.updateObj: places the object the first time, afterwards changes its
/// stage when the stage, the season or the bloom changed; a failure takes the category's data off the square.
#[allow(clippy::too_many_arguments)]
pub fn update_obj(
    base: &CategoryBase,
    env: &mut Env,
    ed: &mut Square,
    data: &mut CategoryData,
    pos: Pos,
    game_obj: &mut ErosionObj,
    tree: bool,
    stage: i32,
    disp_season: i32,
    bloom: bool,
) -> Result<(), String> {
    if !data.has_spawned {
        if !game_obj.place_object(env, pos, stage, tree, disp_season, bloom)? {
            clear_cat_mod_data(base, ed);
            return Ok(());
        }
        data.has_spawned = true;
    } else if data.stage != stage || data.disp_season != disp_season || data.disp_bloom != bloom {
        let Some(index) = game_obj.get_object(env.sq(pos)?) else {
            clear_cat_mod_data(base, ed);
            return Ok(());
        };
        let sm = &mut *env.sm;
        let Some(sq) = env.map.get_grid_square_mut(pos.0, pos.1, pos.2) else {
            return Err("java.lang.NullPointerException: no square".into());
        };
        game_obj.set_stage_object(
            sm,
            env.alias,
            stage,
            &mut sq.objects[index],
            disp_season,
            bloom,
        )?;
    }
    data.stage = stage;
    data.disp_season = disp_season;
    data.disp_bloom = bloom;
    Ok(())
}

/// zombie.erosion.categories.ErosionCategory.clerp: cosine interpolation between a and b.
pub fn clerp(t: f32, a: f32, b: f32) -> f32 {
    let t2 = (1.0 - java_fdlibm::cos(f64::from(t) * std::f64::consts::PI)) as f32 / 2.0;
    a * (1.0 - t2) + b * t2
}

/// zombie.erosion.categories.ErosionCategory.currentBloom
pub fn current_bloom(env: &Env, magic_num: f32, game_obj: &ErosionObj) -> bool {
    let mut bloom = false;
    let seasons = env.season;
    let season = seasons.get_season();
    if game_obj.has_flower && season == 2 {
        let season_day = seasons.get_season_day();
        let season_days = seasons.get_season_days();
        let season_days_half = season_days / 2.0;
        let half_magic_num = season_days_half * magic_num;
        let totalbloomdays = season_days - half_magic_num;
        let curbloomday = season_day - half_magic_num;
        let mut bdays_max = totalbloomdays * game_obj.bloom_end;
        let mut bdays_min = totalbloomdays * game_obj.bloom_start;
        let totalsplit = (bdays_max - bdays_min) / 2.0;
        let offset = totalsplit * magic_num;
        bdays_max = bdays_min + totalsplit + offset;
        bdays_min += offset;
        if curbloomday >= bdays_min && curbloomday <= bdays_max {
            bloom = true;
        }
    }
    bloom
}

/// zombie.erosion.categories.ErosionCategory.validWall: the wall (or, with `do_window`, the window) on the north or
/// west edge of the square at `pos`, as a handle to the object; above ground a roof tile does not count.
pub fn valid_wall(
    env: &Env,
    pos: Option<Pos>,
    north: bool,
    do_window: bool,
) -> Result<Option<ObjectRef>, String> {
    let Some(pos) = pos else {
        return Ok(None);
    };
    let Some(square) = env.map.get_grid_square(pos.0, pos.1, pos.2) else {
        return Ok(None);
    };
    let other = square.get_tile_in_direction(
        if north {
            IsoDirections::N
        } else {
            IsoDirections::W
        },
        &*env.map,
    );
    let mut object: Option<ObjectRef> = None;
    let wall_to = other.is_some_and(|o| square.is_wall_to(o, &*env.map));
    if wall_to {
        if north && square.has(F::cutN) && !square.has(F::canPathN)
            || !north && square.has(F::cutW) && !square.has(F::canPathW)
        {
            object = square.get_wall(env.sm, north).map(|index| ObjectRef {
                x: square.x,
                y: square.y,
                z: square.z,
                index,
            });
        }
    } else if do_window {
        if let Some(o) = other {
            if square.is_window_blocked_to(o, &*env.map) || square.is_window_to(o, &*env.map) {
                object = square.get_window_to(o, &*env.map);
                if object.is_none() {
                    object = square.get_wall(env.sm, north).map(|index| ObjectRef {
                        x: square.x,
                        y: square.y,
                        z: square.z,
                        index,
                    });
                }
            }
        }
    }
    let Some(obj) = object else {
        return Ok(None);
    };
    if square.z > 0 {
        let name = env.object_sprite_name((obj.x, obj.y, obj.z), obj.index)?;
        Ok(match name {
            Some(n) if !n.contains("roof") => Some(obj),
            _ => None,
        })
    } else {
        Ok(Some(obj))
    }
}
