//! assimp's `glTF2Importer.cpp`: `InternReadFile` and the `Import*` functions that turn the loaded asset into an
//! `aiScene` (materials, meshes, nodes with their skins, animations). The importer of the library inside the game's
//! `libjassimp` differs from the 5.0-5.2 sources in two ways that the output shows and that are ported here: the vertices
//! of a primitive are renumbered by first use in its index buffer (`ImportMeshes` builds a per-primitive remapping table
//! that `ExtractData` and the skin weights use), and the faces that point outside the vertex range are dropped.
use super::asset::{self as g, num_components, Asset, Interpolation, Path, Primitive, R};
use super::libcxx_map::UintMap;
use crate::native::assimp::scene::{
    Animation, Bone, Mat3, Mat4, Material, MaterialProperty, Mesh, Node, NodeAnim, Quat, QuatKey,
    Scene, Vec3, VectorKey, VertexWeight, PRIMITIVE_LINE, PRIMITIVE_POINT, PRIMITIVE_TRIANGLE,
};
use crate::native::assimp::x_file_importer::ai_string;

const PT_FLOAT: u32 = 1;
const PT_STRING: u32 = 3;
const PT_INTEGER: u32 = 4;
const PT_BUFFER: u32 = 5;

/// aiTextureType_DIFFUSE and aiTextureType_BASE_COLOR
const TEX_DIFFUSE: u32 = 1;
const TEX_BASE_COLOR: u32 = 12;

fn unported<T>(what: &str) -> R<T> {
    Err(format!("UNPORTED glTF2: {what}"))
}

