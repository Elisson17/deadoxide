//! assimp `code/PostProcessing/RemoveRedundantMaterials.cpp`: `RemoveRedundantMatsProcess` (default configuration: no
//! materials are kept by name).
use crate::native::assimp::scene::{Material, Scene};

/// `SuperFastHash` of `include/assimp/SuperFastHash.h` (Paul Hsieh's hash), seeded with `hash`; an empty input gives 0.
pub fn super_fast_hash(data: &[u8], mut hash: u32) -> u32 {
    if data.is_empty() {
        return 0;
    }
    let get16 = |i: usize| -> u32 { u32::from(data[i]) | (u32::from(data[i + 1]) << 8) };
    let rem = data.len() & 3;
    let words = data.len() >> 2;
    let mut at = 0usize;
    for _ in 0..words {
        hash = hash.wrapping_add(get16(at));
        let tmp = (get16(at + 2) << 11) ^ hash;
        hash = (hash << 16) ^ tmp;
        at += 4;
        hash = hash.wrapping_add(hash >> 11);
    }
    match rem {
        3 => {
            hash = hash.wrapping_add(get16(at));
            hash ^= hash << 16;
            hash ^= ((data[at + 2] as i8 as i32) << 18) as u32;
            hash = hash.wrapping_add(hash >> 11);
        }
        2 => {
            hash = hash.wrapping_add(get16(at));
            hash ^= hash << 11;
            hash = hash.wrapping_add(hash >> 17);
        }
        1 => {
            hash = hash.wrapping_add(data[at] as i8 as i32 as u32);
            hash ^= hash << 10;
            hash = hash.wrapping_add(hash >> 1);
        }
        _ => {}
    }
    hash ^= hash << 3;
    hash = hash.wrapping_add(hash >> 5);
    hash ^= hash << 4;
    hash = hash.wrapping_add(hash >> 17);
    hash ^= hash << 25;
    hash = hash.wrapping_add(hash >> 6);
    hash
}

/// `ComputeMaterialHash(mat, includeMatName = false)` (`MaterialSystem`/`ProcessHelper`): the properties whose key
/// does not start with '?', key, data, semantic and index in turn.
pub fn compute_material_hash(mat: &Material) -> u32 {
    let mut hash: u32 = 1503;
    for prop in &mat.properties {
        if !prop.key.starts_with('?') {
            hash = super_fast_hash(prop.key.as_bytes(), hash);
            hash = super_fast_hash(&prop.data, hash);
            hash = super_fast_hash(&prop.semantic.to_le_bytes(), hash);
            hash = super_fast_hash(&prop.index.to_le_bytes(), hash);
        }
    }
    hash
}

/// RemoveRedundantMatsProcess::Execute: materials no mesh uses are dropped, materials with the same hash are merged
/// into the first, the rest keep their order; the meshes' material indices are remapped.
pub fn execute(scene: &mut Scene) {
    if scene.materials.is_empty() {
        return;
    }
    let n = scene.materials.len();
    let mut referenced = vec![false; n];
    for m in &scene.meshes {
        if let Some(r) = referenced.get_mut(m.material_index as usize) {
            *r = true;
        }
    }
    let mut mapping = vec![0u32; n];
    let mut hashes = vec![0u32; n];
    let mut kept: Vec<usize> = Vec::new();
    for i in 0..n {
        if !referenced[i] {
            continue;
        }
        hashes[i] = compute_material_hash(&scene.materials[i]);
        match kept.iter().find(|&&j| hashes[j] == hashes[i]) {
            Some(&j) => mapping[i] = mapping[j],
            None => {
                mapping[i] = kept.len() as u32;
                kept.push(i);
            }
        }
    }
    if kept.len() == n {
        return;
    }
    let old = std::mem::take(&mut scene.materials);
    scene.materials = kept.iter().map(|&i| old[i].clone()).collect();
    for m in scene.meshes.iter_mut() {
        if let Some(&to) = mapping.get(m.material_index as usize) {
            m.material_index = to;
        }
    }
}
