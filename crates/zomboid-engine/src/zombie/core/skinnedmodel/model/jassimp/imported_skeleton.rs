//! zombie.core.skinnedmodel.model.jassimp.ImportedSkeleton: the bone indices, hierarchy, bind poses and animation
//! clips of an imported scene.
use super::jassimp_importer::{
    find_ai_bone, get_key_frame_position, get_key_frame_rotation, get_key_frame_scale,
    get_matrix_from_ai_matrix, invert_in_place, SceneTree,
};
use super::LoadMode;
use crate::java::{self, equals_ignore_case, JavaHashMap};
use crate::java_fdlibm;
use crate::native::assimp::scene::{Animation, Mesh, NodeAnim, Scene};
use crate::org::lwjgl::util::vector::matrix4f::Matrix4f;
use crate::org::lwjgl::util::vector::quaternion::Quaternion;
use crate::org::lwjgl::util::vector::vector3f::Vector3f;
use crate::zombie::core::skinnedmodel::animation::animation_clip::AnimationClip;
use crate::zombie::core::skinnedmodel::animation::keyframe::Keyframe;
use crate::zombie::core::skinnedmodel::model::skinning_data::SkinningData;

const NPE: &str = "java.lang.NullPointerException";
const IOOBE: &str = "java.lang.IndexOutOfBoundsException";

/// ImportedSkeletonParams (the fields of `ProcessedAiSceneParams` the skeleton reads, and the mesh).
pub struct ImportedSkeletonParams<'a> {
    pub scene: &'a Scene,
    pub tree: &'a SceneTree,
    pub mode: LoadMode,
    pub skinned_to: Option<&'a SkinningData>,
    pub anim_bones_scale_modifier: f32,
    pub anim_bones_rotate_modifier: Option<Quaternion>,
    pub mesh: &'a Mesh,
}

/// ImportedSkeleton
#[derive(Clone, Debug)]
pub struct ImportedSkeleton {
    pub bone_indices: JavaHashMap<i32>,
    pub skeleton_hierarchy: Vec<i32>,
    pub bind_pose: Vec<Matrix4f>,
    pub inv_bind_pose: Vec<Matrix4f>,
    pub skin_offset_matrices: Vec<Matrix4f>,
    /// `rootBoneNode`, the node's name (the node itself lives in the scene tree).
    pub root_bone_node: String,
    pub clips: JavaHashMap<AnimationClip>,
}

/// HelperFunctions.transform(Quaternion, Vector3f, Vector3f): `rotation` is normalised in place first (a zero
/// length is the IllegalStateException).
pub fn helper_transform(rotation: &mut Quaternion, vector: &Vector3f) -> Result<Vector3f, String> {
    rotation
        .normalise()
        .map_err(|_| "java.lang.IllegalStateException".to_string())?;
    let s = rotation.w;
    let (v_x, v_y, v_z) = (rotation.x, rotation.y, rotation.z);
    let s2 = s * s;
    let v2 = v_x * v_x + v_y * v_y + v_z * v_z;
    let (p_x, p_y, p_z) = (vector.x, vector.y, vector.z);
    let v_cross_px = v_y * p_z - v_z * p_y;
    let v_cross_py = v_z * p_x - v_x * p_z;
    let v_cross_pz = v_x * p_y - v_y * p_x;
    let v_dot_p = p_x * v_x + p_y * v_y + p_z * v_z;
    let r_x = (s2 - v2) * p_x + 2.0f32 * s * v_cross_px + 2.0f32 * v_x * v_dot_p;
    let r_y = (s2 - v2) * p_y + 2.0f32 * s * v_cross_py + 2.0f32 * v_y * v_dot_p;
    let r_z = (s2 - v2) * p_z + 2.0f32 * s * v_cross_pz + 2.0f32 * v_z * v_dot_p;
    Ok(Vector3f::of(r_x, r_y, r_z))
}

/// StringUtils.startsWithIgnoreCase: `str.regionMatches(true, 0, prefix, 0, prefix.length())`.
fn starts_with_ignore_case(s: &str, prefix: &str) -> bool {
    let n = prefix.chars().count();
    let head: String = s.chars().take(n).collect();
    head.chars().count() == n && equals_ignore_case(&head, prefix)
}