fn f32s(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn add(mat: &mut Material, key: &str, semantic: u32, index: u32, ptype: u32, data: Vec<u8>) {
    mat.properties.push(MaterialProperty {
        key: key.to_string(),
        semantic,
        index,
        data,
        ptype,
    });
}

/// The floats of a packed element array.
fn floats(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// `glTF2Importer` members that live from `ImportMeshes` to `ImportNode`.
struct Importer<'a> {
    asset: &'a Asset,
    /// `meshOffsets`: the first aiMesh of every glTF mesh, plus the total
    mesh_offsets: Vec<usize>,
    /// the vertex remapping table of every aiMesh (empty when the primitive has no indices)
    remap: Vec<Vec<u32>>,
    meshes: Vec<Mesh>,
}

/// `glTF2Importer::InternReadFile` after the asset has been loaded.
pub fn convert(asset: &Asset) -> R<Scene> {
    let mut scene = Scene::default();
    let embedded = embedded_texture_indices(asset);
    // ImportMaterials: the document's materials, then the default one
    let mut materials = Vec::with_capacity(asset.materials.len() + 1);
    for m in &asset.materials {
        materials.push(import_material(&embedded, asset, m)?);
    }
    materials.push(import_material(&embedded, asset, &g::Material::default())?);
    scene.materials = materials;

    let mut imp = Importer {
        asset,
        mesh_offsets: Vec::new(),
        remap: Vec::new(),
        meshes: Vec::new(),
    };
    imp.import_meshes()?;
    // ImportCameras and ImportLights: the files have none (`UNPORTED` otherwise, see `Asset::read_nodes`)
    scene.root_node = imp.import_nodes()?;
    scene.meshes = std::mem::take(&mut imp.meshes);
    scene.animations = import_animations(asset)?;
    Ok(scene)
}

/// `glTF2Importer::ImportEmbeddedTextures`: the index of the embedded texture of every image (-1: an external file).
fn embedded_texture_indices(asset: &Asset) -> Vec<i32> {
    let mut next = 0;
    asset
        .images
        .iter()
        .map(|img| {
            if img.buffer_view.is_some() {
                next += 1;
                next - 1
            } else {
                -1
            }
        })
        .collect()
}

/// `aiTextureMapMode` of a glTF wrap mode (`Wrap` 0, `Clamp` 1, `Mirror` 2)
fn map_mode(wrap: Option<u32>) -> u32 {
    match wrap {
        Some(33071) => 1,
        Some(33648) => 2,
        _ => 0,
    }
}

/// `SetMaterialTextureProperty(embeddedTexIdxs, asset, TextureInfo, mat, texType, texSlot)`
fn set_texture(
    embedded: &[i32],
    asset: &Asset,
    info: &g::TextureInfo,
    mat: &mut Material,
    tex_type: u32,
) -> R<()> {
    let tex = &asset.textures[info.index];
    let Some(source) = tex.source else {
        return Ok(());
    };
    let uri = if embedded[source] != -1 {
        format!("*{}", embedded[source])
    } else {
        asset.images[source].uri.clone().unwrap_or_default()
    };
    add(mat, "$tex.file", tex_type, 0, PT_STRING, ai_string(&uri));
    add(
        mat,
        "$tex.uvwsrc",
        tex_type,
        0,
        PT_INTEGER,
        info.tex_coord.to_le_bytes().to_vec(),
    );
    let sampler = tex.sampler.map(|si| &asset.samplers[si]);
    if let Some(sampler) = sampler {
        add(
            mat,
            "$tex.mappingname",
            tex_type,
            0,
            PT_STRING,
            ai_string(&sampler.name),
        );
        add(
            mat,
            "$tex.mappingid",
            tex_type,
            0,
            PT_STRING,
            ai_string(&sampler.id),
        );
    }
    // a texture without a sampler wraps in both directions
    let (wrap_s, wrap_t) = sampler.map_or((None, None), |s| (s.wrap_s, s.wrap_t));
    add(
        mat,
        "$tex.mapmodeu",
        tex_type,
        0,
        PT_BUFFER,
        map_mode(wrap_s).to_le_bytes().to_vec(),
    );
    add(
        mat,
        "$tex.mapmodev",
        tex_type,
        0,
        PT_BUFFER,
        map_mode(wrap_t).to_le_bytes().to_vec(),
    );
    let Some(sampler) = sampler else {
        return Ok(());
    };
    if let Some(f) = sampler.mag_filter {
        add(
            mat,
            "$tex.mappingfiltermag",
            tex_type,
            0,
            PT_BUFFER,
            f.to_le_bytes().to_vec(),
        );
    }
    if let Some(f) = sampler.min_filter {
        add(
            mat,
            "$tex.mappingfiltermin",
            tex_type,
            0,
            PT_BUFFER,
            f.to_le_bytes().to_vec(),
        );
    }
    Ok(())
}

/// `ImportMaterial(embeddedTexIdxs, asset, glTF2::Material&)`
fn import_material(embedded: &[i32], asset: &Asset, m: &g::Material) -> R<Material> {
    let mut mat = Material::default();
    if !m.name.is_empty() {
        add(&mut mat, "?mat.name", 0, 0, PT_STRING, ai_string(&m.name));
    }
    add(
        &mut mat,
        "$clr.diffuse",
        0,
        0,
        PT_FLOAT,
        f32s(&m.base_color_factor),
    );
    add(
        &mut mat,
        "$clr.base",
        0,
        0,
        PT_FLOAT,
        f32s(&m.base_color_factor),
    );
    if let Some(t) = &m.base_color_texture {
        set_texture(embedded, asset, t, &mut mat, TEX_DIFFUSE)?;
        set_texture(embedded, asset, t, &mut mat, TEX_BASE_COLOR)?;
    }
    add(
        &mut mat,
        "$mat.metallicFactor",
        0,
        0,
        PT_FLOAT,
        f32s(&[m.metallic_factor]),
    );
    add(
        &mut mat,
        "$mat.roughnessFactor",
        0,
        0,
        PT_FLOAT,
        f32s(&[m.roughness_factor]),
    );
    let s = 1.0f32 - m.roughness_factor;
    let shininess = s * 1000.0 * s;
    add(
        &mut mat,
        "$mat.shininess",
        0,
        0,
        PT_FLOAT,
        f32s(&[shininess]),
    );
    let e = m.emissive_factor;
    add(
        &mut mat,
        "$clr.emissive",
        0,
        0,
        PT_FLOAT,
        f32s(&[e[0], e[1], e[2], 1.0]),
    );
    add(
        &mut mat,
        "$mat.twosided",
        0,
        0,
        PT_BUFFER,
        vec![u8::from(m.double_sided)],
    );
    add(
        &mut mat,
        "$mat.opacity",
        0,
        0,
        PT_FLOAT,
        f32s(&[m.base_color_factor[3]]),
    );
    add(
        &mut mat,
        "$mat.gltf.alphaMode",
        0,
        0,
        PT_STRING,
        ai_string(&m.alpha_mode),
    );
    add(
        &mut mat,
        "$mat.gltf.alphaCutoff",
        0,
        0,
        PT_FLOAT,
        f32s(&[m.alpha_cutoff]),
    );
    // aiShadingMode_PBR_BRDF
    add(
        &mut mat,
        "$mat.shadingm",
        0,
        0,
        PT_BUFFER,
        11i32.to_le_bytes().to_vec(),
    );
    Ok(mat)
}

/// `glTF2::Accessor` of the first entry of an attribute list (the library reads `attr.position[0]` and so on).
fn first(list: &[Option<usize>]) -> Option<usize> {
    list.first().copied().flatten()
}

fn vec3s(packed: &[u8]) -> Vec<Vec3> {
    floats(packed)
        .chunks_exact(3)
        .map(|c| Vec3::new(c[0], c[1], c[2]))
        .collect()
}

impl Importer<'_> {
    /// `glTF2Importer::ImportMeshes`: one aiMesh per primitive.
    fn import_meshes(&mut self) -> R<()> {
        let asset = self.asset;
        let n_materials = asset.materials.len();
        for mesh in &asset.meshes {
            self.mesh_offsets.push(self.meshes.len());
            for (p, prim) in mesh.primitives.iter().enumerate() {
                let mut name = if mesh.name.is_empty() {
                    mesh.id.clone()
                } else {
                    mesh.name.clone()
                };
                if mesh.primitives.len() > 1 {
                    name.push_str(&format!("-{p}"));
                }
                let (aim, remap) = self.import_primitive(&name, &mesh.name, prim, n_materials)?;
                self.meshes.push(aim);
                self.remap.push(remap);
            }
        }
        self.mesh_offsets.push(self.meshes.len());
        Ok(())
    }

    /// The body of the primitive loop of `ImportMeshes`: the aiMesh and the vertex remapping table it was built with.
    fn import_primitive(
        &self,
        name: &str,
        mesh_name: &str,
        prim: &Primitive,
        n_materials: usize,
    ) -> R<(Mesh, Vec<u32>)> {
        let asset = self.asset;
        let attr = &prim.attributes;
        let mut aim = Mesh {
            name: name.to_string(),
            ..Mesh::default()
        };
        aim.primitive_types = match prim.mode {
            0 => PRIMITIVE_POINT,
            1..=3 => PRIMITIVE_LINE,
            4..=6 => PRIMITIVE_TRIANGLE,
            _ => 0,
        };
        let position = first(&attr.position);
        let position_count = position.map_or(0, |a| asset.accessors[a].count);

        // the index buffer, renumbered by first use: the vertex remapping table (`usedVertexIndices`)
        let mut used: Vec<u32> = Vec::new();
        let mut index_buffer: Vec<u32> = Vec::new();
        if let Some(ia) = prim.indices {
            let raw = asset.indices(ia, &g::context(mesh_name, ""))?;
            let mut table: Vec<i64> = Vec::new();
            index_buffer.reserve(raw.len());
            for idx in raw {
                if (idx as usize) < position_count {
                    if table.len() <= idx as usize {
                        table.resize(idx as usize + 1, -1);
                    }
                    if table[idx as usize] == -1 {
                        table[idx as usize] = used.len() as i64;
                        used.push(idx);
                    }
                    index_buffer.push(table[idx as usize] as u32);
                } else {
                    index_buffer.push(idx);
                }
            }
        }
        let remap: Option<&[u32]> = prim.indices.map(|_| used.as_slice());

        if let Some(pa) = position {
            let data = asset.extract(pa, remap, 12)?;
            aim.vertices = vec3s(&data);
        }
        let num_vertices = aim.vertices.len();

        if let Some(na) = first(&attr.normal) {
            if asset.accessors[na].count == position_count {
                aim.normals = vec3s(&asset.extract(na, remap, 12)?);
                // tangents only exist next to normals; the bitangent is (N ^ T) * w
                if let Some(ta) = first(&attr.tangent) {
                    if asset.accessors[ta].count == position_count {
                        let t = floats(&asset.extract(ta, remap, 16)?);
                        for (i, c) in t.chunks_exact(4).enumerate() {
                            let tan = Vec3::new(c[0], c[1], c[2]);
                            let n = aim.normals[i];
                            let w = c[3];
                            aim.tangents.push(tan);
                            aim.bitangents.push(Vec3::new(
                                (n.y * tan.z - n.z * tan.y) * w,
                                (n.z * tan.x - n.x * tan.z) * w,
                                (n.x * tan.y - n.y * tan.x) * w,
                            ));
                        }
                    }
                }
            }
        }

        for (c, acc) in attr.color.iter().enumerate().take(8) {
            let Some(ca) = *acc else { continue };
            let a = &asset.accessors[ca];
            if a.count != position_count {
                continue;
            }
            if a.ty != "VEC4" {
                return unported(&format!("a {} colour accessor", a.ty));
            }
            aim.colors[c] = match a.component_type {
                5121 => asset
                    .extract(ca, remap, 4)?
                    .chunks_exact(4)
                    .map(|v| {
                        [
                            f32::from(v[0]) / 255.0,
                            f32::from(v[1]) / 255.0,
                            f32::from(v[2]) / 255.0,
                            f32::from(v[3]) / 255.0,
                        ]
                    })
                    .collect(),
                5123 => asset
                    .extract(ca, remap, 8)?
                    .chunks_exact(8)
                    .map(|v| {
                        let u =
                            |i: usize| f32::from(u16::from_le_bytes([v[i], v[i + 1]])) / 65535.0;
                        [u(0), u(2), u(4), u(6)]
                    })
                    .collect(),
                5126 => floats(&asset.extract(ca, remap, 16)?)
                    .chunks_exact(4)
                    .map(|v| [v[0], v[1], v[2], v[3]])
                    .collect(),
                other => return unported(&format!("colour component type {other}")),
            };
        }

        for (tc, acc) in attr.texcoord.iter().enumerate().take(8) {
            let Some(ta) = *acc else { continue };
            let a = &asset.accessors[ta];
            if a.count != position_count {
                continue;
            }
            if a.component_type != 5126 {
                return unported(&format!(
                    "texture coordinate component type {}",
                    a.component_type
                ));
            }
            let comps = num_components(&a.ty).unwrap_or(1);
            let packed = asset.extract(ta, remap, 12)?;
            let elem = comps * 4;
            aim.texture_coords[tc] = packed
                .chunks_exact(elem)
                .map(|e| {
                    let f = floats(e);
                    // the library copies `elem` bytes into an aiVector3D; the rest of it is not read
                    let get = |i: usize| f.get(i).copied().unwrap_or(0.0);
                    Vec3::new(get(0), 1.0 - get(1), get(2))
                })
                .collect();
            aim.num_uv_components[tc] = comps as u32;
        }

        // faces
        if prim.mode != 4 {
            if prim.mode < 7 {
                return unported(&format!("primitive mode {}", prim.mode));
            }
        } else {
            let nv = num_vertices as u32;
            let mut count = if prim.indices.is_some() {
                index_buffer.len()
            } else {
                num_vertices
            };
            let n_faces = count / 3;
            count = n_faces * 3;
            let at = |i: usize| -> u32 {
                if prim.indices.is_some() {
                    index_buffer[i]
                } else {
                    i as u32
                }
            };
            for i in (0..count).step_by(3) {
                let (a, b, c) = (at(i), at(i + 1), at(i + 2));
                if a < nv && b < nv && c < nv {
                    aim.faces.push(vec![a, b, c]);
                }
            }
        }
        if aim.faces.is_empty() {
            return Err(format!("Mesh \"{}\" has no faces", aim.name));
        }
        aim.material_index = match prim.material {
            Some(m) => m as u32,
            None => n_materials as u32,
        };
        Ok((aim, used))
    }

    /// `glTF2Importer::ImportNodes`
    fn import_nodes(&mut self) -> R<Option<Node>> {
        let Some(roots) = &self.asset.scene_nodes else {
            return Ok(None);
        };
        let roots = roots.clone();
        match roots.len() {
            0 => Ok(Some(plain_node("ROOT"))),
            1 => Ok(Some(self.import_node(roots[0])?)),
            _ => {
                let mut root = plain_node("ROOT");
                for r in roots {
                    root.children.push(self.import_node(r)?);
                }
                Ok(Some(root))
            }
        }
    }

    /// `glTF2Importer::ImportNode`: the node with its children, its meshes and, for a skinned node, the bones of the
    /// meshes.
    fn import_node(&mut self, index: usize) -> R<Node> {
        let asset = self.asset;
        let node = &asset.nodes[index];
        let mut out = plain_node(if node.name.is_empty() {
            &node.id
        } else {
            &node.name
        });
        for &c in &node.children {
            out.children.push(self.import_node(c)?);
        }
        out.transformation = node_transform(node);
        if !node.meshes.is_empty() {
            if node.meshes.len() > 1 {
                return Err(format!(
                    "GLTF: Invalid input, found {} meshes in {}, but only 1 mesh per node allowed.",
                    node.meshes.len(),
                    g::context(&node.id, &node.name)
                ));
            }
            let m = node.meshes[0];
            let (lo, hi) = (self.mesh_offsets[m], self.mesh_offsets[m + 1]);
            out.meshes = (lo..hi).map(|i| i as u32).collect();
            if let Some(si) = node.skin {
                if hi > lo {
                    for p in 0..hi - lo {
                        self.meshes[lo + p].bones = self.build_bones(si, m, p, lo + p)?;
                    }
                }
            }
        }
        Ok(out)
    }

    /// The skin part of `ImportNode`: `BuildVertexWeightMapping` for one primitive and the aiBones of the skin.
    fn build_bones(&self, skin: usize, mesh: usize, prim: usize, ai_mesh: usize) -> R<Vec<Bone>> {
        let asset = self.asset;
        let skin = &asset.skins[skin];
        let primitive = &asset.meshes[mesh].primitives[prim];
        let attr = &primitive.attributes;
        let n_joints = skin.joints.len();
        let remap = &self.remap[ai_mesh];
        let remap: Option<&[u32]> = primitive.indices.map(|_| remap.as_slice());
        let mut weighting: Vec<Vec<VertexWeight>> = vec![Vec::new(); n_joints];

        let (w0, j0) = (first(&attr.weight), first(&attr.joint));
        if let (Some(w0), Some(j0)) = (w0, j0) {
            if asset.accessors[w0].count == asset.accessors[j0].count {
                for (set, wa) in attr.weight.iter().enumerate() {
                    let wa =
                        wa.ok_or_else(|| "GLTF2: data is null when extracting data".to_string())?;
                    let ja = attr.joint.get(set).copied().flatten().ok_or_else(|| {
                        "UNPORTED glTF2: more weight sets than joint sets".to_string()
                    })?;
                    let acc = &asset.accessors[wa];
                    if acc.component_type != 5126 || acc.ty != "VEC4" {
                        return unported("weights that are not a float VEC4");
                    }
                    let weights = floats(&asset.extract(wa, remap, 16)?);
                    let jacc = &asset.accessors[ja];
                    if jacc.ty != "VEC4" || !matches!(jacc.component_type, 5121 | 5123) {
                        return unported("joints that are not an unsigned byte or short VEC4");
                    }
                    let joints = asset.extract(
                        ja,
                        remap,
                        if jacc.component_type == 5121 { 4 } else { 8 },
                    )?;
                    let n = weights.len() / 4;
                    for v in 0..n {
                        for j in 0..4 {
                            let bone = if jacc.component_type == 5121 {
                                usize::from(joints[v * 4 + j])
                            } else {
                                let o = (v * 4 + j) * 2;
                                usize::from(u16::from_le_bytes([joints[o], joints[o + 1]]))
                            };
                            let weight = weights[v * 4 + j];
                            if weight > 0.0 && bone < n_joints {
                                weighting[bone].push(VertexWeight {
                                    vertex_id: v as u32,
                                    weight,
                                });
                            }
                        }
                    }
                }
            }
        }

        let Some(ibm) = skin.inverse_bind_matrices else {
            return unported("a skin without inverseBindMatrices");
        };
        let ibm_acc = &asset.accessors[ibm];
        if ibm_acc.component_type != 5126 || ibm_acc.ty != "MAT4" {
            return unported("inverse bind matrices that are not a float MAT4");
        }
        let ibm = floats(&asset.extract(ibm, None, 64)?);
        if ibm.len() < n_joints * 16 {
            return Err("GLTF: fewer inverse bind matrices than joints".to_string());
        }
        let mut bones = Vec::with_capacity(n_joints);
        for (i, &ji) in skin.joints.iter().enumerate() {
            let joint = &asset.nodes[ji];
            // an unnamed node is named after its id when it is loaded (`Node::Read`), so the name is never empty
            let name = joint.name.clone();
            let mut weights = std::mem::take(&mut weighting[i]);
            if weights.is_empty() {
                weights.push(VertexWeight {
                    vertex_id: 0,
                    weight: 0.0,
                });
            }
            let m: [f32; 16] = std::array::from_fn(|k| ibm[i * 16 + k]);
            bones.push(Bone {
                name,
                weights,
                offset_matrix: mat_from_gltf(&m),
            });
        }
        Ok(bones)
    }
}

