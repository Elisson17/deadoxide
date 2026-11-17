//! assimp `code/PostProcessing`: the steps `FileTask_LoadMesh` and `FileTask_LoadAnimation` ask for, and the
//! `ScenePreprocessor` that runs before them.
pub mod find_instances;
pub mod flip_winding_order;
pub mod join_vertices;
pub mod limit_bone_weights;
pub mod make_left_handed;
pub mod optimize_meshes;
pub mod remove_redundant_mats;
pub mod scene_preprocessor;
pub mod triangulate;

/// The steps in the order `Importer::ApplyPostProcessing` runs them: the registration order of
/// `GetPostProcessingStepInstanceList` in the game's `libjassimp` (read from the disassembled function: MakeLeftHanded,
/// FlipUVs, FlipWindingOrder, RemoveVC, RemoveRedundantMaterials, EmbedTextures, FindInstances, OptimizeGraph, ...,
/// Triangulate, ..., OptimizeMeshes, ..., JoinVertices, ..., LimitBoneWeights, ImproveCacheLocality), restricted to
/// the steps the game asks for.
const STEP_ORDER: [&str; 7] = [
    "MAKE_LEFT_HANDED",
    "REMOVE_REDUNDANT_MATERIALS",
    "FIND_INSTANCES",
    "TRIANGULATE",
    "OPTIMIZE_MESHES",
    "JOIN_IDENTICAL_VERTICES",
    "LIMIT_BONE_WEIGHTS",
];

/// The post-processing steps of `spec` (`mesh`, `anim`, `none` or a comma separated list of `AiPostProcessSteps`
/// names) applied to `scene`. `Err` names a step this port does not have.
pub fn apply(scene: &mut crate::native::assimp::scene::Scene, spec: &str) -> Result<(), String> {
    let names: Vec<&str> = match spec {
        "none" => Vec::new(),
        "mesh" => vec![
            "FIND_INSTANCES",
            "MAKE_LEFT_HANDED",
            "LIMIT_BONE_WEIGHTS",
            "TRIANGULATE",
            "OPTIMIZE_MESHES",
            "REMOVE_REDUNDANT_MATERIALS",
            "JOIN_IDENTICAL_VERTICES",
        ],
        "anim" => vec!["MAKE_LEFT_HANDED", "REMOVE_REDUNDANT_MATERIALS"],
        list => list.split(',').collect(),
    };
    for n in &names {
        if !STEP_ORDER.contains(n) {
            return Err(format!("UNPORTED: post-process step {n}"));
        }
    }
    for step in STEP_ORDER {
        if !names.contains(&step) {
            continue;
        }
        match step {
            "MAKE_LEFT_HANDED" => make_left_handed::execute(scene),
            "REMOVE_REDUNDANT_MATERIALS" => remove_redundant_mats::execute(scene),
            "LIMIT_BONE_WEIGHTS" => limit_bone_weights::execute(scene)?,
            "JOIN_IDENTICAL_VERTICES" => join_vertices::execute(scene)?,
            "TRIANGULATE" => triangulate::execute(scene),
            "FIND_INSTANCES" => find_instances::execute(scene)?,
            "OPTIMIZE_MESHES" => optimize_meshes::execute(scene)?,
            _ => {}
        }
    }
    Ok(())
}
