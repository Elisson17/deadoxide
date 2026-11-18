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
use zomboid_engine::zombie::pathfind::path::{Mover, Path as PzPath};
use zomboid_engine::zombie::pathfind::path_map::{BitsContext, ChunkUpdateTask};
use zomboid_engine::zombie::pathfind::polygonal_map2::PolygonalMap2;

/// The chunks `PathProbe.loadChunks` loaded (each with `Rand` seeded 12345, `LoadCellBinaryChunk`,
/// `loadInWorldStreamerThread`), looked up by square as `ServerMap.getGridSquare` does.
struct PathWorld {
    world: IsoWorld,
    rand: RandAbstract<java::Random>,
    chunks: HashMap<(i32, i32), IsoChunk>,
    alias: TilePropertyAliasMap,
    /// The order the chunks were loaded in, and how many of them `PolygonalMap2.addChunkToWorld` has seen.
    order: Vec<(i32, i32)>,
    added: usize,
    pm: Option<PolygonalMap2>,
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
            order: Vec::new(),
            added: 0,
            pm: None,
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
                ch.load_in_world_streamer_thread(&*ctx.sprites, &*ctx.meta_grid);
                self.chunks.insert((wx, wy), ch);
                self.order.push((wx, wy));
            }
        }
    }

    /// PathProbe.javaWorld: the Java-mode `PolygonalMap2` on the meta grid, with the chunks of the rectangle (plus a
    /// one-chunk ring) loaded and added with `addChunkToWorld`.
    fn java_world(&mut self, cx1: i32, cy1: i32, cx2: i32, cy2: i32) -> &mut PolygonalMap2 {
        self.load_chunks(cx1 - 1, cy1 - 1, cx2 + 1, cy2 + 1);
        if self.pm.is_none() {
            let mg = &self.world.meta_grid;
            self.pm = Some(PolygonalMap2::new(
                mg.min_x,
                mg.min_y,
                mg.max_x - mg.min_x + 1,
                mg.max_y - mg.min_y + 1,
            ));
        }
        let cx = BitsContext {
            sm: &self.world.sprites,
            alias: &self.alias,
        };
        let pm = self
            .pm
            .get_or_insert_with(|| PolygonalMap2::new(0, 0, 0, 0));
        while self.added < self.order.len() {
            let key = self.order[self.added];
            self.added += 1;
            if let Some(ch) = self.chunks.get(&key) {
                let task = ChunkUpdateTask::init(&cx, ch);
                pm.map.execute_chunk_update(&task);
                pm.mark_rebuild();
            }
        }
        pm.update_thread();
        pm
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

/// `PathProbe.pgraph`: scenario `pgraph:<cx1>:<cy1>:<cx2>:<cy2>`, the pathfinder's cache and its high-level data.
fn pgraph(sb: &mut String, spec: &str, w: &mut PathWorld) -> Result<(), String> {
    let p: Vec<&str> = spec.split(':').collect();
    if p.len() != 5 {
        return Err(format!("bad scenario {spec}"));
    }
    let num = |i: usize| p[i].parse::<i32>().map_err(|e| format!("{spec}: {e}"));
    let (cx1, cy1, cx2, cy2) = (num(1)?, num(2)?, num(3)?, num(4)?);
    let pm = w.java_world(cx1, cy1, cx2, cy2);
    let _ = writeln!(sb, "SCENARIO {spec}");
    pm.high_level_store();
    let store = pm.store();
    for wy in cy1..=cy2 {
        for wx in cx1..=cx2 {
            let Some(chunk) = pm.map.get_chunk_from_chunk_pos(wx, wy) else {
                let _ = writeln!(sb, "C {wx},{wy} none");
                continue;
            };
            let (min_l, max_l) = (chunk.min_level, chunk.max_level);
            let _ = writeln!(sb, "C {wx},{wy} levels={min_l}..{max_l}");
            for z in min_l..=max_l {
                if let Some(cl) = chunk.get_level_data(z) {
                    for y in 0..8 {
                        for x in 0..8 {
                            let Some(sq) = cl.get_local(x, y) else {
                                continue;
                            };
                            let _ = writeln!(
                                sb,
                                "B {},{},{} bits={} cost={} slope={},{},{}",
                                sq.x,
                                sq.y,
                                sq.z - 32,
                                sq.bits,
                                sq.cost,
                                sq.sloped_dir.map_or("null", |d| d.name()),
                                f(sq.sloped_min),
                                f(sq.sloped_max)
                            );
                        }
                    }
                }
                let Some(level) = store.levels.get(&(wx, wy, z)) else {
                    continue;
                };
                for &ri in &level.regions {
                    let r = &store.regions[ri];
                    let mut mask = String::new();
                    for y in 0..8 {
                        for x in 0..8 {
                            mask.push(if r.contains_local(x, y) { '1' } else { '0' });
                        }
                    }
                    let mut edges = String::new();
                    for e in [&r.edge_n, &r.edge_s, &r.edge_w, &r.edge_e] {
                        for v in e.iter() {
                            let _ = write!(edges, "{v},");
                        }
                        edges.push(';');
                    }
                    let _ = writeln!(
                        sb,
                        "R {wx},{wy},{} {},{},{},{} {mask} {edges}",
                        z - 32,
                        r.min_x,
                        r.min_y,
                        r.max_x,
                        r.max_y
                    );
                }
                for (tag, list) in [("S", &level.stairs), ("K", &level.sloped)] {
                    for &ti in list {
                        let t = &store.transitions[ti];
                        let _ = writeln!(
                            sb,
                            "{tag} {wx},{wy},{} {} {},{},{}",
                            z - 32,
                            t.dir.name(),
                            t.bottom_floor_x(),
                            t.bottom_floor_y(),
                            t.bottom_floor_z() - 32
                        );
                    }
                }
            }
        }
    }
    let _ = writeln!(sb, "END {spec}");
    Ok(())
}

/// `PathProbe.pathStr`: `<count> x,y,z,flags ...`
fn path_str(path: &PzPath) -> String {
    let mut s = path.nodes.len().to_string();
    for n in &path.nodes {
        let _ = write!(s, " {},{},{},{}", f(n.x), f(n.y), f(n.z), n.flags);
    }
    s
}

/// `PathProbe.psearch`: scenario `psearch:<cx1>:<cy1>:<cx2>:<cy2>:<n>:<seed>:<mover>`.
fn psearch(sb: &mut String, spec: &str, w: &mut PathWorld) -> Result<(), String> {
    let p: Vec<&str> = spec.split(':').collect();
    if p.len() != 8 && p.len() != 9 {
        return Err(format!("bad scenario {spec}"));
    }
    let num = |i: usize| p[i].parse::<i32>().map_err(|e| format!("{spec}: {e}"));
    let (cx1, cy1, cx2, cy2, n) = (num(1)?, num(2)?, num(3)?, num(4)?, num(5)?);
    let seed: i64 = p[6].parse().map_err(|e| format!("{spec}: {e}"))?;
    let kind = p[7];
    let on_squares = p.len() > 8 && p[8] == "sq";
    let mover = match kind {
        "player" => Mover::Player { npc: false },
        "crawler" => Mover::Zombie {
            crawling: true,
            can_crawl_under_vehicle: true,
            can_walk: true,
            has_target: false,
        },
        "zombie" => Mover::Zombie {
            crawling: false,
            can_crawl_under_vehicle: true,
            can_walk: true,
            has_target: false,
        },
        _ => return Err(format!("{spec}: unknown mover {kind}")),
    };
    let pm = w.java_world(cx1, cy1, cx2, cy2);
    // the walkable squares of the rectangle (`:sq`): not solid, with a floor or stairs
    let mut walk: Vec<(i32, i32, i32)> = Vec::new();
    if on_squares {
        for wy in cy1..=cy2 {
            for wx in cx1..=cx2 {
                let Some(chunk) = pm.map.get_chunk_from_chunk_pos(wx, wy) else {
                    continue;
                };
                for z in chunk.min_level..=chunk.max_level {
                    let Some(cl) = chunk.get_level_data(z) else {
                        continue;
                    };
                    for y in 0..8 {
                        for x in 0..8 {
                            if let Some(sq) = cl.get_local(x, y) {
                                if sq.bits & 1 == 0 && sq.bits & (512 | 504) != 0 {
                                    walk.push((sq.x, sq.y, sq.z - 32));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let _ = writeln!(sb, "SCENARIO {spec}");
    let mut r = java::Random::new(seed);
    let min_x = cx1 * 8;
    let min_y = cy1 * 8;
    let wd = (cx2 - cx1 + 1) * 8;
    let ht = (cy2 - cy1 + 1) * 8;
    for i in 0..n {
        if on_squares {
            if walk.is_empty() {
                return Err(format!("{spec}: no walkable squares"));
            }
            let a = walk[r.next_int_bound(walk.len() as i32) as usize];
            let mut b = a;
            for _ in 0..30 {
                let c = walk[r.next_int_bound(walk.len() as i32) as usize];
                b = c;
                if (c.0 - a.0).abs() <= 24 && (c.1 - a.1).abs() <= 24 && (c.2 - a.2).abs() <= 2 {
                    break;
                }
            }
            let mut sx = a.0 as f32 + 0.5f32;
            let mut sy = a.1 as f32 + 0.5f32;
            if r.next_int_bound(4) == 0 {
                sx = a.0 as f32 + r.next_float();
                sy = a.1 as f32 + r.next_float();
            }
            let mut tx = b.0 as f32 + 0.5f32;
            let mut ty = b.1 as f32 + 0.5f32;
            if r.next_int_bound(4) == 0 {
                tx = b.0 as f32 + r.next_float();
                ty = b.1 as f32 + r.next_float();
            }
            search_one(sb, pm, mover, kind, i, (sx, sy, a.2), (tx, ty, b.2))?;
            continue;
        }
        let sz = if r.next_int_bound(4) == 0 {
            1 + r.next_int_bound(2)
        } else {
            0
        };
        let mut sx = min_x as f32 + r.next_float() * wd as f32;
        let mut sy = min_y as f32 + r.next_float() * ht as f32;
        let (mut tx, mut ty);
        let tk = r.next_int_bound(10);
        if tk < 5 {
            tx = sx + (r.next_float() - 0.5f32) * 12.0f32;
            ty = sy + (r.next_float() - 0.5f32) * 12.0f32;
        } else if tk < 9 {
            tx = sx + (r.next_float() - 0.5f32) * 40.0f32;
            ty = sy + (r.next_float() - 0.5f32) * 40.0f32;
        } else {
            tx = min_x as f32 + r.next_float() * wd as f32;
            ty = min_y as f32 + r.next_float() * ht as f32;
        }
        let mut tz = if r.next_int_bound(5) == 0 {
            sz + if r.next_int_bound(2) == 0 { 1 } else { -1 }
        } else {
            sz
        };
        if tz < 0 {
            tz = 0;
        }
        if r.next_int_bound(3) != 0 {
            sx = f64::from(sx).floor() as f32 + 0.5f32;
            sy = f64::from(sy).floor() as f32 + 0.5f32;
        }
        if r.next_int_bound(3) != 0 {
            tx = f64::from(tx).floor() as f32 + 0.5f32;
            ty = f64::from(ty).floor() as f32 + 0.5f32;
        }
        search_one(sb, pm, mover, kind, i, (sx, sy, sz), (tx, ty, tz))?;
    }
    let _ = writeln!(sb, "END {spec}");
    Ok(())
}

/// `PathProbe.searchOne`: one request, answered by the next `updateThread`.
fn search_one(
    sb: &mut String,
    pm: &mut PolygonalMap2,
    mover: Mover,
    kind: &str,
    i: i32,
    s: (f32, f32, i32),
    t: (f32, f32, i32),
) -> Result<(), String> {
    pm.add_request(mover, s.0, s.1, s.2 as f32, t.0, t.1, t.2 as f32, &[]);
    pm.update_thread();
    let mut answered = pm.update_main();
    if answered.len() != 1 {
        return Err(format!("request {i} answered {} times", answered.len()));
    }
    let req = answered.remove(0);
    let _ = writeln!(
        sb,
        "P {i} {kind} {},{},{} -> {},{},{} done=1 {}",
        f(s.0),
        f(s.1),
        s.2,
        f(t.0),
        f(t.1),
        t.2,
        path_str(&req.path)
    );
    Ok(())
}

/// The `pathfind` case: `PathProbe`'s scenarios, run by the port.
pub fn pathfind(root: &Path, scenarios: &[String]) -> Result<String, String> {
    let mut sb = String::new();
    let mut w = PathWorld::new(root)?;
    for spec in scenarios {
        if spec.starts_with("lcc:") {
            lcc(&mut sb, spec, &mut w)?;
        } else if spec.starts_with("pgraph:") {
            pgraph(&mut sb, spec, &mut w)?;
        } else if spec.starts_with("psearch:") {
            psearch(&mut sb, spec, &mut w)?;
        } else {
            return Err(format!("unknown scenario {spec}"));
        }
    }
    Ok(sb)
}
