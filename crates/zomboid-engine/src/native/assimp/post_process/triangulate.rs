//! assimp `code/PostProcessing/TriangulateProcess.cpp`: `TriangulateProcess::TriangulateMesh`, ported from the x86-64
//! slice of the game's `libjassimp` (the 5.3 line: quads pick the fan start at their concave vertex, polygons with
//! five or more vertices are ear-clipped on a plane projection, and the triangles are rotated so that two consecutive
//! output triangles never start with the same vertex, the NGON encoding).
//!
//! The arithmetic follows the assimp source (`aiVector3D::Length` and `operator*` add x, y, z; the decompiler lists
//! the z term first because it is loaded first) with the single and double precision of the compiled code: the
//! vector math of the quad test is `float`, the ear tests (areas) are `double` on the `float` coordinates.
use crate::native::assimp::scene::{Mesh, Scene, Vec3, PRIMITIVE_POLYGON};

/// `aiPrimitiveType_TRIANGLE | aiPrimitiveType_POLYGON | aiPrimitiveType_NGONEncodingFlag`
const TRIANGLE_POLYGON_NGON: u32 = 4 | 8 | 16;
/// `1e-6f` promoted to `double` for the area tests of the compiled code
const AREA_EPSILON: f64 = 9.999999974752427e-07;

