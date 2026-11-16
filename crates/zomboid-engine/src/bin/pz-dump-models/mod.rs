//! The model and animation-clip dumps of `pz-dump` (cases `txtmodels`, `xscene`, `xmodels`, `animclips`, `fbxmodels`):
//! the Rust side of the probes `TxtModelsProbe`, `AiSceneProbe` and `ProcessedSceneProbe` in `tests/original_probe`.
//! Every section is one line `<label> lines=<n> fnv=<FNV-1a 64>` (or its lines in full with the `full` argument).
use std::fmt::Write as _;
use std::path::Path;

use zomboid_engine::java::{self, float_to_string, string_compare_to};
use zomboid_engine::org::lwjgl::util::vector::matrix4f::Matrix4f;
use zomboid_engine::org::lwjgl::util::vector::quaternion::Quaternion;
use zomboid_engine::org::lwjgl::util::vector::vector3f::Vector3f;
use zomboid_engine::zombie::core::skinnedmodel::model::model_loader;
use zomboid_engine::zombie::core::skinnedmodel::model::model_txt::ModelTxt;
use zomboid_engine::zombie::core::skinnedmodel::model::skinning_data::SkinningData;

/// FNV-1a 64 over the UTF-8 bytes, as 16 hex digits (the probes' `fnv`).
pub fn fnv(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// The probes' `section(label, lines)`.
pub fn section(out: &mut String, full: bool, label: &str, lines: &[String]) {
    if full {
        let _ = writeln!(out, "{label} lines={}", lines.len());
        for l in lines {
            let _ = writeln!(out, "  {l}");
        }
    } else {
        let mut text = String::new();
        for l in lines {
            text.push_str(l);
            text.push('\n');
        }
        let _ = writeln!(out, "{label} lines={} fnv={}", lines.len(), fnv(&text));
    }
}

fn f(v: f32) -> String {
    float_to_string(v)
}

fn mat(m: &Matrix4f) -> String {
    m.to_array()
        .iter()
        .map(|v| f(*v))
        .collect::<Vec<_>>()
        .join(" ")
}

fn v3(v: Option<&Vector3f>) -> String {
    v.map_or("null".to_string(), |v| {
        format!("{},{},{}", f(v.x), f(v.y), f(v.z))
    })
}

fn q4(q: Option<&Quaternion>) -> String {
    q.map_or("null".to_string(), |q| {
        format!("{},{},{},{}", f(q.x), f(q.y), f(q.z), f(q.w))
    })
}

fn sorted_names(dir: &Path, suffix: &str) -> Result<Vec<String>, String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(suffix))
        .collect();
    names.sort_by(|a, b| string_compare_to(a, b));
    Ok(names)
}

/// The text of a file as `FileReader` decodes it (UTF-8, malformed input replaced).
fn read_text(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(String::from_utf16_lossy(&java::utf8_decode_jdk(&bytes)))
}

