//! zombie.pathfind.LineClearCollideMain (`isNotClear` = `isNotClearOld`, `canStandAtOld`) and
//! `PolygonalMap2.supercover` over the squares of a loaded map: plain squares, walls, closed doors, solid squares,
//! holes, stairs, sloped surfaces and `solidtrans` squares next to a window or a hoppable. NOT PORTED (an `Err` where
//! the original would take the branch): the cost check (flag 4, no caller sets it) and the debug rendering (flag 8).
//! Vehicles are not looked at (the squares model has none: UNKNOWN for a vehicle near the line).
use super::liang_barsky::line_rect_intersect;
use crate::zombie::core::math::pz_math;
use crate::zombie::core::tile_property_alias_map::TilePropertyAliasMap;
use crate::zombie::iso::iso_directions::IsoDirections;
use crate::zombie::iso::iso_grid_square::{GetSquare, IsoGridSquare};
use crate::zombie::iso::sprite_details::iso_flag_type::IsoFlagType;
use crate::zombie::iso::sprite_details::iso_object_type::IsoObjectType;
use crate::zombie::iso::vector2::Vector2;

/// zombie.pathfind.PolygonalMap2.RADIUS_DIAGONAL: `(float) Math.sqrt(0.18F)`
fn radius_diagonal() -> f32 {
    f64::from(0.18f32).sqrt() as f32
}

/// zombie.pathfind.LineClearCollideMain.clamp
fn clamp(f1: f32, min: f32, max: f32) -> f32 {
    let mut f = f1;
    if f < min {
        f = min;
    }
    if f > max {
        f = max;
    }
    f
}

/// zombie.pathfind.PolygonalMap2.supercover: the squares a segment crosses, appended to `pts` unless already there.
pub fn supercover(x0: f32, y0: f32, x1: f32, y1: f32, pts: &mut Vec<(i32, i32)>) {
    let dx = f64::from((x1 - x0).abs());
    let dy = f64::from((y1 - y0).abs());
    let mut x = pz_math::fastfloor(x0);
    let mut y = pz_math::fastfloor(y0);
    let mut n: i32 = 1;
    let x_inc: i32;
    let mut error: f64;
    if dx == 0.0 {
        x_inc = 0;
        error = f64::INFINITY;
    } else if x1 > x0 {
        x_inc = 1;
        n += pz_math::fastfloor(x1) - x;
        error = f64::from((pz_math::fastfloor(x0) + 1) as f32 - x0) * dy;
    } else {
        x_inc = -1;
        n += x - pz_math::fastfloor(x1);
        error = f64::from(x0 - pz_math::fastfloor(x0) as f32) * dy;
    }
    let y_inc: i32;
    if dy == 0.0 {
        y_inc = 0;
        error -= f64::INFINITY;
    } else if y1 > y0 {
        y_inc = 1;
        n += pz_math::fastfloor(y1) - y;
        error -= f64::from((pz_math::fastfloor(y0) + 1) as f32 - y0) * dx;
    } else {
        y_inc = -1;
        n += y - pz_math::fastfloor(y1);
        error -= f64::from(y0 - pz_math::fastfloor(y0) as f32) * dx;
    }
    while n > 0 {
        if !pts.contains(&(x, y)) {
            pts.push((x, y));
        }
        if error > 0.0 {
            y += y_inc;
            error -= dx;
        } else {
            x += x_inc;
            error += dy;
        }
        n -= 1;
    }
}

/// `bHasFloor` of canStandAtOld / isNotClearOld: `square != null && (solidFloorCached ? solidFloor :
/// TreatAsSolidFloor())`, and a square without objects (or none) takes the floor from a sloped surface below it that
/// reaches 0.9 at its centre.
fn has_floor(
    g: &dyn GetSquare,
    alias: &TilePropertyAliasMap,
    sx: i32,
    sy: i32,
    z: i32,
    square: Option<&IsoGridSquare>,
) -> bool {
    let mut b_has_floor = square.is_some_and(|s| s.treat_as_solid_floor(alias));
    if square.is_none_or(|s| s.objects.is_empty()) {
        if let Some(below) = g.get_grid_square(sx, sy, z - 1) {
            if below.get_sloped_surface_height_at(alias, 0.5, 0.5) > 0.9f32 {
                b_has_floor = true;
            }
        }
    }
    b_has_floor
}

/// The distance test shared by the edge checks of canStandAtOld: the squared distance from (x, y) to the closest point
/// (`closest_x`, `closest_y`) of the edge is below 0.3 squared.
fn within_radius(x: f32, y: f32, closest_x: f32, closest_y: f32) -> bool {
    let distance_x = x - closest_x;
    let distance_y = y - closest_y;
    let distance_squared = distance_x * distance_x + distance_y * distance_y;
    distance_squared < 0.09f32
}

