//! zombie.ui.FontsFileFont: one `font <id> { fnt = ..., img = ..., type = ... }` block of a `fonts.txt`.

/// zombie.ui.FontsFileFont (`FontsFile` always sets `id` and `fnt`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontsFileFont {
    pub id: String,
    pub fnt: String,
    pub img: Option<String>,
    pub r#type: Option<String>,
}
