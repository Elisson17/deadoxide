//! zombie.core.skinnedmodel.advancedanimation.AnimationSet
use std::collections::HashMap;

use super::anim_node_asset_manager::AnimNodeAssetManager;
use super::anim_state::AnimState;
use crate::zombie::zomboid_file_system::ZomboidFileSystem;

/// zombie.core.skinnedmodel.advancedanimation.AnimationSet: the states of `media/AnimSets/<name>`, by lower-case
/// folder name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimationSet {
    pub states: HashMap<String, AnimState>,
    pub name: String,
}

/// The statics of the original: `AnimationSet.setMap` and `AnimNodeAssetManager.instance`, as one value.
#[derive(Default)]
pub struct AnimationSets {
    pub set_map: HashMap<String, AnimationSet>,
    pub asset_manager: AnimNodeAssetManager,
}

impl AnimationSets {
    /// zombie.core.skinnedmodel.advancedanimation.AnimationSet.GetAnimationSet: the loaded set, else (or with `reload`)
    /// a new set loaded and stored. `Err` is an exception of the loading, which leaves the map as it was.
    pub fn get_animation_set(
        &mut self,
        fs: &ZomboidFileSystem,
        name: &str,
        reload: bool,
    ) -> Result<&AnimationSet, String> {
        if reload || !self.set_map.contains_key(name) {
            let s = AnimationSet::load(name, fs, &mut self.asset_manager)?;
            self.set_map.insert(name.to_string(), s);
        }
        self.set_map
            .get(name)
            .ok_or_else(|| "AnimationSet.GetAnimationSet: not stored".to_string())
    }
}

impl AnimationSet {
    /// zombie.core.skinnedmodel.advancedanimation.AnimationSet.Load: one state per folder directly under
    /// `media/AnimSets/<name>` (`resolveAllDirectories`, not recursive), parsed by `AnimState.Parse` and stored under
    /// its lower-cased name (a later folder with the same lower-case name replaces an earlier one).
    pub fn load(
        name: &str,
        fs: &ZomboidFileSystem,
        assets: &mut AnimNodeAssetManager,
    ) -> Result<AnimationSet, String> {
        let mut set = AnimationSet {
            states: HashMap::new(),
            name: name.to_string(),
        };
        let dirs = fs.resolve_all_directories(&format!("media/AnimSets/{name}"), &|_| true, false);
        for state_dir in dirs {
            let state_name = state_dir.rsplit('/').next().unwrap_or("").to_string();
            let state = AnimState::parse(&state_name, &state_dir, fs, assets)?;
            set.states.insert(state_name.to_lowercase(), state);
        }
        Ok(set)
    }
}
