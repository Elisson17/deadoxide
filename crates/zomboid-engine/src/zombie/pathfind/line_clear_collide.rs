//! zombie.pathfind.LineClearCollide over the squares of the pathfinder's own cache (`PathMap`): `isNotClear` (= the
//! deprecated `isNotClearOld`) and the deprecated `canStandAt`. The search (`PolygonalMap2.findPath`, `smoothPath`)
//! asks it whether a straight segment is free. Vehicles are not modelled (the vehicle loops are empty).
use super::liang_barsky::line_rect_intersect;
use super::line_clear_collide_main::supercover;
use super::path::{ceil_i32, PmMover};
use super::path_map::PathMap;
use super::vga_star::is_blocked_in_all_directions;
use crate::zombie::core::math::pz_math;
use crate::zombie::iso::iso_directions::IsoDirections;
use crate::zombie::iso::vector2::Vector2;

/// zombie.pathfind.PolygonalMap2.RADIUS_DIAGONAL: `(float) Math.sqrt(0.18F)`
fn radius_diagonal() -> f32 {
    f64::from(0.18f32).sqrt() as f32
}

/// zombie.pathfind.LineClearCollide.canStandAt (deprecated): whether a character could stand at (x, y, z) having come
/// from (from_x, from_y): the target square must not be blocked on all four sides and the squares within 0.3 of the
/// point must be walkable floor where the point is.
pub fn can_stand_at(
    m: &PathMap,
    mover: &PmMover,
    from_x: f32,
    from_y: f32,
    x: f32,
    y: f32,
    z: f32,
) -> bool {
    if (pz_math::fastfloor(from_x) != pz_math::fastfloor(x)
        || pz_math::fastfloor(from_y) != pz_math::fastfloor(y))
        && is_blocked_in_all_directions(
            m,
            mover,
            pz_math::fastfloor(x),
            pz_math::fastfloor(y),
            pz_math::fastfloor(z),
        )
    {
        return false;
    }
    let min_x = pz_math::fastfloor(x - 0.3);
    let min_y = pz_math::fastfloor(y - 0.3);
    let max_x = ceil_i32(x + 0.3);
    let max_y = ceil_i32(y + 0.3);
    for sy in min_y..max_y {
        for sx in min_x..max_x {
            let square = m.get_square(sx, sy, pz_math::fastfloor(z));
            let target_square =
                x >= sx as f32 && y >= sy as f32 && x < (sx + 1) as f32 && y < (sy + 1) as f32;
            let walkable = square.is_some_and(|s| !s.is_really_solid(m) && s.has(512));
            if !walkable && target_square {
                return false;
            }
        }
    }
    true
}

/// zombie.pathfind.LineClearCollide.testCollisionHorizontal
#[allow(clippy::too_many_arguments)]
fn test_collision_horizontal(
    from: (f32, f32),
    to: (f32, f32),
    pt_x: f32,
    pt_y: f32,
    skip_n: bool,
    skip_s: bool,
) -> bool {
    let (from_x, from_y) = from;
    let (to_x, to_y) = to;
    let mut w = 0.3f32;
    let mut n = 0.3f32;
    let mut e = 0.3f32;
    let mut s = 0.3f32;
    if from_x < pt_x && to_x < pt_x {
        w = 0.0;
    } else if from_x >= pt_x + 1.0 && to_x >= pt_x + 1.0 {
        e = 0.0;
    }
    if from_y < pt_y && to_y < pt_y {
        n = 0.0;
    } else if from_y >= pt_y && to_y >= pt_y {
        s = 0.0;
    }
    if skip_n {
        n = 0.0;
    }
    if skip_s {
        s = 0.0;
    }
    line_rect_intersect(
        from_x,
        from_y,
        to_x - from_x,
        to_y - from_y,
        pt_x - w,
        pt_y - n,
        pt_x + 1.0 + e,
        pt_y + s,
    )
    .is_some()
}

