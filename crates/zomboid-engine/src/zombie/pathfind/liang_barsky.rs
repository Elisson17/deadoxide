//! zombie.pathfind.LiangBarsky: clipping of a line segment against an axis-aligned rectangle.

/// zombie.pathfind.LiangBarsky.lineRectIntersect: whether the segment from (x, y) along (dx, dy) crosses the rectangle,
/// with the entry and exit parameters (`t1t2`). The `q` terms are float subtractions widened to double, as in the
/// original (`this.q[0] = x - left` with float operands).
#[allow(clippy::too_many_arguments)]
pub fn line_rect_intersect(
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
) -> Option<(f64, f64)> {
    let p = [f64::from(-dx), f64::from(dx), f64::from(-dy), f64::from(dy)];
    let q = [
        f64::from(x - left),
        f64::from(right - x),
        f64::from(y - top),
        f64::from(bottom - y),
    ];
    let mut u1 = 0.0f64;
    let mut u2 = 1.0f64;
    for i in 0..4 {
        if p[i] == 0.0 {
            if q[i] < 0.0 {
                return None;
            }
        } else {
            let t = q[i] / p[i];
            if p[i] < 0.0 && u1 < t {
                u1 = t;
            } else if p[i] > 0.0 && u2 > t {
                u2 = t;
            }
        }
    }
    if u1 >= u2 {
        None
    } else {
        Some((u1, u2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A horizontal segment through a 2x2 box enters at a quarter and leaves at three quarters of its length; one
    /// that passes above the box misses it.
    #[test]
    fn clips_a_segment_to_the_box() {
        assert_eq!(
            line_rect_intersect(0.0, 1.0, 4.0, 0.0, 1.0, 0.0, 3.0, 2.0),
            Some((0.25, 0.75))
        );
        assert_eq!(
            line_rect_intersect(0.0, 5.0, 4.0, 0.0, 1.0, 0.0, 3.0, 2.0),
            None
        );
    }
}
