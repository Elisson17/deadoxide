//! zombie.core.skinnedmodel.model.jassimp.ProcessedAiScene: the skeleton, skinned mesh and static meshes the game
//! builds from a scene the native assimp imported. (`applyToMesh`, `applyToPhysicsShape` and `applyToAnimation` copy
//! these into `ModelMesh`, `PhysicsShape` and `AnimationAsset`, the GL and asset-manager objects, which are not ported.)
use super::imported_skeleton::{ImportedSkeleton, ImportedSkeletonParams};
use super::imported_skinned_mesh::ImportedSkinnedMesh;
use super::imported_static_mesh::ImportedStaticMesh;
use super::jassimp_importer::{get_matrix_from_ai_matrix, SceneTree};
use super::LoadMode;
use crate::java::{equals_ignore_case, is_whitespace};
use crate::native::assimp::scene::Scene;
use crate::org::lwjgl::util::vector::matrix4f::Matrix4f;
use crate::org::lwjgl::util::vector::quaternion::Quaternion;
use crate::zombie::core::skinnedmodel::model::skinning_data::SkinningData;

/// ProcessedAiSceneParams
pub struct ProcessedAiSceneParams<'a> {
    pub scene: &'a Scene,
    pub mode: LoadMode,
    pub skinned_to: Option<&'a SkinningData>,
    pub mesh_name: Option<String>,
    pub anim_bones_scale_modifier: f32,
    pub anim_bones_rotate_modifier: Option<Quaternion>,
    pub all_meshes: bool,
}

impl<'a> ProcessedAiSceneParams<'a> {
    /// ProcessedAiSceneParams.create(): `Normal`, scale 1, no rotation modifier.
    pub fn create(scene: &'a Scene) -> Self {
        ProcessedAiSceneParams {
            scene,
            mode: LoadMode::Normal,
            skinned_to: None,
            mesh_name: None,
            anim_bones_scale_modifier: 1.0,
            anim_bones_rotate_modifier: None,
            all_meshes: false,
        }
    }
}

/// ProcessedAiScene
#[derive(Clone, Debug, Default)]
pub struct ProcessedAiScene {
    pub skeleton: Option<ImportedSkeleton>,
    pub skinned_mesh: Option<ImportedSkinnedMesh>,
    pub static_meshes: Vec<ImportedStaticMesh>,
}

/// StringUtils.isNullOrWhitespace
fn is_null_or_whitespace(s: Option<&str>) -> bool {
    s.is_none_or(|s| s.chars().all(is_whitespace))
}

impl ProcessedAiScene {
    /// ProcessedAiScene.process; `Err` is the Java exception class.
    pub fn process(params: &ProcessedAiSceneParams) -> Result<ProcessedAiScene, String> {
        let mut p = ProcessedAiScene::default();
        let tree = SceneTree::build(
            params
                .scene
                .root_node
                .as_ref()
                .ok_or("java.lang.NullPointerException")?,
        );
        if params.all_meshes {
            p.process_all_meshes(params, &tree)?;
        } else {
            p.process_ai_scene(params, &tree)?;
        }
        Ok(p)
    }

    fn process_ai_scene(
        &mut self,
        params: &ProcessedAiSceneParams,
        tree: &SceneTree,
    ) -> Result<(), String> {
        let scene = params.scene;
        let mode = params.mode;
        let Some(mesh_idx) = Self::find_mesh(scene, tree, params.mesh_name.as_deref())? else {
            return Ok(()); // "No such mesh" is logged
        };
        let mesh = &scene.meshes[mesh_idx];
        if mode != LoadMode::StaticMesh && !mesh.bones.is_empty() {
            let skeleton = ImportedSkeleton::process(&ImportedSkeletonParams {
                scene,
                tree,
                mode,
                skinned_to: params.skinned_to,
                anim_bones_scale_modifier: params.anim_bones_scale_modifier,
                anim_bones_rotate_modifier: params.anim_bones_rotate_modifier,
                mesh,
            })?;
            if mode != LoadMode::AnimationOnly {
                let mut skinned = ImportedSkinnedMesh::new(&skeleton, mesh)?;
                skinned.transform = Self::init_mesh_transform(scene, tree, mesh_idx);
                self.skinned_mesh = Some(skinned);
            }
            self.skeleton = Some(skeleton);
        } else {
            let mut static_mesh = ImportedStaticMesh::new(mesh)?;
            static_mesh.transform = Self::init_mesh_transform(scene, tree, mesh_idx);
            self.static_meshes.push(static_mesh);
        }
        Ok(())
    }

    fn process_all_meshes(
        &mut self,
        params: &ProcessedAiSceneParams,
        tree: &SceneTree,
    ) -> Result<(), String> {
        if params.mode == LoadMode::StaticMesh {
            for (i, mesh) in params.scene.meshes.iter().enumerate() {
                let mut static_mesh = ImportedStaticMesh::new(mesh)?;
                static_mesh.transform = Self::init_mesh_transform(params.scene, tree, i);
                self.static_meshes.push(static_mesh);
            }
        }
        Ok(())
    }

    /// ProcessedAiScene.initMeshTransform: the transform of the first node (pre-order) that refers to the mesh,
    /// multiplied by each ancestor's transform up to the root.
    fn init_mesh_transform(scene: &Scene, tree: &SceneTree, mesh_idx: usize) -> Option<Matrix4f> {
        let _ = scene;
        let node = Self::find_parent_node_for_mesh(tree, mesh_idx as u32, 0)?;
        let mut transform = get_matrix_from_ai_matrix(&tree.nodes[node].transform);
        let mut parent = tree.nodes[node].parent;
        while let Some(p) = parent {
            let p_xfrm = get_matrix_from_ai_matrix(&tree.nodes[p].transform);
            transform = Matrix4f::mul(&p_xfrm, &transform);
            parent = tree.nodes[p].parent;
        }
        Some(transform)
    }

    /// ProcessedAiScene.findMesh: the index of the mesh, `None` for no mesh.
    fn find_mesh(
        scene: &Scene,
        tree: &SceneTree,
        name: Option<&str>,
    ) -> Result<Option<usize>, String> {
        if scene.meshes.is_empty() {
            return Ok(None);
        }
        if is_null_or_whitespace(name) {
            for (i, mesh) in scene.meshes.iter().enumerate() {
                if !mesh.bones.is_empty() {
                    return Ok(Some(i));
                }
            }
            return Ok(Some(0));
        }
        let name = name.unwrap_or("");
        for (i, mesh) in scene.meshes.iter().enumerate() {
            if equals_ignore_case(&mesh.name, name) {
                return Ok(Some(i));
            }
        }
        let node = tree.find_node(name, 0);
        if let Some(n) = node {
            if tree.nodes[n].meshes.len() == 1 {
                let mesh_ref = tree.nodes[n].meshes[0] as usize;
                if mesh_ref >= scene.meshes.len() {
                    return Err("java.lang.IndexOutOfBoundsException".to_string());
                }
                return Ok(Some(mesh_ref));
            }
        }
        Ok(None)
    }

    /// ProcessedAiScene.findParentNodeForMesh
    fn find_parent_node_for_mesh(tree: &SceneTree, mesh_ref: u32, node: usize) -> Option<usize> {
        if tree.nodes[node].meshes.contains(&mesh_ref) {
            return Some(node);
        }
        for &child in &tree.nodes[node].children {
            if let Some(n) = Self::find_parent_node_for_mesh(tree, mesh_ref, child) {
                return Some(n);
            }
        }
        None
    }
}