/// `TxtModelsProbe.dump`.
fn dump_txt(out: &mut String, full: bool, mt: &ModelTxt) {
    match &mt.vertices {
        None => out.push_str("vertices null\n"),
        Some(va) => {
            let mut fmt = Vec::new();
            for (i, e) in va.format.elements.iter().enumerate() {
                fmt.push(format!(
                    "{i} {} size={} offset={}",
                    e.element_type.map_or("null", |t| t.name()),
                    e.byte_size,
                    e.byte_offset
                ));
            }
            fmt.push(format!(
                "stride={} numVertices={}",
                va.format.stride, va.num_vertices
            ));
            section(out, full, "format", &fmt);
            let mut vs = Vec::new();
            for v in 0..va.num_vertices {
                let mut sb = format!("{v}:");
                for (e, el) in va.format.elements.iter().enumerate() {
                    sb.push_str(if e == 0 { " " } else { " | " });
                    for k in 0..el.byte_size / 4 {
                        if k > 0 {
                            sb.push(' ');
                        }
                        sb.push_str(&f(va.get_element_float(v, e, k)));
                    }
                }
                vs.push(sb);
            }
            section(out, full, "vertices", &vs);
        }
    }
    match &mt.elements {
        None => out.push_str("elements null\n"),
        Some(el) => {
            let mut es: Vec<String> = el
                .chunks_exact(3)
                .map(|c| format!("{} {} {}", c[0], c[1], c[2]))
                .collect();
            if el.len() % 3 != 0 {
                es.push(format!("tail {}", el.len()));
            }
            section(out, full, "elements", &es);
        }
    }
    let mut bones = Vec::new();
    for (i, p) in mt.skeleton_hierarchy.iter().enumerate() {
        bones.push(format!("parent {i} {p}"));
    }
    let mut idx: Vec<(&str, i32)> = mt.bone_indices.iter().map(|(k, v)| (k, *v)).collect();
    idx.sort_by(|a, b| string_compare_to(a.0, b.0));
    for (k, v) in idx {
        bones.push(format!("index {k} {v}"));
    }
    section(out, full, "bones", &bones);
    for (name, list) in [
        ("bindPose", &mt.bind_pose),
        ("invBindPose", &mt.inv_bind_pose),
        ("skinOffsetMatrices", &mt.skin_offset_matrices),
    ] {
        let ms: Vec<String> = list.iter().map(mat).collect();
        section(out, full, name, &ms);
    }
    let _ = writeln!(out, "clips {}", mt.clips.len());
    let mut clips: Vec<(&str, _)> = mt.clips.iter().collect();
    clips.sort_by(|a, b| string_compare_to(a.0, b.0));
    for (key, c) in clips {
        let _ = writeln!(
            out,
            "clip {key} name={} duration={} keepLast={} ragdoll={} frames={}",
            c.name,
            f(c.get_duration()),
            c.keep_last_frame,
            c.is_ragdoll,
            c.get_keyframes().len()
        );
        let ks: Vec<String> = c
            .get_keyframes()
            .iter()
            .map(|k| {
                format!(
                    "{} {} {} {} {} {}",
                    k.bone,
                    k.bone_name.as_deref().unwrap_or("null"),
                    f(k.time),
                    v3(k.position.as_ref()),
                    q4(k.rotation.as_ref()),
                    v3(k.scale.as_ref())
                )
            })
            .collect();
        section(out, full, "keyframes", &ks);
        let mut bb = Vec::new();
        for b in 0..60 {
            let fr = c.get_bone_frames_at(b);
            let mut sb = format!("b{b} n={}:", fr.len());
            for k in fr {
                let _ = write!(sb, " {}@{}", k.bone, f(k.time));
            }
            bb.push(sb);
        }
        section(out, full, "byBone", &bb);
    }
}

/// `TxtModelsProbe.skin(rel)`: the SkinningData of a model loaded as a skinned mesh without a skeleton to follow.
fn skin(root: &Path, rel: &str) -> Result<SkinningData, String> {
    let text = read_text(&root.join(rel))?;
    let mt = model_loader::load_txt(&text, false, false, None)?;
    SkinningData::new(
        mt.clips,
        mt.bind_pose,
        mt.inv_bind_pose,
        &mt.skin_offset_matrices,
        mt.skeleton_hierarchy,
        mt.bone_indices,
    )
}

/// `TxtModelsProbe.load`.
fn load_one(
    out: &mut String,
    full: bool,
    root: &Path,
    rel: &str,
    b_static: bool,
    b_reverse: bool,
    skin_name: Option<&str>,
) -> Result<(), String> {
    let _ = writeln!(
        out,
        "load {rel} static={b_static} reverse={b_reverse} skin={}",
        skin_name.unwrap_or("none")
    );
    let result = (|| -> Result<ModelTxt, String> {
        let s = match skin_name {
            Some(n) => Some(skin(root, n)?),
            None => None,
        };
        let text = read_text(&root.join(rel))?;
        model_loader::load_txt(&text, b_static, b_reverse, s.as_ref())
    })();
    match result {
        Ok(mt) => dump_txt(out, full, &mt),
        Err(e) => {
            let _ = writeln!(out, "error {e}");
        }
    }
    Ok(())
}

/// Every `media/models/*.txt` and `media/animsold/*.txt` through `ModelLoader.loadTxt`, in the format of
/// `tests/original_probe/TxtModelsProbe.java`; `args` is empty or `full <file>...`.
pub fn txtmodels(root: &Path, args: &[String]) -> Result<String, String> {
    let mut out = String::new();
    let kate = "media/models/kate.txt";
    let bob = "media/animsold/Bob_Asleep.txt";
    if args.first().map(String::as_str) == Some("full") {
        for rel in &args[1..] {
            load_one(&mut out, true, root, rel, false, false, None)?;
            load_one(&mut out, true, root, rel, true, false, None)?;
        }
        return Ok(out);
    }
    for name in sorted_names(&root.join("media/models"), ".txt")? {
        let rel = format!("media/models/{name}");
        load_one(&mut out, false, root, &rel, true, false, None)?;
        load_one(&mut out, false, root, &rel, false, false, None)?;
        load_one(&mut out, false, root, &rel, false, true, None)?;
        load_one(&mut out, false, root, &rel, false, false, Some(kate))?;
    }
    for name in sorted_names(&root.join("media/animsold"), ".txt")? {
        let rel = format!("media/animsold/{name}");
        load_one(&mut out, false, root, &rel, false, false, None)?;
        load_one(&mut out, false, root, &rel, false, true, None)?;
        load_one(&mut out, false, root, &rel, false, false, Some(kate))?;
        load_one(&mut out, false, root, &rel, false, false, Some(bob))?;
        load_one(&mut out, false, root, &rel, true, false, None)?;
    }
    Ok(out)
}

