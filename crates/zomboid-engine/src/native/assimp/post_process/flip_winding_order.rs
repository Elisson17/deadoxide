//! assimp `code/PostProcessing/ConvertToLHProcess.cpp`: `FlipWindingOrderProcess`.
use crate::native::assimp::scene::Scene;

/// FlipWindingOrderProcess::Execute: every face's indices reversed.
pub fn execute(scene: &mut Scene) {
    for mesh in scene.meshes.iter_mut() {
        for face in mesh.faces.iter_mut() {
            face.reverse();
        }
    }
}
