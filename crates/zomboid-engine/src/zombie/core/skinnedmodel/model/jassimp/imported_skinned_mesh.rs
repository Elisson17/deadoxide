//! zombie.core.skinnedmodel.model.jassimp.ImportedSkinnedMesh
use super::imported_skeleton::ImportedSkeleton;
use super::jassimp_importer::{face_elements, tex_coord_sets, tex_coord_uv};
use crate::native::assimp::scene::Mesh;
use crate::org::lwjgl::util::vector::matrix4f::Matrix4f;
use crate::zombie::core::skinnedmodel::model::vertex_buffer_object::{
    VertexArray, VertexFormat, VertexType,
};

/// ImportedSkinnedMesh: positions, normals, four blend weights and indices per vertex and the uv sets.
#[derive(Clone, Debug)]
pub struct ImportedSkinnedMesh {
    pub name: String,
    pub vertices: VertexArray,
    pub elements: Vec<i32>,
    pub transform: Option<Matrix4f>,
}

impl ImportedSkinnedMesh {
    /// ImportedSkinnedMesh(ImportedSkeleton, AiMesh) -> processAiScene; `Err` is the Java exception class.
    pub fn new(skeleton: &ImportedSkeleton, mesh: &Mesh) -> Result<Self, String> {
        let num_vertices = mesh.vertices.len();
        let bone_arrays_size = num_vertices * 4;
        let mut bone_ids = vec![0i32; bone_arrays_size];
        let mut bone_weights = vec![0.0f32; bone_arrays_size];
        for bone in &mesh.bones {
            let boneindex = *skeleton
                .bone_indices
                .get(&bone.name)
                .ok_or_else(|| "java.lang.NullPointerException".to_string())?;
            for w in &bone.weights {
                let vertex_start = (w.vertex_id as usize)
                    .checked_mul(4)
                    .filter(|s| s + 4 <= bone_arrays_size)
                    .ok_or_else(|| "java.lang.ArrayIndexOutOfBoundsException".to_string())?;
                for k in 0..4 {
                    if bone_weights[vertex_start + k] == 0.0 {
                        bone_weights[vertex_start + k] = w.weight;
                        bone_ids[vertex_start + k] = boneindex;
                        break;
                    }
                }
            }
        }
        for i in 0..num_vertices {
            let mut sum = 0.0f32;
            for k in 0..4 {
                sum += bone_weights[i * 4 + k];
            }
            if sum != 0.0 {
                let inv_sum = 1.0f32 / sum;
                for k in 0..4 {
                    bone_weights[i * 4 + k] *= inv_sum;
                }
            }
        }
        let uvs = tex_coord_sets(mesh);
        let num_uvs = uvs.len();
        let mut format = VertexFormat::new(4 + num_uvs);
        format.set_element(0, VertexType::VertexArray, 12);
        format.set_element(1, VertexType::NormalArray, 12);
        format.set_element(2, VertexType::BlendWeightArray, 16);
        format.set_element(3, VertexType::BlendIndexArray, 16);
        for i in 0..num_uvs {
            format.set_element(4 + i, VertexType::TextureCoordArray, 8);
        }
        format.calculate();
        let mut va = VertexArray::new(&format, num_vertices as i32)?;
        for n in 0..num_vertices {
            let nv = n as i32;
            let p = mesh.vertices[n];
            va.set_element3(nv, 0, p.x, p.y, p.z);
            if !mesh.normals.is_empty() {
                let nm = mesh.normals[n];
                va.set_element3(nv, 1, nm.x, nm.y, nm.z);
            } else {
                va.set_element3(nv, 1, 0.0, 1.0, 0.0);
            }
            va.set_element4(
                nv,
                2,
                bone_weights[n * 4],
                bone_weights[n * 4 + 1],
                bone_weights[n * 4 + 2],
                bone_weights[n * 4 + 3],
            );
            va.set_element4(
                nv,
                3,
                bone_ids[n * 4] as f32,
                bone_ids[n * 4 + 1] as f32,
                bone_ids[n * 4 + 2] as f32,
                bone_ids[n * 4 + 3] as f32,
            );
            for (k, &set) in uvs.iter().enumerate() {
                let (u, v) = tex_coord_uv(mesh, set, n)?;
                va.set_element2(nv, 4 + k, u, 1.0 - v);
            }
        }
        Ok(ImportedSkinnedMesh {
            name: mesh.name.clone(),
            vertices: va,
            elements: face_elements(mesh)?,
            transform: None,
        })
    }
}
