//! zombie.core.skinnedmodel.advancedanimation.Anim2DBlendPicker
use super::anim_2d_blend::Anim2DBlend;
use super::anim_2d_blend_triangle::Anim2DBlendTriangle;
use crate::java;

/// zombie.core.skinnedmodel.advancedanimation.Anim2DBlendPicker: the node's triangles (`tris`, the node's own list)
/// and the hull of the blend space (indices into the node's `blends2d`) with the comparer's centre.
#[derive(Clone, Debug, PartialEq)]
pub struct Anim2DBlendPicker {
    pub hull: Vec<usize>,
    /// `HullComparer.centerX` / `centerY`: the hull's mean position times 1000, as ints.
    pub center_x: i32,
    pub center_y: i32,
}

/// Anim2DBlendPicker.HullComparer.isLessThan: by angle around the centre (ints of position times 1000).
fn is_less_than(center_x: i32, center_y: i32, o1: &Anim2DBlend, o2: &Anim2DBlend) -> bool {
    let o1x = (o1.pos_x * 1000.0) as i32;
    let o1y = (o1.pos_y * 1000.0) as i32;
    let o2x = (o2.pos_x * 1000.0) as i32;
    let o2y = (o2.pos_y * 1000.0) as i32;
    let v1x = o1x.wrapping_sub(center_x);
    let v1y = o1y.wrapping_sub(center_y);
    let v2x = o2x.wrapping_sub(center_x);
    let v2y = o2y.wrapping_sub(center_y);
    if v1y == 0 && v1x > 0 {
        true
    } else if v2y == 0 && v2x > 0 {
        false
    } else if v1y > 0 && v2y < 0 {
        true
    } else if v1y < 0 && v2y > 0 {
        false
    } else {
        v1x.wrapping_mul(v2y).wrapping_sub(v1y.wrapping_mul(v2x)) > 0
    }
}

/// Anim2DBlendPicker.HullComparer.compare
fn hull_compare(
    center_x: i32,
    center_y: i32,
    o1: &Anim2DBlend,
    o2: &Anim2DBlend,
) -> std::cmp::Ordering {
    if is_less_than(center_x, center_y, o1, o2) {
        std::cmp::Ordering::Less
    } else if is_less_than(center_x, center_y, o2, o1) {
        std::cmp::Ordering::Greater
    } else {
        std::cmp::Ordering::Equal
    }
}

/// zombie.core.skinnedmodel.advancedanimation.Anim2DBlendPicker.PickResults (blends as indices of the node's list).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PickResults {
    pub num_nodes: i32,
    pub node1: Option<usize>,
    pub node2: Option<usize>,
    pub node3: Option<usize>,
    pub scale1: f32,
    pub scale2: f32,
    pub scale3: f32,
}

/// Anim2DBlendTriangle.sign (float arithmetic, widened to double).
fn sign(p1x: f32, p1y: f32, p2x: f32, p2y: f32, p3x: f32, p3y: f32) -> f64 {
    f64::from((p1x - p3x) * (p2y - p3y) - (p2x - p3x) * (p1y - p3y))
}

/// Anim2DBlendTriangle.PointInTriangle
#[allow(clippy::too_many_arguments)]
fn point_in_triangle(
    pt_x: f32,
    pt_y: f32,
    v1x: f32,
    v1y: f32,
    v2x: f32,
    v2y: f32,
    v3x: f32,
    v3y: f32,
) -> bool {
    let b1 = sign(pt_x, pt_y, v1x, v1y, v2x, v2y) < 0.0;
    let b2 = sign(pt_x, pt_y, v2x, v2y, v3x, v3y) < 0.0;
    let b3 = sign(pt_x, pt_y, v3x, v3y, v1x, v1y) < 0.0;
    b1 == b2 && b2 == b3
}

/// zombie.core.skinnedmodel.advancedanimation.Anim2DBlendPicker.ProjectPointToLine
fn project_point_to_line(
    x: f32,
    y: f32,
    start_x: f32,
    start_y: f32,
    end_x: f32,
    end_y: f32,
) -> f32 {
    let line_to_pos_x = x - start_x;
    let line_to_pos_y = y - start_y;
    let line_to_end_x = end_x - start_x;
    let line_to_end_y = end_y - start_y;
    (line_to_end_x * line_to_pos_x + line_to_end_y * line_to_pos_y)
        / (line_to_end_x * line_to_end_x + line_to_end_y * line_to_end_y)
}

/// zombie.core.skinnedmodel.advancedanimation.Anim2DBlendPicker.Edge: the two blends ordered by x, then y.
fn edge(a: usize, b: usize, blends: &[Anim2DBlend]) -> (usize, usize) {
    let (pa, pb) = (&blends[a], &blends[b]);
    let swap = if pa.pos_x != pb.pos_x {
        pa.pos_x > pb.pos_x
    } else {
        pa.pos_y > pb.pos_y
    };
    if swap {
        (b, a)
    } else {
        (a, b)
    }
}