fn plain_node(name: &str) -> Node {
    Node {
        name: name.to_string(),
        transformation: Mat4::default(),
        meshes: Vec::new(),
        children: Vec::new(),
    }
}

/// `CopyValue(const glTF2::mat4&, aiMatrix4x4&)`: glTF matrices are column major.
fn mat_from_gltf(v: &[f32; 16]) -> Mat4 {
    Mat4 {
        a1: v[0],
        b1: v[1],
        c1: v[2],
        d1: v[3],
        a2: v[4],
        b2: v[5],
        c2: v[6],
        d2: v[7],
        a3: v[8],
        b3: v[9],
        c3: v[10],
        d3: v[11],
        a4: v[12],
        b4: v[13],
        c4: v[14],
        d4: v[15],
    }
}

/// `aiMatrix4x4(const aiMatrix3x3&)`
fn mat4_from_mat3(m: &Mat3) -> Mat4 {
    Mat4 {
        a1: m.a1,
        a2: m.a2,
        a3: m.a3,
        b1: m.b1,
        b2: m.b2,
        b3: m.b3,
        c1: m.c1,
        c2: m.c2,
        c3: m.c3,
        ..Mat4::default()
    }
}

/// `GetNodeTransform`: the matrix, or translation * rotation * scale multiplied in that order.
fn node_transform(node: &g::Node) -> Mat4 {
    if let Some(m) = &node.matrix {
        return mat_from_gltf(m);
    }
    let mut matrix = Mat4::default();
    if let Some(t) = node.translation {
        matrix = matrix.mul(&Mat4::translation(Vec3::new(t[0], t[1], t[2])));
    }
    if let Some(r) = node.rotation {
        let q = Quat {
            w: r[3],
            x: r[0],
            y: r[1],
            z: r[2],
        };
        matrix = matrix.mul(&mat4_from_mat3(&q.get_matrix()));
    }
    if let Some(s) = node.scale {
        matrix = matrix.mul(&Mat4::scaling(Vec3::new(s[0], s[1], s[2])));
    }
    matrix
}