/// zombie.pathfind.LineClearCollideMain.canStandAtOld (the squares part; the vehicles of the chunks around are not
/// looked at). `flags` bit 1 ignores doors, bit 2 is `closeToWalls`.
pub fn can_stand_at_old(
    g: &dyn GetSquare,
    alias: &TilePropertyAliasMap,
    x: f32,
    y: f32,
    z: f32,
    flags: i32,
) -> Result<bool, String> {
    let ignore_doors = (flags & 1) != 0;
    let close_to_walls = (flags & 2) != 0;
    let min_x = pz_math::fastfloor(x - 0.3f32);
    let min_y = pz_math::fastfloor(y - 0.3f32);
    let max_x = f64::from(x + 0.3f32).ceil() as i32;
    let max_y = f64::from(y + 0.3f32).ceil() as i32;
    let zi = pz_math::fastfloor(z);
    for sy in min_y..max_y {
        for sx in min_x..max_x {
            let b_target_square =
                x >= sx as f32 && y >= sy as f32 && x < (sx + 1) as f32 && y < (sy + 1) as f32;
            let square = g.get_grid_square(sx, sy, zi);
            let b_has_floor = has_floor(g, alias, sx, sy, zi, square);
            let (sxf, syf, sx1, sy1) = (sx as f32, sy as f32, (sx + 1) as f32, (sy + 1) as f32);
            match square {
                Some(sq)
                    if !sq.has(IsoFlagType::solid)
                        && (!sq.has(IsoFlagType::solidtrans)
                            || sq.is_adjacent_to_window(g)
                            || sq.is_adjacent_to_hoppable(g))
                        && b_has_floor =>
                {
                    if sq.has_stairs() {
                        if !close_to_walls && !b_target_square {
                            // N, S, W, E in that order: the closest point of the edge
                            for (edge, cx, cy) in [
                                (IsoDirections::N, clamp(x, sxf, sx1), syf),
                                (IsoDirections::S, clamp(x, sxf, sx1), sy1),
                                (IsoDirections::W, sxf, clamp(y, syf, sy1)),
                                (IsoDirections::E, sx1, clamp(y, syf, sy1)),
                            ] {
                                if sq.is_stairs_edge_blocked(g, edge) && within_radius(x, y, cx, cy)
                                {
                                    return Ok(false);
                                }
                            }
                        }
                    } else if sq.has_sloped_surface(alias) {
                        if !close_to_walls && !b_target_square {
                            for (edge, cx, cy) in [
                                (IsoDirections::N, clamp(x, sxf, sx1), syf),
                                (IsoDirections::S, clamp(x, sxf, sx1), sy1),
                                (IsoDirections::W, sxf, clamp(y, syf, sy1)),
                                (IsoDirections::E, sx1, clamp(y, syf, sy1)),
                            ] {
                                if sq.is_sloped_surface_edge_blocked(alias, g, edge)
                                    && within_radius(x, y, cx, cy)
                                {
                                    return Ok(false);
                                }
                            }
                        }
                    } else if !close_to_walls {
                        if (sq.has(IsoFlagType::collideW)
                            || (!ignore_doors && sq.has_blocked_door(false)))
                            && within_radius(x, y, sxf, clamp(y, syf, sy1))
                        {
                            return Ok(false);
                        }
                        if (sq.has(IsoFlagType::collideN)
                            || (!ignore_doors && sq.has_blocked_door(true)))
                            && within_radius(x, y, clamp(x, sxf, sx1), syf)
                        {
                            return Ok(false);
                        }
                    }
                }
                _ => {
                    if close_to_walls {
                        if b_target_square {
                            return Ok(false);
                        }
                    } else if within_radius(x, y, clamp(x, sxf, sx1), clamp(y, syf, sy1)) {
                        return Ok(false);
                    }
                }
            }
        }
    }
    Ok(true)
}

/// `testCollisionHorizontal` of LineClearCollideMain (without the debug line): the segment crosses the box of a
/// horizontal edge of the square (`pt_x`, `pt_y`), `skip_n`/`skip_s` take the margin off that side.
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

/// `testCollisionVertical` of LineClearCollideMain (without the debug line).
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

