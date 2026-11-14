//! zombie.iso.zones.Zone: a named area of the world (`TownZone`, `Forest`, `Nav` roads, `ZombiesType`, spawn points,
//! zone stories). A zone is a rectangle, a point, a polygon or a polyline with a width; the meta grid registers it in
//! every 8x8 meta chunk it intersects.
use super::zone_geometry_type::ZoneGeometryType;
use crate::java_nio::ByteBuffer;
use crate::zombie::core::math::pz_math;
use crate::zombie::iso::vector2::Vector2;
use crate::zombie::pathfind::liang_barsky;
use crate::zombie::vehicles::clipper::{Clipper, JavaException};
use crate::zombie::vehicles::clipper_offset;

/// zombie.iso.IsoMetaGrid.clipperBuffer: `ByteBuffer.allocateDirect(3072)`, shared by every zone's triangulation.
const CLIPPER_BUFFER_CAPACITY: usize = 3072;

/// zombie.iso.zones.Zone.s_PreferredZoneTypes
const PREFERRED_ZONE_TYPES: [&str; 8] = [
    "DeepForest",
    "Farm",
    "FarmLand",
    "Forest",
    "Vegitation",
    "Nav",
    "TownZone",
    "TrailerPark",
];

/// zombie.iso.zones.Zone. Strings are `None` where the original holds null (an index missing from the save's string
/// map). The zone-story picks are not ported.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Zone {
    pub name: Option<String>,
    pub type_: Option<String>,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub w: i32,
    pub h: i32,
    pub geometry_type: ZoneGeometryType,
    pub polyline_width: i32,
    /// zombie.iso.zones.Zone.points: x, y pairs.
    pub points: Vec<i32>,
    /// zombie.iso.zones.Zone.polylineOutlinePoints: built on first use (`checkPolylineOutline`).
    pub polyline_outline_points: Option<Vec<f32>>,
    pub hour_last_seen: i32,
    pub have_construction: bool,
    pub last_action_timestamp: i32,
    pub original_name: Option<String>,
    /// zombie.iso.zones.Zone.id as (most, least) significant bits; `None` where the original draws `UUID.randomUUID()`
    /// (saves before world version 215), which is not modelled.
    pub id: Option<(i64, i64)>,
    pub is_preferred_zone_for_square: bool,
    /// zombie.iso.zones.Zone.triangles: the triangulation's corners as x, y pairs, made on first use.
    pub triangles: Option<Vec<f32>>,
    /// zombie.iso.zones.Zone.triangleAreas
    pub triangle_areas: Option<Vec<f32>>,
    /// zombie.iso.zones.Zone.totalArea
    pub total_area: f32,
    /// zombie.iso.zones.Zone.triangulateFailed
    pub triangulate_failed: bool,
}

/// org.joml.Vector2f.length(float, float): `(float) Math.sqrt(x * x + y * y)`, the sum in float.
fn vector2f_length(x: f32, y: f32) -> f32 {
    f64::from(x * x + y * y).sqrt() as f32
}

/// zombie.iso.zones.Zone.triangleArea: Heron's formula in float, the root in double.
fn triangle_area(x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let a = vector2f_length(x1 - x0, y1 - y0);
    let b = vector2f_length(x2 - x1, y2 - y1);
    let c = vector2f_length(x0 - x2, y0 - y2);
    let s = (a + b + c) / 2.0;
    f64::from(s * (s - a) * (s - b) * (s - c)).sqrt() as f32
}

/// zombie.iso.zones.Zone.isLeft
fn is_left(x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0)
}

/// The loop of zombie.iso.zones.Zone.isPointInPolygon_WindingNumber / isPointInPolyline_WindingNumber over a closed
/// ring of `n` points, point `i` given by `pt(i)`. True for `PolygonHit.Inside` (winding number not zero).
fn winding_inside(n: usize, pt: impl Fn(usize) -> (f32, f32), x: f32, y: f32) -> bool {
    let mut wn = 0;
    for i in 0..n {
        let (x1, y1) = pt(i);
        let (x2, y2) = pt((i + 1) % n);
        if y1 <= y {
            if y2 > y && is_left(x1, y1, x2, y2, x, y) > 0.0 {
                wn += 1;
            }
        } else if y2 <= y && is_left(x1, y1, x2, y2, x, y) < 0.0 {
            wn -= 1;
        }
    }
    wn != 0
}