impl ImportedSkeleton {
    /// ImportedSkeleton.process; `Err` is the Java exception class.
    pub fn process(params: &ImportedSkeletonParams) -> Result<Self, String> {
        let tree = params.tree;
        let rootnode = 0usize;
        let mesh = params.mesh;
        let mut root_bone_node = tree.find_node("Dummy01", rootnode);
        let b_vehicle;
        if root_bone_node.is_none() {
            root_bone_node = tree.find_node("VehicleSkeleton", rootnode);
            b_vehicle = true;
        } else {
            b_vehicle = false;
        }
        while let Some(rb) = root_bone_node {
            match tree.nodes[rb].parent {
                Some(p) if p != rootnode => root_bone_node = Some(p),
                _ => break,
            }
        }
        let root_bone_node = root_bone_node.unwrap_or(rootnode);
        let mut bone_nodes: Vec<usize> = Vec::new();
        tree.collect_bone_nodes(&mut bone_nodes, root_bone_node);
        if let Some(translation) = tree.find_node("Translation_Data", rootnode) {
            bone_nodes.push(translation);
            let mut parent = tree.nodes[translation].parent;
            while let Some(p) = parent {
                if p == rootnode {
                    break;
                }
                bone_nodes.push(p);
                parent = tree.nodes[p].parent;
            }
        }
        let mut sk = ImportedSkeleton {
            bone_indices: JavaHashMap::default(),
            skeleton_hierarchy: Vec::new(),
            bind_pose: Vec::new(),
            inv_bind_pose: Vec::new(),
            skin_offset_matrices: Vec::new(),
            root_bone_node: tree.nodes[root_bone_node].name.clone(),
            clips: JavaHashMap::default(),
        };
        if let Some(skinned_to) = params.skinned_to {
            sk.bone_indices = skinned_to.bone_indices.copy_of();
            sk.skeleton_hierarchy
                .extend(skinned_to.skeleton_hierarchy.iter().copied());
        }
        for &n in &bone_nodes {
            let bonename = &tree.nodes[n].name;
            if !sk.bone_indices.contains_key(bonename) {
                let index = sk.bone_indices.len() as i32;
                sk.bone_indices.put(bonename.clone(), index);
                if n == root_bone_node {
                    sk.skeleton_hierarchy.push(-1);
                } else {
                    let mut parent = tree.nodes[n].parent;
                    while let Some(p) = parent {
                        if sk.bone_indices.contains_key(&tree.nodes[p].name) {
                            break;
                        }
                        parent = tree.nodes[p].parent;
                    }
                    match parent {
                        Some(p) => {
                            let idx = *sk.bone_indices.get(&tree.nodes[p].name).ok_or(NPE)?;
                            sk.skeleton_hierarchy.push(idx);
                        }
                        None => sk.skeleton_hierarchy.push(0),
                    }
                }
            }
        }
        let mi = Matrix4f::new();
        for _ in 0..sk.bone_indices.len() {
            sk.bind_pose.push(mi);
            sk.skin_offset_matrices.push(mi);
        }
        let bones = &mesh.bones;
        for &node in &bone_nodes {
            let node_bone_name = &tree.nodes[node].name;
            let Some(bone) = find_ai_bone(node_bone_name, bones) else {
                continue;
            };
            let skin_offset = get_matrix_from_ai_matrix(&bone.offset_matrix);
            let mut skin_offseti = skin_offset;
            invert_in_place(&mut skin_offseti);
            let mut skin_offset_parent = Matrix4f::new();
            let parent = tree.nodes[node].parent.ok_or(NPE)?;
            let parent_bone_name = &tree.nodes[parent].name;
            if let Some(pbone) = find_ai_bone(parent_bone_name, bones) {
                skin_offset_parent = get_matrix_from_ai_matrix(&pbone.offset_matrix);
            }
            let mut skin_offset_parenti = skin_offset_parent;
            invert_in_place(&mut skin_offset_parenti);
            let mut bind = Matrix4f::mul(&skin_offseti, &skin_offset_parenti);
            invert_in_place(&mut bind);
            let boneindex = *sk.bone_indices.get(node_bone_name).ok_or(NPE)? as usize;
            *sk.bind_pose.get_mut(boneindex).ok_or(IOOBE)? = bind;
            *sk.skin_offset_matrices.get_mut(boneindex).ok_or(IOOBE)? = skin_offset;
        }
        for ix in 0..sk.bind_pose.len() {
            let mut ib = sk.bind_pose[ix];
            invert_in_place(&mut ib);
            sk.inv_bind_pose.push(ib);
        }
        for src_anim in &params.scene.animations {
            if b_vehicle {
                sk.process_animation(src_anim, b_vehicle, 1.0, None)?;
            } else {
                sk.process_animation(
                    src_anim,
                    b_vehicle,
                    params.anim_bones_scale_modifier,
                    params.anim_bones_rotate_modifier,
                )?;
            }
        }
        Ok(sk)
    }

