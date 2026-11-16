//! zombie.core.skinnedmodel.advancedanimation.AnimState
use super::anim_node::AnimNode;
use super::anim_node_asset_manager::AnimNodeAssetManager;
use crate::zombie::zomboid_file_system::ZomboidFileSystem;

/// zombie.core.skinnedmodel.advancedanimation.AnimState: the nodes of a state folder, the selectable ones ordered by
/// their selection conditions and the abstract ones apart. Each node's `parentState` is this state; `set` is the set
/// that holds it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimState {
    pub name: String,
    pub nodes: Vec<AnimNode>,
    pub abstract_nodes: Vec<AnimNode>,
    pub default_index: i32,
}

impl AnimState {
    /// zombie.core.skinnedmodel.advancedanimation.AnimState.Parse: every `.xml` file under the state folder (recursively,
    /// `resolveAllFiles`), resolved to its absolute path (`resolveFileOrGUID`) and loaded as an AnimNode asset; the
    /// ready ones are added (`addNode`).
    pub fn parse(
        name: &str,
        state_path: &str,
        fs: &ZomboidFileSystem,
        assets: &mut AnimNodeAssetManager,
    ) -> Result<AnimState, String> {
        let mut state = AnimState {
            name: name.to_string(),
            ..AnimState::default()
        };
        let files = fs.resolve_all_files(
            state_path,
            &|f| {
                f.file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with(".xml"))
            },
            true,
        );
        for node_file_name in files {
            let absolute_path = fs.resolve_file_or_guid(&node_file_name);
            if let Some(node) = assets.load(fs, &absolute_path)? {
                state.add_node(node.clone());
            }
        }
        Ok(state)
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimState.addNode: an abstract node is appended to `abstractNodes`;
    /// another goes before the first node it compares above (`compareSelectionConditions` > 0), else at the end.
    pub fn add_node(&mut self, new_node: AnimNode) {
        if new_node.is_abstract() {
            self.abstract_nodes.push(new_node);
            return;
        }
        let insert_at = self
            .nodes
            .iter()
            .position(|node| AnimNode::compare_selection_conditions(&new_node, node) > 0)
            .unwrap_or(self.nodes.len());
        self.nodes.insert(insert_at, new_node);
    }
}
