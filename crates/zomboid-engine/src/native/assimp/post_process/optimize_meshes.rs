//! assimp `code/PostProcessing/OptimizeMeshes.cpp`: `OptimizeMeshesProcess::Execute`, `ProcessNode`, `CanJoin` and
//! `SceneCombiner::MergeMeshes` (read from the disassembled `libjassimp`; the game runs the step with the default
//! limits: no maximum for vertices and faces, `pts` off).
//!
//! The step joins the meshes one node refers to when they have the same vertex format and material and no bones. A
//! mesh a node shares with other nodes is never merged.
use crate::native::assimp::scene::{Mesh, Node, Scene};

/// `OptimizeMeshesProcess::MeshInfo`
#[derive(Clone, Copy)]
struct MeshInfo {
    instance_cnt: u32,
    vertex_format: u32,
    output_id: u32,
}

/// `GetMeshVFormatUnique(aiMesh*)`: one bit per kind of vertex data present (sets are counted while consecutive).
fn vertex_format(m: &Mesh) -> u32 {
    let mut ret = 1u32;
    if !m.normals.is_empty() {
        ret |= 0x2;
    }
    if !m.tangents.is_empty() && !m.bitangents.is_empty() {
        ret |= 0x4;
    }
    for (p, set) in m.texture_coords.iter().enumerate() {
        if set.is_empty() {
            break;
        }
        ret |= 0x100 << p;
        if m.num_uv_components[p] == 3 {
            ret |= 0x1_0000 << p;
        }
    }
    for (p, set) in m.colors.iter().enumerate() {
        if set.is_empty() {
            break;
        }
        ret |= 0x100_0000 << p;
    }
    ret
}

/// OptimizeMeshesProcess::FindInstancedMeshes: how many node slots refer to each mesh.
fn find_instanced_meshes(node: &Node, infos: &mut [MeshInfo]) {
    for &m in &node.meshes {
        if let Some(i) = infos.get_mut(m as usize) {
            i.instance_cnt += 1;
        }
    }
    for c in &node.children {
        find_instanced_meshes(c, infos);
    }
}

/// OptimizeMeshesProcess::CanJoin for the default limits (the format and instancing tests are in `process_node`).
fn can_join(a: &Mesh, b: &Mesh) -> bool {
    if a.material_index != b.material_index {
        return false;
    }
    if a.bones.is_empty() != b.bones.is_empty() {
        return false;
    }
    a.bones.is_empty()
}

/// SceneCombiner::MergeMeshes for meshes without bones: the data of the meshes one after the other, the face indices
/// of every mesh moved by the vertices before it, the names joined with `.`.
fn merge_meshes(list: Vec<Mesh>) -> Result<Mesh, String> {
    if list.iter().any(|m| !m.bones.is_empty()) {
        return Err("UNPORTED: MergeMeshes of skinned meshes".to_string());
    }
    let mut out = Mesh {
        material_index: list[0].material_index,
        num_uv_components: list[0].num_uv_components,
        ..Mesh::default()
    };
    let mut ofs = 0u32;
    let mut name = String::new();
    for (k, m) in list.into_iter().enumerate() {
        if k > 0 {
            name.push('.');
        }
        name.push_str(&m.name);
        out.primitive_types |= m.primitive_types;
        out.vertices.extend_from_slice(&m.vertices);
        out.normals.extend_from_slice(&m.normals);
        out.tangents.extend_from_slice(&m.tangents);
        out.bitangents.extend_from_slice(&m.bitangents);
        for n in 0..8 {
            out.texture_coords[n].extend_from_slice(&m.texture_coords[n]);
            out.colors[n].extend_from_slice(&m.colors[n]);
        }
        for face in &m.faces {
            out.faces.push(face.iter().map(|&q| q + ofs).collect());
        }
        ofs += m.vertices.len() as u32;
    }
    out.name = name;
    Ok(out)
}

struct State {
    infos: Vec<MeshInfo>,
    meshes: Vec<Option<Mesh>>,
    output: Vec<Mesh>,
}

impl State {
    fn mesh(&self, i: u32) -> Result<&Mesh, String> {
        self.meshes
            .get(i as usize)
            .and_then(|m| m.as_ref())
            .ok_or_else(|| "node refers to a mesh that does not exist".to_string())
    }

    /// OptimizeMeshesProcess::ProcessNode
    fn process_node(&mut self, node: &mut Node) -> Result<(), String> {
        let mut i = 0usize;
        while i < node.meshes.len() {
            let im = node.meshes[i];
            let info = *self
                .infos
                .get(im as usize)
                .ok_or_else(|| "node refers to a mesh that does not exist".to_string())?;
            if info.instance_cnt >= 2 {
                node.meshes[i] = info.output_id;
                i += 1;
                continue;
            }
            let mut merge_list: Vec<Mesh> = Vec::new();
            let mut a = i + 1;
            while a < node.meshes.len() {
                let am = node.meshes[a];
                let ainfo = self.infos[am as usize];
                if ainfo.instance_cnt != 1
                    || info.vertex_format != ainfo.vertex_format
                    || !can_join(self.mesh(im)?, self.mesh(am)?)
                {
                    a += 1;
                    continue;
                }
                // the mesh moves to the merge list and the last node slot takes its place
                let taken = self.meshes[am as usize]
                    .take()
                    .ok_or_else(|| "mesh merged twice".to_string())?;
                merge_list.push(taken);
                node.meshes.swap_remove(a);
            }
            let me = self.meshes[im as usize]
                .take()
                .ok_or_else(|| "mesh used twice".to_string())?;
            let out = if merge_list.is_empty() {
                me
            } else {
                merge_list.push(me);
                merge_meshes(merge_list)?
            };
            self.output.push(out);
            let id = (self.output.len() - 1) as u32;
            self.infos[im as usize].output_id = id;
            node.meshes[i] = id;
            i += 1;
        }
        for c in node.children.iter_mut() {
            self.process_node(c)?;
        }
        Ok(())
    }
}

/// OptimizeMeshesProcess::Execute
pub fn execute(scene: &mut Scene) -> Result<(), String> {
    let n = scene.meshes.len();
    if n <= 1 {
        return Ok(());
    }
    let Some(mut root) = scene.root_node.take() else {
        return Ok(());
    };
    let mut infos: Vec<MeshInfo> = scene
        .meshes
        .iter()
        .map(|m| MeshInfo {
            instance_cnt: 0,
            vertex_format: vertex_format(m),
            output_id: 0xffff_ffff,
        })
        .collect();
    find_instanced_meshes(&root, &mut infos);
    let mut st = State {
        infos,
        meshes: std::mem::take(&mut scene.meshes)
            .into_iter()
            .map(Some)
            .collect(),
        output: Vec::new(),
    };
    // meshes several nodes share go to the output first, in mesh order
    for i in 0..n {
        if st.infos[i].instance_cnt >= 2 && st.infos[i].output_id == 0xffff_ffff {
            let m = st.meshes[i]
                .take()
                .ok_or_else(|| "mesh taken twice".to_string())?;
            st.output.push(m);
            st.infos[i].output_id = (st.output.len() - 1) as u32;
        }
    }
    let r = st.process_node(&mut root);
    scene.root_node = Some(root);
    r?;
    scene.meshes = st.output;
    Ok(())
}
