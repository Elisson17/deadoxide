//! zombie.core.skinnedmodel.model.jassimp.JAssImpImporter: the helpers the scene processing uses, and the node tree
//! of an imported scene (jassimp's `AiNode` has parent links; here an arena indexed by node).
use crate::native::assimp::scene::{Mat4, Mesh, Node, NodeAnim};
use crate::org::lwjgl::util::vector::matrix4f::Matrix4f;
use crate::org::lwjgl::util::vector::quaternion::Quaternion;
use crate::org::lwjgl::util::vector::vector3f::Vector3f;

/// One `AiNode`: name, parent, children (indices into the arena, in order), transformation, mesh references.
pub struct TreeNode {
    pub name: String,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub transform: Mat4,
    pub meshes: Vec<u32>,
}

/// The nodes of a scene; index 0 is the root.
pub struct SceneTree {
    pub nodes: Vec<TreeNode>,
}

impl SceneTree {
    /// The tree of `root` (a pre-order numbering).
    pub fn build(root: &Node) -> SceneTree {
        let mut tree = SceneTree { nodes: Vec::new() };
        tree.add(root, None);
        tree
    }

    fn add(&mut self, n: &Node, parent: Option<usize>) -> usize {
        let id = self.nodes.len();
        self.nodes.push(TreeNode {
            name: n.name.clone(),
            parent,
            children: Vec::new(),
            transform: n.transformation,
            meshes: n.meshes.clone(),
        });
        for c in &n.children {
            let cid = self.add(c, Some(id));
            self.nodes[id].children.push(cid);
        }
        id
    }

    /// JAssImpImporter.FindNode(name, node): the first node below `node` (children before their own children,
    /// depth first) with that name; `node` itself is not tested.
    pub fn find_node(&self, name: &str, node: usize) -> Option<usize> {
        for &child in &self.nodes[node].children {
            if self.nodes[child].name == name {
                return Some(child);
            }
            if let Some(n) = self.find_node(name, child) {
                return Some(n);
            }
        }
        None
    }

    /// JAssImpImporter.CollectBoneNodes: `bonenode` and everything below it, pre-order.
    pub fn collect_bone_nodes(&self, bone_nodes: &mut Vec<usize>, bonenode: usize) {
        bone_nodes.push(bonenode);
        for &c in &self.nodes[bonenode].children {
            self.collect_bone_nodes(bone_nodes, c);
        }
    }
}

/// JAssImpImporter.getMatrixFromAiMatrix: `mRC` is the aiMatrix element of row R, column C.
pub fn get_matrix_from_ai_matrix(m: &Mat4) -> Matrix4f {
    Matrix4f {
        m00: m.a1,
        m01: m.a2,
        m02: m.a3,
        m03: m.a4,
        m10: m.b1,
        m11: m.b2,
        m12: m.b3,
        m13: m.b4,
        m20: m.c1,
        m21: m.c2,
        m22: m.c3,
        m23: m.c4,
        m30: m.d1,
        m31: m.d2,
        m32: m.d3,
        m33: m.d4,
    }
}

/// JAssImpImporter.FindAiBone: the mesh's bone of that name.
pub fn find_ai_bone<'a>(
    name: &str,
    bones: &'a [crate::native::assimp::scene::Bone],
) -> Option<&'a crate::native::assimp::scene::Bone> {
    bones.iter().find(|b| b.name == name)
}

/// Matrix4f.invert() on the receiver: a zero determinant leaves it as it is.
pub fn invert_in_place(m: &mut Matrix4f) {
    if let Some(i) = Matrix4f::invert(m) {
        *m = i;
    }
}

/// JAssImpImporter.GetKeyFramePosition(AiNodeAnim, float)
pub fn get_key_frame_position(a: &NodeAnim, time: f32) -> Vector3f {
    let keys = &a.position_keys;
    let mut sidx: i64 = -1;
    for (k, key) in keys.iter().enumerate() {
        let t = key.time as f32;
        if t > time {
            break;
        }
        sidx = k as i64;
        if t == time {
            return Vector3f::of(key.value.x, key.value.y, key.value.z);
        }
    }
    if sidx < 0 {
        return Vector3f::new();
    }
    let s = sidx as usize;
    if keys.len() > s + 1 {
        let t1 = keys[s].time as f32;
        let t2 = keys[s + 1].time as f32;
        let r = t2 - t1;
        let mut sf = time - t1;
        sf /= r;
        let (x1, x2) = (keys[s].value.x, keys[s + 1].value.x);
        let x = x1 + sf * (x2 - x1);
        let (y1, y2) = (keys[s].value.y, keys[s + 1].value.y);
        let y = y1 + sf * (y2 - y1);
        let (z1, z2) = (keys[s].value.z, keys[s + 1].value.z);
        let z = z1 + sf * (z2 - z1);
        Vector3f::of(x, y, z)
    } else {
        Vector3f::of(keys[s].value.x, keys[s].value.y, keys[s].value.z)
    }
}