    fn process_animation(
        &mut self,
        src_anim: &Animation,
        b_vehicle: bool,
        anim_bones_scale_modifier: f32,
        anim_bones_rotate_modifier: Option<Quaternion>,
    ) -> Result<(), String> {
        let mut frames: Vec<Keyframe> = Vec::new();
        let duration = src_anim.duration as f32;
        let ticks = src_anim.ticks_per_second as f32;
        let duration_seconds = duration / ticks;
        let num_bones = self.bone_indices.len();
        let mut frame_times_per_bone: Vec<Option<Vec<f32>>> = vec![None; num_bones];
        let mut channels_per_bone: Vec<Option<Vec<usize>>> = vec![None; num_bones];
        self.collect_bone_frames(src_anim, &mut frame_times_per_bone, &mut channels_per_bone);
        let apply_bone_rotate_modifier = anim_bones_rotate_modifier.is_some();
        let mut anim_bones_rotate_modifier_inv = None;
        if let Some(rm) = &anim_bones_rotate_modifier {
            anim_bones_rotate_modifier_inv = Some(Quaternion::mul_inverse(&Quaternion::new(), rm));
        }
        for bone_idx in 0..num_bones {
            let Some(bone_channels) = channels_per_bone[bone_idx].clone() else {
                if bone_idx == 0 {
                    if let Some(rm) = &anim_bones_rotate_modifier {
                        Self::add_default_anim_track(
                            "RootNode",
                            bone_idx as i32,
                            *rm,
                            Vector3f::of(0.0, 0.0, 0.0),
                            &mut frames,
                            duration_seconds,
                        );
                    }
                }
                continue;
            };
            let Some(frame_times) = frame_times_per_bone[bone_idx].as_mut() else {
                continue;
            };
            // TFloatArrayList.sort(): Arrays.sort(float[]), the total order
            frame_times.sort_by(|a, b| a.total_cmp(b));
            let frame_times = frame_times.clone();
            let parent_bone_idx = self.get_parent_bone_idx(bone_idx as i32)?;
            let parent_bone_adjusted = apply_bone_rotate_modifier
                && (parent_bone_idx == 0
                    || self.does_parent_bone_have_anim_frames(
                        &frame_times_per_bone,
                        &channels_per_bone,
                        bone_idx as i32,
                    )?);
            for &frame_time in &frame_times {
                let frame_seconds = frame_time / ticks;
                for &channel_idx in &bone_channels {
                    let a = &src_anim.channels[channel_idx];
                    let mut f = Keyframe::new();
                    f.position = None;
                    f.rotation = None;
                    f.time = -1.0;
                    f.bone_name = Some(a.node_name.clone());
                    f.bone = bone_idx as i32;
                    f.time = frame_seconds;
                    let (mut position, mut rotation, scale);
                    if !b_vehicle {
                        position = get_key_frame_position(a, frame_time);
                        rotation = get_key_frame_rotation(a, frame_time);
                        scale = get_key_frame_scale(a, frame_time);
                    } else {
                        position = Self::vehicle_position(a, frame_time, f64::from(duration));
                        rotation = Self::vehicle_rotation(a, frame_time, f64::from(duration));
                        scale = Self::vehicle_scale(a, frame_time);
                    }
                    position.x *= anim_bones_scale_modifier;
                    position.y *= anim_bones_scale_modifier;
                    position.z *= anim_bones_scale_modifier;
                    if let (Some(rm), Some(inv)) = (
                        &anim_bones_rotate_modifier,
                        anim_bones_rotate_modifier_inv.as_mut(),
                    ) {
                        if parent_bone_adjusted {
                            rotation = Quaternion::mul(inv, &rotation);
                            let is_translation_bone = starts_with_ignore_case(
                                f.bone_name.as_deref().unwrap_or(""),
                                "Translation_Data",
                            );
                            if !is_translation_bone {
                                position = helper_transform(inv, &position)?;
                            }
                        }
                        rotation = Quaternion::mul(&rotation, rm);
                    }
                    f.position = Some(position);
                    f.rotation = Some(rotation);
                    f.scale = Some(scale);
                    frames.push(f);
                }
            }
        }
        let mut anim_name: &str = &src_anim.name;
        if let Some(p) = anim_name.find('|') {
            if p > 0 {
                anim_name = &anim_name[p + 1..];
            }
        }
        let anim_name = java::trim(anim_name).to_string();
        let clip = AnimationClip::new(duration_seconds, &frames, &anim_name, true);
        self.clips.put(anim_name, clip);
        Ok(())
    }

