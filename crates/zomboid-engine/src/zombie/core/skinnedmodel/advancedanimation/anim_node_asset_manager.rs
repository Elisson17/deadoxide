//! zombie.core.skinnedmodel.advancedanimation.AnimNodeAssetManager
use std::collections::HashMap;

use super::anim_node::AnimNode;
use crate::zombie::zomboid_file_system::ZomboidFileSystem;

/// zombie.core.skinnedmodel.advancedanimation.AnimNodeAssetManager: the AnimNode assets by path (`AssetManager.assets`).
/// An asset loads once (`AnimNode.Parse` runs synchronously in `startLoading`); `None` is a failed asset.
#[derive(Default)]
pub struct AnimNodeAssetManager {
    pub assets: HashMap<String, Option<AnimNode>>,
}

impl AnimNodeAssetManager {
    /// zombie.asset.AssetManager.load(AssetPath) with AnimNodeAssetManager.startLoading: the asset of `path`, parsed
    /// on its first load (`Ok(None)`: failed, `isReady()` false). `Err` is an exception of the parse, which the
    /// original lets through (the asset then stays in the table, not ready).
    pub fn load(
        &mut self,
        fs: &ZomboidFileSystem,
        path: &str,
    ) -> Result<Option<&AnimNode>, String> {
        if path.is_empty() {
            return Ok(None);
        }
        if !self.assets.contains_key(path) {
            self.assets.insert(path.to_string(), None);
            let node = AnimNode::parse(fs, path)?;
            self.assets.insert(path.to_string(), node);
        }
        Ok(self.assets.get(path).and_then(Option::as_ref))
    }
}
