//! assimp `code/PostProcessing/LimitBoneWeightsProcess.cpp`: `LimitBoneWeightsProcess` (`AI_LBW_MAX_WEIGHTS` 4).
//!
//! The weights of a vertex are sorted with `std::sort` and an `operator<` that puts the larger weight first; equal
//! weights end up in the order libc++'s `std::sort` leaves them (`native::assimp::std_sort`).
use crate::native::assimp::scene::{Bone, Mesh, Scene, VertexWeight};
use crate::native::assimp::std_sort;

#[derive(Clone, Copy)]
struct Weight {
    bone: u32,
    weight: f32,
}

/// `Weight::operator<`: "less" means a larger weight.
fn less(a: &Weight, b: &Weight) -> bool {
    a.weight > b.weight
}

/// `std::sort` of the weights of one vertex.
fn std_sort(v: &mut [Weight]) -> Result<(), String> {
    std_sort::sort(v, &less)
}

/// LimitBoneWeightsProcess::ProcessMesh with `mMaxWeights` = 4.
fn process_mesh(mesh: &mut Mesh) -> Result<(), String> {
    const MAX_WEIGHTS: usize = 4;
    if mesh.bones.is_empty() {
        return Ok(());
    }
    let mut vertex_weights: Vec<Vec<Weight>> = vec![Vec::new(); mesh.vertices.len()];
    let mut max_vertex_weights = 0usize;
    for (a, bone) in mesh.bones.iter().enumerate() {
        for w in &bone.weights {
            let list = vertex_weights
                .get_mut(w.vertex_id as usize)
                .ok_or_else(|| "bone weight vertex out of range".to_string())?;
            list.push(Weight {
                bone: a as u32,
                weight: w.weight,
            });
            max_vertex_weights = max_vertex_weights.max(list.len());
        }
    }
    if max_vertex_weights <= MAX_WEIGHTS {
        return Ok(());
    }
    let mut changed = false;
    for list in vertex_weights.iter_mut() {
        if list.len() <= MAX_WEIGHTS {
            continue;
        }
        changed = true;
        std_sort(list)?;
        list.truncate(MAX_WEIGHTS);
        let mut sum = 0.0f32;
        for w in list.iter() {
            sum += w.weight;
        }
        if sum != 0.0 {
            let inv_sum = 1.0f32 / sum;
            for w in list.iter_mut() {
                w.weight *= inv_sum;
            }
        }
    }
    if changed {
        let mut bone_store: Vec<Vec<VertexWeight>> = vec![Vec::new(); mesh.bones.len()];
        for (vid, list) in vertex_weights.iter().enumerate() {
            for w in list {
                bone_store[w.bone as usize].push(VertexWeight {
                    vertex_id: vid as u32,
                    weight: w.weight,
                });
            }
        }
        let old: Vec<Bone> = std::mem::take(&mut mesh.bones);
        for (a, mut bone) in old.into_iter().enumerate() {
            if bone_store[a].is_empty() {
                continue;
            }
            bone.weights = std::mem::take(&mut bone_store[a]);
            mesh.bones.push(bone);
        }
    }
    Ok(())
}

/// LimitBoneWeightsProcess::Execute
pub fn execute(scene: &mut Scene) -> Result<(), String> {
    for m in scene.meshes.iter_mut() {
        process_mesh(m)?;
    }
    Ok(())
}
