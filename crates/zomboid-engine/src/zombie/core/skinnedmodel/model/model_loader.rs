//! zombie.core.skinnedmodel.model.ModelLoader: `loadTxt`, the pure-Java parser of the `.txt` models and animations
//! (`media/models/*.txt`, `media/animsold/*.txt`). Java exceptions are returned as `Err(<class name>)`.
use super::model_txt::ModelTxt;
use super::skinning_data::SkinningData;
use super::vertex_buffer_object::{VertexArray, VertexFormat, VertexType};
use crate::java;
use crate::org::lwjgl::util::vector::matrix4f::Matrix4f;
use crate::org::lwjgl::util::vector::quaternion::Quaternion;
use crate::org::lwjgl::util::vector::vector3f::Vector3f;
use crate::zombie::core::skinnedmodel::animation::animation_clip::AnimationClip;
use crate::zombie::core::skinnedmodel::animation::keyframe::Keyframe;

const NPE: &str = "java.lang.NullPointerException";
const NFE: &str = "java.lang.NumberFormatException";
const AIOOBE: &str = "java.lang.ArrayIndexOutOfBoundsException";
const IOOBE: &str = "java.lang.IndexOutOfBoundsException";
const NASE: &str = "java.lang.NegativeArraySizeException";
/// A path where the Java works on a null String key (HashMap accepts it); `Err` so it cannot pass for a Java result.
pub const UNPORTED_NULL: &str = "UNPORTED: null String key";

/// ModelLoader.LoadMode
#[derive(Clone, Copy, PartialEq, Eq)]
enum LoadMode {
    Version,
    ModelName,
    VertexStrideElementCount,
    VertexCount,
    VertexBuffer,
    NumberOfFaces,
    FaceData,
    NumberOfBones,
    SkeletonHierarchy,
    BindPose,
    InvBindPose,
    SkinOffsetMatrices,
    NumberOfAnims,
    Anim,
}

/// BufferedReader.readLine over the lines of a file; `None` is the end of the stream (a later `split` of it is the NPE).
struct Reader<'a> {
    lines: Vec<&'a str>,
    next: usize,
}

impl<'a> Reader<'a> {
    fn read_line(&mut self) -> Option<&'a str> {
        let l = self.lines.get(self.next).copied();
        if l.is_some() {
            self.next += 1;
        }
        l
    }
}

/// Integer.parseInt(String): `Err` is the NumberFormatException (also for null).
fn parse_int(s: Option<&str>) -> Result<i32, String> {
    s.and_then(java::parse_int).ok_or_else(|| NFE.to_string())
}

/// Float.parseFloat(String): a NumberFormatException, or the NullPointerException for null.
fn parse_float(s: Option<&str>) -> Result<f32, String> {
    java::parse_float(s.ok_or_else(|| NPE.to_string())?).ok_or_else(|| NFE.to_string())
}

/// `line.split(",")` with a possibly null `line`.
fn split(line: Option<&str>) -> Result<Vec<&str>, String> {
    Ok(java::split_char(line.ok_or_else(|| NPE.to_string())?, ','))
}

/// `spl[i]` of a `String.split` result.
fn at<'a>(spl: &[&'a str], i: usize) -> Result<Option<&'a str>, String> {
    spl.get(i)
        .copied()
        .map(Some)
        .ok_or_else(|| AIOOBE.to_string())
}

/// `Float.parseFloat(spl[i].trim())`
fn trimmed_float(spl: &[&str], i: usize) -> Result<f32, String> {
    parse_float(at(spl, i)?.map(java::trim))
}

/// `Integer.parseInt(spl[i].trim())`
fn trimmed_int(spl: &[&str], i: usize) -> Result<i32, String> {
    parse_int(at(spl, i)?.map(java::trim))
}

/// ModelLoader.getVector
fn get_vector(line: Option<&str>) -> Result<Vector3f, String> {
    let sp = split(line)?;
    Ok(Vector3f::of(
        parse_float(at(&sp, 0)?)?,
        parse_float(at(&sp, 1)?)?,
        parse_float(at(&sp, 2)?)?,
    ))
}

/// ModelLoader.getQuaternion
fn get_quaternion(line: Option<&str>) -> Result<Quaternion, String> {
    let sp = split(line)?;
    Ok(Quaternion::of(
        parse_float(at(&sp, 0)?)?,
        parse_float(at(&sp, 1)?)?,
        parse_float(at(&sp, 2)?)?,
        parse_float(at(&sp, 3)?)?,
    ))
}