/// The edge loop of zombie.iso.zones.Zone.lineSegmentIntersects / polylineOutlineSegmentIntersects: whether the
/// segment (sx, sy)-(ex, ey) crosses an edge of the ring, in the original's float arithmetic.
fn segment_crosses_ring(
    n: usize,
    pt: impl Fn(usize) -> (f32, f32),
    sx: f32,
    sy: f32,
    ex: f32,
    ey: f32,
) -> bool {
    // static Vector2 L_lineSegmentIntersects
    let mut dir = Vector2::new(ex - sx, ey - sy);
    let line_segment_length = dir.get_length();
    dir.normalize();
    let (dir_x, dir_y) = (dir.x, dir.y);
    for j in 0..n {
        let (node1x, node1y) = pt(j);
        let (node2x, node2y) = pt((j + 1) % n);
        let doa_x = sx - node1x;
        let doa_y = sy - node1y;
        let dba_x = node2x - node1x;
        let dba_y = node2y - node1y;
        let inv_dba_dir = 1.0 / (dba_y * dir_x - dba_x * dir_y);
        let t = (dba_x * doa_y - dba_y * doa_x) * inv_dba_dir;
        if t >= 0.0 && t <= line_segment_length {
            let t2 = (doa_y * dir_x - doa_x * dir_y) * inv_dba_dir;
            if (0.0..=1.0).contains(&t2) {
                return true;
            }
        }
    }
    false
}

impl Zone {
    /// zombie.iso.zones.Zone(String, String, int, int, int, int, int) without the random id (`UUID.randomUUID()`).
    pub fn new(name: &str, type_: &str, x: i32, y: i32, z: i32, w: i32, h: i32) -> Self {
        Zone {
            name: Some(name.to_string()),
            type_: Some(type_.to_string()),
            original_name: Some(name.to_string()),
            x,
            y,
            z,
            w,
            h,
            ..Zone::default()
        }
    }

    /// zombie.iso.zones.Zone.load(ByteBuffer, int, Map, SharedStrings): name and type as indices into the file's
    /// string map, the geometry (`loadData`), the original name's index and, from world version 215, the id.
    pub fn load(
        b: &mut ByteBuffer,
        world_version: i32,
        string_map: &[String],
    ) -> Result<Self, String> {
        let lookup = |i: i16| {
            usize::try_from(i)
                .ok()
                .and_then(|i| string_map.get(i).cloned())
        };
        let mut zone = Zone {
            name: lookup(b.get_short()?),
            type_: lookup(b.get_short()?),
            ..Zone::default()
        };
        zone.load_data(b)?;
        zone.original_name = lookup(b.get_short()?);
        if world_version >= 215 {
            // GameWindow.ReadUUID: new UUID(getLong(), getLong())
            zone.id = Some((b.get_long()?, b.get_long()?));
        } else {
            b.get_double()?;
            zone.id = None;
        }
        Ok(zone)
    }

    /// zombie.iso.zones.Zone.loadData
    fn load_data(&mut self, b: &mut ByteBuffer) -> Result<(), String> {
        self.x = b.get_int()?;
        self.y = b.get_int()?;
        self.z = i32::from(b.get()?);
        self.w = b.get_int()?;
        self.h = b.get_int()?;
        let mut polyline_width = 0;
        let ordinal = i32::from(b.get()?);
        self.geometry_type = ZoneGeometryType::from_ordinal(if (0..4).contains(&ordinal) {
            ordinal
        } else {
            0
        });
        if self.geometry_type != ZoneGeometryType::Invalid {
            if self.geometry_type == ZoneGeometryType::Polyline {
                polyline_width = pz_math::clamp_i32(i32::from(b.get()?), 0, 255);
            }
            let num_points = b.get_short()?;
            for _ in 0..num_points {
                self.points.push(b.get_int()?);
            }
            self.polyline_width = polyline_width;
        }
        self.is_preferred_zone_for_square =
            Self::is_preferred_zone_for_square(self.type_.as_deref());
        self.hour_last_seen = b.get_int()?;
        self.have_construction = b.get()? != 0;
        self.last_action_timestamp = b.get_int()?;
        Ok(())
    }

