//! assimp `code/PostProcessing/FindInstancesProcess.cpp`: `FindInstancesProcess`.
//!
//! The step replaces a mesh by an earlier one when the two hold the same data and use the same material. Only the
//! detection is ported: a scene that would be changed is refused (`Err`), a scene it leaves alone passes. The game's
//! `.X` models have none that it changes; `Sandwich.X` and `SandwichGrilled.X` hold two meshes with identical
//! geometry and different materials, and the original keeps both.
use crate::native::assimp::scene::{Mesh, Scene};

fn same_data(a: &Mesh, b: &Mesh) -> bool {
    a.material_index == b.material_index
        && a.vertices == b.vertices
        && a.normals == b.normals
        && a.tangents == b.tangents
        && a.bitangents == b.bitangents
        && a.colors == b.colors
        && a.texture_coords == b.texture_coords
        && a.faces == b.faces
        && a.bones.len() == b.bones.len()
}

/// FindInstancesProcess::Execute
pub fn execute(scene: &Scene) -> Result<(), String> {
    for (i, a) in scene.meshes.iter().enumerate() {
        for b in &scene.meshes[i + 1..] {
            if same_data(a, b) {
                return Err("UNPORTED: FindInstances replaces an equal mesh".to_string());
            }
        }
    }
    Ok(())
}
