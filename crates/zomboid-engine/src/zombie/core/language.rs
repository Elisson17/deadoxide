//! zombie.core.Language: one translation language (a folder of `media/lua/shared/Translate`).

/// zombie.core.Language: name (the folder name, `toString()`), display text, base language name, AZERTY flag.
/// Java compares languages by identity; the port shares them as `Rc<Language>` and compares with `Rc::ptr_eq`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Language {
    pub name: String,
    pub text: Option<String>,
    pub base: Option<String>,
    pub azerty: bool,
}
