//! Package `zombie.core.skinnedmodel.advancedanimation`: animation sets (`media/AnimSets`), their states and nodes
//! as the game loads them, and the runtime that selects and blends the nodes (`advanced_animator`: `AdvancedAnimator`,
//! `AnimLayer`, `LiveAnimNode`). Bone poses, deferred movement, ragdolls, node blend curves and grappling are not
//! ported (see the UNKNOWN list in `advanced_animator`).
pub mod advanced_animator;
pub mod anim_2d_blend;
pub mod anim_2d_blend_picker;
pub mod anim_2d_blend_triangle;
pub mod anim_bone_weight;
pub mod anim_condition;
pub mod anim_event;
pub mod anim_node;
pub mod anim_node_asset_manager;
pub mod anim_state;
pub mod anim_transition;
pub mod animation_set;
pub mod animation_variable_reference;
pub mod animation_variable_source;
pub mod grapple_offset_behaviour;
pub mod i_animation_variable_source;