use zomboid_engine::java::double_to_string;
use zomboid_engine::native::assimp::scene::{Mat4, Node, Scene};

fn mat4(m: &Mat4) -> String {
    m.to_array()
        .iter()
        .map(|v| f(*v))
        .collect::<Vec<_>>()
        .join(" ")
}

fn walk(n: &Node, depth: usize, lines: &mut Vec<String>) {
    lines.push(format!(
        "{depth} {} meshes=[{}] transform={}",
        n.name,
        n.meshes
            .iter()
            .map(|m| m.to_string())
            .collect::<Vec<_>>()
            .join(", "),
        mat4(&n.transformation)
    ));
    for c in &n.children {
        walk(c, depth + 1, lines);
    }
}

/// `AiSceneProbe.dumpScene`.
fn dump_scene(out: &mut String, full: bool, sc: &Scene) {
    let _ = writeln!(
        out,
        "scene meshes={} materials={} animations={} cameras=0 lights=0",
        sc.meshes.len(),
        sc.materials.len(),
        sc.animations.len()
    );
    for (i, m) in sc.meshes.iter().enumerate() {
        let mut ptypes = Vec::new();
        for (bit, name) in [
            (1u32, "POINT"),
            (2, "LINE"),
            (4, "TRIANGLE"),
            (8, "POLYGON"),
        ] {
            if m.primitive_types & bit != 0 {
                ptypes.push(name);
            }
        }
        let has_normals = !m.normals.is_empty();
        let has_tangents = !m.tangents.is_empty() && !m.bitangents.is_empty();
        let mut h = format!(
            "mesh {i} name={} material={} vertices={} faces={} ptypes=[{}] bones={} normals={} tangents={}",
            m.name,
            m.material_index,
            m.vertices.len(),
            m.faces.len(),
            ptypes.join(", "),
            m.bones.len(),
            has_normals,
            has_tangents
        );
        for u in 0..8 {
            if !m.texture_coords[u].is_empty() {
                let _ = write!(h, " uv{u}={}", m.num_uv_components[u]);
            }
        }
        for u in 0..8 {
            if !m.colors[u].is_empty() {
                let _ = write!(h, " color{u}");
            }
        }
        out.push_str(&h);
        out.push('\n');
        let l: Vec<String> = m
            .vertices
            .iter()
            .map(|v| format!("{} {} {}", f(v.x), f(v.y), f(v.z)))
            .collect();
        section(out, full, "positions", &l);
        if has_normals {
            let l: Vec<String> = m
                .normals
                .iter()
                .map(|v| format!("{} {} {}", f(v.x), f(v.y), f(v.z)))
                .collect();
            section(out, full, "normals", &l);
        }
        if has_tangents {
            let l: Vec<String> = (0..m.vertices.len())
                .map(|v| {
                    let (t, b) = (&m.tangents[v], &m.bitangents[v]);
                    format!(
                        "{} {} {} {} {} {}",
                        f(t.x),
                        f(t.y),
                        f(t.z),
                        f(b.x),
                        f(b.y),
                        f(b.z)
                    )
                })
                .collect();
            section(out, full, "tangents", &l);
        }
        for u in 0..8 {
            if m.texture_coords[u].is_empty() {
                continue;
            }
            let l: Vec<String> = m.texture_coords[u]
                .iter()
                .map(|v| {
                    if m.num_uv_components[u] == 3 {
                        format!("{} {} {}", f(v.x), f(v.y), f(v.z))
                    } else {
                        format!("{} {}", f(v.x), f(v.y))
                    }
                })
                .collect();
            section(out, full, &format!("uv{u}"), &l);
        }
        for u in 0..8 {
            if m.colors[u].is_empty() {
                continue;
            }
            let l: Vec<String> = m.colors[u]
                .iter()
                .map(|c| format!("{} {} {} {}", f(c[0]), f(c[1]), f(c[2]), f(c[3])))
                .collect();
            section(out, full, &format!("colors{u}"), &l);
        }
        let l: Vec<String> = m
            .faces
            .iter()
            .map(|face| {
                face.iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        section(out, full, "faces", &l);
        let mut l = Vec::new();
        for b in &m.bones {
            l.push(format!(
                "bone {} weights={} offset={}",
                b.name,
                b.weights.len(),
                mat4(&b.offset_matrix)
            ));
            for w in &b.weights {
                l.push(format!("{} {}", w.vertex_id, f(w.weight)));
            }
        }
        section(out, full, "bones", &l);
    }
    let mut nodes = Vec::new();
    if let Some(root) = &sc.root_node {
        walk(root, 0, &mut nodes);
    }
    section(out, full, "nodes", &nodes);
    for (a, an) in sc.animations.iter().enumerate() {
        let _ = writeln!(
            out,
            "animation {a} name={} duration={} tps={} channels={}",
            an.name,
            double_to_string(an.duration),
            double_to_string(an.ticks_per_second),
            an.channels.len()
        );
        let mut l = Vec::new();
        for c in &an.channels {
            let behaviour = |v: i32| ["DEFAULT", "CONSTANT", "LINEAR", "REPEAT"][v as usize];
            l.push(format!(
                "channel {} pos={} rot={} scale={} pre={} post={}",
                c.node_name,
                c.position_keys.len(),
                c.rotation_keys.len(),
                c.scaling_keys.len(),
                behaviour(c.pre_state),
                behaviour(c.post_state)
            ));
            for k in &c.position_keys {
                l.push(format!(
                    "p {} {} {} {}",
                    double_to_string(k.time),
                    f(k.value.x),
                    f(k.value.y),
                    f(k.value.z)
                ));
            }
            for k in &c.rotation_keys {
                l.push(format!(
                    "r {} {} {} {} {}",
                    double_to_string(k.time),
                    f(k.value.w),
                    f(k.value.x),
                    f(k.value.y),
                    f(k.value.z)
                ));
            }
            for k in &c.scaling_keys {
                l.push(format!(
                    "s {} {} {} {}",
                    double_to_string(k.time),
                    f(k.value.x),
                    f(k.value.y),
                    f(k.value.z)
                ));
            }
        }
        section(out, full, "channels", &l);
    }
}

/// `AiSceneProbe.collect`: the files under `dir` with one of the extensions (case-insensitive), names in order,
/// directories descended where they sort.
fn collect(root: &Path, rel: &str, exts: &[&str], acc: &mut Vec<String>) -> Result<(), String> {
    let Ok(rd) = std::fs::read_dir(root.join(rel)) else {
        return Ok(());
    };
    let mut names: Vec<String> = rd
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .collect();
    names.sort_by(|a, b| string_compare_to(a, b));
    for n in names {
        let sub = format!("{rel}/{n}");
        if root.join(&sub).is_dir() {
            collect(root, &sub, exts, acc)?;
        } else {
            let low = n.to_lowercase();
            for e in exts {
                if low.ends_with(&format!(".{e}")) {
                    acc.push(sub.clone());
                }
            }
        }
    }
    Ok(())
}

/// `AiSceneProbe.one`: the post-process steps of `spec` are applied by `native::assimp::post_process`.
fn scene_one(out: &mut String, full: bool, root: &Path, rel: &str, spec: &str) {
    let _ = writeln!(out, "file {rel} flags={spec}");
    let result = std::fs::read(root.join(rel))
        .map_err(|e| format!("java.io.IOException: {e}"))
        .and_then(|bytes| {
            zomboid_engine::native::assimp::import_file(rel, &bytes, spec)
                .map_err(|e| format!("java.io.IOException: {e}"))
        });
    match result {
        Ok(sc) => dump_scene(out, full, &sc),
        Err(e) => {
            let _ = writeln!(out, "error {e}");
        }
    }
}

/// Cases `xscene`, `animscene`, `fbxscene`, ... in the format of `tests/original_probe/AiSceneProbe.java`.
/// `args`: `<flags> <extensions> <dir>...` or `full <flags> <file>...`.
pub fn aiscene(root: &Path, args: &[String]) -> Result<String, String> {
    let mut out = String::new();
    if args.first().map(String::as_str) == Some("full") {
        for rel in &args[2..] {
            scene_one(&mut out, true, root, rel, &args[1]);
        }
        return Ok(out);
    }
    if args.len() < 3 {
        return Err("aiscene: <flags> <extensions> <dir>...".to_string());
    }
    let exts: Vec<&str> = args[1].split(',').collect();
    let mut files = Vec::new();
    for d in &args[2..] {
        collect(root, d, &exts, &mut files)?;
    }
    for rel in &files {
        scene_one(&mut out, false, root, rel, &args[0]);
    }
    Ok(out)
}