    /// zombie.iso.zones.Zone.isPreferredZoneForSquare
    pub fn is_preferred_zone_for_square(type_: Option<&str>) -> bool {
        type_.is_some_and(|t| PREFERRED_ZONE_TYPES.contains(&t))
    }

    /// zombie.iso.zones.Zone.isPoint
    pub fn is_point(&self) -> bool {
        self.geometry_type == ZoneGeometryType::Point
    }
    /// zombie.iso.zones.Zone.isPolygon
    pub fn is_polygon(&self) -> bool {
        self.geometry_type == ZoneGeometryType::Polygon
    }
    /// zombie.iso.zones.Zone.isPolyline
    pub fn is_polyline(&self) -> bool {
        self.geometry_type == ZoneGeometryType::Polyline
    }
    /// zombie.iso.zones.Zone.isRectangle
    pub fn is_rectangle(&self) -> bool {
        self.geometry_type == ZoneGeometryType::Invalid
    }

    /// zombie.iso.zones.Zone.contains: the square's centre, for polygons and wide polylines by winding number.
    pub fn contains(&mut self, x: i32, y: i32, z: i32) -> bool {
        if z != self.z
            || x < self.x
            || x >= self.x + self.w
            || y < self.y
            || y >= self.y + self.h
            || self.is_point()
        {
            false
        } else if self.is_polyline() {
            if self.polyline_width > 0 {
                self.check_polyline_outline();
                self.is_point_in_polyline_winding_number(x as f32 + 0.5, y as f32 + 0.5)
            } else {
                false
            }
        } else if self.is_polygon() {
            self.is_point_in_polygon_winding_number(x as f32 + 0.5, y as f32 + 0.5)
        } else {
            true
        }
    }

    /// zombie.iso.zones.Zone.intersects: the bounding box first, then the shape (`z == i32::MAX` matches any level).
    pub fn intersects(&mut self, x: i32, y: i32, z: i32, w: i32, h: i32) -> bool {
        if z != i32::MAX && self.z != z {
            return false;
        }
        if !(x + w > self.x && x < self.x + self.w) {
            return false;
        }
        if y + h <= self.y || y >= self.y + self.h {
            return false;
        }
        if self.is_polygon() {
            self.polygon_rect_intersect(x, y, w, h)
        } else if self.is_polyline() {
            if self.polyline_width > 0 {
                self.check_polyline_outline();
                self.polyline_outline_rect_intersect(x, y, w, h)
            } else {
                let p = &self.points;
                let mut i = 0;
                while (i as i64) < p.len() as i64 - 2 {
                    let (x1, y1, x2, y2) = (p[i], p[i + 1], p[i + 2], p[i + 3]);
                    if liang_barsky::line_rect_intersect(
                        x1 as f32,
                        y1 as f32,
                        (x2 - x1) as f32,
                        (y2 - y1) as f32,
                        x as f32,
                        y as f32,
                        (x + w) as f32,
                        (y + h) as f32,
                    )
                    .is_some()
                    {
                        return true;
                    }
                    i += 2;
                }
                false
            }
        } else {
            true
        }
    }

    /// zombie.iso.zones.Zone.getClippedSegmentOfPolyline: the first segment (offset by half a square for odd widths)
    /// that crosses the rectangle, with its entry and exit parameters.
    pub fn get_clipped_segment_of_polyline(
        &self,
        clip_x1: i32,
        clip_y1: i32,
        clip_x2: i32,
        clip_y2: i32,
    ) -> Option<(usize, f64, f64)> {
        if !self.is_polyline() {
            return None;
        }
        let dxy = if self.polyline_width % 2 == 0 {
            0.0
        } else {
            0.5
        };
        let p = &self.points;
        let mut i = 0;
        while (i as i64) < p.len() as i64 - 2 {
            let (x1, y1, x2, y2) = (p[i], p[i + 1], p[i + 2], p[i + 3]);
            if let Some((t1, t2)) = liang_barsky::line_rect_intersect(
                x1 as f32 + dxy,
                y1 as f32 + dxy,
                (x2 - x1) as f32,
                (y2 - y1) as f32,
                clip_x1 as f32,
                clip_y1 as f32,
                clip_x2 as f32,
                clip_y2 as f32,
            ) {
                return Some((i / 2, t1, t2));
            }
            i += 2;
        }
        None
    }

