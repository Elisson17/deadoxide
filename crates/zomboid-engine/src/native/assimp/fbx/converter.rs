//! assimp `FBXConverter.cpp` (and the `FBXImporter::InternReadFile` settings it runs with): the FBX DOM converted to
//! an `aiScene`. The importer settings are the library's defaults (`ImportSettings`: read all layers, materials,
//! textures, cameras, lights, animations and weights, preserve pivots, optimize empty animation curves, remove empty
//! bones; no strict mode, no unit conversion).
use super::document::*;
use super::parser::R;
use super::properties::*;
use crate::native::assimp::scene::{
    Animation, Bone, Mat3, Mat4, Material, MaterialProperty, Mesh, Node, NodeAnim, Quat, QuatKey,
    Scene, Vec3, VectorKey, VertexWeight, PRIMITIVE_LINE, PRIMITIVE_POINT, PRIMITIVE_POLYGON,
    PRIMITIVE_TRIANGLE,
};
use crate::native::assimp::x_file_importer::ai_string;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

const MAGIC_NODE_TAG: &str = "_$AssimpFbx$";
/// `std::numeric_limits<float>::epsilon()`, the library's `Math::getEpsilon<float>()` in this build
const EPSILON: f32 = 1.1920929e-7;
const NO_MATERIAL_SEPARATION: i64 = -1;

// aiPropertyTypeInfo
const PT_FLOAT: u32 = 1;
const PT_STRING: u32 = 3;
const PT_INTEGER: u32 = 4;
const PT_BUFFER: u32 = 5;

// aiTextureType
const TT_DIFFUSE: u32 = 1;
const TT_SPECULAR: u32 = 2;
const TT_AMBIENT: u32 = 3;
const TT_EMISSIVE: u32 = 4;
const TT_HEIGHT: u32 = 5;
const TT_NORMALS: u32 = 6;
const TT_SHININESS: u32 = 7;
const TT_OPACITY: u32 = 8;
const TT_DISPLACEMENT: u32 = 9;
const TT_REFLECTION: u32 = 11;
const TT_BASE_COLOR: u32 = 12;
const TT_NORMAL_CAMERA: u32 = 13;
const TT_EMISSION_COLOR: u32 = 14;
const TT_METALNESS: u32 = 15;
const TT_DIFFUSE_ROUGHNESS: u32 = 16;
const TT_AMBIENT_OCCLUSION: u32 = 17;
const TT_UNKNOWN: u32 = 18;

// FBXConverter.h TransformationComp
const TC_GEOMETRIC_SCALING_INVERSE: usize = 0;
const TC_GEOMETRIC_ROTATION_INVERSE: usize = 1;
const TC_GEOMETRIC_TRANSLATION_INVERSE: usize = 2;
const TC_TRANSLATION: usize = 3;
const TC_ROTATION_OFFSET: usize = 4;
const TC_ROTATION_PIVOT: usize = 5;
const TC_PRE_ROTATION: usize = 6;
const TC_ROTATION: usize = 7;
const TC_POST_ROTATION: usize = 8;
const TC_ROTATION_PIVOT_INVERSE: usize = 9;
const TC_SCALING_OFFSET: usize = 10;
const TC_SCALING_PIVOT: usize = 11;
const TC_SCALING: usize = 12;
const TC_SCALING_PIVOT_INVERSE: usize = 13;
const TC_GEOMETRIC_TRANSLATION: usize = 14;
const TC_GEOMETRIC_ROTATION: usize = 15;
const TC_GEOMETRIC_SCALING: usize = 16;
const TC_MAXIMUM: usize = 17;

fn comp_name(c: usize) -> &'static str {
    match c {
        TC_TRANSLATION => "Translation",
        TC_SCALING => "Scaling",
        TC_ROTATION => "Rotation",
        TC_ROTATION_OFFSET => "RotationOffset",
        TC_ROTATION_PIVOT => "RotationPivot",
        TC_PRE_ROTATION => "PreRotation",
        TC_POST_ROTATION => "PostRotation",
        TC_ROTATION_PIVOT_INVERSE => "RotationPivotInverse",
        TC_SCALING_OFFSET => "ScalingOffset",
        TC_SCALING_PIVOT => "ScalingPivot",
        TC_SCALING_PIVOT_INVERSE => "ScalingPivotInverse",
        TC_GEOMETRIC_TRANSLATION => "GeometricTranslation",
        TC_GEOMETRIC_ROTATION => "GeometricRotation",
        TC_GEOMETRIC_SCALING => "GeometricScaling",
        TC_GEOMETRIC_SCALING_INVERSE => "GeometricScalingInverse",
        TC_GEOMETRIC_ROTATION_INVERSE => "GeometricRotationInverse",
        _ => "GeometricTranslationInverse",
    }
}

fn comp_property(c: usize) -> &'static str {
    match c {
        TC_TRANSLATION => "Lcl Translation",
        TC_SCALING => "Lcl Scaling",
        TC_ROTATION => "Lcl Rotation",
        TC_ROTATION_OFFSET => "RotationOffset",
        TC_ROTATION_PIVOT => "RotationPivot",
        TC_PRE_ROTATION => "PreRotation",
        TC_POST_ROTATION => "PostRotation",
        TC_SCALING_OFFSET => "ScalingOffset",
        TC_SCALING_PIVOT => "ScalingPivot",
        TC_GEOMETRIC_TRANSLATION => "GeometricTranslation",
        TC_GEOMETRIC_ROTATION => "GeometricRotation",
        TC_GEOMETRIC_SCALING => "GeometricScaling",
        _ => "",
    }
}

fn chain_node_name(name: &str, comp: usize) -> String {
    format!("{name}{MAGIC_NODE_TAG}_{}", comp_name(comp))
}

/// aiVector3D::SquareLength
fn sq_len(v: Vec3) -> f32 {
    v.x * v.x + v.y * v.y + v.z * v.z
}

/// `(v - (1, 1, 1)).SquareLength()`
fn sq_len_minus_one(v: Vec3) -> f32 {
    (v.x + -1.0) * (v.x + -1.0) + (v.y + -1.0) * (v.y + -1.0) + (v.z + -1.0) * (v.z + -1.0)
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

/// `AI_DEG_TO_RAD(x)`: `x * (ai_real)0.0174532925`
fn deg_to_rad(x: f32) -> f32 {
    x * 0.017453292_f32
}

/// `Model::RotOrder` and `GetRotationMatrix`
fn rotation_matrix(order: i32, rotation: Vec3) -> Mat4 {
    if order == 6 {
        // RotOrder_SphericXYZ is not supported
        return Mat4::default();
    }
    let eps = EPSILON;
    let mut is_id = [true; 3];
    let mut temp = [Mat4::default(); 3];
    if rotation.z.abs() > eps {
        temp[2] = Mat4::rotation_z(deg_to_rad(rotation.z));
        is_id[2] = false;
    }
    if rotation.y.abs() > eps {
        temp[1] = Mat4::rotation_y(deg_to_rad(rotation.y));
        is_id[1] = false;
    }
    if rotation.x.abs() > eps {
        temp[0] = Mat4::rotation_x(deg_to_rad(rotation.x));
        is_id[0] = false;
    }
    // rotation order is inverted since the matrices are left-multiplied
    let ord: [usize; 3] = match order {
        0 => [2, 1, 0],
        1 => [1, 2, 0],
        2 => [0, 2, 1],
        3 => [2, 0, 1],
        4 => [1, 0, 2],
        _ => [0, 1, 2],
    };
    let mut out = Mat4::default();
    if !is_id[ord[0]] {
        out = temp[ord[0]];
    }
    if !is_id[ord[1]] {
        out = mul_in_library_order(&out, &temp[ord[1]]);
    }
    if !is_id[ord[2]] {
        out = mul_in_library_order(&out, &temp[ord[2]]);
    }
    out
}

/// `a * b` with the sums in the order the compiled `GetRotationMatrix` evaluates them (column 4 first, then 3, 1, 2)
fn mul_in_library_order(a: &Mat4, b: &Mat4) -> Mat4 {
    let x = [
        [a.a1, a.a2, a.a3, a.a4],
        [a.b1, a.b2, a.b3, a.b4],
        [a.c1, a.c2, a.c3, a.c4],
        [a.d1, a.d2, a.d3, a.d4],
    ];
    let t = [
        [b.a1, b.a2, b.a3, b.a4],
        [b.b1, b.b2, b.b3, b.b4],
        [b.c1, b.c2, b.c3, b.c4],
        [b.d1, b.d2, b.d3, b.d4],
    ];
    let mut r = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            r[i][j] =
                ((x[i][3] * t[3][j] + x[i][2] * t[2][j]) + x[i][0] * t[0][j]) + x[i][1] * t[1][j];
        }
    }
    Mat4 {
        a1: r[0][0],
        a2: r[0][1],
        a3: r[0][2],
        a4: r[0][3],
        b1: r[1][0],
        b2: r[1][1],
        b3: r[1][2],
        b4: r[1][3],
        c1: r[2][0],
        c2: r[2][1],
        c3: r[2][2],
        c4: r[2][3],
        d1: r[3][0],
        d2: r[3][1],
        d3: r[3][2],
        d4: r[3][3],
    }
}

fn mat3_of(m: &Mat4) -> Mat3 {
    Mat3 {
        a1: m.a1,
        a2: m.a2,
        a3: m.a3,
        b1: m.b1,
        b2: m.b2,
        b3: m.b3,
        c1: m.c1,
        c2: m.c2,
        c3: m.c3,
    }
}

