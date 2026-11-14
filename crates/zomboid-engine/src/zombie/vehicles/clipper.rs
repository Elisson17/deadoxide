//! zombie.vehicles.Clipper: the game's JNI wrapper (`libPZClipper`, class `JNIClipper`) over a Clipper2 `Clipper64`
//! and mapbox earcut, ported from the arm64 build shipped with 42.21.0 for the calls that have callers here.
//!
//! What the native side does (read from the disassembly):
//! - `newInstance`: a default `Clipper64` (`PreserveCollinear` true, `ReverseSolution` false), an empty `PolyTree64`,
//!   an empty list of open result paths and an empty list of `JNIPolygon` (an outer ring and its holes);
//! - `clear`: `ClipperBase::Clear` (the added paths only);
//! - `addPath(n, buffer, bClip, bClosed)`: `n` float pairs read from the buffer, each `floor(v * 1000 + 0.5)` (one fused
//!   multiply-add in f32) as an int64 point; a clip path when `bClip`, otherwise a subject, open unless `bClosed`;
//! - `generatePolygons(clipType, delta, joinType)`: the polygon list and the tree are emptied, then
//!   `ExecuteInternal(clipType, Positive, polytree)` and `BuildTree64`; on success `getPolygons`: every top-level node
//!   becomes a polygon (its ring, its children as holes, the islands inside those holes as further polygons, depth
//!   first, right after their parent), then every open result path a polygon of its own. Returns their count, or -1
//!   when the clipper failed. With `delta != 0` the tree is first grown by a `ClipperOffset` (not ported here);
//! - `triangulate(i, buffer)`: earcut of polygon `i` (outer ring, then holes, indices into their points in that
//!   order), each corner written as `(float) v / 1000f`; returns the number of corners. The buffer's limit is checked
//!   before every float (`BufferOverflowException`).
use crate::native::clipper2::core::{Path64, Point64};
use crate::native::clipper2::engine::{ClipType, Clipper64, FillRule, PolyTree64};
use crate::native::earcut::earcut;

/// JNIPolygon: an outer ring and its holes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JniPolygon {
    pub outer: Path64,
    pub holes: Vec<Path64>,
}

/// zombie.vehicles.Clipper (one native `JNIClipper`).
#[derive(Debug, Default)]
pub struct Clipper {
    clipper: Clipper64,
    polygons: Vec<JniPolygon>,
}

/// A Java exception the native side raised (`NewJavaException`), by class name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JavaException(pub &'static str);

impl Clipper {
    /// zombie.vehicles.Clipper.<init> (`newInstance`)
    pub fn new() -> Self {
        Self::default()
    }

    /// zombie.vehicles.Clipper.clear
    pub fn clear(&mut self) {
        self.clipper.clear();
    }

    /// zombie.vehicles.Clipper.addPath(int, ByteBuffer, boolean, boolean): the points are the float pairs the Java side
    /// put in the buffer.
    pub fn add_path(&mut self, points: &[(f32, f32)], b_clip: bool, b_closed: bool) {
        let path: Path64 = points
            .iter()
            .map(|&(x, y)| {
                Point64::new(
                    i64::from(x.mul_add(1000.0, 0.5).floor() as i32),
                    i64::from(y.mul_add(1000.0, 0.5).floor() as i32),
                )
            })
            .collect();
        if b_clip {
            self.clipper.add_clip(&[path]);
        } else if b_closed {
            self.clipper.add_subject(&[path]);
        } else {
            self.clipper.add_open_subject(&[path]);
        }
    }

