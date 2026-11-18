//! assimp's glTF2 importer (`code/AssetLib/glTF2`): `glTF2Asset`, the rapidjson reader and `glTF2Importer`, for the binary
//! container (`.glb`) the game ships.
pub mod asset;
pub mod importer;
pub mod json;
pub mod libcxx_map;

/// `glTF2Importer::InternReadFile` of a `.glb`: the scene before the post-processing steps.
pub fn import(bytes: &[u8]) -> Result<crate::native::assimp::scene::Scene, String> {
    let (text, body) = asset::read_glb(bytes)?;
    let doc = json::parse(text)?;
    let a = asset::Asset::load(&doc, body)?;
    importer::convert(&a)
}
