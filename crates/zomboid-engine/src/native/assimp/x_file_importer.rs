//! assimp `code/AssetLib/X/XFileImporter.cpp`: `XFileImporter::InternReadFile` and the conversion of the parsed
//! `XFile::Scene` into an `aiScene`, including the `MakeLeftHandedProcess` and `FlipWindingOrderProcess` it runs
//! itself, then the `ScenePreprocessor` that `Importer::ReadFile` runs on every imported scene.
use super::post_process::{flip_winding_order, make_left_handed, scene_preprocessor};
use super::scene::{
    Animation, Bone, Material, MaterialProperty, Mesh, Node, NodeAnim, Quat, QuatKey, Scene, Vec3,
    VectorKey, VertexWeight,
};
use super::x_file_parser::{self, XMaterial, XMesh, XNode, XScene};

/// aiShadingMode_Gouraud and aiShadingMode_Phong
const SHADING_GOURAUD: i32 = 0x2;
const SHADING_PHONG: i32 = 0x3;
/// aiTextureType_DIFFUSE and aiTextureType_NORMALS
const TEXTURE_DIFFUSE: u32 = 1;
const TEXTURE_NORMALS: u32 = 6;

fn ai_string(s: &str) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&(s.len() as u32).to_le_bytes());
    data.extend_from_slice(s.as_bytes());
    data
}

