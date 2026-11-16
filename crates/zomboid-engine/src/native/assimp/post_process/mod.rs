//! assimp `code/PostProcessing`: the steps `FileTask_LoadMesh` and `FileTask_LoadAnimation` ask for, and the
//! `ScenePreprocessor` that runs before them.
pub mod flip_winding_order;
pub mod make_left_handed;
pub mod scene_preprocessor;

/// The post-processing steps `Importer::ApplyPostProcessing` runs for the flags of `spec` (`mesh`, `anim`, `none` or
/// a comma separated list of `AiPostProcessSteps` names), in the library's registration order.
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
    let has = |n: &str| names.contains(&n);
    if has("MAKE_LEFT_HANDED") {
        make_left_handed::execute(scene);
    }
    for n in &names {
        if !matches!(*n, "MAKE_LEFT_HANDED") {
            return Err(format!("UNPORTED: post-process step {n}"));
        }
    }
    Ok(())
}