/// `AnimationSamplers`: the sampler of each path of one animated node (an index into the animation's samplers)
#[derive(Default)]
struct Samplers {
    translation: Option<usize>,
    rotation: Option<usize>,
    scale: Option<usize>,
    weights: Option<usize>,
}

/// `GatherSamplers(glTF2::Animation&)`: the samplers by animated node, in the order of libc++'s `unordered_map`.
fn gather_samplers(asset: &Asset, anim: &g::Animation) -> UintMap<Samplers> {
    let mut map: UintMap<Samplers> = UintMap::default();
    for ch in &anim.channels {
        let Ok(si) = usize::try_from(ch.sampler) else {
            continue;
        };
        let Some(s) = anim.samplers.get(si) else {
            continue;
        };
        let (Some(input), Some(output)) = (s.input, s.output) else {
            continue; // "Missing sampler input/output. Skipping."
        };
        if asset.accessors[output].count < asset.accessors[input].count {
            continue; // "Number of keyframes in sampler input exceeds number of keyframes in sampler output"
        }
        let e = map.entry(ch.node as u32);
        match ch.path {
            Path::Translation => e.translation = Some(si),
            Path::Rotation => e.rotation = Some(si),
            Path::Scale => e.scale = Some(si),
            Path::Weights => e.weights = Some(si),
        }
    }
    map
}