fn floats(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn prop(key: &str, semantic: u32, index: u32, data: Vec<u8>) -> MaterialProperty {
    MaterialProperty {
        key: key.to_string(),
        semantic,
        index,
        data,
    }
}

/// XFileImporter::ConvertMaterials: appends the new materials to the scene and records each one's scene index.
fn convert_materials(scene: &mut Scene, materials: &mut [XMaterial]) -> Result<(), String> {
    for old in materials.iter_mut() {
        if old.is_reference {
            for (i, m) in scene.materials.iter().enumerate() {
                let name = m.properties.iter().find(|p| p.key == "?mat.name");
                if name.is_some_and(|p| p.data.get(4..) == Some(old.name.as_bytes())) {
                    old.scene_index = i as u32;
                    break;
                }
            }
            continue;
        }
        let mut mat = Material::default();
        mat.properties
            .push(prop("?mat.name", 0, 0, ai_string(&old.name)));
        let shade_mode = if (old.specular_exponent as i32) as f32 == 0.0 {
            SHADING_GOURAUD
        } else {
            SHADING_PHONG
        };
        mat.properties.push(prop(
            "$mat.shadingm",
            0,
            0,
            shade_mode.to_le_bytes().to_vec(),
        ));
        mat.properties
            .push(prop("$clr.emissive", 0, 0, floats(&old.emissive)));
        mat.properties
            .push(prop("$clr.diffuse", 0, 0, floats(&old.diffuse)));
        mat.properties
            .push(prop("$clr.specular", 0, 0, floats(&old.specular)));
        mat.properties.push(prop(
            "$mat.shininess",
            0,
            0,
            floats(&[old.specular_exponent]),
        ));
        if old.textures.len() == 1 {
            let (name, is_normal) = &old.textures[0];
            if !name.is_empty() {
                let semantic = if *is_normal {
                    TEXTURE_NORMALS
                } else {
                    TEXTURE_DIFFUSE
                };
                mat.properties
                    .push(prop("$tex.file", semantic, 0, ai_string(name)));
            }
        } else if !old.textures.is_empty() {
            return Err("UNPORTED: X material with several textures".to_string());
        }
        old.scene_index = scene.materials.len() as u32;
        scene.materials.push(mat);
    }
    Ok(())
}

/// XFileImporter::CreateMeshes: one aiMesh per used material of each source mesh; returns their scene indices.
fn create_meshes(scene: &mut Scene, source_meshes: &mut [XMesh]) -> Result<Vec<u32>, String> {
    let mut indices = Vec::new();
    for source in source_meshes.iter_mut() {
        convert_materials(scene, &mut source.materials)?;
        let num_materials = source.materials.len().max(1);
        for b in 0..num_materials {
            let mut faces: Vec<usize> = Vec::new();
            let mut num_vertices = 0usize;
            if !source.face_materials.is_empty() {
                for (c, m) in source.face_materials.iter().enumerate() {
                    if *m as usize == b {
                        faces.push(c);
                        num_vertices += source
                            .pos_faces
                            .get(c)
                            .ok_or_else(|| "face material index out of range".to_string())?
                            .len();
                    }
                }
            } else {
                for (c, f) in source.pos_faces.iter().enumerate() {
                    faces.push(c);
                    num_vertices += f.len();
                }
            }
            if num_vertices == 0 {
                continue;
            }
            let mut mesh = Mesh {
                name: source.name.clone(),
                ..Mesh::default()
            };
            mesh.vertices = Vec::with_capacity(num_vertices);
            if !source.normals.is_empty() {
                mesh.normals = Vec::with_capacity(num_vertices);
            }
            for e in 0..8 {
                if !source.tex_coords[e].is_empty() {
                    mesh.texture_coords[e] = Vec::with_capacity(num_vertices);
                }
                if !source.colors[e].is_empty() {
                    mesh.colors[e] = Vec::with_capacity(num_vertices);
                }
            }
            let mut org_points: Vec<u32> = Vec::with_capacity(num_vertices);
            let mut new_index = 0u32;
            for &fc in &faces {
                let source_face = &source.pos_faces[fc];
                let mut face = Vec::with_capacity(source_face.len());
                for (d, &p_idx) in source_face.iter().enumerate() {
                    if p_idx as usize >= source.positions.len() {
                        return Err(format!("Invalid vertex index in face {} of mesh.", fc));
                    }
                    mesh.vertices.push(source.positions[p_idx as usize]);
                    if !source.normals.is_empty() {
                        let n_idx = *source
                            .norm_faces
                            .get(fc)
                            .and_then(|f| f.get(d))
                            .ok_or_else(|| "Invalid normal face".to_string())?
                            as usize;
                        if n_idx >= source.normals.len() {
                            return Err(format!(
                                "Invalid vertex index in normal face {} of mesh.",
                                fc
                            ));
                        }
                        mesh.normals.push(source.normals[n_idx]);
                    }
                    for e in 0..8 {
                        if !source.tex_coords[e].is_empty() {
                            let tex = source.tex_coords[e][p_idx as usize];
                            mesh.texture_coords[e].push(Vec3::new(tex[0], 1.0 - tex[1], 0.0));
                        }
                        if !source.colors[e].is_empty() {
                            mesh.colors[e].push(source.colors[e][p_idx as usize]);
                        }
                    }
                    org_points.push(p_idx);
                    face.push(new_index);
                    new_index += 1;
                }
                mesh.faces.push(face);
            }
            for e in 0..8 {
                if !source.tex_coords[e].is_empty() {
                    mesh.num_uv_components[e] = 2;
                }
            }
            if !source.bones.is_empty() {
                for obone in &source.bones {
                    let mut old_weights = vec![0.0f32; source.positions.len()];
                    for (v, w) in &obone.weights {
                        *old_weights
                            .get_mut(*v as usize)
                            .ok_or_else(|| "bone weight vertex out of range".to_string())? = *w;
                    }
                    let mut new_weights = Vec::new();
                    for (d, org) in org_points.iter().enumerate() {
                        let w = old_weights[*org as usize];
                        if w > 0.0 {
                            new_weights.push(VertexWeight {
                                vertex_id: d as u32,
                                weight: w,
                            });
                        }
                    }
                    if new_weights.is_empty() {
                        continue;
                    }
                    mesh.bones.push(Bone {
                        name: obone.name.clone(),
                        weights: new_weights,
                        offset_matrix: obone.offset_matrix,
                    });
                }
            }
            mesh.material_index = if source.materials.is_empty() {
                0
            } else {
                source.materials[b].scene_index
            };
            indices.push(scene.meshes.len() as u32);
            scene.meshes.push(mesh);
        }
    }
    Ok(indices)
}

/// XFileImporter::CreateNodes
fn create_nodes(scene: &mut Scene, p_node: &mut XNode) -> Result<Node, String> {
    let meshes = create_meshes(scene, &mut p_node.meshes)?;
    let mut node = Node {
        name: p_node.name.clone(),
        transformation: p_node.trafo_matrix,
        meshes,
        children: Vec::new(),
    };
    for c in p_node.children.iter_mut() {
        node.children.push(create_nodes(scene, c)?);
    }
    Ok(node)
}

/// XFileImporter::CreateAnimations
fn create_animations(scene: &mut Scene, data: &XScene) {
    for anim in &data.anims {
        if anim.anims.is_empty() {
            continue;
        }
        let mut nanim = Animation {
            name: anim.name.clone(),
            duration: 0.0,
            ticks_per_second: f64::from(data.anim_ticks_per_second),
            channels: Vec::new(),
        };
        for bone in &anim.anims {
            let mut nbone = NodeAnim {
                node_name: bone.bone_name.clone(),
                position_keys: Vec::new(),
                rotation_keys: Vec::new(),
                scaling_keys: Vec::new(),
                pre_state: 0,
                post_state: 0,
            };
            if !bone.trafo_keys.is_empty() {
                for (time, m) in &bone.trafo_keys {
                    let (scale, rot, pos) = m.decompose();
                    nbone.position_keys.push(VectorKey {
                        time: *time,
                        value: pos,
                    });
                    nbone.rotation_keys.push(QuatKey {
                        time: *time,
                        value: rot,
                    });
                    nbone.scaling_keys.push(VectorKey {
                        time: *time,
                        value: scale,
                    });
                }
            } else {
                for (time, v) in &bone.pos_keys {
                    nbone.position_keys.push(VectorKey {
                        time: *time,
                        value: *v,
                    });
                }
                for (time, q) in &bone.rot_keys {
                    let quat = Quat {
                        w: q[0],
                        x: q[1],
                        y: q[2],
                        z: q[3],
                    };
                    let rotmat = quat.get_matrix();
                    let mut value = Quat::from_matrix(&rotmat);
                    value.w *= -1.0;
                    nbone.rotation_keys.push(QuatKey { time: *time, value });
                }
                for (time, v) in &bone.scale_keys {
                    nbone.scaling_keys.push(VectorKey {
                        time: *time,
                        value: *v,
                    });
                }
            }
            if let Some(k) = nbone.position_keys.last() {
                nanim.duration = nanim.duration.max(k.time);
            }
            if let Some(k) = nbone.rotation_keys.last() {
                nanim.duration = nanim.duration.max(k.time);
            }
            if let Some(k) = nbone.scaling_keys.last() {
                nanim.duration = nanim.duration.max(k.time);
            }
            nanim.channels.push(nbone);
        }
        scene.animations.push(nanim);
    }
}

/// XFileImporter::CreateDataRepresentationFromImport
fn create_data_representation(data: &mut XScene) -> Result<Scene, String> {
    let mut scene = Scene::default();
    convert_materials(&mut scene, &mut data.global_materials)?;
    let mut root = match data.root_node.as_mut() {
        Some(r) => Some(create_nodes(&mut scene, r)?),
        None => None,
    };
    create_animations(&mut scene, data);
    if !data.global_meshes.is_empty() {
        let mut meshes = std::mem::take(&mut data.global_meshes);
        let indices = create_meshes(&mut scene, &mut meshes)?;
        let r = root.get_or_insert_with(|| Node {
            name: "$dummy_node".to_string(),
            transformation: Default::default(),
            meshes: Vec::new(),
            children: Vec::new(),
        });
        r.meshes.extend(indices);
    }
    scene.root_node = root;
    make_left_handed::execute(&mut scene);
    flip_winding_order::execute(&mut scene);
    Ok(scene)
}

/// XFileImporter::InternReadFile followed by `Importer::ReadFile`'s ScenePreprocessor: the scene of an `.x` file as it
/// is before any post-processing step. `Err` is the DeadlyImportError text.
pub fn import(buffer: &[u8]) -> Result<Scene, String> {
    let mut data = x_file_parser::parse(buffer)?;
    let mut scene = create_data_representation(&mut data)?;
    if scene.root_node.is_none() {
        return Err("XFile is ill-formatted - no content imported.".to_string());
    }
    scene_preprocessor::process_scene(&mut scene);
    Ok(scene)
}
