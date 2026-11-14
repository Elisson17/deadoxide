//! zombie.vehicles.ClipperOffset: the game's native wrapper (`libPZClipper`, JNIClipperOffset) over Clipper2 1.5.x
//! `ClipperOffset`, ported for what the world loader uses it for: the outline of a polyline zone (one open path, miter
//! joins, butt ends, `polylineWidth / 2` as delta). What the native side does, read from the arm64 build shipped with
//! 42.21.0:
//! - `addPath`: each float point becomes `floor(x * 1000 + 0.5)` (one fused multiply-add in f32), as an int64 point;
//! - `newInstance`: `ClipperOffset(miter_limit 2.0, arc_tolerance 0, preserve_collinear false, reverse_solution false)`;
//! - `execute(d)`: `Execute(d * 1000.0, PolyTree64)`, i.e. the offset paths are merged by a Clipper64 union with the
//!   positive fill rule;
//! - `getPolygon(0)`: the outer ring of the first top-level polygon, each coordinate as `(float) v / 1000f`.
//!
//! Clipper2 is identified as 1.5.x by the binary (`std::sqrt` instead of `hypot` in `GetUnitNormal`, no `log10` in
//! the arc tolerance, the member layout of `ClipperOffset`). The compiler fused `a * b + c` into one rounding in the
//! functions used here (`fmadd`/`fmla`); the port uses `mul_add` at exactly those places.
//!
//! The union is Clipper64 itself (`crate::native::clipper2`, the Vatti sweep ported from Clipper2 1.5.4). Measured on
//! the 664 wide polylines of the reference save (`sh tools/oracle.sh zoneoutlines`): every outline is identical to the
//! native one, point for point in the native order (first vertex included), and every meta chunk registration that
//! uses them (`sh tools/oracle.sh mapzone`) is identical.
use crate::native::clipper2::core::Point64;
use crate::native::clipper2::engine::{ClipType, Clipper64, FillRule, PolyTree64};

type P64 = (i64, i64);

#[derive(Clone, Copy, Debug, PartialEq)]
struct PointD {
    x: f64,
    y: f64,
}

/// Clipper2 `Point64(double, double)`: `std::round` (half away from zero) of each coordinate.
fn p64(x: f64, y: f64) -> P64 {
    (x.round() as i64, y.round() as i64)
}

/// Clipper2 `GetUnitNormal` (1.5.x: `Hypot` is `sqrt(x * x + y * y)`, compiled as `fma(dx, dx, dy * dy)`).
fn get_unit_normal(pt1: P64, pt2: P64) -> PointD {
    if pt1 == pt2 {
        return PointD { x: 0.0, y: 0.0 };
    }
    let dx = (pt2.0 - pt1.0) as f64;
    let dy = (pt2.1 - pt1.1) as f64;
    let inverse_hypot = 1.0 / dx.mul_add(dx, dy * dy).sqrt();
    PointD {
        x: dy * inverse_hypot,
        y: -(dx * inverse_hypot),
    }
}

/// Clipper2 `GetPerpendic`: `Point64(pt + norm * delta)`, fused.
fn get_perpendic(pt: P64, norm: PointD, delta: f64) -> P64 {
    p64(
        norm.x.mul_add(delta, pt.0 as f64),
        norm.y.mul_add(delta, pt.1 as f64),
    )
}

/// The state of Clipper2 `ClipperOffset` while one open path is offset.
struct Offsetter {
    group_delta: f64,
    temp_lim: f64,
    norms: Vec<PointD>,
    path_out: Vec<P64>,
}

impl Offsetter {
    /// Clipper2 `ClipperOffset::BuildNormals`
    fn build_normals(&mut self, path: &[P64]) {
        self.norms.clear();
        if path.is_empty() {
            return;
        }
        for w in path.windows(2) {
            self.norms.push(get_unit_normal(w[0], w[1]));
        }
        self.norms
            .push(get_unit_normal(path[path.len() - 1], path[0]));
    }

