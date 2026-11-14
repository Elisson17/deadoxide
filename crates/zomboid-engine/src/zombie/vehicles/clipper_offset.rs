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
//! The union is not the Vatti engine of Clipper64 (not ported yet). It cuts the offset path at its self-intersections,
//! keeps the pieces that separate positive from non-positive winding (decided exactly, in i128, on the original
//! edges), and places each crossing where Clipper does (`intersect_point`). The ring can start at another vertex than
//! Clipper's and has no collinear vertices. Measured on the 664 wide polylines of the reference save
//! (`sh tools/oracle.sh zoneoutlines`): 662 outlines are identical (rotated to their smallest point); in 2 a crossing
//! that lies within a unit of a scanline is still a vertex here, while the Vatti sweep, which compares the edges'
//! ROUNDED x at the scanline (`TopX`), never saw it cross and ends the edge at its local maximum instead. Every meta
//! chunk registration that uses these outlines (`sh tools/oracle.sh mapzone`) is identical. Where the union yields
//! several outer rings (not seen for a connected polyline), the largest is taken; Clipper's order is not modelled.
use std::collections::{BTreeMap, BTreeSet};

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

fn cross(o: P64, a: P64, b: P64) -> i128 {
    (a.0 - o.0) as i128 * (b.1 - o.1) as i128 - (a.1 - o.1) as i128 * (b.0 - o.0) as i128
}

/// `num / den` (`den > 0`) rounded half away from zero.
fn div_round(num: i128, den: i128) -> i64 {
    let q = (2 * num.abs() + den) / (2 * den);
    (if num < 0 { -q } else { q }) as i64
}

/// Clipper2 `TopX`: the x of an active edge at scanline `y`, `bot.x + nearbyint(dx * (y - bot.y))` (ties to even).
fn top_x(bot: P64, top: P64, y: i64) -> i64 {
    if y == top.1 || top.0 == bot.0 {
        top.0
    } else if y == bot.1 {
        bot.0
    } else {
        bot.0 + (get_dx(bot, top) * (y - bot.1) as f64).round_ties_even() as i64
    }
}

/// Clipper2 `GetDx`: x change per unit of y along the edge (horizontal edges: ∓f64::MAX).
fn get_dx(pt1: P64, pt2: P64) -> f64 {
    let dy = (pt2.1 - pt1.1) as f64;
    if dy != 0.0 {
        (pt2.0 - pt1.0) as f64 / dy
    } else if pt2.0 > pt1.0 {
        -f64::MAX
    } else {
        f64::MAX
    }
}

/// An edge as Clipper's Vatti engine holds it: `bot` is the end with the larger y (scanbeams go from large y to small).
fn bot_top(e: (P64, P64)) -> (P64, P64) {
    if e.0 .1 >= e.1 .1 {
        (e.0, e.1)
    } else {
        (e.1, e.0)
    }
}

/// The crossing of edges `ea` and `eb` (at parameter `t` along `ea`) as Clipper64 makes it a vertex of the union.
/// With a horizontal edge, `ClipperBase::DoHorizontal` puts it at the other edge's `TopX` on the horizontal's level.
/// Otherwise `AddNewIntersectNode(e1, e2)` with `e1` the edge on the left below the crossing (the smaller `dx`), and
/// `GetSegmentIntersectPt(e1.bot, e1.top, e2.bot, e2.top)` of Clipper2 1.5.x: `t` in double, the point truncated
/// toward zero (`static_cast<int64_t>`). The products are exact in double for these coordinates. The correction of a
/// point that rounding put outside its scanbeam is not modelled (UNKNOWN whether the saves reach it).
fn intersect_point(ea: (P64, P64), eb: (P64, P64), t: Frac) -> P64 {
    let (ba, ta) = bot_top(ea);
    let (bb, tb) = bot_top(eb);
    if ba.1 == ta.1 || bb.1 == tb.1 {
        let (h, (eb_, et)) = if ba.1 == ta.1 {
            (ba, (bb, tb))
        } else {
            (bb, (ba, ta))
        };
        if eb_.1 == et.1 {
            // both horizontal and collinear: the cut is an end point of one of them
            let (a, b) = ea;
            return (
                a.0 + div_round(t.n * (b.0 - a.0) as i128, t.d),
                a.1 + div_round(t.n * (b.1 - a.1) as i128, t.d),
            );
        }
        return (top_x(eb_, et, h.1), h.1);
    }
    // dx_a < dx_b, both denominators negative: compare the cross-multiplied numerators
    let a_left = ((ta.0 - ba.0) as i128 * (tb.1 - bb.1) as i128)
        < ((tb.0 - bb.0) as i128 * (ta.1 - ba.1) as i128);
    let ((l1a, l1b), (l2a, l2b)) = if a_left {
        ((ba, ta), (bb, tb))
    } else {
        ((bb, tb), (ba, ta))
    };
    let dx1 = (l1b.0 - l1a.0) as f64;
    let dy1 = (l1b.1 - l1a.1) as f64;
    let dx2 = (l2b.0 - l2a.0) as f64;
    let dy2 = (l2b.1 - l2a.1) as f64;
    let det = dy1 * dx2 - dy2 * dx1;
    if det == 0.0 {
        let (a, b) = ea;
        return (
            a.0 + div_round(t.n * (b.0 - a.0) as i128, t.d),
            a.1 + div_round(t.n * (b.1 - a.1) as i128, t.d),
        );
    }
    let tt = ((l1a.0 - l2a.0) as f64 * dy2 - (l1a.1 - l2a.1) as f64 * dx2) / det;
    if tt <= 0.0 {
        l1a
    } else if tt >= 1.0 {
        l1b
    } else {
        (
            (l1a.0 as f64 + tt * dx1) as i64,
            (l1a.1 as f64 + tt * dy1) as i64,
        )
    }
}

