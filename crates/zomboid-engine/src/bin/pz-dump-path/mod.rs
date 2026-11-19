//! The pathfinding dumps of `pz-dump` (case `pathfind`): the Rust side of `PathProbe` in `tests/original_probe`, which
//! runs the original `zombie.pathfind` inside the booted server over real chunks of Rosewood.
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

use zomboid_engine::java::{self, float_to_string as f};
use zomboid_engine::zombie::core::random::rand_abstract::RandAbstract;
use zomboid_engine::zombie::core::tile_property_alias_map::TilePropertyAliasMap;
use zomboid_engine::zombie::iso::cell_loader::{self, LoadContext};
use zomboid_engine::zombie::iso::iso_chunk::IsoChunk;
use zomboid_engine::zombie::iso::iso_grid_square::{GetSquare, IsoGridSquare};
use zomboid_engine::zombie::iso::iso_meta_grid::AlarmSettings;
use zomboid_engine::zombie::iso::iso_world::IsoWorld;
use zomboid_engine::zombie::pathfind::line_clear_collide_main;

/// The chunks `PathProbe.loadChunks` loaded (each with `Rand` seeded 12345, `LoadCellBinaryChunk`,
/// `loadInWorldStreamerThread`), looked up by square as `ServerMap.getGridSquare` does.
struct PathWorld {
    world: IsoWorld,
    rand: RandAbstract<java::Random>,
    chunks: HashMap<(i32, i32), IsoChunk>,
    alias: TilePropertyAliasMap,
}

impl GetSquare for PathWorld {
    fn get_grid_square(&self, x: i32, y: i32, z: i32) -> Option<&IsoGridSquare> {
        self.chunks
            .get(&(x.div_euclid(8), y.div_euclid(8)))?
            .get_grid_square(x.rem_euclid(8), y.rem_euclid(8), z)
    }
}

impl PathWorld {
    fn new(root: &Path) -> Result<Self, String> {
        let mut rand = RandAbstract::new(java::Random::new(12345));
        let alarm = AlarmSettings {
            last_stand: false,
            power_grid_exists: true,
            alarm: 1,
            alarm_decay: 1,
        };
        let mut world = IsoWorld::init(root, "Muldraugh, KY", &|_| false, &alarm, None, &mut rand)?;
        let alias = std::mem::take(&mut world.alias);
        Ok(PathWorld {
            world,
            rand,
            chunks: HashMap::new(),
            alias,
        })
    }

    /// PathProbe.loadChunks: the chunks of the rectangle that are not loaded yet.
    fn load_chunks(&mut self, wx1: i32, wy1: i32, wx2: i32, wy2: i32) {
        let IsoWorld {
            sprites,
            tile_definitions,
            meta_grid,
            ..
        } = &mut self.world;
        let mut ctx = LoadContext {
            sprites,
            alias: &mut self.alias,
            glass_removed_window_sprite_map: &tile_definitions.glass_removed_window_sprite_map,
            smashed_window_sprite_map: &tile_definitions.smashed_window_sprite_map,
            meta_grid,
            rand: &mut self.rand,
            locked_houses: 6,
            missing_tiles: Vec::new(),
            lotpack_cache: None,
        };
        let have: HashSet<(i32, i32)> = self.chunks.keys().copied().collect();
        for wx in wx1..=wx2 {
            for wy in wy1..=wy2 {
                if have.contains(&(wx, wy)) {
                    continue;
                }
                *ctx.rand = RandAbstract::new(java::Random::new(12345));
                let mut ch = IsoChunk::new(wx, wy);
                cell_loader::load_cell_binary_chunk(&mut ctx, wx, wy, &mut ch, &mut Vec::new());
                ch.load_in_world_streamer_thread(&*ctx.sprites, &*ctx.meta_grid, ctx.alias);
                self.chunks.insert((wx, wy), ch);
            }
        }
    }
}

/// `PathProbe.lcc`: scenario `lcc:<cx1>:<cy1>:<cx2>:<cy2>:<n>:<seed>`.
fn lcc(sb: &mut String, spec: &str, w: &mut PathWorld) -> Result<(), String> {
    let p: Vec<&str> = spec.split(':').collect();
    if p.len() != 7 {
        return Err(format!("bad scenario {spec}"));
    }
    let num = |i: usize| p[i].parse::<i32>().map_err(|e| format!("{spec}: {e}"));
    let (cx1, cy1, cx2, cy2, n) = (num(1)?, num(2)?, num(3)?, num(4)?, num(5)?);
    let seed: i64 = p[6].parse().map_err(|e| format!("{spec}: {e}"))?;
    w.load_chunks(cx1 - 1, cy1 - 1, cx2 + 1, cy2 + 1);
    let _ = writeln!(sb, "SCENARIO {spec}");
    let mut r = java::Random::new(seed);
    let min_x = cx1 * 8;
    let min_y = cy1 * 8;
    let wd = (cx2 - cx1 + 1) * 8;
    let ht = (cy2 - cy1 + 1) * 8;
    for i in 0..n {
        let z = if r.next_int_bound(4) == 0 {
            1 + r.next_int_bound(2)
        } else {
            0
        };
        let flags = r.next_int_bound(4);
        let mut fx = min_x as f32 + r.next_float() * wd as f32;
        let mut fy = min_y as f32 + r.next_float() * ht as f32;
        let kind = r.next_int_bound(10);
        let (mut tx, mut ty);
        if kind < 4 {
            tx = fx + (r.next_float() - 0.5f32) * 5.0f32;
            ty = fy + (r.next_float() - 0.5f32) * 5.0f32;
        } else if kind < 8 {
            tx = fx + (r.next_float() - 0.5f32) * 16.0f32;
            ty = fy + (r.next_float() - 0.5f32) * 16.0f32;
        } else {
            tx = min_x as f32 + r.next_float() * wd as f32;
            ty = min_y as f32 + r.next_float() * ht as f32;
        }
        if r.next_int_bound(3) == 0 {
            fx = f64::from(fx).floor() as f32 + 0.5f32;
            fy = f64::from(fy).floor() as f32 + 0.5f32;
        }
        if r.next_int_bound(3) == 0 {
            tx = f64::from(tx).floor() as f32 + 0.5f32;
            ty = f64::from(ty).floor() as f32 + 0.5f32;
        }
        let blocked =
            line_clear_collide_main::is_not_clear_old(&*w, &w.alias, fx, fy, tx, ty, z, flags)?;
        let s1 = line_clear_collide_main::can_stand_at_old(&*w, &w.alias, fx, fy, z as f32, flags)?;
        let s2 = line_clear_collide_main::can_stand_at_old(&*w, &w.alias, tx, ty, z as f32, flags)?;
        let _ = writeln!(
            sb,
            "L {i} z={z} f={flags} {},{} {},{} blocked={} stand={}{}",
            f(fx),
            f(fy),
            f(tx),
            f(ty),
            u8::from(blocked),
            u8::from(s1),
            u8::from(s2)
        );
    }
    let _ = writeln!(sb, "END {spec}");
    Ok(())
}

/// The `pathfind` case: `PathProbe`'s scenarios, run by the port.
pub fn pathfind(root: &Path, scenarios: &[String]) -> Result<String, String> {
    let mut sb = String::new();
    let mut w = PathWorld::new(root)?;
    for spec in scenarios {
        if spec.starts_with("lcc:") {
            lcc(&mut sb, spec, &mut w)?;
        } else {
            return Err(format!("unknown scenario {spec}"));
        }
    }
    Ok(sb)
}