/// `CreateNodeAnim`: the key arrays of one node; the times are `seconds * 1000.0f`.
fn create_node_anim(
    asset: &Asset,
    anim: &g::Animation,
    node: &g::Node,
    s: &Samplers,
) -> R<NodeAnim> {
    let name = if node.name.is_empty() {
        &node.id
    } else {
        &node.name
    };
    let mut out = NodeAnim {
        node_name: name.clone(),
        position_keys: Vec::new(),
        rotation_keys: Vec::new(),
        scaling_keys: Vec::new(),
        pre_state: 0,
        post_state: 0,
    };
    // times and values of a sampler; the values of CUBICSPLINE are in (in-tangent, value, out-tangent) triples
    let channel = |si: usize, comps: usize| -> R<(Vec<f32>, Vec<f32>, bool)> {
        let sampler = &anim.samplers[si];
        let (Some(input), Some(output)) = (sampler.input, sampler.output) else {
            return Err("GLTF: sampler without input or output".to_string());
        };
        let (ia, oa) = (&asset.accessors[input], &asset.accessors[output]);
        if ia.component_type != 5126 || ia.ty != "SCALAR" {
            return unported("animation times that are not float scalars");
        }
        if oa.component_type != 5126 || num_components(&oa.ty) != Some(comps) {
            return unported("animation values that are not floats of the expected type");
        }
        let times = floats(&asset.extract(input, None, 4)?);
        let values = floats(&asset.extract(output, None, comps * 4)?);
        Ok((
            times,
            values,
            sampler.interpolation == Interpolation::CubicSpline,
        ))
    };
    let key_value = |values: &[f32], comps: usize, ii: usize| -> R<Vec<f32>> {
        values
            .get(ii * comps..ii * comps + comps)
            .map(<[f32]>::to_vec)
            .ok_or_else(|| "GLTF: animation output has fewer values than keys".to_string())
    };
    let ms = 1000.0f32;

    if let Some(si) = s.translation {
        let (times, values, cubic) = channel(si, 3)?;
        let mut ii = usize::from(cubic);
        for t in &times {
            let v = key_value(&values, 3, ii)?;
            out.position_keys.push(VectorKey {
                time: f64::from(t * ms),
                value: Vec3::new(v[0], v[1], v[2]),
            });
            ii += if cubic { 3 } else { 1 };
        }
    } else if let Some(t) = node.translation {
        out.position_keys.push(VectorKey {
            time: 0.0,
            value: Vec3::new(t[0], t[1], t[2]),
        });
    }

    if let Some(si) = s.rotation {
        let (times, values, cubic) = channel(si, 4)?;
        let mut ii = usize::from(cubic);
        for t in &times {
            let v = key_value(&values, 4, ii)?;
            out.rotation_keys.push(QuatKey {
                time: f64::from(t * ms),
                value: Quat {
                    w: v[3],
                    x: v[0],
                    y: v[1],
                    z: v[2],
                },
            });
            ii += if cubic { 3 } else { 1 };
        }
    } else if let Some(r) = node.rotation {
        out.rotation_keys.push(QuatKey {
            time: 0.0,
            value: Quat {
                w: r[3],
                x: r[0],
                y: r[1],
                z: r[2],
            },
        });
    }

    if let Some(si) = s.scale {
        let (times, values, cubic) = channel(si, 3)?;
        let mut ii = usize::from(cubic);
        for t in &times {
            let v = key_value(&values, 3, ii)?;
            out.scaling_keys.push(VectorKey {
                time: f64::from(t * ms),
                value: Vec3::new(v[0], v[1], v[2]),
            });
            ii += if cubic { 3 } else { 1 };
        }
    } else if let Some(sc) = node.scale {
        out.scaling_keys.push(VectorKey {
            time: 0.0,
            value: Vec3::new(sc[0], sc[1], sc[2]),
        });
    }
    Ok(out)
}