    /// Clipper2 `ClipperOffset::DoBevel`
    fn do_bevel(&mut self, path: &[P64], j: usize, k: usize) {
        let (px, py) = (path[j].0 as f64, path[j].1 as f64);
        let (pt1, pt2) = if j == k {
            let abs_delta = self.group_delta.abs();
            let n = self.norms[j];
            (
                ((-n.x).mul_add(abs_delta, px), (-n.y).mul_add(abs_delta, py)),
                (n.x.mul_add(abs_delta, px), n.y.mul_add(abs_delta, py)),
            )
        } else {
            let (nk, nj) = (self.norms[k], self.norms[j]);
            (
                (
                    self.group_delta.mul_add(nk.x, px),
                    self.group_delta.mul_add(nk.y, py),
                ),
                (
                    self.group_delta.mul_add(nj.x, px),
                    self.group_delta.mul_add(nj.y, py),
                ),
            )
        };
        self.path_out.push(p64(pt1.0, pt1.1));
        self.path_out.push(p64(pt2.0, pt2.1));
    }

    /// Clipper2 `ClipperOffset::DoMiter`: `q = delta / (cos_a + 1)`, then `pt + (nk + nj) * q`, fused.
    fn do_miter(&mut self, path: &[P64], j: usize, k: usize, cos_a: f64) {
        let q = self.group_delta / (cos_a + 1.0);
        let (nk, nj) = (self.norms[k], self.norms[j]);
        self.path_out.push(p64(
            (nk.x + nj.x).mul_add(q, path[j].0 as f64),
            (nk.y + nj.y).mul_add(q, path[j].1 as f64),
        ));
    }

    /// Clipper2 `ClipperOffset::DoSquare` (a join sharper than the miter limit allows). Not reached by the polylines of
    /// the saves checked so far; ported from the source without the compiler's fused operations (UNKNOWN parity).
    fn do_square(&mut self, path: &[P64], j: usize, k: usize) {
        let (nj, nk) = (self.norms[j], self.norms[k]);
        let vec = if j == k {
            PointD { x: nj.y, y: -nj.x }
        } else {
            let (x, y) = (-nk.y + nj.y, nk.x - nj.x);
            let h = (x * x + y * y).sqrt();
            if h.abs() < 0.001 {
                PointD { x: 0.0, y: 0.0 }
            } else {
                PointD {
                    x: x * (1.0 / h),
                    y: y * (1.0 / h),
                }
            }
        };
        let abs_delta = self.group_delta.abs();
        let gd = self.group_delta;
        let pt_q = PointD {
            x: path[j].0 as f64 + abs_delta * vec.x,
            y: path[j].1 as f64 + abs_delta * vec.y,
        };
        let pt1 = PointD {
            x: pt_q.x + gd * vec.y,
            y: pt_q.y + gd * -vec.x,
        };
        let pt2 = PointD {
            x: pt_q.x + gd * -vec.y,
            y: pt_q.y + gd * vec.x,
        };
        let pt3 = PointD {
            x: path[k].0 as f64 + nk.x * gd,
            y: path[k].1 as f64 + nk.y * gd,
        };
        let reflect = |pt: PointD| PointD {
            x: pt_q.x + (pt_q.x - pt.x),
            y: pt_q.y + (pt_q.y - pt.y),
        };
        let pt4 = if j == k {
            PointD {
                x: pt3.x + vec.x * gd,
                y: pt3.y + vec.y * gd,
            }
        } else {
            PointD {
                x: path[j].0 as f64 + nk.x * gd,
                y: path[j].1 as f64 + nk.y * gd,
            }
        };
        let pt = segment_intersect_d(pt1, pt2, pt3, pt4).unwrap_or(pt_q);
        let (a, b) = if j == k {
            (reflect(pt), pt)
        } else {
            (pt, reflect(pt))
        };
        self.path_out.push(p64(a.x, a.y));
        self.path_out.push(p64(b.x, b.y));
    }