    /// ImportedSkeleton.addDefaultAnimTrack
    fn add_default_anim_track(
        bone_name: &str,
        bone_idx: i32,
        rotation: Quaternion,
        position: Vector3f,
        frames: &mut Vec<Keyframe>,
        duration_seconds: f32,
    ) {
        let one = Vector3f::of(1.0, 1.0, 1.0);
        for time in [0.0f32, duration_seconds] {
            let mut f = Keyframe::new();
            f.bone_name = Some(bone_name.to_string());
            f.bone = bone_idx;
            f.time = time;
            f.position = Some(position);
            f.rotation = Some(rotation);
            f.scale = Some(one);
            frames.push(f);
        }
    }

    fn does_parent_bone_have_anim_frames(
        &self,
        frame_times_per_bone: &[Option<Vec<f32>>],
        channels_per_bone: &[Option<Vec<usize>>],
        bone_idx: i32,
    ) -> Result<bool, String> {
        let parent_bone_idx = self.get_parent_bone_idx(bone_idx)?;
        if parent_bone_idx < 0 {
            Ok(false)
        } else {
            Ok(Self::does_bone_have_anim_frames(
                frame_times_per_bone,
                channels_per_bone,
                parent_bone_idx as usize,
            ))
        }
    }

    fn does_bone_have_anim_frames(
        frame_times_per_bone: &[Option<Vec<f32>>],
        channels_per_bone: &[Option<Vec<usize>>],
        bone_idx: usize,
    ) -> bool {
        match &frame_times_per_bone[bone_idx] {
            Some(ft) if !ft.is_empty() => channels_per_bone[bone_idx]
                .as_ref()
                .is_some_and(|c| !c.is_empty()),
            _ => false,
        }
    }

    /// ImportedSkeleton.collectBoneFrames: the channels of each bone and the distinct key times of the bone (in the
    /// order found: positions, rotations, scales per channel).
    fn collect_bone_frames(
        &self,
        src_anim: &Animation,
        frame_times_per_bone: &mut [Option<Vec<f32>>],
        channels_per_bone: &mut [Option<Vec<usize>>],
    ) {
        for (j, a) in src_anim.channels.iter().enumerate() {
            let Some(&bone_idx) = self.bone_indices.get(&a.node_name) else {
                continue; // "Could not find bone index for node name" is logged
            };
            let bone_idx = bone_idx as usize;
            channels_per_bone[bone_idx]
                .get_or_insert_with(Vec::new)
                .push(j);
            let frametimes = frame_times_per_bone[bone_idx].get_or_insert_with(Vec::new);
            let times = a
                .position_keys
                .iter()
                .map(|k| k.time)
                .chain(a.rotation_keys.iter().map(|k| k.time))
                .chain(a.scaling_keys.iter().map(|k| k.time));
            for t in times {
                let t = t as f32;
                if !frametimes.contains(&t) {
                    frametimes.push(t);
                }
            }
        }
    }

    /// ImportedSkeleton.getParentBoneIdx
    fn get_parent_bone_idx(&self, bone_idx: i32) -> Result<i32, String> {
        if bone_idx > -1 {
            self.skeleton_hierarchy
                .get(bone_idx as usize)
                .copied()
                .ok_or_else(|| IOOBE.to_string())
        } else {
            Ok(-1)
        }
    }

    /// ImportedSkeleton.getNumBoneAncestors
    pub fn get_num_bone_ancestors(&self, bone_idx: i32) -> Result<i32, String> {
        let mut num = 0;
        let mut parent = self.get_parent_bone_idx(bone_idx)?;
        while parent > -1 {
            num += 1;
            parent = self.get_parent_bone_idx(parent)?;
        }
        Ok(num)
    }

