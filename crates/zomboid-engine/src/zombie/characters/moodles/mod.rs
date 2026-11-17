//! Package `zombie.characters.Moodles`: the moodle levels a character shows, from its stats and body.
pub mod moodle;
pub mod moodle_stat;
// zombie.characters.Moodles.Moodles keeps its name inside the package
#[allow(clippy::module_inception)]
pub mod moodles;
