//! The `fmod` package of the game's jar (`fmod.javafmod`, `fmod.javafmodJNI`, `fmod.fmod.*`): the Java side of the FMOD
//! sound system. The native library behind it is in `crate::native::fmod`.
// the jar's own package is `fmod.fmod` (FMODManager, FMOD_GUID, ...), mirrored as is
#[allow(clippy::module_inception)]
pub mod fmod;