/// zombie.pathfind.LineClearCollideMain.isNotClearOld without a vehicle and without the cost check: whether the segment
/// from (from_x, from_y) to (to_x, to_y) on floor `z` is blocked for a character (`true` = not clear). `flags` bit 1
/// ignores doors, bit 2 is `closeToWalls`.
#[allow(clippy::too_many_arguments)]
pub fn is_not_clear_old(
    g: &dyn GetSquare,
    alias: &TilePropertyAliasMap,
    from_x: f32,
    from_y: f32,
    to_x: f32,
    to_y: f32,
    z: i32,
    flags: i32,
) -> Result<bool, String> {
    let ignore_doors = (flags & 1) != 0;
    if (flags & 4) != 0 {
        return Err(
            "LineClearCollideMain.isNotClearOld: the cost check (flag 4) is not ported".into(),
        );
    }
    if (flags & 8) != 0 {
        return Err(
            "LineClearCollideMain.isNotClearOld: the debug rendering (flag 8) is not ported".into(),
        );
    }
    if !can_stand_at_old(g, alias, to_x, to_y, z as f32, flags)? {
        return Ok(true);
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
        let square = g.get_grid_square(px, py, z);
        let b_has_floor = has_floor(g, alias, px, py, z, square);
        let (pxf, pyf) = (px as f32, py as f32);
        let blocked_square = match square {
            None => true,
            Some(s) => {
                s.has(IsoFlagType::solid)
                    || (s.has(IsoFlagType::solidtrans)
                        && !s.is_adjacent_to_window(g)
                        && !s.is_adjacent_to_hoppable(g))
                    || !b_has_floor
            }
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
                return Ok(true);
            }
        } else if let Some(sq) = square {
            if sq.has_stairs() {
                if sq.has_stairs_north()
                    && (test_collision_vertical(from, to, pxf, pyf, false, true)
                        || test_collision_vertical(from, to, pxf + 1.0, pyf, true, false)
                        || (sq.has_type(IsoObjectType::stairsTN)
                            && test_collision_horizontal(from, to, pxf, pyf, false, true)))
                {
                    return Ok(true);
                }
                if sq.has_stairs_west()
                    && (test_collision_horizontal(from, to, pxf, pyf, false, true)
                        || test_collision_horizontal(from, to, pxf, pyf + 1.0, true, false)
                        || (sq.has_type(IsoObjectType::stairsTW)
                            && test_collision_vertical(from, to, pxf, pyf, false, true)))
                {
                    return Ok(true);
                }
            } else if sq.has_sloped_surface(alias) {
                let adj = |d: IsoDirections| g.get_grid_square(px + d.dx(), py + d.dy(), z);
                let Some(dir) = sq.get_sloped_surface_direction(alias) else {
                    continue;
                };
                if dir == IsoDirections::N || dir == IsoDirections::S {
                    if adj(IsoDirections::W)
                        .is_none_or(|o| !sq.has_identical_sloped_surface(o, alias))
                        && test_collision_vertical(from, to, pxf, pyf, false, true)
                    {
                        return Ok(true);
                    }
                    if adj(IsoDirections::E)
                        .is_none_or(|o| !sq.has_identical_sloped_surface(o, alias))
                        && test_collision_vertical(from, to, pxf + 1.0, pyf, true, false)
                    {
                        return Ok(true);
                    }
                    let max = sq.get_sloped_surface_height_max(alias);
                    if dir == IsoDirections::N
                        && adj(IsoDirections::N)
                            .is_none_or(|o| max != o.get_sloped_surface_height_at(alias, 0.5, 1.0))
                        && test_collision_horizontal(from, to, pxf, pyf, false, true)
                    {
                        return Ok(true);
                    }
                    if dir == IsoDirections::S
                        && adj(IsoDirections::S)
                            .is_none_or(|o| max != o.get_sloped_surface_height_at(alias, 0.5, 0.0))
                        && test_collision_horizontal(from, to, pxf, pyf + 1.0, true, false)
                    {
                        return Ok(true);
                    }
                }
                if dir == IsoDirections::W || dir == IsoDirections::E {
                    if adj(IsoDirections::N)
                        .is_none_or(|o| !sq.has_identical_sloped_surface(o, alias))
                        && test_collision_horizontal(from, to, pxf, pyf, false, true)
                    {
                        return Ok(true);
                    }
                    if adj(IsoDirections::S)
                        .is_none_or(|o| !sq.has_identical_sloped_surface(o, alias))
                        && test_collision_horizontal(from, to, pxf, pyf + 1.0, true, false)
                    {
                        return Ok(true);
                    }
                    let max = sq.get_sloped_surface_height_max(alias);
                    if dir == IsoDirections::W
                        && adj(IsoDirections::W)
                            .is_none_or(|o| max != o.get_sloped_surface_height_at(alias, 1.0, 0.5))
                        && test_collision_vertical(from, to, pxf, pyf, false, true)
                    {
                        return Ok(true);
                    }
                    if dir == IsoDirections::E
                        && adj(IsoDirections::E)
                            .is_none_or(|o| max != o.get_sloped_surface_height_at(alias, 0.0, 0.5))
                        && test_collision_vertical(from, to, pxf + 1.0, pyf, true, false)
                    {
                        return Ok(true);
                    }
                }
            } else {
                if sq.has(IsoFlagType::collideW) || (!ignore_doors && sq.has_blocked_door(false)) {
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
                        return Ok(true);
                    }
                }
                if sq.has(IsoFlagType::collideN) || (!ignore_doors && sq.has_blocked_door(true)) {
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
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}
