//! zombie.pathfind.LineClearCollideMain (`isNotClear` = `isNotClearOld`, `canStandAtOld`) and
//! `PolygonalMap2.supercover`, for the squares of a flat floor: plain squares, walls (`collideW`/`collideN`), closed
//! doors, solid squares and holes. NOT PORTED (an `Err` where the original would take the branch): stairs, sloped
//! surfaces, `solidtrans` squares (their adjacency to windows and hoppables), the cost check of `SquareUpdateTask`
//! (flag 4, only for non-zombies) and the debug rendering. Vehicles are not looked at either (UNKNOWN: a vehicle in the
//! chunks around the line would make the original answer "not clear").
use super::liang_barsky::line_rect_intersect;
use crate::zombie::core::math::pz_math;
use crate::zombie::core::tile_property_alias_map::TilePropertyAliasMap;
use crate::zombie::iso::iso_grid_square::{GetSquare, IsoGridSquare};
use crate::zombie::iso::sprite_details::iso_flag_type::IsoFlagType;
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

/// `square == null || square.getObjects().isEmpty()` then the sloped surface of the square below: only a sloped surface
/// below changes the floor test, and that is not ported.
fn has_floor(
    g: &dyn GetSquare,
    alias: &TilePropertyAliasMap,
    sx: i32,
    sy: i32,
    z: i32,
    square: Option<&IsoGridSquare>,
) -> Result<bool, String> {
    let has_floor = square.is_some_and(|s| s.treat_as_solid_floor(alias));
    if square.is_none_or(|s| s.objects.is_empty()) {
        if let Some(below) = g.get_grid_square(sx, sy, z - 1) {
            if below.get_sloped_surface_direction(alias).is_some() {
                return Err(
                    "LineClearCollideMain: the sloped surface of the square below is not ported".into(),
                );
            }
        }
    }
    Ok(has_floor)
}

/// zombie.pathfind.LineClearCollideMain.canStandAtOld (the squares part; the vehicles of the chunks around are not
/// looked at).
pub fn can_stand_at_old(
    g: &dyn GetSquare,
    alias: &TilePropertyAliasMap,
    x: f32,
    y: f32,
    z: f32,
    ignore_doors: bool,
    close_to_walls: bool,
) -> Result<bool, String> {
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
            let b_has_floor = has_floor(g, alias, sx, sy, zi, square)?;
            match square {
                Some(sq) if !sq.has(IsoFlagType::solid) && (!sq.has(IsoFlagType::solidtrans)) && b_has_floor => {
                    if sq.has_stairs() || sq.get_sloped_surface_direction(alias).is_some() {
                        return Err(
                            "LineClearCollideMain.canStandAtOld: a stairs or sloped square is not ported".into(),
                        );
                    }
                    if !close_to_walls {
                        if sq.has(IsoFlagType::collideW) || (!ignore_doors && sq.has_blocked_door(false)) {
                            let closest_x = sx as f32;
                            let closest_y = clamp(y, sy as f32, (sy + 1) as f32);
                            let distance_x = x - closest_x;
                            let distance_y = y - closest_y;
                            let distance_squared = distance_x * distance_x + distance_y * distance_y;
                            if distance_squared < 0.09f32 {
                                return Ok(false);
                            }
                        }
                        if sq.has(IsoFlagType::collideN) || (!ignore_doors && sq.has_blocked_door(true)) {
                            let closest_x = clamp(x, sx as f32, (sx + 1) as f32);
                            let closest_y = sy as f32;
                            let distance_x = x - closest_x;
                            let distance_y = y - closest_y;
                            let distance_squared = distance_x * distance_x + distance_y * distance_y;
                            if distance_squared < 0.09f32 {
                                return Ok(false);
                            }
                        }
                    }
                }
                Some(sq) if !sq.has(IsoFlagType::solid) && sq.has(IsoFlagType::solidtrans) && b_has_floor => {
                    return Err(
                        "LineClearCollideMain.canStandAtOld: a solidtrans square (adjacent window or hoppable) is not ported"
                            .into(),
                    );
                }
                _ => {
                    if close_to_walls {
                        if b_target_square {
                            return Ok(false);
                        }
                    } else {
                        let closest_x = clamp(x, sx as f32, (sx + 1) as f32);
                        let closest_y = clamp(y, sy as f32, (sy + 1) as f32);
                        let distance_x = x - closest_x;
                        let distance_y = y - closest_y;
                        let distance_squared = distance_x * distance_x + distance_y * distance_y;
                        if distance_squared < 0.09f32 {
                            return Ok(false);
                        }
                    }
                }
            }
        }
    }
    Ok(true)
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
    let close_to_walls = (flags & 2) != 0;
    let check_cost = (flags & 4) != 0;
    if check_cost {
        return Err("LineClearCollideMain.isNotClearOld: the cost check (flag 4) is not ported".into());
    }
    if !can_stand_at_old(g, alias, to_x, to_y, z as f32, ignore_doors, close_to_walls)? {
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
    let rect = |left: f32, top: f32, right: f32, bottom: f32| {
        line_rect_intersect(from_x, from_y, to_x - from_x, to_y - from_y, left, top, right, bottom)
            .is_some()
    };
    for &(px, py) in &pts {
        let square = g.get_grid_square(px, py, z);
        let b_has_floor = has_floor(g, alias, px, py, z, square)?;
        let (pxf, pyf) = (px as f32, py as f32);
        let blocked_square = match square {
            None => true,
            Some(s) => {
                if s.has(IsoFlagType::solid) {
                    true
                } else if s.has(IsoFlagType::solidtrans) {
                    return Err(
                        "LineClearCollideMain.isNotClearOld: a solidtrans square (adjacent window or hoppable) is not ported"
                            .into(),
                    );
                } else {
                    !b_has_floor
                }
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
            if sq.has_stairs() || sq.get_sloped_surface_direction(alias).is_some() {
                return Err(
                    "LineClearCollideMain.isNotClearOld: a stairs or sloped square is not ported".into(),
                );
            }
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
    Ok(false)
}