/// JAssImpImporter.GetKeyFrameRotation(AiNodeAnim, float)
pub fn get_key_frame_rotation(a: &NodeAnim, time: f32) -> Quaternion {
    let keys = &a.rotation_keys;
    let mut found = false;
    let mut found_quat = Quaternion::new();
    let mut sidx: i64 = -1;
    for (k, key) in keys.iter().enumerate() {
        let t = key.time as f32;
        if t > time {
            break;
        }
        sidx = k as i64;
        if t == time {
            found_quat.set(key.value.x, key.value.y, key.value.z, key.value.w);
            found = true;
            break;
        }
    }
    if !found && sidx < 0 {
        return Quaternion::new();
    }
    let s = sidx as usize;
    if !found && keys.len() > s + 1 {
        let t1 = keys[s].time as f32;
        let t2 = keys[s + 1].time as f32;
        let r = t2 - t1;
        let mut sf = time - t1;
        sf /= r;
        let (k1, k2) = (&keys[s].value, &keys[s + 1].value);
        let x = k1.x + sf * (k2.x - k1.x);
        let y = k1.y + sf * (k2.y - k1.y);
        let z = k1.z + sf * (k2.z - k1.z);
        let w = k1.w + sf * (k2.w - k1.w);
        found_quat.set(x, y, z, w);
        found = true;
    }
    if !found && keys.len() > s {
        let k = &keys[s].value;
        found_quat.set(k.x, k.y, k.z, k.w);
    }
    found_quat
}

/// JAssImpImporter.GetKeyFrameScale(AiNodeAnim, float)
pub fn get_key_frame_scale(a: &NodeAnim, time: f32) -> Vector3f {
    let keys = &a.scaling_keys;
    let mut sidx: i64 = -1;
    for (k, key) in keys.iter().enumerate() {
        let t = key.time as f32;
        if t > time {
            break;
        }
        sidx = k as i64;
        if t == time {
            return Vector3f::of(key.value.x, key.value.y, key.value.z);
        }
    }
    if sidx < 0 {
        return Vector3f::of(1.0, 1.0, 1.0);
    }
    let s = sidx as usize;
    if keys.len() > s + 1 {
        let t1 = keys[s].time as f32;
        let t2 = keys[s + 1].time as f32;
        let r = t2 - t1;
        let mut sf = time - t1;
        sf /= r;
        let (x1, x2) = (keys[s].value.x, keys[s + 1].value.x);
        let x = x1 + sf * (x2 - x1);
        let (y1, y2) = (keys[s].value.y, keys[s + 1].value.y);
        let y = y1 + sf * (y2 - y1);
        let (z1, z2) = (keys[s].value.z, keys[s + 1].value.z);
        let z = z1 + sf * (z2 - z1);
        Vector3f::of(x, y, z)
    } else {
        Vector3f::of(keys[s].value.x, keys[s].value.y, keys[s].value.z)
    }
}

/// `AiMesh.hasTexCoords(i)` for the eight sets, as the indices of the sets the mesh has.
pub fn tex_coord_sets(mesh: &Mesh) -> Vec<usize> {
    (0..8)
        .filter(|&i| !mesh.texture_coords[i].is_empty())
        .collect()
}

/// `(mesh.getTexCoordU(n, set), mesh.getTexCoordV(n, set))`: `Err` is the IllegalArgumentException of a set with fewer
/// than two components.
pub fn tex_coord_uv(mesh: &Mesh, set: usize, n: usize) -> Result<(f32, f32), String> {
    if mesh.num_uv_components[set] < 2 {
        return Err("java.lang.IllegalArgumentException".to_string());
    }
    let uv = mesh.texture_coords[set][n];
    Ok((uv.x, uv.y))
}

/// `elements[f * 3 + 2] = getFaceVertex(f, 0); [f * 3 + 1] = (f, 1); [f * 3] = (f, 2)`: `Err` is the
/// IndexOutOfBoundsException of a face with fewer than three indices.
pub fn face_elements(mesh: &Mesh) -> Result<Vec<i32>, String> {
    let mut e = vec![0i32; mesh.faces.len() * 3];
    for (f, face) in mesh.faces.iter().enumerate() {
        if face.len() < 3 {
            return Err("java.lang.IndexOutOfBoundsException".to_string());
        }
        e[f * 3 + 2] = face[0] as i32;
        e[f * 3 + 1] = face[1] as i32;
        e[f * 3] = face[2] as i32;
    }
    Ok(e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::assimp::scene::{QuatKey, Vec3, VectorKey};

    fn anim(times: &[f64]) -> NodeAnim {
        NodeAnim {
            node_name: "n".to_string(),
            position_keys: times
                .iter()
                .map(|&t| VectorKey {
                    time: t,
                    value: Vec3::new(t as f32 * 2.0, 0.0, 0.0),
                })
                .collect(),
            rotation_keys: times
                .iter()
                .map(|&t| QuatKey {
                    time: t,
                    value: crate::native::assimp::scene::Quat {
                        w: 1.0,
                        x: t as f32,
                        y: 0.0,
                        z: 0.0,
                    },
                })
                .collect(),
            scaling_keys: Vec::new(),
            pre_state: 0,
            post_state: 0,
        }
    }

    #[test]
    fn position_interpolates_between_keys_and_holds_the_last() {
        let a = anim(&[0.0, 10.0]);
        assert_eq!(get_key_frame_position(&a, 5.0).x, 10.0);
        assert_eq!(get_key_frame_position(&a, 10.0).x, 20.0);
        assert_eq!(get_key_frame_position(&a, 12.0).x, 20.0);
    }

    #[test]
    fn before_the_first_key_the_defaults_apply() {
        let a = anim(&[1.0, 2.0]);
        assert_eq!(get_key_frame_position(&a, 0.0), Vector3f::new());
        assert_eq!(get_key_frame_rotation(&a, 0.0), Quaternion::new());
        assert_eq!(get_key_frame_scale(&a, 0.0), Vector3f::of(1.0, 1.0, 1.0));
    }

    #[test]
    fn rotation_is_interpolated_componentwise() {
        let a = anim(&[0.0, 4.0]);
        let q = get_key_frame_rotation(&a, 1.0);
        assert_eq!((q.x, q.w), (1.0, 1.0));
    }
}
