//! zombie.ui.TextManager before the renderer's `TextManager.Init`: no AngelCodeFont is loaded (`font` and every
//! `enumToFont` slot are null), the state the game's Lua sees when it is loaded standalone. AngelCodeFont is not
//! ported, so a loaded font cannot be represented yet.

/// zombie.ui.TextManager (the fields the scripts reach).
#[derive(Clone, Debug, PartialEq)]
pub struct TextManager {
    /// TextManager.currentCodeFont: the name of a UIFont constant.
    pub current_code_font: &'static str,
}

impl Default for TextManager {
    /// zombie.ui.TextManager.<init>: `currentCodeFont = UIFont.CodeSmall`.
    fn default() -> Self {
        TextManager {
            current_code_font: "CodeSmall",
        }
    }
}

/// A loaded font (zombie.core.fonts.AngelCodeFont): none exists before TextManager.Init.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AngelCodeFont {}

impl TextManager {
    /// zombie.ui.TextManager.getFontFromEnum: `enumToFont[font.ordinal()]`, or `this.font` when that slot (or the
    /// argument) is null. Both are null before Init.
    pub fn get_font_from_enum(&self, _font: Option<i32>) -> Option<AngelCodeFont> {
        None
    }

    /// zombie.ui.TextManager.getCurrentCodeFont
    pub fn get_current_code_font(&self) -> &'static str {
        self.current_code_font
    }
}
