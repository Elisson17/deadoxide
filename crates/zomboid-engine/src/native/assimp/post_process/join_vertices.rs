//! assimp `code/PostProcessing/JoinVerticesProcess.cpp`: `JoinVerticesProcess::ProcessMesh` and `updateXMeshVertices`
//! as compiled into the game's `libjassimp` (assimp 5.3 line): the vertices are looked up in a
//! `std::unordered_map<Vertex, int>` (libc++) instead of a `SpatialSort`.
//!
//! The hash of a vertex covers its position only (the bit patterns of x, y, z, `-0.0` hashing like `0.0`, combined with
//! `h ^= v + 0x9e3779b9 + (h << 6) + (h >> 2)`); two vertices are the same when every attribute is within `1e-5`
//! (squared distance `<= 1e-5f * 1e-5f`). The map is reserved for `mNumVertices` buckets, so it never rehashes, and a
//! new node goes to the front of its bucket: of the unique vertices with an equal hash the newest is tried first.
//! Vertices that no face uses are not visited and disappear.
use crate::native::assimp::scene::{Mesh, Scene, Vec3, VertexWeight};
use std::collections::HashMap;

/// `1e-5f * 1e-5f` as the library's constant (`0x2edbe6fe`).
const SQUARE_EPSILON: f32 = f32::from_bits(0x2edb_e6fe);

fn square_length3(a: Vec3, b: Vec3) -> f32 {
    let (x, y, z) = (a.x - b.x, a.y - b.y, a.z - b.z);
    (x * x + y * y) + z * z
}

fn square_length4(a: [f32; 4], b: [f32; 4]) -> f32 {
    let (r, g, bl, al) = (a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]);
    ((r * r + g * g) + bl * bl) + al * al
}

/// `Assimp::Vertex`: every attribute the mesh has, zero for the others.
struct Vertex {
    position: Vec3,
    normal: Vec3,
    tangent: Vec3,
    bitangent: Vec3,
    texcoords: [Vec3; 8],
    colors: [[f32; 4]; 8],
}

impl Vertex {
    /// Vertex::Vertex(const aiMesh*, unsigned int)
    fn new(mesh: &Mesh, i: usize) -> Self {
        let zero = Vec3::new(0.0, 0.0, 0.0);
        let mut v = Vertex {
            position: mesh.vertices[i],
            normal: zero,
            tangent: zero,
            bitangent: zero,
            texcoords: [zero; 8],
            colors: [[0.0; 4]; 8],
        };
        if !mesh.normals.is_empty() {
            v.normal = mesh.normals[i];
        }
        if !mesh.tangents.is_empty() && !mesh.bitangents.is_empty() {
            v.tangent = mesh.tangents[i];
            v.bitangent = mesh.bitangents[i];
        }
        for (a, set) in mesh.texture_coords.iter().enumerate() {
            if set.is_empty() {
                break;
            }
            v.texcoords[a] = set[i];
        }
        for (a, set) in mesh.colors.iter().enumerate() {
            if set.is_empty() {
                break;
            }
            v.colors[a] = set[i];
        }
        v
    }

    /// `std::hash<Vertex>`: `hash<float>` of x, y, z combined boost-style (`hash<float>` is 0 for either zero).
    fn hash(&self) -> u64 {
        let mut seed = 0u64;
        for f in [self.position.x, self.position.y, self.position.z] {
            let h = if f == 0.0 { 0 } else { u64::from(f.to_bits()) };
            let mixed = h
                .wrapping_add(0x9e37_79b9)
                .wrapping_add(seed << 6)
                .wrapping_add(seed >> 2);
            seed ^= mixed;
        }
        seed
    }

