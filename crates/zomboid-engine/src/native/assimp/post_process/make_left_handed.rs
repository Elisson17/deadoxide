//! assimp `code/PostProcessing/ConvertToLHProcess.cpp`: `MakeLeftHandedProcess`.
use crate::native::assimp::scene::{Mat4, Mesh, Node, NodeAnim, Scene};

/// MakeLeftHandedProcess::ProcessNode (the parent rotation it threads through is not used by the original either)
fn process_node(node: &mut Node) {
    node.transformation.c1 = -node.transformation.c1;
    node.transformation.c2 = -node.transformation.c2;
    node.transformation.c3 = -node.transformation.c3;
    node.transformation.c4 = -node.transformation.c4;
    node.transformation.a3 = -node.transformation.a3;
    node.transformation.b3 = -node.transformation.b3;
    node.transformation.c3 = -node.transformation.c3;
    node.transformation.d3 = -node.transformation.d3;
    for c in node.children.iter_mut() {
        process_node(c);
    }
}

/// MakeLeftHandedProcess::ProcessMesh
fn process_mesh(mesh: &mut Mesh) {
    let has_normals = !mesh.normals.is_empty();
    let has_tangents = !mesh.tangents.is_empty() && !mesh.bitangents.is_empty();
    for a in 0..mesh.vertices.len() {
        mesh.vertices[a].z *= -1.0;
        if has_normals {
            mesh.normals[a].z *= -1.0;
        }
        if has_tangents {
            mesh.tangents[a].z *= -1.0;
            mesh.bitangents[a].z *= -1.0;
        }
    }
    for bone in mesh.bones.iter_mut() {
        let m: &mut Mat4 = &mut bone.offset_matrix;
        m.a3 = -m.a3;
        m.b3 = -m.b3;
        m.d3 = -m.d3;
        m.c1 = -m.c1;
        m.c2 = -m.c2;
        m.c4 = -m.c4;
    }
    if has_tangents {
        for b in mesh.bitangents.iter_mut() {
            b.x *= -1.0;
            b.y *= -1.0;
            b.z *= -1.0;
        }
    }
}

/// MakeLeftHandedProcess::ProcessAnimation
fn process_animation(anim: &mut NodeAnim) {
    for k in anim.position_keys.iter_mut() {
        k.value.z *= -1.0;
    }
    for k in anim.rotation_keys.iter_mut() {
        // the quaternion mirrored at the Z axis: x and y change sign
        k.value.x *= -1.0;
        k.value.y *= -1.0;
    }
}

/// MakeLeftHandedProcess::Execute
pub fn execute(scene: &mut Scene) {
    if let Some(root) = scene.root_node.as_mut() {
        process_node(root);
    }
    for m in scene.meshes.iter_mut() {
        process_mesh(m);
    }
    for a in scene.animations.iter_mut() {
        for c in a.channels.iter_mut() {
            process_animation(c);
        }
    }
}