    /// Clipper2 `ClipperOffset::OffsetPoint` for a miter join (no delta callback, delta not near zero).
    fn offset_point(&mut self, path: &[P64], j: usize, k: usize) {
        if path[j] == path[k] {
            return;
        }
        let (nj, nk) = (self.norms[j], self.norms[k]);
        // CrossProduct(norms[j], norms[k]) = v1.y * v2.x - v2.y * v1.x and DotProduct, each compiled as one fma.
        let mut sin_a = nj.y.mul_add(nk.x, -(nk.y * nj.x));
        let cos_a = nj.x.mul_add(nk.x, nj.y * nk.y);
        sin_a = sin_a.clamp(-1.0, 1.0);
        if self.group_delta.abs() <= 1e-12 {
            self.path_out.push(path[j]);
            return;
        }
        if cos_a > -0.999 && sin_a * self.group_delta < 0.0 {
            // concave: three points that make a negative region, removed by the final union
            self.path_out
                .push(get_perpendic(path[j], nk, self.group_delta));
            self.path_out.push(path[j]);
            self.path_out
                .push(get_perpendic(path[j], nj, self.group_delta));
        } else if cos_a > 0.999 || cos_a > self.temp_lim - 1.0 {
            self.do_miter(path, j, k, cos_a);
        } else {
            self.do_square(path, j, k);
        }
    }

    /// Clipper2 `ClipperOffset::OffsetOpenPath` with butt ends.
    fn offset_open_path(&mut self, path: &[P64]) {
        if self.group_delta.abs() <= 1e-12 {
            self.path_out.push(path[0]);
        } else {
            self.do_bevel(path, 0, 0);
        }
        let high_i = path.len() - 1;
        for j in 1..high_i {
            self.offset_point(path, j, j - 1);
        }
        for i in (1..=high_i).rev() {
            let n = self.norms[i - 1];
            self.norms[i] = PointD { x: -n.x, y: -n.y };
        }
        self.norms[0] = self.norms[high_i];
        if self.group_delta.abs() <= 1e-12 {
            self.path_out.push(path[high_i]);
        } else {
            self.do_bevel(path, high_i, high_i);
        }
        let mut j = high_i.wrapping_sub(1);
        let mut k = high_i;
        while j > 0 && j != usize::MAX {
            self.offset_point(path, j, k);
            k = j;
            j -= 1;
        }
    }
}

/// Clipper2 `GetSegmentIntersectPt` (PointD), used by `DoSquare` only.
fn segment_intersect_d(a1: PointD, a2: PointD, b1: PointD, b2: PointD) -> Option<PointD> {
    let (dx1, dy1) = (a2.x - a1.x, a2.y - a1.y);
    let (dx2, dy2) = (b2.x - b1.x, b2.y - b1.y);
    let det = dy1 * dx2 - dy2 * dx1;
    if det == 0.0 {
        return None;
    }
    let t = ((a1.x - b1.x) * dy2 - (a1.y - b1.y) * dx2) / det;
    Some(if t <= 0.0 {
        a1
    } else if t >= 1.0 {
        a2
    } else {
        PointD {
            x: a1.x + t * dx1,
            y: a1.y + t * dy1,
        }
    })
}