/// A parameter along an edge as the exact fraction `n / d` (`d > 0`).
#[derive(Clone, Copy, Debug)]
struct Frac {
    n: i128,
    d: i128,
}

impl Frac {
    fn new(n: i128, d: i128) -> Self {
        if d < 0 {
            Frac { n: -n, d: -d }
        } else {
            Frac { n, d }
        }
    }
    fn cmp(self, o: Frac) -> std::cmp::Ordering {
        (self.n * o.d).cmp(&(o.n * self.d))
    }
}

/// The winding number of the closed path `edges` around `T = a + t·(b - a) + ε·nv` where `a`, `b` is edge `i` and
/// `ε` is infinitesimal, counting counter-clockwise turns positive (Clipper's convention for a path of positive area).
/// Exact: `T` is kept as `a + P / t.d` with `P = t.n·(b - a)`, so every test is an i128 sign; a tie (T's level on a
/// vertex's level, T on an edge's line) is broken by `nv`, which is what makes the two sides of edge `i` differ.
fn winding_at(edges: &[(P64, P64)], i: usize, t: Frac, nv: (i128, i128)) -> i32 {
    let (a, b) = edges[i];
    let dd = t.d;
    let px = t.n * (b.0 - a.0) as i128;
    let py = t.n * (b.1 - a.1) as i128;
    // vertex level <= T level, with T.y = P.y + ε nv.y
    let le = |y: i128| y < py || (y == py && nv.1 >= 0);
    let mut w = 0;
    for &(c, e) in edges {
        let cy = (c.1 - a.1) as i128 * dd;
        let ey = (e.1 - a.1) as i128 * dd;
        let side = || {
            let cx = (c.0 - a.0) as i128 * dd;
            let (ex, ey_) = ((e.0 - c.0) as i128, (e.1 - c.1) as i128);
            let s = ex * (py - cy) - ey_ * (px - cx);
            if s != 0 {
                s.signum()
            } else {
                (ex * nv.1 - ey_ * nv.0).signum()
            }
        };
        if le(cy) {
            if !le(ey) && side() > 0 {
                w += 1;
            }
        } else if le(ey) && side() < 0 {
            w -= 1;
        }
    }
    w
}