impl Anim2DBlendPicker {
    /// zombie.core.skinnedmodel.advancedanimation.Anim2DBlendPicker.SetPickTriangles with `BuildHull`: every edge of the
    /// triangles counted (edges are equal when they join the same two blends), the blends of the edges used once form
    /// the hull, which is sorted around its mean by `HullComparer` (`List.sort`). The original collects the hull in a
    /// HashSet of identity hashes, so its order before the sort (and the float sum of the mean) is not reproducible;
    /// here it is ascending blend index. `Err` is the NullPointerException of a triangle corner no blend has.
    pub fn set_pick_triangles(
        tris: &[Anim2DBlendTriangle],
        blends: &[Anim2DBlend],
    ) -> Result<Anim2DBlendPicker, String> {
        let npe = || "java.lang.NullPointerException: Anim2DBlendPicker.Edge".to_string();
        let mut edges: Vec<((usize, usize), i32)> = Vec::new();
        for tri in tris {
            let (n1, n2, n3) = (
                tri.node1.ok_or_else(npe)?,
                tri.node2.ok_or_else(npe)?,
                tri.node3.ok_or_else(npe)?,
            );
            for (a, b) in [(n1, n2), (n2, n3), (n3, n1)] {
                let e = edge(a, b, blends);
                match edges.iter_mut().find(|(k, _)| *k == e) {
                    Some((_, count)) => *count += 1,
                    None => edges.push((e, 1)),
                }
            }
        }
        let mut hull: Vec<usize> = Vec::new();
        for ((a, b), count) in &edges {
            if *count == 1 {
                for n in [*a, *b] {
                    if !hull.contains(&n) {
                        hull.push(n);
                    }
                }
            }
        }
        hull.sort_unstable();
        let (mut cx, mut cy) = (0.0f32, 0.0f32);
        for &n in &hull {
            cx += blends[n].pos_x;
            cy += blends[n].pos_y;
        }
        cx /= hull.len() as f32;
        cy /= hull.len() as f32;
        let center_x = (cx * 1000.0) as i32;
        let center_y = (cy * 1000.0) as i32;
        // HullComparer.compare
        java::list_sort(&mut hull, |&a, &b| {
            hull_compare(center_x, center_y, &blends[a], &blends[b])
        })?;
        Ok(Anim2DBlendPicker {
            hull,
            center_x,
            center_y,
        })
    }

    /// zombie.core.skinnedmodel.advancedanimation.Anim2DBlendPicker.Pick: the triangle that holds (x, y) with the
    /// barycentric weights, else (the point scaled by 1.1) the hull edge it projects onto: one blend or two. The hull's
    /// order is the one `set_pick_triangles` has (the original's is not reproducible before its sort: UNKNOWN for
    /// blends the comparer ties).
    pub fn pick(
        &self,
        x: f32,
        y: f32,
        tris: &[Anim2DBlendTriangle],
        blends: &[Anim2DBlend],
    ) -> Result<PickResults, String> {
        let npe = || "java.lang.NullPointerException: Anim2DBlendPicker.Pick".to_string();
        let mut result = PickResults::default();
        for tri in tris {
            let (n1, n2, n3) = (
                tri.node1.ok_or_else(npe)?,
                tri.node2.ok_or_else(npe)?,
                tri.node3.ok_or_else(npe)?,
            );
            let (b1, b2, b3) = (&blends[n1], &blends[n2], &blends[n3]);
            if point_in_triangle(
                x, y, b1.pos_x, b1.pos_y, b2.pos_x, b2.pos_y, b3.pos_x, b3.pos_y,
            ) {
                result.num_nodes = 3;
                result.node1 = Some(n1);
                result.node2 = Some(n2);
                result.node3 = Some(n3);
                let (v1x, v1y, v2x, v2y, v3x, v3y) =
                    (b1.pos_x, b1.pos_y, b2.pos_x, b2.pos_y, b3.pos_x, b3.pos_y);
                result.scale1 = ((v2y - v3y) * (x - v3x) + (v3x - v2x) * (y - v3y))
                    / ((v2y - v3y) * (v1x - v3x) + (v3x - v2x) * (v1y - v3y));
                result.scale2 = ((v3y - v1y) * (x - v3x) + (v1x - v3x) * (y - v3y))
                    / ((v2y - v3y) * (v1x - v3x) + (v3x - v2x) * (v1y - v3y));
                result.scale3 = 1.0 - result.scale1 - result.scale2;
                return Ok(result);
            }
        }
        let x = x * 1.1;
        let y = y * 1.1;
        let dummy = Anim2DBlend {
            pos_x: x,
            pos_y: y,
            ..Anim2DBlend::default()
        };
        // LowerBoundIdx
        let (mut lower, mut upper) = (0usize, self.hull.len());
        while lower != upper {
            let mid = (lower + upper) / 2;
            if hull_compare(
                self.center_x,
                self.center_y,
                &dummy,
                &blends[self.hull[mid]],
            ) == std::cmp::Ordering::Less
            {
                upper = mid;
            } else {
                lower = mid + 1;
            }
        }
        let mut idx = lower;
        if idx == self.hull.len() {
            idx = 0;
        }
        let idx2 = if idx > 0 {
            idx - 1
        } else {
            self.hull.len() - 1
        };
        let n1 = *self.hull.get(idx).ok_or_else(npe)?;
        let n2 = *self.hull.get(idx2).ok_or_else(npe)?;
        let s = project_point_to_line(
            x,
            y,
            blends[n1].pos_x,
            blends[n1].pos_y,
            blends[n2].pos_x,
            blends[n2].pos_y,
        );
        if s < 0.0 {
            result.num_nodes = 1;
            result.node1 = Some(n1);
            result.scale1 = 1.0;
        } else if s > 1.0 {
            result.num_nodes = 1;
            result.node1 = Some(n2);
            result.scale1 = 1.0;
        } else {
            result.num_nodes = 2;
            result.node1 = Some(n1);
            result.node2 = Some(n2);
            result.scale1 = 1.0 - s;
            result.scale2 = s;
        }
        Ok(result)
    }
}
