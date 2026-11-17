//! Package `zombie.core.skinnedmodel.model.jassimp`: what the game does with a scene the native assimp imported
//! (`ProcessedAiScene`, `ImportedSkeleton`, `ImportedSkinnedMesh`, `ImportedStaticMesh`, `JAssImpImporter`).
pub mod imported_skeleton;
pub mod imported_skinned_mesh;
pub mod imported_static_mesh;
pub mod jassimp_importer;
pub mod processed_ai_scene;

/// JAssImpImporter.LoadMode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadMode {
    Normal,
    StaticMesh,
    AnimationOnly,
}