/// The offset ring of one open path (Clipper2 `ExecuteInternal` before the union), in Clipper units.
fn offset_ring(path_in: &[P64], delta: f64) -> Vec<P64> {
    // Group: StripDuplicates for an open path (consecutive duplicates only)
    let mut path: Vec<P64> = Vec::with_capacity(path_in.len());
    for &p in path_in {
        if path.last() != Some(&p) {
            path.push(p);
        }
    }
    if path.is_empty() {
        return Vec::new();
    }
    if delta.abs() < 0.5 {
        return path;
    }
    let mut o = Offsetter {
        // DoGroupOffset of an open path: group_delta_ = |delta|
        group_delta: delta.abs(),
        // miter_limit_ 2: temp_lim_ = 2 / (2 * 2)
        temp_lim: 2.0 / (2.0 * 2.0),
        norms: Vec::new(),
        path_out: Vec::new(),
    };
    if path.len() == 1 {
        if o.group_delta < 1.0 {
            return Vec::new();
        }
        // a single vertex with a non-round join: Rect64(pt - d, pt + d).AsPath()
        let d = o.group_delta.abs().ceil() as i32 as i64;
        let (x, y) = path[0];
        return vec![
            (x - d, y - d),
            (x + d, y - d),
            (x + d, y + d),
            (x - d, y + d),
        ];
    }
    o.build_normals(&path);
    o.offset_open_path(&path);
    o.path_out
}

/// zombie.vehicles.ClipperOffset: `clear`, `addPath(points, Miter, Butt)`, `execute(delta)`, `getPolygon(0)` as the
/// polyline zones use them. Returns the outline as x, y pairs, or `None` when the native side yields no polygon
/// (`getPolygonCount() < 1`, logged by the original as "Failed to generate polyline outline").
pub fn polyline_outline(points: &[(f32, f32)], delta: f32) -> Option<Vec<f32>> {
    let path: Vec<P64> = points
        .iter()
        .map(|&(x, y)| {
            (
                x.mul_add(1000.0, 0.5).floor() as i32 as i64,
                y.mul_add(1000.0, 0.5).floor() as i32 as i64,
            )
        })
        .collect();
    let ring = offset_ring(&path, f64::from(delta) * 1000.0);
    if ring.is_empty() {
        return None;
    }
    // ExecuteInternal: no closed-polygon group, so paths_reversed is false: ReverseSolution(false != false),
    // PreserveCollinear(false) from the ClipperOffset, then Union with the positive rule into the PolyTree64.
    let mut c = Clipper64::new();
    c.set_preserve_collinear(false);
    c.set_reverse_solution(false);
    c.add_subject(&[ring.iter().map(|&(x, y)| Point64::new(x, y)).collect()]);
    let mut tree = PolyTree64::default();
    let mut open = Vec::new();
    c.execute_tree(ClipType::Union, FillRule::Positive, &mut tree, &mut open);
    // JNIClipperOffset getPolygon(0): the first top-level node's own ring
    let first = *tree.children(0).first()?;
    Some(
        tree.nodes[first]
            .polygon
            .iter()
            .flat_map(|p| [p.x as f32 / 1000.0, p.y as f32 / 1000.0])
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A straight road from (0,0) to (10,0), 4 wide: the butt-ended band is the rectangle x 0..10, y -2..2.
    #[test]
    fn a_straight_polyline_becomes_its_band() {
        let o = polyline_outline(&[(0.0, 0.0), (10.0, 0.0)], 2.0).expect("outline");
        let mut pts: Vec<(i64, i64)> = o.chunks(2).map(|c| (c[0] as i64, c[1] as i64)).collect();
        pts.sort_unstable();
        assert_eq!(pts, vec![(0, -2), (0, 2), (10, -2), (10, 2)]);
    }

    /// An L-shaped road, 2 wide, turning 90 degrees at (10,0): the concave join's negative triangle is gone and the
    /// outline is the L of six corners, inner corner at (9,1), outer miter corner at (11,-1).
    #[test]
    fn a_right_angle_turn_keeps_only_the_band() {
        let o = polyline_outline(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], 1.0).expect("outline");
        let mut pts: Vec<(i64, i64)> = o.chunks(2).map(|c| (c[0] as i64, c[1] as i64)).collect();
        pts.sort_unstable();
        assert_eq!(
            pts,
            vec![(0, -1), (0, 1), (9, 1), (9, 10), (11, -1), (11, 10)]
        );
    }
}