/// ModelLoader.getMatrix: four lines of four floats, row by row (`m00 m01 m02 m03` is the first line).
fn get_matrix(
    l1: Option<&str>,
    l2: Option<&str>,
    l3: Option<&str>,
    l4: Option<&str>,
) -> Result<Matrix4f, String> {
    let mut m = Matrix4f::new();
    let row = |line: Option<&str>| -> Result<[f32; 4], String> {
        let sp = split(line)?;
        Ok([
            parse_float(at(&sp, 0)?)?,
            parse_float(at(&sp, 1)?)?,
            parse_float(at(&sp, 2)?)?,
            parse_float(at(&sp, 3)?)?,
        ])
    };
    let r = row(l1)?;
    m.m00 = r[0];
    m.m01 = r[1];
    m.m02 = r[2];
    m.m03 = r[3];
    let r = row(l2)?;
    m.m10 = r[0];
    m.m11 = r[1];
    m.m12 = r[2];
    m.m13 = r[3];
    let r = row(l3)?;
    m.m20 = r[0];
    m.m21 = r[1];
    m.m22 = r[2];
    m.m23 = r[3];
    let r = row(l4)?;
    m.m30 = r[0];
    m.m31 = r[1];
    m.m32 = r[2];
    m.m33 = r[3];
    Ok(m)
}

/// ModelLoader.RemapMatrices: `original[i]` goes to slot `boneRemapping[i]` of a list of `newCount` identities.
fn remap_matrices(
    bone_remapping: &[i32],
    original: &[Matrix4f],
    new_count: usize,
) -> Result<Vec<Matrix4f>, String> {
    let mut remapped = vec![Matrix4f::new(); new_count];
    for (i, target) in bone_remapping.iter().enumerate() {
        let slot = usize::try_from(*target)
            .ok()
            .filter(|t| *t < remapped.len())
            .ok_or_else(|| IOOBE.to_string())?;
        remapped[slot] = *original.get(i).ok_or_else(|| IOOBE.to_string())?;
    }
    Ok(remapped)
}

/// The `try { ... } catch (Exception e)` block at the end of `loadTxt`: remaps the model's bones onto `skinned_to`'s
/// skeleton. An exception leaves `mt` as far as it got (the caller ignores the `Err`).
fn remap_to_skeleton(mt: &mut ModelTxt, skinned_to: &SkinningData) -> Result<(), String> {
    let mut bone_remapping = vec![0i32; mt.bone_indices.len()];
    let original_hierarchy = mt.skeleton_hierarchy.clone();
    let original_indices = mt.bone_indices.clone();
    let mut remapped_bone_indices = skinned_to.bone_indices.copy_of();
    let mut remapped_skeleton_hierarchy = skinned_to.skeleton_hierarchy.clone();
    for (bone_name, original_index) in original_indices.iter() {
        let mut remapped_idx = remapped_bone_indices.get_or_default(bone_name, -1);
        if remapped_idx == -1 {
            remapped_idx = remapped_bone_indices.len() as i32;
            remapped_bone_indices.put(bone_name.to_string(), remapped_idx);
            let original_parent_idx = *usize::try_from(*original_index)
                .ok()
                .and_then(|i| original_hierarchy.get(i))
                .ok_or_else(|| IOOBE.to_string())?;
            if original_parent_idx >= 0 {
                let mapped = *bone_remapping
                    .get(original_parent_idx as usize)
                    .ok_or_else(|| AIOOBE.to_string())?;
                remapped_skeleton_hierarchy.push(mapped);
            }
        }
        let slot = usize::try_from(*original_index)
            .ok()
            .filter(|i| *i < bone_remapping.len())
            .ok_or_else(|| AIOOBE.to_string())?;
        bone_remapping[slot] = remapped_idx;
    }
    mt.bone_indices = remapped_bone_indices;
    mt.skeleton_hierarchy = remapped_skeleton_hierarchy;

    let remap_slot = |v: i32| -> Result<i32, String> {
        if v >= 0 {
            bone_remapping
                .get(v as usize)
                .copied()
                .ok_or_else(|| AIOOBE.to_string())
        } else {
            Ok(v)
        }
    };
    if let Some(vertices) = mt.vertices.as_mut() {
        for i in 0..vertices.num_vertices {
            let x = vertices.get_element_float(i, 5, 0) as i32;
            let y = vertices.get_element_float(i, 5, 1) as i32;
            let z = vertices.get_element_float(i, 5, 2) as i32;
            let w = vertices.get_element_float(i, 5, 3) as i32;
            let x = remap_slot(x)?;
            let y = remap_slot(y)?;
            let z = remap_slot(z)?;
            let w = remap_slot(w)?;
            vertices.set_element4(i, 5, x as f32, y as f32, z as f32, w as f32);
        }
    } else {
        return Err(NPE.to_string());
    }
    for clip in mt.clips.values_mut() {
        for kf in clip.get_keyframes_mut() {
            kf.bone = *usize::try_from(kf.bone)
                .ok()
                .and_then(|b| bone_remapping.get(b))
                .ok_or_else(|| AIOOBE.to_string())?;
        }
    }
    let count = mt.bone_indices.len();
    mt.skin_offset_matrices = remap_matrices(&bone_remapping, &mt.skin_offset_matrices, count)?;
    mt.bind_pose = remap_matrices(&bone_remapping, &mt.bind_pose, count)?;
    mt.inv_bind_pose = remap_matrices(&bone_remapping, &mt.inv_bind_pose, count)?;
    Ok(())
}

