//! Clipper2Lib `clipper.core.h` (Clipper2 1.5.4, Angus Johnson, Boost Software License 1.0; see `mod.rs`): the
//! integer point and rectangle types and the geometric predicates the engine uses, for `int64_t` coordinates only.
//! The `__int128` branches are the ones the arm64 build compiles (clang, 64-bit pointers).

/// Clipper2Lib::Point64 (no Z).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Point64 {
    pub x: i64,
    pub y: i64,
}

impl Point64 {
    pub const fn new(x: i64, y: i64) -> Self {
        Point64 { x, y }
    }
    /// Clipper2Lib::Point<int64_t>(double, double): `std::round` of each coordinate (half away from zero).
    pub fn from_f64(x: f64, y: f64) -> Self {
        Point64 {
            x: x.round() as i64,
            y: y.round() as i64,
        }
    }
}

pub type Path64 = Vec<Point64>;
pub type Paths64 = Vec<Path64>;

/// Clipper2Lib::Rect64 (left, top, right, bottom).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect64 {
    pub left: i64,
    pub top: i64,
    pub right: i64,
    pub bottom: i64,
}

impl Rect64 {
    pub const fn new(left: i64, top: i64, right: i64, bottom: i64) -> Self {
        Rect64 {
            left,
            top,
            right,
            bottom,
        }
    }
    /// Clipper2Lib::Rect::IsEmpty
    pub fn is_empty(&self) -> bool {
        self.bottom <= self.top || self.right <= self.left
    }
    /// Clipper2Lib::Rect::Contains (Rect)
    pub fn contains(&self, rec: &Rect64) -> bool {
        rec.left >= self.left
            && rec.right <= self.right
            && rec.top >= self.top
            && rec.bottom <= self.bottom
    }
    /// Clipper2Lib::Rect::MidPoint: integer halves (truncating).
    pub fn mid_point(&self) -> Point64 {
        Point64::new((self.left + self.right) / 2, (self.top + self.bottom) / 2)
    }
}

/// Clipper2Lib::GetBounds (Path)
pub fn get_bounds(path: &[Point64]) -> Rect64 {
    let (mut xmin, mut ymin, mut xmax, mut ymax) = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
    for p in path {
        if p.x < xmin {
            xmin = p.x;
        }
        if p.x > xmax {
            xmax = p.x;
        }
        if p.y < ymin {
            ymin = p.y;
        }
        if p.y > ymax {
            ymax = p.y;
        }
    }
    Rect64::new(xmin, ymin, xmax, ymax)
}

/// Clipper2Lib::CrossProductSign: the sign of (pt2 - pt1) x (pt3 - pt2), exact in 128 bits.
pub fn cross_product_sign(pt1: Point64, pt2: Point64, pt3: Point64) -> i32 {
    let a = pt2.x - pt1.x;
    let b = pt3.y - pt2.y;
    let c = pt2.y - pt1.y;
    let d = pt3.x - pt2.x;
    let ab = i128::from(a) * i128::from(b);
    let cd = i128::from(c) * i128::from(d);
    match ab.cmp(&cd) {
        std::cmp::Ordering::Greater => 1,
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
    }
}

/// Clipper2Lib::ProductsAreEqual: a * b == c * d, exact.
pub fn products_are_equal(a: i64, b: i64, c: i64, d: i64) -> bool {
    i128::from(a) * i128::from(b) == i128::from(c) * i128::from(d)
}

/// Clipper2Lib::IsCollinear
pub fn is_collinear(pt1: Point64, shared_pt: Point64, pt2: Point64) -> bool {
    let a = shared_pt.x - pt1.x;
    let b = pt2.y - shared_pt.y;
    let c = shared_pt.y - pt1.y;
    let d = pt2.x - shared_pt.x;
    products_are_equal(a, b, c, d)
}

/// Clipper2Lib::CrossProduct (three points), in double.
pub fn cross_product(pt1: Point64, pt2: Point64, pt3: Point64) -> f64 {
    (pt2.x - pt1.x) as f64 * (pt3.y - pt2.y) as f64
        - (pt2.y - pt1.y) as f64 * (pt3.x - pt2.x) as f64
}

/// Clipper2Lib::DotProduct (three points), in double.
pub fn dot_product(pt1: Point64, pt2: Point64, pt3: Point64) -> f64 {
    (pt2.x - pt1.x) as f64 * (pt3.x - pt2.x) as f64
        + (pt2.y - pt1.y) as f64 * (pt3.y - pt2.y) as f64
}

/// Clipper2Lib::PerpendicDistFromLineSqrd
pub fn perpendic_dist_from_line_sqrd(pt: Point64, line1: Point64, line2: Point64) -> f64 {
    let a = (pt.x - line1.x) as f64;
    let b = (pt.y - line1.y) as f64;
    let c = (line2.x - line1.x) as f64;
    let d = (line2.y - line1.y) as f64;
    if c == 0.0 && d == 0.0 {
        return 0.0;
    }
    let s = a * d - c * b;
    s * s / (c * c + d * d)
}

/// Clipper2Lib::GetSign (double)
fn get_sign(val: f64) -> i32 {
    if val == 0.0 {
        0
    } else if val > 0.0 {
        1
    } else {
        -1
    }
}

/// Clipper2Lib::SegmentsIntersect (not inclusive): the segments cross at a point inside both.
pub fn segments_intersect(seg1a: Point64, seg1b: Point64, seg2a: Point64, seg2b: Point64) -> bool {
    get_sign(cross_product(seg1a, seg2a, seg2b)) * get_sign(cross_product(seg1b, seg2a, seg2b)) < 0
        && get_sign(cross_product(seg2a, seg1a, seg1b))
            * get_sign(cross_product(seg2b, seg1a, seg1b))
            < 0
}

