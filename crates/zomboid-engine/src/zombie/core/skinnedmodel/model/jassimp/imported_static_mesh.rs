//! zombie.core.skinnedmodel.model.jassimp.ImportedStaticMesh
use super::jassimp_importer::{face_elements, tex_coord_sets, tex_coord_uv};
use crate::native::assimp::scene::Mesh;
use crate::org::lwjgl::util::vector::matrix4f::Matrix4f;
use crate::zombie::core::skinnedmodel::model::vertex_buffer_object::{
    VertexArray, VertexFormat, VertexType,
};

/// ImportedStaticMesh: positions, normals and the uv sets of a mesh as a vertex array (no skinning data).
#[derive(Clone, Debug)]
pub struct ImportedStaticMesh {
    pub vertices_unskinned: VertexArray,
    pub elements: Vec<i32>,
    pub min_xyz: [f32; 3],
    pub max_xyz: [f32; 3],
    pub transform: Option<Matrix4f>,
}

/// org.joml.Vector3f.min component: `a < b ? a : b` (the second operand for NaN).
fn jmin(a: f32, b: f32) -> f32 {
    if a < b {
        a
    } else {
        b
    }
}

/// org.joml.Vector3f.max component: `a > b ? a : b` (the second operand for NaN).
fn jmax(a: f32, b: f32) -> f32 {
    if a > b {
        a
    } else {
        b
    }
}

impl ImportedStaticMesh {
    /// ImportedStaticMesh(AiMesh) -> processAiScene; `Err` is the Java exception class.
    pub fn new(mesh: &Mesh) -> Result<Self, String> {
        let num_vertices = mesh.vertices.len() as i32;
        let uvs = tex_coord_sets(mesh);
        let num_uvs = uvs.len();
        let mut format = VertexFormat::new(2 + num_uvs);
        format.set_element(0, VertexType::VertexArray, 12);
        format.set_element(1, VertexType::NormalArray, 12);
        for i in 0..num_uvs {
            format.set_element(2 + i, VertexType::TextureCoordArray, 8);
        }
        format.calculate();
        let mut va = VertexArray::new(&format, num_vertices)?;
        let mut min_xyz = [f32::MAX; 3];
        let mut max_xyz = [-f32::MAX; 3];
        for n in 0..mesh.vertices.len() {
            let p = mesh.vertices[n];
            min_xyz = [
                jmin(min_xyz[0], p.x),
                jmin(min_xyz[1], p.y),
                jmin(min_xyz[2], p.z),
            ];
            max_xyz = [
                jmax(max_xyz[0], p.x),
                jmax(max_xyz[1], p.y),
                jmax(max_xyz[2], p.z),
            ];
            let nv = n as i32;
            va.set_element3(nv, 0, p.x, p.y, p.z);
            if !mesh.normals.is_empty() {
                let nm = mesh.normals[n];
                va.set_element3(nv, 1, nm.x, nm.y, nm.z);
            } else {
                va.set_element3(nv, 1, 0.0, 1.0, 0.0);
            }
            for (k, &set) in uvs.iter().enumerate() {
                let (u, v) = tex_coord_uv(mesh, set, n)?;
                va.set_element2(nv, 2 + k, u, 1.0 - v);
            }
        }
        Ok(ImportedStaticMesh {
            vertices_unskinned: va,
            elements: face_elements(mesh)?,
            min_xyz,
            max_xyz,
            transform: None,
        })
    }
}
