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
        let is_less_than = |o1: &Anim2DBlend, o2: &Anim2DBlend| -> bool {
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
        };
        // HullComparer.compare
        java::list_sort(&mut hull, |&a, &b| {
            if is_less_than(&blends[a], &blends[b]) {
                std::cmp::Ordering::Less
            } else if is_less_than(&blends[b], &blends[a]) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })?;
        Ok(Anim2DBlendPicker {
            hull,
            center_x,
            center_y,
        })
    }
}