/// `glTF2Importer::ImportAnimations`
fn import_animations(asset: &Asset) -> R<Vec<Animation>> {
    if asset.scene_nodes.is_none() {
        return Ok(Vec::new());
    }
    let mut out = Vec::with_capacity(asset.animations.len());
    for anim in &asset.animations {
        let samplers = gather_samplers(asset, anim);
        let mut channels = Vec::new();
        for (node, s) in samplers.iter() {
            if s.translation.is_some() || s.rotation.is_some() || s.scale.is_some() {
                channels.push(create_node_anim(
                    asset,
                    anim,
                    &asset.nodes[node as usize],
                    s,
                )?);
            }
        }
        // the duration is the latest key of any channel
        let mut duration = 0.0f64;
        for c in &channels {
            if let Some(k) = c.position_keys.last() {
                if k.time > duration {
                    duration = k.time;
                }
            }
            if let Some(k) = c.rotation_keys.last() {
                if k.time > duration {
                    duration = k.time;
                }
            }
            if let Some(k) = c.scaling_keys.last() {
                if k.time > duration {
                    duration = k.time;
                }
            }
        }
        out.push(Animation {
            name: anim.name.clone(),
            duration,
            ticks_per_second: 1000.0,
            channels,
        });
    }
    Ok(out)
}