/// `aiVector3D::Normalize` of the compiled code: the reciprocal of the length multiplies the components, a vector of
/// length 0 stays as it is
fn normalize(v: Vec3) -> Vec3 {
    let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    if len == 0.0 {
        return v;
    }
    let inv = 1.0f32 / len;
    Vec3::new(v.x * inv, v.y * inv, v.z * inv)
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

fn component(v: Vec3, i: usize) -> f32 {
    match i {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

/// `GetArea2D(v1, v2, v3)`: half the cross product, in `double`
fn area(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f64 {
    let (ax, ay) = (f64::from(a[0]), f64::from(a[1]));
    let (bx, by) = (f64::from(b[0]), f64::from(b[1]));
    let (cx, cy) = (f64::from(c[0]), f64::from(c[1]));
    (ax * (cy - by) + bx * (ay - cy) + cx * (by - ay)) * 0.5
}

/// the three-way test of `PointInTriangle2D` as compiled: every edge area is positive or nearly zero
fn point_in_triangle(p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], v: [f32; 2]) -> bool {
    let (p0x, p0y) = (f64::from(p0[0]), f64::from(p0[1]));
    let (p1x, p1y) = (f64::from(p1[0]), f64::from(p1[1]));
    let (p2x, p2y) = (f64::from(p2[0]), f64::from(p2[1]));
    let (vx, vy) = (f64::from(v[0]), f64::from(v[1]));
    let a = ((vy - p0y) * p1x + (p1y - vy) * p0x + (p0y - p1y) * vx) * 0.5;
    let ok_a = a > 0.0 || a.abs() < AREA_EPSILON;
    let b = ((vy - p2y) * p0x + vx * (p2y - p0y) + (p0y - vy) * p2x) * 0.5;
    let c = ((vy - p1y) * p2x + vx * (p1y - p2y) + (p2y - vy) * p1x) * 0.5;
    ok_a && (c.abs() < AREA_EPSILON || c > 0.0) && (b.abs() < AREA_EPSILON || b > 0.0)
}

/// An output triangle starting with the vertex the previous output triangle started with is rotated to
/// `(i2, i0, i1)`; `last` follows (`-1` before the first triangle of a mesh)
fn fix_first(tri: &mut [u32; 3], last: &mut i64) {
    if i64::from(tri[0]) == *last {
        *tri = [tri[2], tri[0], tri[1]];
    }
    *last = i64::from(tri[0]);
}

/// `TriangulateProcess::TriangulateMesh`; `false` when the mesh has nothing to do
fn triangulate_mesh(mesh: &mut Mesh) -> bool {
    if mesh.primitive_types == 0 {
        if mesh.faces.iter().all(|f| f.len() == 3) {
            return false;
        }
    } else if mesh.primitive_types & PRIMITIVE_POLYGON == 0 {
        return false;
    }
    mesh.primitive_types = (mesh.primitive_types & !TRIANGLE_POLYGON_NGON) | 4 | 16;
    let faces = std::mem::take(&mut mesh.faces);
    let verts = &mesh.vertices;
    let mut out: Vec<Vec<u32>> = Vec::with_capacity(faces.len());
    let mut last: i64 = -1;
    for face in faces {
        let num = face.len();
        if num <= 3 {
            // a point, a line or a triangle is copied
            if num == 3 {
                let mut t = [face[0], face[1], face[2]];
                fix_first(&mut t, &mut last);
                out.push(t.to_vec());
            } else {
                out.push(face);
            }
        } else if num == 4 {
            // a quad has at most one concave vertex, the fan starts there
            let mut start = 0usize;
            for i in 0..4usize {
                let v0 = verts[face[(i + 3) % 4] as usize];
                let v1 = verts[face[(i + 2) % 4] as usize];
                let v2 = verts[face[(i + 1) % 4] as usize];
                let v = verts[face[i] as usize];
                let left = normalize(sub(v0, v));
                let diag = normalize(sub(v1, v));
                let right = normalize(sub(v2, v));
                let angle = dot(left, diag).acos() + dot(right, diag).acos();
                if angle > std::f32::consts::PI {
                    start = i;
                    break;
                }
            }
            let t = [face[0], face[1], face[2], face[3]];
            let mut first = [t[start], t[(start + 1) % 4], t[(start + 2) % 4]];
            let mut second = [t[start], t[(start + 2) % 4], t[(start + 3) % 4]];
            if i64::from(first[0]) == last {
                first = [first[2], first[0], first[1]];
                second = [second[1], second[2], second[0]];
            }
            last = i64::from(first[0]);
            out.push(first.to_vec());
            out.push(second.to_vec());
        } else {
            let tris = ear_clip(verts, &face);
            for local in tris {
                let mut t = [
                    face[local[0] as usize],
                    face[local[1] as usize],
                    face[local[2] as usize],
                ];
                fix_first(&mut t, &mut last);
                out.push(t.to_vec());
            }
        }
    }
    mesh.faces = out;
    true
}

/// the ear clipping of a polygon with five or more vertices: triangles as indices into the polygon
fn ear_clip(verts: &[Vec3], face: &[u32]) -> Vec<[u32; 3]> {
    let max = face.len();
    let pts: Vec<Vec3> = face.iter().map(|&i| verts[i as usize]).collect();
    // Newell normal of the polygon (single precision sums)
    let at = |i: usize| pts[i % max];
    let (mut sum_yz, mut sum_zx, mut sum_xy) = (0.0f32, 0.0f32, 0.0f32);
    for t in 0..max {
        let (low, mid, high) = (at(t), at(t + 1), at(t + 2));
        sum_yz += mid.y * (high.z - low.z);
        sum_zx += mid.z * (high.x - low.x);
        sum_xy += mid.x * (high.y - low.y);
    }
    let n = Vec3::new(sum_yz, sum_zx, sum_xy);
    // the coordinate with the largest normal component is dropped
    let (ax, ay, az) = (n.x.abs(), n.y.abs(), n.z.abs());
    let (mut ac, mut bc, mut inv) = (0usize, 1usize, n.z);
    if ax > ay {
        if ax > az {
            ac = 1;
            bc = 2;
            inv = n.x;
        }
    } else if ay > az {
        ac = 2;
        bc = 0;
        inv = n.y;
    }
    if inv < 0.0 {
        std::mem::swap(&mut ac, &mut bc);
    }
    let p2d: Vec<[f32; 2]> = pts
        .iter()
        .map(|&v| [component(v, ac), component(v, bc)])
        .collect();
    let mut done = vec![false; max];
    let mut tris: Vec<[u32; 3]> = Vec::new();
    let mut num = max;
    let (mut prev, mut next) = (max - 1, 0usize);
    let mut ear;
    while num > 3 {
        // find the next ear of the polygon
        let mut num_found = 0;
        ear = next;
        let mut failed = false;
        loop {
            next = ear + 1;
            loop {
                if next >= max {
                    next = 0;
                }
                if !done[next] {
                    break;
                }
                next += 1;
            }
            if next < ear {
                num_found += 1;
                if num_found == 2 {
                    failed = true;
                    break;
                }
            }
            let (p0, p1, p2) = (p2d[prev], p2d[ear], p2d[next]);
            let cont = 'ear: {
                // a convex point lies on the right of the line between the neighbours
                let a = area(p0, p1, p2);
                if a.abs() >= AREA_EPSILON && a > 0.0 {
                    break 'ear true;
                }
                // three points in a line
                let (lx, ly) = (p0[0] - p1[0], p0[1] - p1[1]);
                let (rx, ry) = (p2[0] - p1[0], p2[1] - p1[1]);
                let ll = (lx * lx + ly * ly).sqrt();
                let rl = (rx * rx + ry * ry).sqrt();
                let mul = (ry / rl) * (ly / ll) + (rx / rl) * (lx / ll);
                // (a NaN product, a zero-length side from two equal points, does not skip the ear)
                if (mul + -1.0).abs() < 1e-6_f32 || (mul + 1.0).abs() < 1e-6_f32 {
                    break 'ear true;
                }
                // no other point may lie in the triangle
                for v in &p2d {
                    if *v == p1 || *v == p2 || *v == p0 {
                        continue;
                    }
                    if point_in_triangle(p0, p1, p2, *v) {
                        break 'ear true;
                    }
                }
                false
            };
            if !cont {
                break;
            }
            prev = ear;
            ear = next;
        }
        if failed {
            // "Failed to triangulate polygon (no ear found)": the triangles found so far stay
            return tris;
        }
        tris.push([prev as u32, ear as u32, next as u32]);
        done[ear] = true;
        num -= 1;
    }
    if num > 0 {
        // the three vertices left
        let mut rest = (0..max).filter(|&i| !done[i]);
        let a = rest.next().unwrap_or(0);
        let b = rest.next().unwrap_or(0);
        let c = rest.next().unwrap_or(0);
        tris.push([a as u32, b as u32, c as u32]);
    }
    tris
}

/// TriangulateProcess::Execute
pub fn execute(scene: &mut Scene) {
    for mesh in scene.meshes.iter_mut() {
        triangulate_mesh(mesh);
    }
}