    /// zombie.iso.zones.Zone.checkPolylineOutline: the outline of a wide polyline, made once by the native
    /// ClipperOffset (points shifted by half a square for odd widths, miter joins, butt ends, half the width as
    /// delta). When the native side yields nothing the original logs "Failed to generate polyline outline" and tries
    /// again on the next call; the result would be the same, so it is not cached either.
    pub fn check_polyline_outline(&mut self) {
        if self.polyline_outline_points.is_some() || !self.is_polyline() || self.polyline_width <= 0
        {
            return;
        }
        let dxy = if self.polyline_width % 2 == 0 {
            0.0f32
        } else {
            0.5
        };
        let pts: Vec<(f32, f32)> = self
            .points
            .chunks_exact(2)
            .map(|c| (c[0] as f32 + dxy, c[1] as f32 + dxy))
            .collect();
        self.polyline_outline_points =
            clipper_offset::polyline_outline(&pts, self.polyline_width as f32 / 2.0);
    }

    /// zombie.iso.zones.Zone.isPointInPolygon_WindingNumber
    pub fn is_point_in_polygon_winding_number(&self, x: f32, y: f32) -> bool {
        let p = &self.points;
        winding_inside(
            p.len() / 2,
            |i| (p[i * 2] as f32, p[i * 2 + 1] as f32),
            x,
            y,
        )
    }

    /// zombie.iso.zones.Zone.isPointInPolyline_WindingNumber: `Outside` while there is no outline.
    pub fn is_point_in_polyline_winding_number(&self, x: f32, y: f32) -> bool {
        let Some(p) = &self.polyline_outline_points else {
            return false;
        };
        winding_inside(p.len() / 2, |i| (p[i * 2], p[i * 2 + 1]), x, y)
    }

    /// zombie.iso.zones.Zone.polygonRectIntersect
    fn polygon_rect_intersect(&self, x: i32, y: i32, w: i32, h: i32) -> bool {
        if self.x >= x && self.x + self.w <= x + w && self.y >= y && self.y + self.h <= y + h {
            return true;
        }
        self.line_segment_intersects(x as f32, y as f32, (x + w) as f32, y as f32)
            || self.line_segment_intersects(
                (x + w) as f32,
                y as f32,
                (x + w) as f32,
                (y + h) as f32,
            )
            || self.line_segment_intersects(
                (x + w) as f32,
                (y + h) as f32,
                x as f32,
                (y + h) as f32,
            )
            || self.line_segment_intersects(x as f32, (y + h) as f32, x as f32, y as f32)
    }

    /// zombie.iso.zones.Zone.lineSegmentIntersects: an edge of the polygon crosses the segment, or the segment's
    /// midpoint is inside.
    fn line_segment_intersects(&self, sx: f32, sy: f32, ex: f32, ey: f32) -> bool {
        let p = &self.points;
        let pt = |i: usize| (p[i * 2] as f32, p[i * 2 + 1] as f32);
        segment_crosses_ring(p.len() / 2, pt, sx, sy, ex, ey)
            || self.is_point_in_polygon_winding_number((sx + ex) / 2.0, (sy + ey) / 2.0)
    }

