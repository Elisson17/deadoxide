//! The Open Asset Import Library (assimp 5.3, the build inside the game's `libjassimp`) as far as the game uses it
//! through `jassimp.Jassimp.importFile`: the DirectX `.x` importer, the post-processing steps the game asks for, and
//! the `aiScene` they work on. Each source file names the assimp file it follows.
//!
//! assimp is distributed under the 3-clause BSD license (Copyright (c) 2006-2023, assimp team); this port is a derived
//! work of its C++ sources and keeps that notice.
//!
//! The C++ sources are not part of this tree: each step is written from the library's published algorithm and
//! checked against the game's own `libjassimp` (the oracle cases `xscene`, ...), file by file over every game asset.
//!
//! Float arithmetic follows the library's `float` (`ai_real`) code on the x86-64 slice of the game's `libjassimp`
//! (SSE, one rounding per operation). The arm64 slice contracts multiply-add pairs and returns different last bits
//! for some results (the quaternion -> matrix -> quaternion round trip of the X importer's rotation keys); it is not
//! the reference.
pub mod fast_atof;
pub mod fbx;
pub mod post_process;
pub mod scene;
pub mod std_sort;
pub mod x_file_importer;
pub mod x_file_parser;

/// `Importer::ReadFile(path, steps)` as `jassimp.Jassimp.importFile` calls it: the scene of the file at `path` (by
/// extension) and then the post-processing steps of `spec` (`mesh`, `anim`, `none` or a comma separated list of
/// `AiPostProcessSteps` names). `Err` is the error text of the library.
pub fn import_file(path: &str, bytes: &[u8], spec: &str) -> Result<scene::Scene, String> {
    let lower = path.to_lowercase();
    let mut scene = if lower.ends_with(".x") {
        x_file_importer::import(bytes)?
    } else {
        return Err(format!(
            "No suitable reader found for the file format of file \"{path}\"."
        ));
    };
    post_process::apply(&mut scene, spec)?;
    Ok(scene)
}