    /// `std::equal_to<Vertex>` with the functor's channel counts: position, normal, tangent, bitangent, the texture
    /// coordinate sets and the color sets must all be within the epsilon.
    fn equals(&self, other: &Vertex, num_uv: usize, num_col: usize) -> bool {
        if square_length3(self.position, other.position) > SQUARE_EPSILON {
            return false;
        }
        if square_length3(self.normal, other.normal) > SQUARE_EPSILON {
            return false;
        }
        if square_length3(self.tangent, other.tangent) > SQUARE_EPSILON {
            return false;
        }
        if square_length3(self.bitangent, other.bitangent) > SQUARE_EPSILON {
            return false;
        }
        for i in 0..num_uv {
            if square_length3(self.texcoords[i], other.texcoords[i]) > SQUARE_EPSILON {
                return false;
            }
        }
        for i in 0..num_col {
            if square_length4(self.colors[i], other.colors[i]) > SQUARE_EPSILON {
                return false;
            }
        }
        true
    }
}

/// JoinVerticesProcess::ProcessMesh
fn process_mesh(mesh: &mut Mesh) -> Result<(), String> {
    if mesh.vertices.is_empty() || mesh.faces.is_empty() {
        return Ok(());
    }
    let n = mesh.vertices.len();
    // the vertices some face refers to
    let mut used = vec![false; n];
    for face in &mesh.faces {
        for &idx in face {
            *used
                .get_mut(idx as usize)
                .ok_or_else(|| "face index out of range".to_string())? = true;
        }
    }
    let num_uv = mesh
        .texture_coords
        .iter()
        .take_while(|s| !s.is_empty())
        .count();
    let num_col = mesh.colors.iter().take_while(|s| !s.is_empty()).count();

    let mut unique: Vec<Vertex> = Vec::with_capacity(n);
    // the flag 0x80000000 marks a vertex that was merged into an earlier one
    let mut replace_index: Vec<u32> = vec![0xffff_ffff; n];
    // hash -> unique indices in insertion order (the libc++ bucket is walked newest first)
    let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();
    for a in 0..n {
        if !used[a] {
            continue;
        }
        let v = Vertex::new(mesh, a);
        let hash = v.hash();
        let found = buckets.get(&hash).and_then(|chain| {
            chain
                .iter()
                .rev()
                .find(|&&u| unique[u].equals(&v, num_uv, num_col))
                .copied()
        });
        match found {
            Some(u) => replace_index[a] = (u as u32) | 0x8000_0000,
            None => {
                replace_index[a] = unique.len() as u32;
                buckets.entry(hash).or_default().push(unique.len());
                unique.push(v);
            }
        }
    }

    // updateXMeshVertices
    mesh.vertices = unique.iter().map(|u| u.position).collect();
    if !mesh.normals.is_empty() {
        mesh.normals = unique.iter().map(|u| u.normal).collect();
    }
    if !mesh.tangents.is_empty() && !mesh.bitangents.is_empty() {
        mesh.tangents = unique.iter().map(|u| u.tangent).collect();
        mesh.bitangents = unique.iter().map(|u| u.bitangent).collect();
    }
    for a in 0..8 {
        if !mesh.texture_coords[a].is_empty() {
            mesh.texture_coords[a] = unique.iter().map(|u| u.texcoords[a]).collect();
        }
        if !mesh.colors[a].is_empty() {
            mesh.colors[a] = unique.iter().map(|u| u.colors[a]).collect();
        }
    }
    for face in mesh.faces.iter_mut() {
        for idx in face.iter_mut() {
            *idx = replace_index[*idx as usize] & !0x8000_0000;
        }
    }
    for bone in mesh.bones.iter_mut() {
        let mut new_weights: Vec<VertexWeight> = Vec::with_capacity(bone.weights.len());
        for ow in &bone.weights {
            let r = *replace_index
                .get(ow.vertex_id as usize)
                .ok_or_else(|| "bone weight vertex out of range".to_string())?;
            if r & 0x8000_0000 == 0 {
                new_weights.push(VertexWeight {
                    vertex_id: r,
                    weight: ow.weight,
                });
            }
        }
        if !new_weights.is_empty() {
            bone.weights = new_weights;
        }
    }
    Ok(())
}

/// JoinVerticesProcess::Execute
pub fn execute(scene: &mut Scene) -> Result<(), String> {
    for m in scene.meshes.iter_mut() {
        process_mesh(m)?;
    }
    Ok(())
}