    /// zombie.vehicles.Clipper.generatePolygons(int, double, int). `Err` for what is not ported (an offset `delta`).
    pub fn generate_polygons(
        &mut self,
        clip_type: i32,
        delta: f64,
        _join_type: i32,
    ) -> Result<i32, String> {
        let ct = match clip_type {
            0 => ClipType::NoClip,
            1 => ClipType::Intersection,
            2 => ClipType::Union,
            3 => ClipType::Difference,
            4 => ClipType::Xor,
            other => {
                return Err(format!(
                    "not ported: Clipper.generatePolygons clip type {other}"
                ))
            }
        };
        if delta != 0.0 {
            return Err("not ported: Clipper.generatePolygons with an offset delta".to_string());
        }
        self.polygons.clear();
        let mut tree = PolyTree64::default();
        let mut open = Vec::new();
        if !self
            .clipper
            .execute_tree(ct, FillRule::Positive, &mut tree, &mut open)
        {
            return Ok(-1);
        }
        // JNIClipper::getPolygons(double, JoinType) with delta 0: the tree's top level, then the open paths
        for &top in tree.children(0) {
            let i = self.polygons.len();
            self.polygons.push(JniPolygon::default());
            self.collect(&tree, top, i);
        }
        for path in open {
            self.polygons.push(JniPolygon {
                outer: path,
                holes: Vec::new(),
            });
        }
        Ok(self.polygons.len() as i32)
    }

    /// JNIClipper::getPolygons(const PolyPath64&, JNIPolygon*)
    fn collect(&mut self, tree: &PolyTree64, node: usize, poly: usize) {
        self.polygons[poly].outer = tree.nodes[node].polygon.clone();
        for &hole in tree.children(node) {
            self.polygons[poly]
                .holes
                .push(tree.nodes[hole].polygon.clone());
            for &island in tree.children(hole) {
                let i = self.polygons.len();
                self.polygons.push(JniPolygon::default());
                self.collect(tree, island, i);
            }
        }
    }

    /// The polygons of the last `generate_polygons`.
    pub fn polygons(&self) -> &[JniPolygon] {
        &self.polygons
    }

    /// zombie.vehicles.Clipper.triangulate(int, ByteBuffer): the triangle corners as x, y pairs, written into a buffer
    /// of `capacity` bytes from position 0. `Err` for the exceptions the native side raises.
    pub fn triangulate(&self, index: i32, capacity: usize) -> Result<Vec<f32>, JavaException> {
        let poly = usize::try_from(index)
            .ok()
            .and_then(|i| self.polygons.get(i))
            // `std::vector::at` throws `std::out_of_range`, which nothing catches on the native side
            .ok_or(JavaException("native std::out_of_range"))?;
        let mut rings: Vec<Path64> = Vec::with_capacity(1 + poly.holes.len());
        rings.push(poly.outer.clone());
        rings.extend(poly.holes.iter().cloned());
        let points: Vec<Point64> = rings.iter().flatten().copied().collect();
        let indices = earcut(&rings);
        let mut out = Vec::with_capacity(indices.len() * 2);
        let mut pos = 0usize;
        for &i in &indices {
            let p = points[i as usize];
            for v in [p.x, p.y] {
                if capacity < pos + 4 {
                    return Err(JavaException("java/nio/BufferOverflowException"));
                }
                out.push(v as f32 / 1000.0);
                pos += 4;
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit-square zone path (the Java side's floats): one polygon, two triangles whose corners are the square's.
    #[test]
    fn a_square_path_gives_one_polygon_of_two_triangles() {
        let mut c = Clipper::new();
        c.clear();
        c.add_path(
            &[(10.0, 20.0), (14.0, 20.0), (14.0, 23.0), (10.0, 23.0)],
            false,
            true,
        );
        assert_eq!(c.generate_polygons(3, 0.0, 0), Ok(1));
        let t = c.triangulate(0, 3072).expect("triangles");
        assert_eq!(t.len(), 12);
        let mut corners: Vec<(i32, i32)> =
            t.chunks(2).map(|p| (p[0] as i32, p[1] as i32)).collect();
        corners.sort_unstable();
        corners.dedup();
        assert_eq!(corners, vec![(10, 20), (10, 23), (14, 20), (14, 23)]);
        assert_eq!(
            c.triangulate(0, 40),
            Err(JavaException("java/nio/BufferOverflowException"))
        );
        assert_eq!(
            c.triangulate(1, 3072),
            Err(JavaException("native std::out_of_range"))
        );
    }
}