fn mat4_of(m: &Mat3) -> Mat4 {
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

/// `pre * key` with the sums in the order the library evaluates them (single precision, no fused multiply-add)
fn quat_pre_multiply(p: Quat, k: Quat) -> Quat {
    Quat {
        w: ((-p.z * k.z + k.w * p.w) + (-p.x * k.x)) + k.y * -p.y,
        x: ((-p.z * k.y + k.x * p.w) + k.w * p.x) + k.z * p.y,
        y: ((-p.x * k.z + k.y * p.w) + k.w * p.y) + k.x * p.z,
        z: ((-p.y * k.x + k.z * p.w) + k.w * p.z) + k.y * p.x,
    }
}

/// `key * post`, same convention
fn quat_post_multiply(k: Quat, p: Quat) -> Quat {
    Quat {
        w: ((-k.y * p.y + k.w * p.w) + (-k.x * p.x)) - k.z * p.z,
        x: ((k.y * p.z + k.w * p.x) + p.w * k.x) - k.z * p.y,
        y: ((k.z * p.x + k.w * p.y) + k.y * p.w) - k.x * p.z,
        z: ((k.x * p.y + k.w * p.z) + k.z * p.w) - k.y * p.x,
    }
}

fn euler_to_quaternion(rot: Vec3, order: i32) -> Quat {
    Quat::from_matrix(&mat3_of(&rotation_matrix(order, rot)))
}

fn convert_fbx_time(t: i64) -> f64 {
    t as f64 / 46_186_158_000.0
}

fn fix_node_name(name: &str) -> String {
    name.strip_prefix("Model::").unwrap_or(name).to_string()
}

/// FrameRateToDouble
fn frame_rate_to_double(fps: i32, custom: f32) -> f64 {
    match fps {
        0 => 1.0,
        1 => 120.0,
        2 => 100.0,
        3 => 60.0,
        4 => 50.0,
        5 => 48.0,
        6 | 7 => 30.0,
        8 | 9 => 29.9700262,
        10 => 25.0,
        11 => 24.0,
        12 => 1000.0,
        13 => 23.976,
        14 => f64::from(custom),
        _ => -1.0,
    }
}

fn add_property(
    mat: &mut Material,
    key: &str,
    semantic: u32,
    index: u32,
    ptype: u32,
    data: Vec<u8>,
) {
    if let Some(p) = mat
        .properties
        .iter_mut()
        .find(|p| p.key == key && p.semantic == semantic && p.index == index)
    {
        p.data = data;
        p.ptype = ptype;
        return;
    }
    mat.properties.push(MaterialProperty {
        key: key.to_string(),
        semantic,
        index,
        data,
        ptype,
    });
}

fn f32s(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn add_color(mat: &mut Material, key: &str, c: [f32; 3]) {
    add_property(mat, key, 0, 0, PT_FLOAT, f32s(&c));
}

fn add_float(mat: &mut Material, key: &str, v: f32) {
    add_property(mat, key, 0, 0, PT_FLOAT, f32s(&[v]));
}

type KeyFrameList = (Rc<Vec<i64>>, Rc<Vec<f32>>, usize);

struct Conv<'a> {
    doc: &'a Document,
    meshes: Vec<Mesh>,
    materials: Vec<Material>,
    animations: Vec<Animation>,
    num_cameras: u32,
    num_lights: u32,
    default_material: Option<u32>,
    materials_converted: HashMap<u64, u32>,
    meshes_converted: HashMap<u64, Vec<u32>>,
    node_anim_chain_bits: HashMap<String, u32>,
    node_names: HashMap<String, u32>,
    anim_fps: f64,
    bone_map: HashMap<String, usize>,
}

pub fn convert(doc: &Document) -> R<Scene> {
    let mut c = Conv {
        doc,
        meshes: Vec::new(),
        materials: Vec::new(),
        animations: Vec::new(),
        num_cameras: 0,
        num_lights: 0,
        default_material: None,
        materials_converted: HashMap::new(),
        meshes_converted: HashMap::new(),
        node_anim_chain_bits: HashMap::new(),
        node_names: HashMap::new(),
        anim_fps: 0.0,
        bone_map: HashMap::new(),
    };
    // animations first: they fill `node_anim_chain_bits`, which the node conversion consults
    c.convert_animations()?;
    let mut root = Node {
        name: String::new(),
        transformation: Mat4::default(),
        meshes: Vec::new(),
        children: Vec::new(),
    };
    root.name = c.unique_name("RootNode");
    root.children = c.convert_nodes(0, &root.transformation.clone());
    Ok(Scene {
        meshes: c.meshes,
        materials: c.materials,
        animations: c.animations,
        root_node: Some(root),
        num_cameras: c.num_cameras,
        num_lights: c.num_lights,
    })
}

impl<'a> Conv<'a> {
    /// GetUniqueName
    fn unique_name(&mut self, name: &str) -> String {
        let mut unique = name.to_string();
        let mut inserted = !self.node_names.contains_key(name);
        self.node_names.entry(name.to_string()).or_insert(0);
        while !inserted {
            let i = {
                let e = self.node_names.get_mut(name).unwrap();
                *e += 1;
                *e
            };
            unique = format!("{name}{i:03}");
            inserted = !self.node_names.contains_key(&unique);
            self.node_names.entry(unique.clone()).or_insert(0);
        }
        unique
    }

    // ---------------------------------------------------------------------------------------------------------
    // nodes
    // ---------------------------------------------------------------------------------------------------------

    /// ConvertNodes: the nodes of the models connected to `id`; an error while converting drops them all.
    fn convert_nodes(&mut self, id: u64, parent_transform: &Mat4) -> Vec<Node> {
        self.convert_nodes_inner(id, parent_transform)
            .unwrap_or_default()
    }

    fn convert_nodes_inner(&mut self, id: u64, parent_transform: &Mat4) -> R<Vec<Node>> {
        let conns = self.doc.conns_by_dest(id, Some(&["Model"]));
        let mut nodes: Vec<Node> = Vec::new();
        for con in conns {
            if !con.prop.is_empty() {
                continue;
            }
            let Some(object) = self.doc.source_object(&con) else {
                continue;
            };
            let Kind::Model(model) = &object.kind else {
                continue;
            };
            let mut new_abs_transform = *parent_transform;
            let node_name = {
                let n = fix_node_name(&object.name);
                self.unique_name(&n)
            };
            let (mut chain, post_chain, need_additional) =
                self.generate_transformation_node_chain(model, &node_name)?;
            if need_additional {
                chain.push(Node {
                    name: node_name.clone(),
                    transformation: Mat4::default(),
                    meshes: Vec::new(),
                    children: Vec::new(),
                });
            }
            for n in &chain {
                new_abs_transform = new_abs_transform.mul(&n.transformation);
            }
            // attach geometry
            let mut mesh_ids = Vec::new();
            self.convert_model(
                model,
                &object,
                &chain.last().unwrap().name.clone(),
                &new_abs_transform,
                &mut mesh_ids,
            )?;
            chain.last_mut().unwrap().meshes = mesh_ids;

            let has_children = !self
                .doc
                .conns_by_dest(object.id, Some(&["Model"]))
                .is_empty();
            if has_children {
                for p in post_chain {
                    new_abs_transform = new_abs_transform.mul(&p.transformation);
                    chain.push(p);
                }
            }
            let last_transform = chain.last().unwrap().transformation;
            let children = self.convert_nodes(object.id, &last_transform);

            // lights and cameras
            for att in &model.attributes {
                if let Kind::NodeAttr(a) = &att.kind {
                    match a.kind {
                        AttrKind::Light => self.num_lights += 1,
                        AttrKind::Camera => self.num_cameras += 1,
                        _ => {}
                    }
                }
            }

            // link the chain
            let mut node = chain.pop().unwrap();
            node.children = children;
            while let Some(mut p) = chain.pop() {
                p.children = vec![node];
                node = p;
            }
            nodes.push(node);
        }
        Ok(nodes)
    }

    /// GenerateTransformationNodeChain: the nodes, the post (inverse geometric) nodes and whether the model's own node
    /// still has to be added. The library keeps a bit mask of the components that differ from their default; the
    /// chain's nodes are the masked components (plus the animated ones), not the non-identity matrices.
    fn generate_transformation_node_chain(
        &mut self,
        model: &Model,
        name: &str,
    ) -> R<(Vec<Node>, Vec<Node>, bool)> {
        let props = &model.props;
        let rot = props.int("RotationOrder", 0)?;
        let mut chain = [Mat4::default(); TC_MAXIMUM];
        let mut mask = 0u32;
        let bit = |c: usize| 1u32 << c;
        let neg = |v: Vec3| Vec3::new(-v.x, -v.y, -v.z);

        if let Some(v) = props.opt_vec3("PreRotation", false)? {
            if sq_len(v) > EPSILON {
                chain[TC_PRE_ROTATION] = rotation_matrix(0, v);
                mask |= bit(TC_PRE_ROTATION);
            }
        }
        if let Some(v) = props.opt_vec3("PostRotation", false)? {
            if sq_len(v) > EPSILON {
                chain[TC_POST_ROTATION] = rotation_matrix(0, v);
                mask |= bit(TC_POST_ROTATION);
            }
        }
        if let Some(v) = props.opt_vec3("RotationPivot", false)? {
            if sq_len(v) > EPSILON {
                chain[TC_ROTATION_PIVOT] = Mat4::translation(v);
                chain[TC_ROTATION_PIVOT_INVERSE] = Mat4::translation(neg(v));
                mask |= bit(TC_ROTATION_PIVOT) | bit(TC_ROTATION_PIVOT_INVERSE);
            }
        }
        if let Some(v) = props.opt_vec3("RotationOffset", false)? {
            if sq_len(v) > EPSILON {
                chain[TC_ROTATION_OFFSET] = Mat4::translation(v);
                mask |= bit(TC_ROTATION_OFFSET);
            }
        }
        if let Some(v) = props.opt_vec3("ScalingOffset", false)? {
            if sq_len(v) > EPSILON {
                chain[TC_SCALING_OFFSET] = Mat4::translation(v);
                mask |= bit(TC_SCALING_OFFSET);
            }
        }
        if let Some(v) = props.opt_vec3("ScalingPivot", false)? {
            if sq_len(v) > EPSILON {
                chain[TC_SCALING_PIVOT] = Mat4::translation(v);
                chain[TC_SCALING_PIVOT_INVERSE] = Mat4::translation(neg(v));
                mask |= bit(TC_SCALING_PIVOT) | bit(TC_SCALING_PIVOT_INVERSE);
            }
        }
        if let Some(v) = props.opt_vec3("Lcl Translation", false)? {
            if sq_len(v) > EPSILON {
                chain[TC_TRANSLATION] = Mat4::translation(v);
                mask |= bit(TC_TRANSLATION);
            }
        }
        if let Some(v) = props.opt_vec3("Lcl Scaling", false)? {
            if sq_len_minus_one(v) > EPSILON {
                chain[TC_SCALING] = Mat4::scaling(v);
                mask |= bit(TC_SCALING);
            }
        }
        if let Some(v) = props.opt_vec3("Lcl Rotation", false)? {
            if sq_len(v) > EPSILON {
                chain[TC_ROTATION] = rotation_matrix(rot, v);
                mask |= bit(TC_ROTATION);
            }
        }
        if let Some(v) = props.opt_vec3("GeometricScaling", false)? {
            if sq_len_minus_one(v) > EPSILON {
                chain[TC_GEOMETRIC_SCALING] = Mat4::scaling(v);
                if v.x.abs() <= EPSILON || v.y.abs() <= EPSILON || v.z.abs() <= EPSILON {
                    // "cannot invert geometric scaling matrix with a 0.0 scale component"
                    mask += bit(TC_GEOMETRIC_SCALING);
                } else {
                    chain[TC_GEOMETRIC_SCALING_INVERSE] =
                        Mat4::scaling(Vec3::new(1.0f32 / v.x, 1.0f32 / v.y, 1.0f32 / v.z));
                    mask |= bit(TC_GEOMETRIC_SCALING) | bit(TC_GEOMETRIC_SCALING_INVERSE);
                }
            }
        }
        if let Some(v) = props.opt_vec3("GeometricRotation", false)? {
            if sq_len(v) > EPSILON {
                chain[TC_GEOMETRIC_ROTATION] = rotation_matrix(rot, v);
                chain[TC_GEOMETRIC_ROTATION_INVERSE] = rotation_matrix(rot, v).inverse();
                mask |= bit(TC_GEOMETRIC_ROTATION) | bit(TC_GEOMETRIC_ROTATION_INVERSE);
            }
        }
        if let Some(v) = props.opt_vec3("GeometricTranslation", false)? {
            if sq_len(v) > EPSILON {
                chain[TC_GEOMETRIC_TRANSLATION] = Mat4::translation(v);
                chain[TC_GEOMETRIC_TRANSLATION_INVERSE] = Mat4::translation(neg(v));
                mask |= bit(TC_GEOMETRIC_TRANSLATION) | bit(TC_GEOMETRIC_TRANSLATION_INVERSE);
            }
        }

        let mut output = Vec::new();
        let mut post = Vec::new();
        // everything but Translation, Rotation and Scaling makes the chain "complex"
        const COMPLEX_BITS: u32 = 0x1ef77;
        if mask & COMPLEX_BITS != 0 {
            mask |= self.node_anim_chain_bits.get(name).copied().unwrap_or(0);
            #[allow(clippy::needless_range_loop)]
            for i in 0..TC_MAXIMUM {
                if mask & bit(i) == 0 {
                    continue;
                }
                if i == TC_POST_ROTATION {
                    chain[i] = chain[i].inverse();
                }
                let nd = Node {
                    name: chain_node_name(name, i),
                    transformation: chain[i],
                    meshes: Vec::new(),
                    children: Vec::new(),
                };
                if i < 3 {
                    post.push(nd);
                } else {
                    output.push(nd);
                }
            }
            return Ok((output, post, true));
        }
        let mut t = Mat4::default();
        for m in &chain {
            t = t.mul(m);
        }
        output.push(Node {
            name: name.to_string(),
            transformation: t,
            meshes: Vec::new(),
            children: Vec::new(),
        });
        Ok((output, post, false))
    }

    // ---------------------------------------------------------------------------------------------------------
    // meshes
    // ---------------------------------------------------------------------------------------------------------

    fn convert_model(
        &mut self,
        model: &Model,
        _object: &Rc<Object>,
        node_name: &str,
        absolute_transform: &Mat4,
        out: &mut Vec<u32>,
    ) -> R<()> {
        for geo in &model.geometry {
            if let Kind::Mesh(mesh) = &geo.kind {
                let ids = self.convert_mesh(geo, mesh, model, node_name, absolute_transform)?;
                out.extend(ids);
            }
        }
        Ok(())
    }

    fn convert_mesh(
        &mut self,
        geo: &Rc<Object>,
        mesh: &MeshGeometry,
        model: &Model,
        parent_name: &str,
        absolute_transform: &Mat4,
    ) -> R<Vec<u32>> {
        if let Some(v) = self.meshes_converted.get(&geo.id) {
            return Ok(v.clone());
        }
        if mesh.vertices.is_empty() || mesh.faces.is_empty() {
            return Ok(Vec::new());
        }
        let mindices = &mesh.materials;
        if !mindices.is_empty() {
            let base = mindices[0];
            if mindices.iter().any(|&i| i != base) {
                let mut had = HashSet::new();
                let mut out = Vec::new();
                for &index in mindices {
                    if had.insert(index) {
                        out.push(self.convert_mesh_multi_material(
                            geo,
                            mesh,
                            model,
                            index,
                            parent_name,
                            absolute_transform,
                        )?);
                    }
                }
                return Ok(out);
            }
        }
        Ok(vec![self.convert_mesh_single_material(
            geo,
            mesh,
            model,
            parent_name,
            absolute_transform,
        )?])
    }

    /// SetupEmptyMesh
    fn setup_empty_mesh(&mut self, geo: &Object, parent_name: &str) -> usize {
        let mut m = Mesh::default();
        let name = geo.name.strip_prefix("Geometry::").unwrap_or(&geo.name);
        m.name = if name.is_empty() {
            parent_name.to_string()
        } else {
            name.to_string()
        };
        self.meshes.push(m);
        let idx = self.meshes.len() - 1;
        self.meshes_converted
            .entry(geo.id)
            .or_default()
            .push(idx as u32);
        idx
    }

    fn primitive_flag(count: usize) -> u32 {
        match count {
            1 => PRIMITIVE_POINT,
            2 => PRIMITIVE_LINE,
            3 => PRIMITIVE_TRIANGLE,
            _ => PRIMITIVE_POLYGON,
        }
    }

    fn tangents_of(mesh: &MeshGeometry) -> Option<(Vec<Vec3>, Vec<Vec3>)> {
        if mesh.tangents.is_empty() {
            return None;
        }
        let binormals: Vec<Vec3> = if mesh.binormals.is_empty() {
            if mesh.normals.is_empty() {
                return None;
            }
            // normals[i] ^ tangents[i]
            mesh.tangents
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let n = mesh.normals[i];
                    Vec3::new(
                        n.y * t.z - n.z * t.y,
                        n.z * t.x - n.x * t.z,
                        n.x * t.y - n.y * t.x,
                    )
                })
                .collect()
        } else {
            mesh.binormals.clone()
        };
        Some((mesh.tangents.clone(), binormals))
    }

    fn convert_mesh_single_material(
        &mut self,
        geo: &Rc<Object>,
        mesh: &MeshGeometry,
        model: &Model,
        parent_name: &str,
        absolute_transform: &Mat4,
    ) -> R<u32> {
        let idx = self.setup_empty_mesh(geo, parent_name);
        let mut out = std::mem::take(&mut self.meshes[idx]);
        out.vertices = mesh.vertices.clone();
        let mut cursor = 0u32;
        for &pcount in &mesh.faces {
            out.primitive_types |= Self::primitive_flag(pcount as usize);
            let f: Vec<u32> = (0..pcount)
                .map(|_| {
                    let c = cursor;
                    cursor += 1;
                    c
                })
                .collect();
            out.faces.push(f);
        }
        if !mesh.normals.is_empty() {
            out.normals = mesh.normals.clone();
        }
        if let Some((t, b)) = Self::tangents_of(mesh) {
            out.tangents = t;
            out.bitangents = b;
        }
        for i in 0..8 {
            if mesh.uvs[i].is_empty() {
                break;
            }
            out.texture_coords[i] = mesh.uvs[i]
                .iter()
                .map(|v| Vec3::new(v[0], v[1], 0.0))
                .collect();
            out.num_uv_components[i] = 2;
        }
        for i in 0..8 {
            if mesh.colors[i].is_empty() {
                break;
            }
            out.colors[i] = mesh.colors[i].clone();
        }
        if mesh.materials.is_empty() {
            out.material_index = self.get_default_material();
        } else {
            out.material_index = self.convert_material_for_mesh(model, mesh, mesh.materials[0])?;
        }
        self.meshes[idx] = out;
        if mesh.skin.is_some() {
            self.convert_weights(idx, mesh, absolute_transform, NO_MATERIAL_SEPARATION, None)?;
        }
        Ok(idx as u32)
    }

    fn convert_mesh_multi_material(
        &mut self,
        geo: &Rc<Object>,
        mesh: &MeshGeometry,
        model: &Model,
        index: i32,
        parent_name: &str,
        absolute_transform: &Mat4,
    ) -> R<u32> {
        let idx = self.setup_empty_mesh(geo, parent_name);
        let mut out = std::mem::take(&mut self.meshes[idx]);
        let mindices = &mesh.materials;
        let process_weights = mesh.skin.is_some();
        let vertices = &mesh.vertices;

        let mut count_vertices = 0usize;
        for (it, &itf) in mindices.iter().zip(mesh.faces.iter()) {
            if *it == index {
                count_vertices += itf as usize;
            }
        }
        let mut reverse_mapping: Vec<u32> = Vec::new();
        if process_weights {
            reverse_mapping.resize(count_vertices, 0);
        }
        let tb = Self::tangents_of(mesh);
        let num_uvs = (0..8).take_while(|&i| !mesh.uvs[i].is_empty()).count();
        let num_vcs = (0..8).take_while(|&i| !mesh.colors[i].is_empty()).count();
        out.vertices = vec![Vec3::default(); count_vertices];
        if !mesh.normals.is_empty() {
            out.normals = vec![Vec3::default(); count_vertices];
        }
        if tb.is_some() {
            out.tangents = vec![Vec3::default(); count_vertices];
            out.bitangents = vec![Vec3::default(); count_vertices];
        }
        for i in 0..num_uvs {
            out.texture_coords[i] = vec![Vec3::default(); count_vertices];
            out.num_uv_components[i] = 2;
        }
        for i in 0..num_vcs {
            out.colors[i] = vec![[0.0; 4]; count_vertices];
        }
        let _ = vertices;
        let mut cursor = 0usize;
        let mut in_cursor = 0usize;
        for (it, &pcount) in mindices.iter().zip(mesh.faces.iter()) {
            let pcount = pcount as usize;
            if *it != index {
                in_cursor += pcount;
                continue;
            }
            out.primitive_types |= Self::primitive_flag(pcount);
            let mut f = Vec::with_capacity(pcount);
            for _ in 0..pcount {
                f.push(cursor as u32);
                if process_weights {
                    reverse_mapping[cursor] = in_cursor as u32;
                }
                out.vertices[cursor] = mesh.vertices[in_cursor];
                if !mesh.normals.is_empty() {
                    out.normals[cursor] = mesh.normals[in_cursor];
                }
                if let Some((t, b)) = &tb {
                    out.tangents[cursor] = t[in_cursor];
                    out.bitangents[cursor] = b[in_cursor];
                }
                for j in 0..num_uvs {
                    let uv = mesh.uvs[j][in_cursor];
                    out.texture_coords[j][cursor] = Vec3::new(uv[0], uv[1], 0.0);
                }
                for j in 0..num_vcs {
                    out.colors[j][cursor] = mesh.colors[j][in_cursor];
                }
                cursor += 1;
                in_cursor += 1;
            }
            out.faces.push(f);
        }
        out.material_index = self.convert_material_for_mesh(model, mesh, index)?;
        self.meshes[idx] = out;
        if process_weights {
            self.convert_weights(
                idx,
                mesh,
                absolute_transform,
                i64::from(index),
                Some(&reverse_mapping),
            )?;
        }
        Ok(idx as u32)
    }

    /// ConvertWeights
    fn convert_weights(
        &mut self,
        out_idx: usize,
        geo: &MeshGeometry,
        // passed along by the library, which does not use it any more
        _absolute_transform: &Mat4,
        material_index: i64,
        output_vert_start_indices: Option<&Vec<u32>>,
    ) -> R<()> {
        let skin = geo.skin.as_ref().unwrap();
        let Kind::Skin(sk) = &skin.kind else {
            return Ok(());
        };
        let no_mat_check = material_index == NO_MATERIAL_SEPARATION;
        let mut bones: Vec<Bone> = Vec::new();
        self.bone_map.clear();
        for cluster_obj in &sk.clusters {
            let Kind::Cluster(cluster) = &cluster_obj.kind else {
                continue;
            };
            // every cluster yields a bone, also one without a single weight for this mesh
            let indices = &cluster.indices;
            let mats = &geo.materials;
            let mut count_out: Vec<usize> = Vec::new();
            let mut index_out: Vec<usize> = Vec::new();
            let mut out_indices: Vec<usize> = Vec::new();
            for &index in indices {
                if index < 0 || index as usize >= geo.mapping_counts.len() {
                    return Err("vector::at".to_string());
                }
                let vi = index as usize;
                let off = geo.mapping_offsets[vi] as usize;
                let cnt = geo.mapping_counts[vi] as usize;
                index_out.push(usize::MAX);
                count_out.push(0);
                for i in 0..cnt {
                    let out_idx_v = geo.mappings[off + i] as usize;
                    let take = no_mat_check || {
                        // FaceForVertexIndex
                        let face = face_for_vertex_index(geo, out_idx_v)?;
                        mats.get(face).copied().map(i64::from) == Some(material_index)
                    };
                    if take {
                        if *index_out.last().unwrap() == usize::MAX {
                            *index_out.last_mut().unwrap() = out_indices.len();
                        }
                        if no_mat_check {
                            out_indices.push(out_idx_v);
                        } else {
                            // std::lower_bound over the output vertex start indices of this mesh
                            let starts = output_vert_start_indices.unwrap();
                            out_indices.push(starts.partition_point(|&v| (v as usize) < out_idx_v));
                        }
                        *count_out.last_mut().unwrap() += 1;
                    }
                }
            }
            self.convert_cluster(&mut bones, cluster, &out_indices, &index_out, &count_out);
        }
        self.bone_map.clear();
        self.meshes[out_idx].bones = bones;
        Ok(())
    }

    /// ConvertCluster
    fn convert_cluster(
        &mut self,
        local_mesh_bones: &mut Vec<Bone>,
        cl: &Cluster,
        out_indices: &[usize],
        index_out_indices: &[usize],
        count_out_indices: &[usize],
    ) {
        let Kind::Model(_) = &cl.node.kind else {
            return;
        };
        let deformer_name = cl.node.name.clone();
        let bone_name = fix_node_name(&deformer_name);
        #[allow(clippy::map_entry)]
        if !self.bone_map.contains_key(&deformer_name) {
            // the library's bone offset matrix is the cluster's `Transform` as it is (no inverse, no absolute transform)
            let offset = cl.transform;
            let mut weights = Vec::with_capacity(out_indices.len());
            for (i, &index_index) in index_out_indices.iter().enumerate() {
                if index_index == usize::MAX {
                    continue;
                }
                for j in 0..count_out_indices[i] {
                    weights.push(VertexWeight {
                        vertex_id: out_indices[index_index + j] as u32,
                        weight: cl.weights[i],
                    });
                }
            }
            weights.resize(
                out_indices.len(),
                VertexWeight {
                    vertex_id: 0,
                    weight: 0.0,
                },
            );
            local_mesh_bones.push(Bone {
                name: bone_name,
                weights,
                offset_matrix: offset,
            });
            self.bone_map
                .insert(deformer_name, local_mesh_bones.len() - 1);
        } else {
            let b = local_mesh_bones[self.bone_map[&deformer_name]].clone();
            local_mesh_bones.push(b);
        }
    }

    // ---------------------------------------------------------------------------------------------------------
    // materials
    // ---------------------------------------------------------------------------------------------------------

    fn get_default_material(&mut self) -> u32 {
        if let Some(i) = self.default_material {
            return i;
        }
        let mut m = Material::default();
        add_color(&mut m, "$clr.diffuse", [0.8, 0.8, 0.8]);
        add_property(
            &mut m,
            "?mat.name",
            0,
            0,
            PT_STRING,
            ai_string("DefaultMaterial"),
        );
        self.materials.push(m);
        let i = (self.materials.len() - 1) as u32;
        self.default_material = Some(i);
        i
    }

    /// ConvertMaterialForMesh
    fn convert_material_for_mesh(
        &mut self,
        model: &Model,
        geo: &MeshGeometry,
        index: i32,
    ) -> R<u32> {
        if index < 0 || index as usize >= model.materials.len() {
            return Ok(self.get_default_material());
        }
        let mat = model.materials[index as usize].clone();
        if let Some(&i) = self.materials_converted.get(&mat.id) {
            return Ok(i);
        }
        let i = self.convert_material(&mat, Some(geo))?;
        self.materials_converted.insert(mat.id, i);
        Ok(i)
    }

    fn color_factored(
        props: &PropertyTable,
        color_name: &str,
        factor_name: &str,
        use_template: bool,
    ) -> R<Option<[f32; 3]>> {
        let Some(mut base) = props.opt_vec3(color_name, use_template)? else {
            return Ok(None);
        };
        if factor_name.is_empty() {
            return Ok(Some([base.x, base.y, base.z]));
        }
        if let Some(f) = props.opt_float(factor_name, use_template)? {
            base = Vec3::new(base.x * f, base.y * f, base.z * f);
        }
        Ok(Some([base.x, base.y, base.z]))
    }

    fn color_from_material(props: &PropertyTable, base: &str) -> R<Option<[f32; 3]>> {
        Self::color_factored(
            props,
            &format!("{base}Color"),
            &format!("{base}Factor"),
            true,
        )
    }

    fn color_property(
        props: &PropertyTable,
        name: &str,
        use_template: bool,
    ) -> R<Option<[f32; 3]>> {
        Ok(props.opt_vec3(name, use_template)?.map(|v| [v.x, v.y, v.z]))
    }

    /// SetShadingPropertiesCommon
    fn set_shading_properties_common(out: &mut Material, props: &PropertyTable) -> R<()> {
        if let Some(c) = Self::color_from_material(props, "Diffuse")? {
            add_color(out, "$clr.diffuse", c);
        }
        if let Some(c) = Self::color_from_material(props, "Emissive")? {
            add_color(out, "$clr.emissive", c);
        } else if let Some(c) = Self::color_property(props, "Maya|emissive", true)? {
            add_color(out, "$clr.emissive", c);
        }
        if let Some(c) = Self::color_from_material(props, "Ambient")? {
            add_color(out, "$clr.ambient", c);
        }
        if let Some(c) = Self::color_property(props, "SpecularColor", true)? {
            add_color(out, "$clr.specular", c);
        }
        if let Some(v) = props.opt_float("SpecularFactor", true)? {
            add_float(out, "$mat.shinpercent", v);
        }
        if let Some(s) = props.opt_float("ShininessExponent", false)? {
            add_float(out, "$mat.shininess", s);
            let r = 1.0f32 - s.sqrt() / 10.0f32;
            add_float(out, "$mat.roughnessFactor", r);
        }
        let mut opacity = 1.0f32;
        if let Some(c) =
            Self::color_factored(props, "TransparentColor", "TransparencyFactor", true)?
        {
            add_color(out, "$clr.transparent", c);
            opacity = 1.0f32 - (c[0] + c[1] + c[2]) / 3.0f32;
        }
        if let Some(v) = props.opt_float("TransparencyFactor", false)? {
            add_float(out, "$mat.transparencyfactor", v);
        }
        match props.opt_float("Opacity", false)? {
            Some(v) => add_float(out, "$mat.opacity", v),
            None => {
                if opacity != 1.0 {
                    add_float(out, "$mat.opacity", opacity);
                }
            }
        }
        if let Some(c) = Self::color_property(props, "ReflectionColor", true)? {
            add_color(out, "$clr.reflective", c);
        }
        if let Some(v) = props.opt_float("ReflectionFactor", true)? {
            add_float(out, "$mat.reflectivity", v);
        }
        if let Some(v) = props.opt_float("BumpFactor", false)? {
            add_float(out, "$mat.bumpscaling", v);
        }
        if let Some(v) = props.opt_float("DisplacementFactor", false)? {
            add_float(out, "$mat.displacementscaling", v);
        }
        // Maya PBR (Stingray PBS)
        if let Some(c) = Self::color_property(props, "Maya|base_color", true)? {
            add_color(out, "$clr.base", c);
        }
        for (prop, key) in [
            ("Maya|use_color_map", "$mat.useColorMap"),
            ("Maya|use_metallic_map", "$mat.useMetallicMap"),
            ("Maya|metallic", "$mat.metallicFactor"),
            ("Maya|use_roughness_map", "$mat.useRoughnessMap"),
            ("Maya|roughness", "$mat.roughnessFactor"),
            ("Maya|use_emissive_map", "$mat.useEmissiveMap"),
            ("Maya|emissive_intensity", "$mat.emissiveIntensity"),
            ("Maya|use_ao_map", "$mat.useAOMap"),
        ] {
            if let Some(v) = props.opt_float(prop, false)? {
                add_float(out, key, v);
            }
        }
        Ok(())
    }

    /// The texture path, its UV transform and its UV channel index (shared by `$raw.*` and `$tex.*`).
    fn texture_values(tex: &Texture, mesh: Option<&MeshGeometry>) -> R<(String, [f32; 5], i32)> {
        let path = tex.relative_file_name.clone();
        if let Some(media) = &tex.media {
            if let Kind::Video(v) = &media.kind {
                if v.content.as_ref().is_some_and(|c| !c.is_empty()) {
                    return Err("UNPORTED: embedded texture".to_string());
                }
            }
        }
        let uvtrafo = [
            tex.uv_trans[0],
            tex.uv_trans[1],
            tex.uv_scaling[0],
            tex.uv_scaling[1],
            0.0,
        ];
        let mut uv_index = 0i32;
        if let Some(uv_set) = tex.props.opt_string("UVSet", false)? {
            if uv_set != "default" && !uv_set.is_empty() {
                uv_index = -1;
                match mesh {
                    Some(mesh) => {
                        let mut index = -1i32;
                        for i in 0..8 {
                            if mesh.uvs[i].is_empty() {
                                break;
                            }
                            if mesh.uv_names[i] == uv_set {
                                index = i as i32;
                                break;
                            }
                        }
                        if uv_index == -1 {
                            uv_index = index;
                        }
                    }
                    None => return Err("UNPORTED: UV set lookup without a mesh".to_string()),
                }
                if uv_index == -1 {
                    uv_index = 0;
                }
            }
        }
        Ok((path, uvtrafo, uv_index))
    }

    /// SetShadingPropertiesRaw
    fn set_shading_properties_raw(
        out: &mut Material,
        props: &PropertyTable,
        textures: &BTreeMap<String, Rc<Object>>,
        mesh: Option<&MeshGeometry>,
    ) -> R<()> {
        for (name, p) in props.unparsed()? {
            let key = format!("$raw.{name}");
            match p {
                Prop::Vec3(v) => add_property(out, &key, 0, 0, PT_FLOAT, f32s(&[v.x, v.y, v.z])),
                Prop::Float(f) => add_property(out, &key, 0, 0, PT_FLOAT, f32s(&[f])),
                Prop::Int(i) => add_property(out, &key, 0, 0, PT_INTEGER, i.to_le_bytes().to_vec()),
                Prop::Vec4(v) => add_property(out, &key, 0, 0, PT_FLOAT, f32s(&v)),
                // a bool is added as the 4 bytes of an unsigned int holding the byte
                Prop::Bool(b) => add_property(
                    out,
                    &key,
                    0,
                    0,
                    PT_INTEGER,
                    u32::from(b).to_le_bytes().to_vec(),
                ),
                Prop::Str(s) => add_property(out, &key, 0, 0, PT_STRING, ai_string(&s)),
                Prop::ULong(_) | Prop::Int64(_) => {}
            }
        }
        for (name, tex_obj) in textures {
            let Kind::Texture(tex) = &tex_obj.kind else {
                continue;
            };
            let key = format!("$raw.{name}");
            let (path, uvtrafo, uv_index) = Self::texture_values(tex, mesh)?;
            add_property(
                out,
                &format!("{key}|file"),
                TT_UNKNOWN,
                0,
                PT_STRING,
                ai_string(&path),
            );
            add_property(
                out,
                &format!("{key}|uvtrafo"),
                TT_UNKNOWN,
                0,
                PT_FLOAT,
                f32s(&uvtrafo),
            );
            add_property(
                out,
                &format!("{key}|uvwsrc"),
                TT_UNKNOWN,
                0,
                PT_INTEGER,
                uv_index.to_le_bytes().to_vec(),
            );
        }
        Ok(())
    }

    /// `TrySetTextureProperties`: the texture connected as `name`, as the `$tex.*` properties of semantic `target`.
    fn try_set_texture_properties(
        out: &mut Material,
        textures: &BTreeMap<String, Rc<Object>>,
        name: &str,
        target: u32,
        mesh: Option<&MeshGeometry>,
    ) -> R<()> {
        let Some(tex_obj) = textures.get(name) else {
            return Ok(());
        };
        let Kind::Texture(tex) = &tex_obj.kind else {
            return Ok(());
        };
        let (path, uvtrafo, uv_index) = Self::texture_values(tex, mesh)?;
        add_property(out, "$tex.file", target, 0, PT_STRING, ai_string(&path));
        add_property(out, "$tex.uvtrafo", target, 0, PT_FLOAT, f32s(&uvtrafo));
        add_property(
            out,
            "$tex.uvwsrc",
            target,
            0,
            PT_INTEGER,
            uv_index.to_le_bytes().to_vec(),
        );
        Ok(())
    }

    /// `aiGetMaterialIntegerArray(mat, key, 0, 0, &value, nullptr)`: the first int of an integer or buffer property
    /// (a float property is cast, a string parsed)
    fn material_integer(mat: &Material, key: &str) -> Option<i32> {
        let p = mat
            .properties
            .iter()
            .find(|p| p.key == key && p.semantic == 0 && p.index == 0)?;
        let d = &p.data;
        match p.ptype {
            PT_INTEGER | PT_BUFFER if d.len() >= 4 => {
                Some(i32::from_le_bytes([d[0], d[1], d[2], d[3]]))
            }
            PT_FLOAT if d.len() >= 4 => Some(f32::from_le_bytes([d[0], d[1], d[2], d[3]]) as i32),
            _ => None,
        }
    }

    /// SetTextureProperties: the table of `FBXConverter::SetTextureProperties` of the 5.3 line, in its order
    fn set_texture_properties(
        out: &mut Material,
        textures: &BTreeMap<String, Rc<Object>>,
        mesh: Option<&MeshGeometry>,
    ) -> R<()> {
        const MAP: [(&str, u32); 40] = [
            ("DiffuseColor", TT_DIFFUSE),
            ("AmbientColor", TT_AMBIENT),
            ("EmissiveColor", TT_EMISSIVE),
            ("SpecularColor", TT_SPECULAR),
            ("SpecularFactor", TT_SPECULAR),
            ("TransparentColor", TT_OPACITY),
            ("ReflectionColor", TT_REFLECTION),
            ("DisplacementColor", TT_DISPLACEMENT),
            ("NormalMap", TT_NORMALS),
            ("Bump", TT_HEIGHT),
            ("ShininessExponent", TT_SHININESS),
            ("TransparencyFactor", TT_OPACITY),
            ("EmissiveFactor", TT_EMISSIVE),
            ("ReflectionFactor", TT_METALNESS),
            ("Maya|DiffuseTexture", TT_DIFFUSE),
            ("Maya|NormalTexture", TT_NORMALS),
            ("Maya|SpecularTexture", TT_SPECULAR),
            ("Maya|FalloffTexture", TT_OPACITY),
            ("Maya|ReflectionMapTexture", TT_REFLECTION),
            ("Maya|baseColor", TT_BASE_COLOR),
            ("Maya|normalCamera", TT_NORMAL_CAMERA),
            ("Maya|emissionColor", TT_EMISSION_COLOR),
            ("Maya|metalness", TT_METALNESS),
            ("Maya|diffuseRoughness", TT_DIFFUSE_ROUGHNESS),
            ("Maya|TEX_color_map", TT_BASE_COLOR),
            ("Maya|TEX_normal_map", TT_NORMAL_CAMERA),
            ("Maya|TEX_emissive_map", TT_EMISSION_COLOR),
            ("Maya|TEX_metallic_map", TT_METALNESS),
            ("Maya|TEX_roughness_map", TT_DIFFUSE_ROUGHNESS),
            ("Maya|TEX_ao_map", TT_AMBIENT_OCCLUSION),
            ("3dsMax|Parameters|base_color_map", TT_BASE_COLOR),
            ("3dsMax|Parameters|bump_map", TT_NORMAL_CAMERA),
            ("3dsMax|Parameters|emission_map", TT_EMISSION_COLOR),
            ("3dsMax|Parameters|metalness_map", TT_METALNESS),
            ("3dsMax|Parameters|roughness_map", TT_DIFFUSE_ROUGHNESS),
            ("3dsMax|main|base_color_map", TT_BASE_COLOR),
            ("3dsMax|main|norm_map", TT_NORMAL_CAMERA),
            ("3dsMax|main|emit_color_map", TT_EMISSION_COLOR),
            ("3dsMax|main|ao_map", TT_AMBIENT_OCCLUSION),
            ("3dsMax|main|opacity_map", TT_OPACITY),
        ];
        for (name, target) in MAP {
            Self::try_set_texture_properties(out, textures, name, target, mesh)?;
        }
        Self::try_set_texture_properties(
            out,
            textures,
            "3dsMax|main|metalness_map",
            TT_METALNESS,
            mesh,
        )?;
        Self::try_set_texture_properties(
            out,
            textures,
            "3dsMax|main|specular_map",
            TT_SPECULAR,
            mesh,
        )?;
        // roughness and glossiness maps of a 3dsMax PBR material depend on its useGlossiness value
        // (2: a roughness workflow, 1: the maps are shininess; anything else only logs a warning)
        match Self::material_integer(out, "$raw.3dsMax|main|useGlossiness") {
            Some(2) => {
                Self::try_set_texture_properties(
                    out,
                    textures,
                    "3dsMax|main|roughness_map",
                    TT_DIFFUSE_ROUGHNESS,
                    mesh,
                )?;
                Self::try_set_texture_properties(
                    out,
                    textures,
                    "3dsMax|main|glossiness_map",
                    TT_DIFFUSE_ROUGHNESS,
                    mesh,
                )?;
            }
            Some(1) => {
                Self::try_set_texture_properties(
                    out,
                    textures,
                    "3dsMax|main|roughness_map",
                    TT_SHININESS,
                    mesh,
                )?;
                Self::try_set_texture_properties(
                    out,
                    textures,
                    "3dsMax|main|glossiness_map",
                    TT_SHININESS,
                    mesh,
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    /// ConvertMaterial
    fn convert_material(
        &mut self,
        material_obj: &Rc<Object>,
        mesh: Option<&MeshGeometry>,
    ) -> R<u32> {
        let Kind::Material(material) = &material_obj.kind else {
            return Err("not a material".to_string());
        };
        let props = material.props.clone();
        let mut out = Material::default();
        self.materials_converted
            .insert(material_obj.id, self.materials.len() as u32);
        let name = material_obj
            .name
            .strip_prefix("Material::")
            .unwrap_or(&material_obj.name);
        if !name.is_empty() {
            add_property(&mut out, "?mat.name", 0, 0, PT_STRING, ai_string(name));
        }
        if material.shading == "phong" {
            add_property(
                &mut out,
                "$mat.shadingm",
                0,
                0,
                PT_BUFFER,
                3i32.to_le_bytes().to_vec(),
            );
        }
        let r = (|| -> R<()> {
            Self::set_shading_properties_common(&mut out, &props)?;
            Self::set_shading_properties_raw(&mut out, &props, &material.textures, mesh)?;
            Self::set_texture_properties(&mut out, &material.textures, mesh)?;
            Ok(())
        })();
        self.materials.push(out);
        r?;
        Ok((self.materials.len() - 1) as u32)
    }

    // ---------------------------------------------------------------------------------------------------------
    // animations
    // ---------------------------------------------------------------------------------------------------------

    fn convert_animations(&mut self) -> R<()> {
        let fps = self.doc.globals.int("TimeMode", 0)?;
        let custom = self.doc.globals.float("CustomFrameRate", -1.0)?;
        let fps = if (0..15).contains(&fps) { fps } else { 0 };
        self.anim_fps = frame_rate_to_double(fps, custom);
        for st in self.doc.animation_stacks.clone() {
            if let Some(obj) = self.doc.object(st) {
                if let Kind::AnimStack(stack) = &obj.kind {
                    self.convert_animation_stack(&obj, stack)?;
                }
            }
        }
        Ok(())
    }

    fn convert_animation_stack(&mut self, obj: &Object, st: &AnimStack) -> R<()> {
        if st.layers.is_empty() {
            return Ok(());
        }
        let mut name = obj.name.as_str();
        if let Some(n) = name.strip_prefix("AnimationStack::") {
            name = n;
        } else if let Some(n) = name.strip_prefix("AnimStack::") {
            name = n;
        }
        let name = name.to_string();

        const WHITELIST: [&str; 4] = [
            "Lcl Scaling",
            "Lcl Rotation",
            "Lcl Translation",
            "DeformPercent",
        ];
        // NodeMap: model name -> curve nodes (std::map order)
        let mut node_map: BTreeMap<String, Vec<Rc<Object>>> = BTreeMap::new();
        for layer in &st.layers {
            let Kind::AnimLayer(l) = &layer.kind else {
                continue;
            };
            for node in self.doc.layer_nodes(l, Some(&WHITELIST)) {
                let Kind::AnimCurveNode(n) = &node.kind else {
                    continue;
                };
                let Some(target) = n.target else { continue };
                let Some(t) = self.doc.object(target) else {
                    continue;
                };
                if let Kind::Model(_) = &t.kind {
                    node_map
                        .entry(fix_node_name(&t.name))
                        .or_default()
                        .push(node.clone());
                }
            }
        }

        let mut node_anims: Vec<NodeAnim> = Vec::new();
        let mut min_time = 1e10_f64;
        let mut max_time = -1e10_f64;

        let mut start_time = st.props.opt_int64("LocalStart", true)?.unwrap_or(0);
        let mut stop_time = st.props.opt_int64("LocalStop", true)?.unwrap_or(0);
        let has_local_startstop = start_time != 0 || stop_time != 0;
        if !has_local_startstop {
            // no time range given: the range of all the curves
            start_time = i64::MAX;
            stop_time = i64::MIN;
            for nodes in node_map.values() {
                for n in nodes {
                    let Kind::AnimCurveNode(cn) = &n.kind else {
                        continue;
                    };
                    for curve in self.doc.curve_node_curves(cn).values() {
                        let Kind::AnimCurve(c) = &curve.kind else {
                            continue;
                        };
                        if let (Some(&f), Some(&l)) = (c.keys.first(), c.keys.last()) {
                            start_time = start_time.min(f);
                            stop_time = stop_time.max(l);
                        }
                    }
                }
            }
        }

        for (fixed_name, curves) in &node_map {
            self.generate_node_animations(
                &mut node_anims,
                fixed_name,
                curves,
                start_time,
                stop_time,
                &mut max_time,
                &mut min_time,
            )?;
        }
        if node_anims.is_empty() {
            return Ok(());
        }
        let start_fps = if has_local_startstop {
            convert_fbx_time(start_time) * self.anim_fps
        } else {
            min_time
        };
        let stop_fps = if has_local_startstop {
            convert_fbx_time(stop_time) * self.anim_fps
        } else {
            max_time
        };
        for c in node_anims.iter_mut() {
            for k in c.position_keys.iter_mut() {
                k.time -= start_fps;
            }
            for k in c.rotation_keys.iter_mut() {
                k.time -= start_fps;
            }
            for k in c.scaling_keys.iter_mut() {
                k.time -= start_fps;
            }
        }
        self.animations.push(Animation {
            name,
            duration: stop_fps - start_fps,
            ticks_per_second: self.anim_fps,
            channels: node_anims,
        });
        Ok(())
    }

    fn is_redundant_animation_data(
        &self,
        target: &Model,
        comp: usize,
        curves: &[Rc<Object>],
    ) -> R<bool> {
        if curves.len() > 1 {
            return Ok(false);
        }
        let Kind::AnimCurveNode(nd) = &curves[0].kind else {
            return Ok(false);
        };
        let sub = self.doc.curve_node_curves(nd);
        let (Some(dx), Some(dy), Some(dz)) = (sub.get("d|X"), sub.get("d|Y"), sub.get("d|Z"))
        else {
            return Ok(false);
        };
        let (Kind::AnimCurve(vx), Kind::AnimCurve(vy), Kind::AnimCurve(vz)) =
            (&dx.kind, &dy.kind, &dz.kind)
        else {
            return Ok(false);
        };
        if vx.values.len() != 1 || vy.values.len() != 1 || vz.values.len() != 1 {
            return Ok(false);
        }
        let dyn_val = Vec3::new(vx.values[0], vy.values[0], vz.values[0]);
        let default = if comp == TC_SCALING {
            Vec3::new(1.0, 1.0, 1.0)
        } else {
            Vec3::default()
        };
        let static_val = target.props.vec3(comp_property(comp), default)?;
        Ok(sq_len(sub_v(dyn_val, static_val)) < EPSILON)
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_node_animations(
        &mut self,
        node_anims: &mut Vec<NodeAnim>,
        fixed_name: &str,
        curves: &[Rc<Object>],
        start: i64,
        stop: i64,
        max_time: &mut f64,
        min_time: &mut f64,
    ) -> R<()> {
        // node_property_map: target property -> curve nodes (std::map order)
        let mut node_property_map: BTreeMap<String, Vec<Rc<Object>>> = BTreeMap::new();
        let mut curve_node: Option<Rc<Object>> = None;
        for node in curves {
            let Kind::AnimCurveNode(n) = &node.kind else {
                continue;
            };
            if n.prop.is_empty() {
                continue;
            }
            curve_node = Some(node.clone());
            if self.doc.curve_node_curves(n).is_empty() {
                continue;
            }
            node_property_map
                .entry(n.prop.clone())
                .or_default()
                .push(node.clone());
        }
        let Some(curve_node) = curve_node else {
            return Ok(());
        };
        let Kind::AnimCurveNode(cn) = &curve_node.kind else {
            return Ok(());
        };
        let Some(target_obj) = cn.target.and_then(|t| self.doc.object(t)) else {
            return Ok(());
        };
        let Kind::Model(target) = &target_obj.kind else {
            return Ok(());
        };

        let mut chain: Vec<Option<&Vec<Rc<Object>>>> = vec![None; TC_MAXIMUM];
        let mut has_any = false;
        let mut has_complex = false;
        for (i, slot) in chain.iter_mut().enumerate() {
            if i == TC_ROTATION_PIVOT_INVERSE || i == TC_SCALING_PIVOT_INVERSE {
                continue;
            }
            let Some(found) = node_property_map.get(comp_property(i)) else {
                continue;
            };
            *slot = Some(found);
            if self.is_redundant_animation_data(target, i, found)? {
                *slot = Some(found);
                // `continue` in the library keeps `chain[i]` set but does not count the channel
                continue;
            }
            has_any = true;
            if i != TC_ROTATION && i != TC_SCALING && i != TC_TRANSLATION {
                has_complex = true;
            }
        }
        if !has_any {
            return Ok(());
        }

        if !has_complex && !self.needs_complex_transformation_chain(target)? {
            let na = self.generate_simple_node_anim(
                fixed_name, target, &chain, start, stop, max_time, min_time,
            )?;
            if na.has_keys() {
                node_anims.push(na);
            }
            return Ok(());
        }

        // the chain bits are collected while the node animations are generated (the node chain consults them)
        let mut flags = 0u32;
        #[allow(clippy::needless_range_loop)]
        for i in 0..TC_MAXIMUM {
            let Some(nodes) = chain[i] else { continue };
            let chain_name = chain_node_name(fixed_name, i);
            flags |= 1 << i;
            let na = match i {
                TC_PRE_ROTATION | TC_ROTATION | TC_POST_ROTATION | TC_GEOMETRIC_ROTATION => self
                    .generate_rotation_node_anim(
                        &chain_name,
                        target,
                        nodes,
                        start,
                        stop,
                        max_time,
                        min_time,
                    )?,
                TC_TRANSLATION
                | TC_ROTATION_OFFSET
                | TC_ROTATION_PIVOT
                | TC_SCALING_OFFSET
                | TC_SCALING_PIVOT
                | TC_GEOMETRIC_TRANSLATION => {
                    let na = self.generate_translation_node_anim(
                        &chain_name,
                        nodes,
                        start,
                        stop,
                        max_time,
                        min_time,
                        false,
                    )?;
                    // pivots need their inverse (negated) channel as well; it goes first and is dropped when empty
                    if i == TC_ROTATION_PIVOT || i == TC_SCALING_PIVOT {
                        let inv = if i == TC_ROTATION_PIVOT {
                            TC_ROTATION_PIVOT_INVERSE
                        } else {
                            TC_SCALING_PIVOT_INVERSE
                        };
                        let inv_na = self.generate_translation_node_anim(
                            &chain_node_name(fixed_name, inv),
                            nodes,
                            start,
                            stop,
                            max_time,
                            min_time,
                            true,
                        )?;
                        if inv_na.has_keys() {
                            node_anims.push(inv_na);
                        }
                        // the library records the inverse bit for the rotation pivot only
                        if i == TC_ROTATION_PIVOT {
                            flags |= 1 << inv;
                        }
                    }
                    na
                }
                TC_SCALING | TC_GEOMETRIC_SCALING => self.generate_scaling_node_anim(
                    &chain_name,
                    nodes,
                    start,
                    stop,
                    max_time,
                    min_time,
                )?,
                // the inverse components are never animated: the library dereferences a null channel here
                _ => continue,
            };
            if na.has_keys() {
                node_anims.push(na);
            }
        }
        self.node_anim_chain_bits
            .insert(fixed_name.to_string(), flags);
        Ok(())
    }

    /// NeedsComplexTransformationChain: any of the pivot, offset and geometric components (not Translation,
    /// PreRotation, Rotation, PostRotation, Scaling) differs from its default.
    fn needs_complex_transformation_chain(&self, model: &Model) -> R<bool> {
        for i in 0..TC_MAXIMUM {
            if matches!(
                i,
                TC_TRANSLATION | TC_PRE_ROTATION | TC_ROTATION | TC_POST_ROTATION | TC_SCALING
            ) {
                continue;
            }
            let Some(v) = model.props.opt_vec3(comp_property(i), false)? else {
                continue;
            };
            let d = if i == TC_GEOMETRIC_SCALING {
                sq_len_minus_one(v)
            } else {
                sq_len(v)
            };
            if d > EPSILON {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn get_keyframe_list(&self, nodes: &[Rc<Object>], start: i64, stop: i64) -> Vec<KeyFrameList> {
        let mut inputs = Vec::new();
        let adj_start = start.wrapping_sub(10000);
        let adj_stop = stop.wrapping_add(10000);
        for node in nodes {
            let Kind::AnimCurveNode(n) = &node.kind else {
                continue;
            };
            for (name, curve) in self.doc.curve_node_curves(n) {
                let mapto = match name.as_str() {
                    "d|X" => 0,
                    "d|Y" => 1,
                    "d|Z" => 2,
                    _ => continue,
                };
                let Kind::AnimCurve(c) = &curve.kind else {
                    continue;
                };
                let mut keys = Vec::new();
                let mut values = Vec::new();
                for (k, v) in c.keys.iter().zip(c.values.iter()) {
                    if *k >= adj_start && *k <= adj_stop {
                        keys.push(*k);
                        values.push(*v);
                    }
                }
                inputs.push((Rc::new(keys), Rc::new(values), mapto));
            }
        }
        inputs
    }

    /// GetRotationKeyframeList: like `get_keyframe_list`, but a jump of 180 degrees or more between two keys gets
    /// intermediate keys (at 179 degrees per step) so the quaternion interpolation takes the long way round.
    fn get_rotation_keyframe_list(
        &self,
        nodes: &[Rc<Object>],
        start: i64,
        stop: i64,
    ) -> R<Vec<KeyFrameList>> {
        let mut inputs = Vec::new();
        let adj_start = start.wrapping_sub(10000);
        let adj_stop = stop.wrapping_add(10000);
        for node in nodes {
            let Kind::AnimCurveNode(n) = &node.kind else {
                continue;
            };
            for (name, curve) in self.doc.curve_node_curves(n) {
                let mapto = match name.as_str() {
                    "d|X" => 0,
                    "d|Y" => 1,
                    "d|Z" => 2,
                    _ => continue,
                };
                let Kind::AnimCurve(c) = &curve.kind else {
                    continue;
                };
                let count = c.keys.len();
                if count == 0 || c.values.is_empty() {
                    return Err("vector::at".to_string());
                }
                let in_range = |t: i64| adj_start <= t && t <= adj_stop;
                let mut keys = vec![c.keys[0]];
                let mut values = vec![c.values[0]];
                if count > 1 {
                    if c.values.len() < 2 {
                        return Err("vector::at".to_string());
                    }
                    let mut tp = c.keys[0];
                    let mut vp = c.values[0];
                    let mut tc = c.keys[1];
                    let mut vc = c.values[1];
                    for n in 1..count {
                        loop {
                            let d = vc - vp;
                            #[allow(clippy::neg_cmp_op_on_partial_ord)]
                            if !(180.0 <= d.abs()) {
                                break;
                            }
                            let dt = tc.wrapping_sub(tp);
                            let step = ((dt as f64) / f64::from(d.abs()) * 179.0).floor();
                            let tnew = tp.wrapping_add(step as i64);
                            if !in_range(tnew) {
                                break;
                            }
                            let vnew = d * ((step / (dt as f64)) as f32) + vp;
                            keys.push(tnew);
                            values.push(vnew);
                            tp = tnew;
                            vp = vnew;
                        }
                        if in_range(tc) {
                            keys.push(tc);
                            values.push(vc);
                        }
                        if n + 1 < count {
                            tp = tc;
                            vp = vc;
                            let (Some(&nk), Some(&nv)) = (c.keys.get(n + 1), c.values.get(n + 1))
                            else {
                                return Err("vector::at".to_string());
                            };
                            tc = nk;
                            vc = nv;
                        }
                    }
                }
                inputs.push((Rc::new(keys), Rc::new(values), mapto));
            }
        }
        Ok(inputs)
    }

    fn get_key_time_list(inputs: &[KeyFrameList]) -> Vec<i64> {
        let mut keys = Vec::new();
        let mut next_pos = vec![0usize; inputs.len()];
        loop {
            let mut min_tick = i64::MAX;
            for (i, kfl) in inputs.iter().enumerate() {
                if kfl.0.len() > next_pos[i] && kfl.0[next_pos[i]] < min_tick {
                    min_tick = kfl.0[next_pos[i]];
                }
            }
            if min_tick == i64::MAX {
                break;
            }
            keys.push(min_tick);
            for (i, kfl) in inputs.iter().enumerate() {
                while kfl.0.len() > next_pos[i] && kfl.0[next_pos[i]] == min_tick {
                    next_pos[i] += 1;
                }
            }
        }
        keys
    }

    /// InterpolateKeys (vector keys)
    fn interpolate_keys(
        &self,
        keys: &[i64],
        inputs: &[KeyFrameList],
        def_value: Vec3,
        max_time: &mut f64,
        min_time: &mut f64,
    ) -> Vec<VectorKey> {
        let mut out = Vec::with_capacity(keys.len());
        let mut next_pos = vec![0usize; inputs.len()];
        for &time in keys {
            let mut result = [def_value.x, def_value.y, def_value.z];
            for (i, kfl) in inputs.iter().enumerate() {
                let ksize = kfl.0.len();
                if ksize > next_pos[i] && kfl.0[next_pos[i]] == time {
                    next_pos[i] += 1;
                }
                let id0 = if next_pos[i] > 0 { next_pos[i] - 1 } else { 0 };
                let id1 = if next_pos[i] == ksize {
                    ksize - 1
                } else {
                    next_pos[i]
                };
                let value_a = kfl.1[id0];
                let value_b = kfl.1[id1];
                let time_a = kfl.0[id0];
                let time_b = kfl.0[id1];
                let factor: f32 = if time_b == time_a {
                    0.0
                } else {
                    (time - time_a) as f32 / ((time_b - time_a) as f32)
                };
                let interp = value_a + (value_b - value_a) * factor;
                result[kfl.2] = interp;
            }
            let t = convert_fbx_time(time) * self.anim_fps;
            *min_time = min_time.min(t);
            *max_time = max_time.max(t);
            out.push(VectorKey {
                time: t,
                value: Vec3::new(result[0], result[1], result[2]),
            });
        }
        out
    }

    /// InterpolateKeys (quaternion keys)
    fn interpolate_rotation_keys(
        &self,
        keys: &[i64],
        inputs: &[KeyFrameList],
        def_value: Vec3,
        max_time: &mut f64,
        min_time: &mut f64,
        order: i32,
    ) -> Vec<QuatKey> {
        let temp = self.interpolate_keys(keys, inputs, def_value, max_time, min_time);
        let mut out = Vec::with_capacity(temp.len());
        let mut lastq = Quat::default();
        for t in temp {
            let m = rotation_matrix(order, t.value);
            let mut quat = Quat::from_matrix(&mat3_of(&m));
            // the library adds the products as w, (z, (x, y)) (read from the assembly of InterpolateKeys)
            if quat.w * lastq.w + (quat.z * lastq.z + (quat.x * lastq.x + quat.y * lastq.y)) < 0.0 {
                quat.x = -quat.x;
                quat.y = -quat.y;
                quat.z = -quat.z;
                quat.w = -quat.w;
            }
            lastq = quat;
            out.push(QuatKey {
                time: t.time,
                value: quat,
            });
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_simple_node_anim(
        &self,
        name: &str,
        target: &Model,
        chain: &[Option<&Vec<Rc<Object>>>],
        start: i64,
        stop: i64,
        max_time: &mut f64,
        min_time: &mut f64,
    ) -> R<NodeAnim> {
        let props = &target.props;
        let def_scale = props.vec3("Lcl Scaling", Vec3::new(1.0, 1.0, 1.0))?;
        let def_translate = props.vec3("Lcl Translation", Vec3::default())?;
        let def_rot = props.vec3("Lcl Rotation", Vec3::default())?;
        let order = props.int("RotationOrder", 0)?;

        // every component of the chain that has curves contributes its key times (also the redundant ones)
        let mut lists: Vec<Vec<KeyFrameList>> = vec![Vec::new(); TC_MAXIMUM];
        let mut times: Vec<i64> = Vec::new();
        for i in 0..TC_MAXIMUM {
            let Some(nodes) = chain[i] else { continue };
            lists[i] = if matches!(
                i,
                TC_PRE_ROTATION | TC_ROTATION | TC_POST_ROTATION | TC_GEOMETRIC_ROTATION
            ) {
                self.get_rotation_keyframe_list(nodes, start, stop)?
            } else {
                self.get_keyframe_list(nodes, start, stop)
            };
            for kfl in &lists[i] {
                times.extend(kfl.0.iter().copied());
            }
            times.sort_unstable();
            times.dedup();
        }
        let scaling = std::mem::take(&mut lists[TC_SCALING]);
        let translation = std::mem::take(&mut lists[TC_TRANSLATION]);
        let rotation = std::mem::take(&mut lists[TC_ROTATION]);

        let mut out_quat: Vec<QuatKey> = Vec::new();
        let mut out_scale: Vec<VectorKey> = Vec::new();
        let mut out_translation: Vec<VectorKey> = Vec::new();
        if !times.is_empty() {
            if !rotation.is_empty() {
                out_quat = self.interpolate_rotation_keys(
                    &times, &rotation, def_rot, max_time, min_time, order,
                );
            } else {
                for &t in &times {
                    out_quat.push(QuatKey {
                        time: convert_fbx_time(t) * self.anim_fps,
                        value: euler_to_quaternion(def_rot, order),
                    });
                }
            }
            if !scaling.is_empty() {
                out_scale = self.interpolate_keys(&times, &scaling, def_scale, max_time, min_time);
            } else {
                for &t in &times {
                    out_scale.push(VectorKey {
                        time: convert_fbx_time(t) * self.anim_fps,
                        value: def_scale,
                    });
                }
            }
            if !translation.is_empty() {
                out_translation =
                    self.interpolate_keys(&times, &translation, def_translate, max_time, min_time);
            } else {
                for &t in &times {
                    out_translation.push(VectorKey {
                        time: convert_fbx_time(t) * self.anim_fps,
                        value: def_translate,
                    });
                }
            }

            // PreRotation is applied in front of every rotation key, PostRotation behind it (EulerXYZ, 1e-6 threshold)
            if let Some(v) = props.opt_vec3("PreRotation", false)? {
                if sq_len(v) > 1e-6_f32 {
                    let p = euler_to_quaternion(v, 0);
                    for k in &mut out_quat {
                        k.value = quat_pre_multiply(p, k.value);
                    }
                }
            }
            if let Some(v) = props.opt_vec3("PostRotation", false)? {
                if sq_len(v) > 1e-6_f32 {
                    let p = euler_to_quaternion(v, 0);
                    for k in &mut out_quat {
                        k.value = quat_post_multiply(k.value, p);
                    }
                }
            }

            // TRS -> SRT
            for i in 0..times.len() {
                let r = out_quat[i].value;
                let s = out_scale[i].value;
                let t = out_translation[i].value;
                // translation * rotation * scaling (the library multiplies the rotation in before the scaling)
                let mut mat = Mat4::translation(t);
                mat = mat.mul(&mat4_of(&r.get_matrix()));
                mat = mat.mul(&Mat4::scaling(s));
                let (ds, dr, dt) = mat.decompose();
                out_scale[i].value = ds;
                out_quat[i].value = dr;
                out_translation[i].value = dt;
            }
        }
        Ok(NodeAnim {
            node_name: name.to_string(),
            position_keys: out_translation,
            rotation_keys: out_quat,
            scaling_keys: out_scale,
            pre_state: 0,
            post_state: 0,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_rotation_node_anim(
        &self,
        name: &str,
        target: &Model,
        curves: &[Rc<Object>],
        start: i64,
        stop: i64,
        max_time: &mut f64,
        min_time: &mut f64,
    ) -> R<NodeAnim> {
        let order = target.props.int("RotationOrder", 0)?;
        let inputs = self.get_rotation_keyframe_list(curves, start, stop)?;
        let keys = Self::get_key_time_list(&inputs);
        let rot = if keys.is_empty() {
            Vec::new()
        } else {
            self.interpolate_rotation_keys(
                &keys,
                &inputs,
                Vec3::default(),
                max_time,
                min_time,
                order,
            )
        };
        Ok(NodeAnim {
            node_name: name.to_string(),
            position_keys: vec![VectorKey {
                time: 0.0,
                value: Vec3::default(),
            }],
            rotation_keys: rot,
            scaling_keys: vec![VectorKey {
                time: 0.0,
                value: Vec3::new(1.0, 1.0, 1.0),
            }],
            pre_state: 0,
            post_state: 0,
        })
    }

    fn generate_scaling_node_anim(
        &self,
        name: &str,
        curves: &[Rc<Object>],
        start: i64,
        stop: i64,
        max_time: &mut f64,
        min_time: &mut f64,
    ) -> R<NodeAnim> {
        let inputs = self.get_keyframe_list(curves, start, stop);
        let keys = Self::get_key_time_list(&inputs);
        let sc = if keys.is_empty() {
            Vec::new()
        } else {
            self.interpolate_keys(&keys, &inputs, Vec3::new(1.0, 1.0, 1.0), max_time, min_time)
        };
        Ok(NodeAnim {
            node_name: name.to_string(),
            position_keys: vec![VectorKey {
                time: 0.0,
                value: Vec3::default(),
            }],
            rotation_keys: vec![QuatKey {
                time: 0.0,
                value: Quat::default(),
            }],
            scaling_keys: sc,
            pre_state: 0,
            post_state: 0,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_translation_node_anim(
        &self,
        name: &str,
        curves: &[Rc<Object>],
        start: i64,
        stop: i64,
        max_time: &mut f64,
        min_time: &mut f64,
        inverse: bool,
    ) -> R<NodeAnim> {
        let inputs = self.get_keyframe_list(curves, start, stop);
        let keys = Self::get_key_time_list(&inputs);
        let mut tr = if keys.is_empty() {
            Vec::new()
        } else {
            self.interpolate_keys(&keys, &inputs, Vec3::default(), max_time, min_time)
        };
        if inverse {
            for k in &mut tr {
                k.value = Vec3::new(-k.value.x, -k.value.y, -k.value.z);
            }
        }
        Ok(NodeAnim {
            node_name: name.to_string(),
            position_keys: tr,
            rotation_keys: vec![QuatKey {
                time: 0.0,
                value: Quat::default(),
            }],
            scaling_keys: vec![VectorKey {
                time: 0.0,
                value: Vec3::new(1.0, 1.0, 1.0),
            }],
            pre_state: 0,
            post_state: 0,
        })
    }
}

fn sub_v(a: Vec3, b: Vec3) -> Vec3 {
    sub(a, b)
}

/// MeshGeometry::FaceForVertexIndex
fn face_for_vertex_index(geo: &MeshGeometry, in_index: usize) -> R<usize> {
    if in_index >= geo.vertices.len() {
        return Err("vertex index out of range".to_string());
    }
    // `m_facesVertexStartIndices` is built on first use
    let mut start = 0usize;
    let mut starts = Vec::with_capacity(geo.faces.len());
    for &c in &geo.faces {
        starts.push(start);
        start += c as usize;
    }
    // std::upper_bound(...) - 1
    let pos = starts.partition_point(|&s| s <= in_index);
    Ok(pos - 1)
}