/// zombie.pathfind.LineClearCollide.testCollisionVertical
#[allow(clippy::too_many_arguments)]
fn test_collision_vertical(
    from: (f32, f32),
    to: (f32, f32),
    pt_x: f32,
    pt_y: f32,
    skip_w: bool,
    skip_e: bool,
) -> bool {
    let (from_x, from_y) = from;
    let (to_x, to_y) = to;
    let mut w = 0.3f32;
    let mut n = 0.3f32;
    let mut e = 0.3f32;
    let mut s = 0.3f32;
    if from_x < pt_x && to_x < pt_x {
        w = 0.0;
    } else if from_x >= pt_x && to_x >= pt_x {
        e = 0.0;
    }
    if from_y < pt_y && to_y < pt_y {
        n = 0.0;
    } else if from_y >= pt_y + 1.0 && to_y >= pt_y + 1.0 {
        s = 0.0;
    }
    if skip_w {
        w = 0.0;
    }
    if skip_e {
        e = 0.0;
    }
    line_rect_intersect(
        from_x,
        from_y,
        to_x - from_x,
        to_y - from_y,
        pt_x - w,
        pt_y - n,
        pt_x + e,
        pt_y + 1.0 + s,
    )
    .is_some()
}

/// zombie.pathfind.LineClearCollide.isNotClear: `isNotClearOld`; `true` = the segment from (from_x, from_y) to
/// (to_x, to_y) on level `z` (the cache's z, +32) is blocked. `flags` bit 4 makes any square with a cost block it.
#[allow(clippy::too_many_arguments)]
pub fn is_not_clear(
    m: &PathMap,
    mover: &PmMover,
    from_x: f32,
    from_y: f32,
    to_x: f32,
    to_y: f32,
    z: i32,
    flags: i32,
) -> bool {
    let check_cost = (flags & 4) != 0;
    if !can_stand_at(m, mover, from_x, from_y, to_x, to_y, z as f32) {
        return true;
    }
    let rd = radius_diagonal();
    let perp_x = to_y - from_y;
    let perp_y = -(to_x - from_x);
    let mut perp = Vector2::new(perp_x, perp_y);
    perp.normalize();
    let x0 = from_x + perp.x * rd;
    let y0 = from_y + perp.y * rd;
    let x1 = to_x + perp.x * rd;
    let y1 = to_y + perp.y * rd;
    perp = Vector2::new(-perp_x, -perp_y);
    perp.normalize();
    let x2 = from_x + perp.x * rd;
    let y2 = from_y + perp.y * rd;
    let x3 = to_x + perp.x * rd;
    let y3 = to_y + perp.y * rd;
    let mut pts: Vec<(i32, i32)> = Vec::new();
    pts.push((pz_math::fastfloor(from_x), pz_math::fastfloor(from_y)));
    if pz_math::fastfloor(from_x) != pz_math::fastfloor(to_x)
        || pz_math::fastfloor(from_y) != pz_math::fastfloor(to_y)
    {
        pts.push((pz_math::fastfloor(to_x), pz_math::fastfloor(to_y)));
    }
    supercover(x0, y0, x1, y1, &mut pts);
    supercover(x2, y2, x3, y3, &mut pts);
    let from = (from_x, from_y);
    let to = (to_x, to_y);
    let rect = |left: f32, top: f32, right: f32, bottom: f32| {
        line_rect_intersect(
            from_x,
            from_y,
            to_x - from_x,
            to_y - from_y,
            left,
            top,
            right,
            bottom,
        )
        .is_some()
    };
    for &(px, py) in &pts {
        let square = m.get_square(px, py, z);
        if check_cost && square.is_some_and(|s| s.cost > 0) {
            return true;
        }
        let (pxf, pyf) = (px as f32, py as f32);
        let blocked_square = match square {
            None => true,
            Some(s) => s.is_really_solid(m) || !s.has(512),
        };
        if blocked_square {
            let mut w = 0.3f32;
            let mut n = 0.3f32;
            let mut e = 0.3f32;
            let mut s = 0.3f32;
            if from_x < pxf && to_x < pxf {
                w = 0.0;
            } else if from_x >= (px + 1) as f32 && to_x >= (px + 1) as f32 {
                e = 0.0;
            }
            if from_y < pyf && to_y < pyf {
                n = 0.0;
            } else if from_y >= (py + 1) as f32 && to_y >= (py + 1) as f32 {
                s = 0.0;
            }
            if rect(pxf - w, pyf - n, pxf + 1.0 + e, pyf + 1.0 + s) {
                return true;
            }
            continue;
        }
        let Some(sq) = square else { continue };
        if sq.has(504) {
            if sq.has(448) {
                if test_collision_vertical(from, to, pxf, pyf, false, true) {
                    return true;
                }
                if test_collision_vertical(from, to, pxf + 1.0, pyf, true, false) {
                    return true;
                }
                if sq.has(64) && test_collision_horizontal(from, to, pxf, pyf, false, true) {
                    return true;
                }
            }
            if sq.has(56) {
                if test_collision_horizontal(from, to, pxf, pyf, false, true) {
                    return true;
                }
                if test_collision_horizontal(from, to, pxf, pyf + 1.0, true, false) {
                    return true;
                }
                if sq.has(8) && test_collision_vertical(from, to, pxf, pyf, false, true) {
                    return true;
                }
            }
        } else if sq.has_sloped_surface() {
            let Some(dir) = sq.sloped_dir else { continue };
            let adj = |d: IsoDirections| sq.get_adjacent_square(m, d);
            if dir == IsoDirections::N || dir == IsoDirections::S {
                if adj(IsoDirections::W).is_none_or(|o| !sq.has_identical_sloped_surface(o))
                    && test_collision_vertical(from, to, pxf, pyf, false, true)
                {
                    return true;
                }
                if adj(IsoDirections::E).is_none_or(|o| !sq.has_identical_sloped_surface(o))
                    && test_collision_vertical(from, to, pxf + 1.0, pyf, true, false)
                {
                    return true;
                }
                let square_n = adj(IsoDirections::N);
                if dir == IsoDirections::N
                    && square_n
                        .is_none_or(|o| sq.sloped_max != o.get_sloped_surface_height_at(0.5, 1.0))
                    && test_collision_horizontal(from, to, pxf, pyf, false, true)
                {
                    return true;
                }
                let square_s = adj(IsoDirections::S);
                if dir == IsoDirections::S
                    && square_s
                        .is_none_or(|o| sq.sloped_max != o.get_sloped_surface_height_at(0.5, 0.0))
                    && test_collision_horizontal(from, to, pxf, pyf + 1.0, true, false)
                {
                    return true;
                }
            }
            if dir == IsoDirections::W || dir == IsoDirections::E {
                let square_n = adj(IsoDirections::N);
                if square_n.is_none_or(|o| !sq.has_identical_sloped_surface(o))
                    && test_collision_horizontal(from, to, pxf, pyf, false, true)
                {
                    return true;
                }
                let square_s = adj(IsoDirections::S);
                if square_s.is_none_or(|o| !sq.has_identical_sloped_surface(o))
                    && test_collision_horizontal(from, to, pxf, pyf + 1.0, true, false)
                {
                    return true;
                }
                let square_w = adj(IsoDirections::W);
                if dir == IsoDirections::W
                    && square_w
                        .is_none_or(|o| sq.sloped_max != o.get_sloped_surface_height_at(1.0, 0.5))
                    && test_collision_vertical(from, to, pxf, pyf, false, true)
                {
                    return true;
                }
                let square_e = adj(IsoDirections::E);
                if dir == IsoDirections::E
                    && square_e
                        .is_none_or(|o| sq.sloped_max != o.get_sloped_surface_height_at(0.0, 0.5))
                    && test_collision_vertical(from, to, pxf + 1.0, pyf, true, false)
                {
                    return true;
                }
            }
        } else {
            if sq.is_collide_w(m) {
                let mut wx = 0.3f32;
                let mut nx = 0.3f32;
                let mut ex = 0.3f32;
                let mut sx = 0.3f32;
                if from_x < pxf && to_x < pxf {
                    wx = 0.0;
                } else if from_x >= pxf && to_x >= pxf {
                    ex = 0.0;
                }
                if from_y < pyf && to_y < pyf {
                    nx = 0.0;
                } else if from_y >= (py + 1) as f32 && to_y >= (py + 1) as f32 {
                    sx = 0.0;
                }
                if rect(pxf - wx, pyf - nx, pxf + ex, pyf + 1.0 + sx) {
                    return true;
                }
            }
            if sq.is_collide_n(m) {
                let mut wxx = 0.3f32;
                let mut nxx = 0.3f32;
                let mut exx = 0.3f32;
                let mut sxx = 0.3f32;
                if from_x < pxf && to_x < pxf {
                    wxx = 0.0;
                } else if from_x >= (px + 1) as f32 && to_x >= (px + 1) as f32 {
                    exx = 0.0;
                }
                if from_y < pyf && to_y < pyf {
                    nxx = 0.0;
                } else if from_y >= pyf && to_y >= pyf {
                    sxx = 0.0;
                }
                if rect(pxf - wxx, pyf - nxx, pxf + 1.0 + exx, pyf + sxx) {
                    return true;
                }
            }
        }
    }
    false
}