/// ModelLoader.loadTxt(String filename, boolean bStatic, boolean bReverse, SkinningData skinnedTo), on the text of the
/// file (`FileReader` and `BufferedReader` are the caller's: UTF-8 decoded, lines as `buffered_reader_lines`).
pub fn load_txt(
    text: &str,
    b_static: bool,
    b_reverse: bool,
    skinned_to: Option<&SkinningData>,
) -> Result<ModelTxt, String> {
    let mut mt = ModelTxt::new();
    mt.is_static = b_static;
    mt.reverse = b_reverse;
    let mut format = VertexFormat::new(if b_static { 4 } else { 6 });
    format.set_element(0, VertexType::VertexArray, 12);
    format.set_element(1, VertexType::NormalArray, 12);
    format.set_element(2, VertexType::TangentArray, 12);
    format.set_element(3, VertexType::TextureCoordArray, 8);
    if !b_static {
        format.set_element(4, VertexType::BlendWeightArray, 16);
        format.set_element(5, VertexType::BlendIndexArray, 16);
    }
    format.calculate();

    let mut reader = Reader {
        lines: java::buffered_reader_lines(text),
        next: 0,
    };
    let mut mode = LoadMode::Version;
    let mut skip = 0i32;
    let mut num_vertices = 0i32;
    let mut num_elements = 0i32;
    let mut num_bones = 0i32;
    let mut b_has_tangent = false;

    while let Some(first) = reader.read_line() {
        let mut line: Option<&str> = Some(first);
        if first.starts_with('#') {
            continue;
        }
        if first.contains("Tangent") {
            if b_static {
                skip += 2;
            }
            b_has_tangent = true;
        }
        if skip > 0 {
            skip -= 1;
            continue;
        }
        match mode {
            LoadMode::Version => mode = LoadMode::ModelName,
            LoadMode::ModelName => mode = LoadMode::VertexStrideElementCount,
            LoadMode::VertexStrideElementCount => {
                mode = LoadMode::VertexCount;
                skip = if b_static { 7 } else { 13 };
            }
            LoadMode::VertexCount => {
                num_vertices = parse_int(line)?;
                mode = LoadMode::VertexBuffer;
                mt.vertices = Some(VertexArray::new(&format, num_vertices)?);
            }
            LoadMode::VertexBuffer => {
                let mut n = 0;
                while n < num_vertices {
                    let spl = split(line)?;
                    let x = trimmed_float(&spl, 0)?;
                    let y = trimmed_float(&spl, 1)?;
                    let z = trimmed_float(&spl, 2)?;
                    line = reader.read_line();
                    let spl = split(line)?;
                    let nx = trimmed_float(&spl, 0)?;
                    let ny = trimmed_float(&spl, 1)?;
                    let nz = trimmed_float(&spl, 2)?;
                    let (mut tx, mut ty, mut tz) = (0.0f32, 0.0f32, 0.0f32);
                    if b_has_tangent {
                        line = reader.read_line();
                        let spl = split(line)?;
                        tx = trimmed_float(&spl, 0)?;
                        ty = trimmed_float(&spl, 1)?;
                        tz = trimmed_float(&spl, 2)?;
                    }
                    line = reader.read_line();
                    let spl = split(line)?;
                    let texx = trimmed_float(&spl, 0)?;
                    let texy = trimmed_float(&spl, 1)?;
                    let (mut bx, mut by, mut bz, mut bw) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
                    let (mut bix, mut biy, mut biz, mut biw) = (0i32, 0i32, 0i32, 0i32);
                    if !b_static {
                        line = reader.read_line();
                        let spl = split(line)?;
                        bx = trimmed_float(&spl, 0)?;
                        by = trimmed_float(&spl, 1)?;
                        bz = trimmed_float(&spl, 2)?;
                        bw = trimmed_float(&spl, 3)?;
                        line = reader.read_line();
                        let spl = split(line)?;
                        bix = trimmed_int(&spl, 0)?;
                        biy = trimmed_int(&spl, 1)?;
                        biz = trimmed_int(&spl, 2)?;
                        biw = trimmed_int(&spl, 3)?;
                    }
                    line = reader.read_line();
                    let v = mt.vertices.as_mut().ok_or_else(|| NPE.to_string())?;
                    v.set_element3(n, 0, x, y, z);
                    v.set_element3(n, 1, nx, ny, nz);
                    v.set_element3(n, 2, tx, ty, tz);
                    v.set_element2(n, 3, texx, texy);
                    if !b_static {
                        v.set_element4(n, 4, bx, by, bz, bw);
                        v.set_element4(n, 5, bix as f32, biy as f32, biz as f32, biw as f32);
                    }
                    n += 1;
                }
                mode = LoadMode::NumberOfFaces;
            }
            LoadMode::NumberOfFaces => {
                num_elements = parse_int(line)?;
                let len = num_elements.wrapping_mul(3);
                if len < 0 {
                    return Err(NASE.to_string());
                }
                mt.elements = Some(vec![0; len as usize]);
                mode = LoadMode::FaceData;
            }
            LoadMode::FaceData => {
                for n in 0..num_elements {
                    let spl = split(line)?;
                    let xx = trimmed_int(&spl, 0)?;
                    let yy = trimmed_int(&spl, 1)?;
                    let zz = trimmed_int(&spl, 2)?;
                    let e = mt.elements.as_mut().ok_or_else(|| NPE.to_string())?;
                    let n3 = (n * 3) as usize;
                    if b_reverse {
                        e[n3 + 2] = xx;
                        e[n3 + 1] = yy;
                        e[n3] = zz;
                    } else {
                        e[n3] = xx;
                        e[n3 + 1] = yy;
                        e[n3 + 2] = zz;
                    }
                    line = reader.read_line();
                }
                mode = LoadMode::NumberOfBones;
            }
            LoadMode::NumberOfBones => {
                num_bones = parse_int(line)?;
                mode = LoadMode::SkeletonHierarchy;
            }
            LoadMode::SkeletonHierarchy => {
                for _ in 0..num_bones {
                    let index = parse_int(line)?;
                    line = reader.read_line();
                    let pindex = parse_int(line)?;
                    line = reader.read_line();
                    // SharedStrings.get(null) is null, and HashMap takes a null key: not ported.
                    let bone = line.ok_or_else(|| UNPORTED_NULL.to_string())?.to_string();
                    line = reader.read_line();
                    mt.skeleton_hierarchy.push(pindex);
                    mt.bone_indices.put(bone, index);
                }
                mode = LoadMode::BindPose;
            }
            LoadMode::BindPose | LoadMode::InvBindPose | LoadMode::SkinOffsetMatrices => {
                let mut list: Vec<Matrix4f> = Vec::new();
                for _ in 0..num_bones {
                    let l1 = reader.read_line();
                    let l2 = reader.read_line();
                    let l3 = reader.read_line();
                    let l4 = reader.read_line();
                    list.push(get_matrix(l1, l2, l3, l4)?);
                    reader.read_line();
                }
                match mode {
                    LoadMode::BindPose => {
                        mt.bind_pose = list;
                        mode = LoadMode::InvBindPose;
                    }
                    LoadMode::InvBindPose => {
                        mt.inv_bind_pose = list;
                        mode = LoadMode::SkinOffsetMatrices;
                    }
                    _ => {
                        mt.skin_offset_matrices = list;
                        mode = LoadMode::NumberOfAnims;
                    }
                }
            }
            LoadMode::NumberOfAnims => mode = LoadMode::Anim,
            LoadMode::Anim => {
                let mut frames: Vec<Keyframe> = Vec::new();
                // The clip name is the line that selected this mode (the bytecode copies it to a local of its own
                // before `line` is read again; the decompiled source shows only `line`).
                let anim_name = first;
                line = reader.read_line();
                let duration = parse_float(line)?;
                line = reader.read_line();
                let n_frames = parse_int(line)?;
                line = reader.read_line();
                for n in 0..n_frames {
                    let mut f = Keyframe::new();
                    let bindex = parse_int(line)?;
                    let bone = reader.read_line();
                    let time_in_sec = parse_float(reader.read_line())?;
                    line = reader.read_line();
                    let line2 = reader.read_line();
                    let vec = get_vector(line)?;
                    let q = get_quaternion(line2)?;
                    if n < n_frames - 1 {
                        line = reader.read_line();
                    }
                    f.bone = bindex;
                    f.bone_name = Some(bone.ok_or_else(|| UNPORTED_NULL.to_string())?.to_string());
                    f.time = time_in_sec;
                    f.rotation = Some(q);
                    f.position = Some(vec);
                    frames.push(f);
                }
                let clip = AnimationClip::new(duration, &frames, anim_name, false);
                mt.clips.put(anim_name.to_string(), clip);
            }
        }
    }

    if !b_static {
        if let Some(skinned_to) = skinned_to {
            // catch (Exception e) { e.toString(); }: the half-remapped state stays.
            let _ = remap_to_skeleton(&mut mt, skinned_to);
        }
    }
    Ok(mt)
}