    /// zombie.iso.zones.Zone.polylineOutlineRectIntersect
    fn polyline_outline_rect_intersect(&self, x: i32, y: i32, w: i32, h: i32) -> bool {
        if self.polyline_outline_points.is_none() {
            return false;
        }
        if self.x >= x && self.x + self.w <= x + w && self.y >= y && self.y + self.h <= y + h {
            return true;
        }
        self.polyline_outline_segment_intersects(x as f32, y as f32, (x + w) as f32, y as f32)
            || self.polyline_outline_segment_intersects(
                (x + w) as f32,
                y as f32,
                (x + w) as f32,
                (y + h) as f32,
            )
            || self.polyline_outline_segment_intersects(
                (x + w) as f32,
                (y + h) as f32,
                x as f32,
                (y + h) as f32,
            )
            || self.polyline_outline_segment_intersects(
                x as f32,
                (y + h) as f32,
                x as f32,
                y as f32,
            )
    }

    /// zombie.iso.zones.Zone.polylineOutlineSegmentIntersects
    fn polyline_outline_segment_intersects(&self, sx: f32, sy: f32, ex: f32, ey: f32) -> bool {
        let Some(p) = &self.polyline_outline_points else {
            return false;
        };
        let pt = |i: usize| (p[i * 2], p[i * 2 + 1]);
        segment_crosses_ring(p.len() / 2, pt, sx, sy, ex, ey)
            || self.is_point_in_polyline_winding_number((sx + ex) / 2.0, (sy + ey) / 2.0)
    }

    /// zombie.iso.zones.Zone.getTotalArea: the triangulated area of a polygon or wide polyline (triangulating it on
    /// first use), otherwise `width * height` (int multiplication). `Err` for an exception the triangulation raises.
    pub fn get_total_area(&mut self, clipper: &mut Clipper) -> Result<f32, JavaException> {
        if !self.is_rectangle()
            && !self.is_point()
            && (!self.is_polyline() || self.polyline_width > 0)
        {
            self.get_polygon_triangles(clipper)?;
            self.get_polyline_outline_triangles(clipper)?;
            Ok(self.total_area)
        } else {
            Ok(self.w.wrapping_mul(self.h) as f32)
        }
    }

    /// zombie.iso.zones.Zone.isClockwise: polygons only; the int products summed in float.
    fn is_clockwise(&self) -> bool {
        if !self.is_polygon() {
            return false;
        }
        let p = &self.points;
        let n = p.len();
        let mut sum = 0.0f32;
        let mut i = 0;
        while i < n {
            let (p1x, p1y) = (p[i], p[i + 1]);
            let (p2x, p2y) = (p[(i + 2) % n], p[(i + 3) % n]);
            sum += p2x.wrapping_sub(p1x).wrapping_mul(p2y.wrapping_add(p1y)) as f32;
            i += 2;
        }
        f64::from(sum) > 0.0
    }

    /// zombie.iso.zones.Zone.getPolygonTriangles
    pub fn get_polygon_triangles(
        &mut self,
        clipper: &mut Clipper,
    ) -> Result<Option<&[f32]>, JavaException> {
        if self.triangles.is_some() {
            return Ok(self.triangles.as_deref());
        }
        if self.triangulate_failed || !self.is_polygon() {
            return Ok(None);
        }
        let pts: Vec<f32> = self.points.iter().map(|&v| v as f32).collect();
        self.triangulate_points(clipper, &pts)
    }

    /// zombie.iso.zones.Zone.getPolylineOutlineTriangles
    pub fn get_polyline_outline_triangles(
        &mut self,
        clipper: &mut Clipper,
    ) -> Result<Option<&[f32]>, JavaException> {
        if self.triangles.is_some() {
            return Ok(self.triangles.as_deref());
        }
        if !self.is_polyline() || self.polyline_width <= 0 || self.triangulate_failed {
            return Ok(None);
        }
        self.check_polyline_outline();
        let Some(points) = self.polyline_outline_points.clone() else {
            self.triangulate_failed = true;
            return Ok(None);
        };
        self.triangulate_points(clipper, &points)
    }

