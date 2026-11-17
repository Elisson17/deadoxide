//! assimp `code/Common/ScenePreprocessor.cpp`: the fix-ups every imported scene gets before the post-processing steps.
use crate::native::assimp::scene::{
    Animation, Material, MaterialProperty, Mesh, Node, Quat, Scene, Vec3, PRIMITIVE_LINE,
    PRIMITIVE_POINT, PRIMITIVE_POLYGON, PRIMITIVE_TRIANGLE,
};

/// aiNode::FindNode: the node with this name, searching depth first from `node`.
pub fn find_node<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    if node.name == name {
        return Some(node);
    }
    node.children.iter().find_map(|c| find_node(c, name))
}

/// ScenePreprocessor::ProcessMesh
fn process_mesh(mesh: &mut Mesh) {
    for i in 0..8 {
        if mesh.texture_coords[i].is_empty() {
            mesh.num_uv_components[i] = 0;
        } else {
            if mesh.num_uv_components[i] == 0 {
                mesh.num_uv_components[i] = 2;
            }
            match mesh.num_uv_components[i] {
                2 => {
                    for p in mesh.texture_coords[i].iter_mut() {
                        p.z = 0.0;
                    }
                }
                1 => {
                    for p in mesh.texture_coords[i].iter_mut() {
                        p.z = 0.0;
                        p.y = 0.0;
                    }
                }
                _ => {}
            }
        }
    }
    if mesh.primitive_types == 0 {
        for face in &mesh.faces {
            mesh.primitive_types |= match face.len() {
                3 => PRIMITIVE_TRIANGLE,
                2 => PRIMITIVE_LINE,
                1 => PRIMITIVE_POINT,
                _ => PRIMITIVE_POLYGON,
            };
        }
    }
}

/// ScenePreprocessor::ProcessAnimation: channels without rotation, scaling or position keys get a one-key track from
/// the transformation of the node they animate.
fn process_animation(root: Option<&Node>, anim: &mut Animation) {
    let mut first = 10e10f64;
    let mut last = -10e10f64;
    for channel in anim.channels.iter_mut() {
        if anim.duration == -1.0 {
            if let (Some(a), Some(b)) =
                (channel.position_keys.first(), channel.position_keys.last())
            {
                first = first.min(a.time);
                last = last.max(b.time);
            }
            if let (Some(a), Some(b)) =
                (channel.rotation_keys.first(), channel.rotation_keys.last())
            {
                first = first.min(a.time);
                last = last.max(b.time);
            }
            if let (Some(a), Some(b)) = (channel.scaling_keys.first(), channel.scaling_keys.last())
            {
                first = first.min(a.time);
                last = last.max(b.time);
            }
        }
        if channel.rotation_keys.is_empty()
            || channel.position_keys.is_empty()
            || channel.scaling_keys.is_empty()
        {
            let node = root.and_then(|r| find_node(r, &channel.node_name));
            if let Some(node) = node {
                let (scaling, rotation, position): (Vec3, Quat, Vec3) =
                    node.transformation.decompose();
                if channel.rotation_keys.is_empty() {
                    channel
                        .rotation_keys
                        .push(crate::native::assimp::scene::QuatKey {
                            time: 0.0,
                            value: rotation,
                        });
                }
                if channel.scaling_keys.is_empty() {
                    channel
                        .scaling_keys
                        .push(crate::native::assimp::scene::VectorKey {
                            time: 0.0,
                            value: scaling,
                        });
                }
                if channel.position_keys.is_empty() {
                    channel
                        .position_keys
                        .push(crate::native::assimp::scene::VectorKey {
                            time: 0.0,
                            value: position,
                        });
                }
            }
        }
    }
    if anim.duration == -1.0 {
        anim.duration = last - first.min(0.0);
    }
}

/// The default material `ScenePreprocessor::ProcessScene` adds when a scene with meshes has none.
fn default_material() -> Material {
    let name = "DefaultMaterial";
    let mut data = Vec::new();
    data.extend_from_slice(&(name.len() as u32).to_le_bytes());
    data.extend_from_slice(name.as_bytes());
    data.push(0);
    let mut m = Material::default();
    m.properties.push(MaterialProperty {
        key: "?mat.name".to_string(),
        semantic: 0,
        index: 0,
        data,
        ptype: 3,
    });
    m
}

/// ScenePreprocessor::ProcessScene
pub fn process_scene(scene: &mut Scene) {
    for mesh in scene.meshes.iter_mut() {
        process_mesh(mesh);
    }
    for anim in scene.animations.iter_mut() {
        process_animation(scene.root_node.as_ref(), anim);
    }
    if scene.materials.is_empty() && !scene.meshes.is_empty() {
        scene.materials.push(default_material());
    }
}