/// Clipper2Lib::GetSegmentIntersectPt (Point64, without CLIPPER2_HI_PRECISION): `t` in double along line 1, the
/// point clamped to its ends and otherwise truncated toward zero (`static_cast<int64_t>`). `None` for parallel lines.
pub fn get_segment_intersect_pt(
    ln1a: Point64,
    ln1b: Point64,
    ln2a: Point64,
    ln2b: Point64,
) -> Option<Point64> {
    let dx1 = (ln1b.x - ln1a.x) as f64;
    let dy1 = (ln1b.y - ln1a.y) as f64;
    let dx2 = (ln2b.x - ln2a.x) as f64;
    let dy2 = (ln2b.y - ln2a.y) as f64;
    let det = dy1 * dx2 - dy2 * dx1;
    if det == 0.0 {
        return None;
    }
    let t = ((ln1a.x - ln2a.x) as f64 * dy2 - (ln1a.y - ln2a.y) as f64 * dx2) / det;
    Some(if t <= 0.0 {
        ln1a
    } else if t >= 1.0 {
        ln1b
    } else {
        Point64::new(
            (ln1a.x as f64 + t * dx1) as i64,
            (ln1a.y as f64 + t * dy1) as i64,
        )
    })
}

/// Clipper2Lib::GetClosestPointOnSegment (integral): `nearbyint` (ties to even) of the offsets along the segment.
pub fn get_closest_point_on_segment(off_pt: Point64, seg1: Point64, seg2: Point64) -> Point64 {
    if seg1 == seg2 {
        return seg1;
    }
    let dx = (seg2.x - seg1.x) as f64;
    let dy = (seg2.y - seg1.y) as f64;
    let mut q =
        ((off_pt.x - seg1.x) as f64 * dx + (off_pt.y - seg1.y) as f64 * dy) / (dx * dx + dy * dy);
    q = q.clamp(0.0, 1.0);
    Point64::new(
        seg1.x + (q * dx).round_ties_even() as i64,
        seg1.y + (q * dy).round_ties_even() as i64,
    )
}

/// Clipper2Lib::PointInPolygonResult
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointInPolygonResult {
    IsOn,
    IsInside,
    IsOutside,
}

/// Clipper2Lib::PointInPolygon (Point64, Path64)
pub fn point_in_polygon(pt: Point64, polygon: &[Point64]) -> PointInPolygonResult {
    use PointInPolygonResult::*;
    if polygon.len() < 3 {
        return IsOutside;
    }
    let mut val = 0;
    let cbegin = 0usize;
    let mut cend = polygon.len();
    let mut first = 0usize;
    while first != cend && polygon[first].y == pt.y {
        first += 1;
    }
    if first == cend {
        return IsOutside;
    }
    let mut is_above = polygon[first].y < pt.y;
    let starting_above = is_above;
    let mut curr = first + 1;
    let mut prev;
    loop {
        if curr == cend {
            if cend == first || first == cbegin {
                break;
            }
            cend = first;
            curr = cbegin;
        }
        if is_above {
            while curr != cend && polygon[curr].y < pt.y {
                curr += 1;
            }
            if curr == cend {
                continue;
            }
        } else {
            while curr != cend && polygon[curr].y > pt.y {
                curr += 1;
            }
            if curr == cend {
                continue;
            }
        }
        prev = if curr == cbegin {
            polygon.len() - 1
        } else {
            curr - 1
        };
        if polygon[curr].y == pt.y {
            if polygon[curr].x == pt.x
                || (polygon[curr].y == polygon[prev].y
                    && ((pt.x < polygon[prev].x) != (pt.x < polygon[curr].x)))
            {
                return IsOn;
            }
            curr += 1;
            if curr == first {
                break;
            }
            continue;
        }
        if pt.x < polygon[curr].x && pt.x < polygon[prev].x {
            // only edges crossing on the left count
        } else if pt.x > polygon[prev].x && pt.x > polygon[curr].x {
            val = 1 - val;
        } else {
            let d = cross_product(polygon[prev], polygon[curr], pt);
            if d == 0.0 {
                return IsOn;
            }
            if (d < 0.0) == is_above {
                val = 1 - val;
            }
        }
        is_above = !is_above;
        curr += 1;
    }
    if is_above != starting_above {
        cend = polygon.len();
        if curr == cend {
            curr = cbegin;
        }
        let prev = if curr == cbegin { cend - 1 } else { curr - 1 };
        let d = cross_product(polygon[prev], polygon[curr], pt);
        if d == 0.0 {
            return IsOn;
        }
        if (d < 0.0) == is_above {
            val = 1 - val;
        }
    }
    if val == 0 {
        IsOutside
    } else {
        IsInside
    }
}

/// Clipper2Lib::Path2ContainsPath1 (paths; `clipper.h`): decided by the first two vertices of path 1 that are not on
/// path 2, then by the midpoint of path 1's bounds.
pub fn path2_contains_path1(path1: &[Point64], path2: &[Point64]) -> bool {
    use PointInPolygonResult::*;
    let mut pip = IsOn;
    for &pt in path1 {
        match point_in_polygon(pt, path2) {
            IsOutside => {
                if pip == IsOutside {
                    return false;
                }
                pip = IsOutside;
            }
            IsInside => {
                if pip == IsInside {
                    return true;
                }
                pip = IsInside;
            }
            IsOn => {}
        }
    }
    if pip != IsInside {
        return false;
    }
    let mp1 = get_bounds(path1).mid_point();
    point_in_polygon(mp1, path2) == IsInside
}
