//! Port of what the game's native FMOD Studio library (`libfmodstudio`, FMOD 2.03.09, reached through
//! `fmod.javafmodJNI`) reads from the `.bank` files in `media/sound/banks/Desktop`. FMOD ships as a closed binary and
//! the repository holds no documentation or source of the bank format: everything here is reverse engineered from the
//! files and CHECKED against the ORIGINAL (the oracle cases `fmodbanks`, `fmodstrings`, `fmodevents`, `fmodsamples`
//! run the game's own `FMODManager.init()` headless and print what the API reports). A field that no API call exposes
//! is parsed but never claimed: the docs table in COMPATIBILITY.md says which is which.
pub mod bank;
pub mod riff;
pub mod strings;
pub mod studio;
