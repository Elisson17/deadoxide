//! Package `zombie.characters.BodyDamage`: wounds, health, temperature, nutrition and fitness of a character.
// The Java package and its class share the name (zombie.characters.BodyDamage.BodyDamage); the tree mirrors it.
#[allow(clippy::module_inception)]
pub mod body_damage;
pub mod body_fields;
pub mod body_part;
pub mod body_part_type;
pub mod body_sim;
pub mod body_update;
pub mod fitness;
pub mod metabolics;
pub mod nutrition;
pub mod thermoregulator;