    /// ImportedSkeleton.GetKeyFramePosition(AiNodeAnim, float, double): the vehicle variant, which wraps around the
    /// last key.
    // `!(time < key)` is the Java condition as written (it differs from `time >= key` for NaN)
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn vehicle_position(a: &NodeAnim, time: f32, duration: f64) -> Vector3f {
        let keys = &a.position_keys;
        let mut pos = Vector3f::new();
        if keys.is_empty() {
            return pos;
        }
        let mut frame = 0usize;
        while frame < keys.len() - 1 && !(f64::from(time) < keys[frame + 1].time) {
            frame += 1;
        }
        let next_frame = (frame + 1) % keys.len();
        let t1 = keys[frame].time as f32;
        let t2 = keys[next_frame].time as f32;
        let mut diff_time = t2 - t1;
        if diff_time < 0.0 {
            diff_time = (f64::from(diff_time) + duration) as f32;
        }
        if diff_time > 0.0 {
            let r = t2 - t1;
            let mut s = time - t1;
            s /= r;
            let (x1, x2) = (keys[frame].value.x, keys[next_frame].value.x);
            let x = x1 + s * (x2 - x1);
            let (y1, y2) = (keys[frame].value.y, keys[next_frame].value.y);
            let y = y1 + s * (y2 - y1);
            let (z1, z2) = (keys[frame].value.z, keys[next_frame].value.z);
            let z = z1 + s * (z2 - z1);
            pos.set(x, y, z);
        } else {
            pos.set(
                keys[frame].value.x,
                keys[frame].value.y,
                keys[frame].value.z,
            );
        }
        pos
    }

    /// ImportedSkeleton.GetKeyFrameRotation(AiNodeAnim, float, double): the vehicle variant (spherical interpolation).
    // `!(time < key)` is the Java condition as written (it differs from `time >= key` for NaN)
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn vehicle_rotation(a: &NodeAnim, time: f32, duration: f64) -> Quaternion {
        let keys = &a.rotation_keys;
        let mut found_quat = Quaternion::new();
        if keys.is_empty() {
            return found_quat;
        }
        let mut frame = 0usize;
        while frame < keys.len() - 1 && !(f64::from(time) < keys[frame + 1].time) {
            frame += 1;
        }
        let next_frame = (frame + 1) % keys.len();
        let t1 = keys[frame].time as f32;
        let t2 = keys[next_frame].time as f32;
        let mut diff_time = t2 - t1;
        if diff_time < 0.0 {
            diff_time = (f64::from(diff_time) + duration) as f32;
        }
        if diff_time > 0.0 {
            let p_factor = (time - t1) / diff_time;
            let p_start = keys[frame].value;
            let p_end = keys[next_frame].value;
            let mut cosom = f64::from(
                p_start.x * p_end.x
                    + p_start.y * p_end.y
                    + p_start.z * p_end.z
                    + p_start.w * p_end.w,
            );
            let mut end = Quaternion::of(p_end.x, p_end.y, p_end.z, p_end.w);
            if cosom < 0.0 {
                cosom *= -1.0;
                end.x = -end.x;
                end.y = -end.y;
                end.z = -end.z;
                end.w = -end.w;
            }
            let (sclp, sclq);
            if 1.0 - cosom > 1.0E-4 {
                let omega = java_fdlibm::acos(cosom);
                let sinom = java_fdlibm::sin(omega);
                sclp = java_fdlibm::sin((1.0 - f64::from(p_factor)) * omega) / sinom;
                sclq = java_fdlibm::sin(f64::from(p_factor) * omega) / sinom;
            } else {
                sclp = 1.0 - f64::from(p_factor);
                sclq = f64::from(p_factor);
            }
            found_quat.set(
                (sclp * f64::from(p_start.x) + sclq * f64::from(end.x)) as f32,
                (sclp * f64::from(p_start.y) + sclq * f64::from(end.y)) as f32,
                (sclp * f64::from(p_start.z) + sclq * f64::from(end.z)) as f32,
                (sclp * f64::from(p_start.w) + sclq * f64::from(end.w)) as f32,
            );
        } else {
            let k = &keys[frame].value;
            found_quat.set(k.x, k.y, k.z, k.w);
        }
        found_quat
    }

    /// ImportedSkeleton.GetKeyFrameScale(AiNodeAnim, float, double): the vehicle variant (the key at or before the time).
    // `!(time < key)` is the Java condition as written (it differs from `time >= key` for NaN)
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn vehicle_scale(a: &NodeAnim, time: f32) -> Vector3f {
        let keys = &a.scaling_keys;
        let mut scale = Vector3f::of(1.0, 1.0, 1.0);
        if keys.is_empty() {
            return scale;
        }
        let mut frame = 0usize;
        while frame < keys.len() - 1 && !(f64::from(time) < keys[frame + 1].time) {
            frame += 1;
        }
        scale.set(
            keys[frame].value.x,
            keys[frame].value.y,
            keys[frame].value.z,
        );
        scale
    }
}