/// The boundary of the region where `ring` winds positively (Clipper64 `Union` with `FillRule::Positive` of one closed
/// path), as rings with the region on their left; collinear vertices removed. The path is cut at every crossing and
/// overlap; each piece is kept when the winding number left of it is positive and right of it is not (or the other way
/// round, then reversed), decided exactly on the original edges. Only the vertices of the result are rounded.
fn positive_union(ring: &[P64]) -> Vec<Vec<P64>> {
    let mut v: Vec<P64> = Vec::new();
    for &p in ring {
        if v.last() != Some(&p) {
            v.push(p);
        }
    }
    while v.len() > 1 && v.first() == v.last() {
        v.pop();
    }
    if v.len() < 3 {
        return Vec::new();
    }
    let n = v.len();
    let edges: Vec<(P64, P64)> = (0..n).map(|i| (v[i], v[(i + 1) % n])).collect();
    // cut points per edge: (parameter along the edge, point)
    let mut splits: Vec<Vec<(Frac, P64)>> = edges
        .iter()
        .map(|&(a, b)| vec![(Frac::new(0, 1), a), (Frac::new(1, 1), b)])
        .collect();
    for i in 0..n {
        for j in i + 1..n {
            let ((a1, b1), (a2, b2)) = (edges[i], edges[j]);
            let d1 = ((b1.0 - a1.0) as i128, (b1.1 - a1.1) as i128);
            let d2 = ((b2.0 - a2.0) as i128, (b2.1 - a2.1) as i128);
            let den = d1.0 * d2.1 - d1.1 * d2.0;
            let w = ((a2.0 - a1.0) as i128, (a2.1 - a1.1) as i128);
            if den != 0 {
                let t = Frac::new(w.0 * d2.1 - w.1 * d2.0, den);
                let u = Frac::new(w.0 * d1.1 - w.1 * d1.0, den);
                if (0..=t.d).contains(&t.n) && (0..=u.d).contains(&u.n) {
                    let p = intersect_point(edges[i], edges[j], t);
                    splits[i].push((t, p));
                    splits[j].push((u, p));
                }
            } else if cross(a1, b1, a2) == 0 {
                // collinear: each endpoint of one edge that lies on the other cuts it
                let dot = |p: P64, a: P64, d: (i128, i128)| {
                    Frac::new(
                        (p.0 - a.0) as i128 * d.0 + (p.1 - a.1) as i128 * d.1,
                        d.0 * d.0 + d.1 * d.1,
                    )
                };
                for p in [a2, b2] {
                    let f = dot(p, a1, d1);
                    if (0..=f.d).contains(&f.n) {
                        splits[i].push((f, p));
                    }
                }
                for p in [a1, b1] {
                    let f = dot(p, a2, d2);
                    if (0..=f.d).contains(&f.n) {
                        splits[j].push((f, p));
                    }
                }
            }
        }
    }
    let mut kept: BTreeSet<(P64, P64)> = BTreeSet::new();
    for (i, cuts) in splits.iter_mut().enumerate() {
        cuts.sort_by(|x, y| x.0.cmp(y.0));
        cuts.dedup_by(|x, y| x.0.cmp(y.0).is_eq());
        let (a, b) = edges[i];
        let (dx, dy) = ((b.0 - a.0) as i128, (b.1 - a.1) as i128);
        for w in cuts.windows(2) {
            let ((f1, p), (f2, q)) = (w[0], w[1]);
            if p == q {
                continue;
            }
            // the mediant lies strictly between the two cuts and keeps the numbers small
            let t = Frac::new(f1.n + f2.n, f1.d + f2.d);
            let wl = winding_at(&edges, i, t, (-dy, dx));
            let wr = winding_at(&edges, i, t, (dy, -dx));
            if (wl > 0) != (wr > 0) {
                kept.insert(if wl > 0 { (p, q) } else { (q, p) });
            }
        }
    }
    // chain the kept edges into rings
    let mut out_edges: BTreeMap<P64, Vec<P64>> = BTreeMap::new();
    for &(p, q) in &kept {
        out_edges.entry(p).or_default().push(q);
    }
    let mut rings = Vec::new();
    while let Some((&start, _)) = out_edges.iter().find(|(_, l)| !l.is_empty()) {
        let mut r = vec![start];
        let mut prev = start;
        let mut cur = match out_edges.get_mut(&start).and_then(|l| l.pop()) {
            Some(c) => c,
            None => break,
        };
        while cur != start {
            r.push(cur);
            let Some(list) = out_edges.get_mut(&cur) else {
                break;
            };
            if list.is_empty() {
                break;
            }
            // several ways on: take the sharpest right turn, which keeps touching rings apart
            let din = ((cur.0 - prev.0) as f64, (cur.1 - prev.1) as f64);
            let mut best = 0;
            let mut best_a = f64::INFINITY;
            for (idx, nx) in list.iter().enumerate() {
                let dout = ((nx.0 - cur.0) as f64, (nx.1 - cur.1) as f64);
                let a = (din.0 * dout.1 - din.1 * dout.0).atan2(din.0 * dout.0 + din.1 * dout.1);
                if a < best_a {
                    best_a = a;
                    best = idx;
                }
            }
            let next = list.swap_remove(best);
            prev = cur;
            cur = next;
        }
        // remove collinear vertices and spikes
        let mut changed = true;
        while changed && r.len() >= 3 {
            changed = false;
            let len = r.len();
            for i in 0..len {
                let (a, b, c) = (r[(i + len - 1) % len], r[i], r[(i + 1) % len]);
                if cross(a, b, c) == 0 {
                    r.remove(i);
                    changed = true;
                    break;
                }
            }
        }
        if r.len() >= 3 {
            rings.push(r);
        }
    }
    rings
}

fn area2(r: &[P64]) -> i128 {
    let n = r.len();
    (0..n)
        .map(|i| {
            let (a, b) = (r[i], r[(i + 1) % n]);
            a.0 as i128 * b.1 as i128 - b.0 as i128 * a.1 as i128
        })
        .sum()
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
    let outer = positive_union(&ring)
        .into_iter()
        .filter(|r| area2(r) > 0)
        .max_by_key(|r| area2(r))?;
    Some(
        outer
            .iter()
            .flat_map(|&(x, y)| [x as f32 / 1000.0, y as f32 / 1000.0])
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