    /// The common tail of getPolygonTriangles / getPolylineOutlineTriangles: the points go into the shared clipper
    /// buffer (pairs in reverse order when the polygon is clockwise), the native Clipper unions them
    /// (`generatePolygons()`: difference, no offset) and triangulates its first polygon; then `initTriangleAreas`.
    fn triangulate_points(
        &mut self,
        clipper: &mut Clipper,
        pts: &[f32],
    ) -> Result<Option<&[f32]>, JavaException> {
        if pts.len() * 4 > CLIPPER_BUFFER_CAPACITY {
            return Err(JavaException("java/nio/BufferOverflowException"));
        }
        let pairs: Vec<(f32, f32)> = if self.is_clockwise() {
            // for (i = size - 1; i > 0; i -= 2) put(points[i - 1]), put(points[i])
            pts.chunks_exact(2).rev().map(|c| (c[0], c[1])).collect()
        } else {
            pts.chunks_exact(2).map(|c| (c[0], c[1])).collect()
        };
        clipper.clear();
        clipper.add_path(&pairs, false, true);
        let num_polys = clipper
            .generate_polygons(3, 0.0, 0)
            .map_err(|_| JavaException("not ported"))?;
        if num_polys < 1 {
            self.triangulate_failed = true;
            return Ok(None);
        }
        self.triangles = Some(clipper.triangulate(0, CLIPPER_BUFFER_CAPACITY)?);
        self.init_triangle_areas();
        Ok(self.triangles.as_deref())
    }

    /// zombie.iso.zones.Zone.initTriangleAreas
    fn init_triangle_areas(&mut self) {
        let t = self.triangles.as_deref().unwrap_or(&[]);
        let mut areas = vec![0.0f32; t.len() / 6];
        let mut total = 0.0f32;
        for (k, c) in t.chunks_exact(6).enumerate() {
            let area = triangle_area(c[0], c[1], c[2], c[3], c[4], c[5]);
            areas[k] = area;
            total += area;
        }
        self.triangle_areas = Some(areas);
        self.total_area = total;
    }

    /// zombie.iso.zones.Zone.Dispose
    pub fn dispose(&mut self) {
        self.points.clear();
        self.polyline_outline_points = None;
        self.triangles = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn polygon(points: &[i32]) -> Zone {
        let (xs, ys): (Vec<i32>, Vec<i32>) = points.chunks(2).map(|c| (c[0], c[1])).unzip();
        let (x, y) = (
            *xs.iter().min().unwrap_or(&0),
            *ys.iter().min().unwrap_or(&0),
        );
        let mut z = Zone::new(
            "",
            "Forest",
            x,
            y,
            0,
            xs.iter().max().unwrap_or(&0) - x,
            ys.iter().max().unwrap_or(&0) - y,
        );
        z.geometry_type = ZoneGeometryType::Polygon;
        z.points = points.to_vec();
        z
    }

    /// A right triangle with legs of 16 along the axes: the 8x8 chunk at its right angle and the one on the
    /// hypotenuse intersect it; the chunk beyond the hypotenuse does not.
    #[test]
    fn polygon_zone_intersects_chunks_by_shape() {
        let mut t = polygon(&[0, 0, 16, 0, 0, 16]);
        assert!(t.intersects(0, 0, 0, 8, 8));
        assert!(t.intersects(8, 0, 0, 8, 8));
        assert!(!t.intersects(9, 9, 0, 6, 6));
        assert!(!t.intersects(0, 0, 1, 8, 8));
        assert!(t.contains(1, 1, 0));
        assert!(!t.contains(14, 14, 0));
    }

    /// A road 4 squares wide from x 0 to 40 along y 10 covers the squares y 8..11 (even width: no half-square
    /// shift); its zero-width twin only intersects boxes its centre line crosses.
    #[test]
    fn polyline_zone_uses_its_outline() {
        let mut road = Zone::new("", "Nav", 0, 8, 0, 40, 4);
        road.geometry_type = ZoneGeometryType::Polyline;
        road.polyline_width = 4;
        road.points = vec![0, 10, 40, 10];
        assert!(road.contains(5, 8, 0));
        assert!(road.contains(5, 11, 0));
        assert!(!road.contains(5, 12, 0));
        assert_eq!(road.polyline_outline_points.as_ref().map(Vec::len), Some(8));
        let mut line = road.clone();
        line.polyline_width = 0;
        line.polyline_outline_points = None;
        assert!(line.intersects(8, 8, 0, 8, 8));
        assert!(!line.intersects(8, 11, 0, 8, 1));
    }
}
