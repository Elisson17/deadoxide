//! zombie.ui.UIFont: the font names the UI asks `TextManager` for.

/// zombie.ui.UIFont, in declaration order (`ordinal()` is the discriminant).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UIFont {
    Small,
    Medium,
    Large,
    Massive,
    MainMenu1,
    MainMenu2,
    Cred1,
    Cred2,
    NewSmall,
    NewMedium,
    NewLarge,
    Code,
    CodeSmall,
    CodeMedium,
    CodeLarge,
    MediumNew,
    AutoNormSmall,
    AutoNormMedium,
    AutoNormLarge,
    Dialogue,
    Intro,
    Handwritten,
    DebugConsole,
    Title,
    SdfRegular,
    SdfBold,
    SdfItalic,
    SdfBoldItalic,
    SdfOldRegular,
    SdfOldBold,
    SdfOldItalic,
    SdfOldBoldItalic,
    SdfRobertoSans,
    SdfCaveat,
}

use UIFont::*;
const VALUES: [UIFont; 34] = [
    Small,
    Medium,
    Large,
    Massive,
    MainMenu1,
    MainMenu2,
    Cred1,
    Cred2,
    NewSmall,
    NewMedium,
    NewLarge,
    Code,
    CodeSmall,
    CodeMedium,
    CodeLarge,
    MediumNew,
    AutoNormSmall,
    AutoNormMedium,
    AutoNormLarge,
    Dialogue,
    Intro,
    Handwritten,
    DebugConsole,
    Title,
    SdfRegular,
    SdfBold,
    SdfItalic,
    SdfBoldItalic,
    SdfOldRegular,
    SdfOldBold,
    SdfOldItalic,
    SdfOldBoldItalic,
    SdfRobertoSans,
    SdfCaveat,
];

impl UIFont {
    /// java.lang.Enum: the implicit `UIFont.values()` (declaration order).
    pub fn values() -> &'static [UIFont] {
        &VALUES
    }

    /// java.lang.Enum.ordinal of a UIFont.
    pub fn ordinal(self) -> usize {
        self as usize
    }

    /// java.lang.Enum.name of a UIFont.
    pub fn name(self) -> &'static str {
        match self {
            Small => "Small",
            Medium => "Medium",
            Large => "Large",
            Massive => "Massive",
            MainMenu1 => "MainMenu1",
            MainMenu2 => "MainMenu2",
            Cred1 => "Cred1",
            Cred2 => "Cred2",
            NewSmall => "NewSmall",
            NewMedium => "NewMedium",
            NewLarge => "NewLarge",
            Code => "Code",
            CodeSmall => "CodeSmall",
            CodeMedium => "CodeMedium",
            CodeLarge => "CodeLarge",
            MediumNew => "MediumNew",
            AutoNormSmall => "AutoNormSmall",
            AutoNormMedium => "AutoNormMedium",
            AutoNormLarge => "AutoNormLarge",
            Dialogue => "Dialogue",
            Intro => "Intro",
            Handwritten => "Handwritten",
            DebugConsole => "DebugConsole",
            Title => "Title",
            SdfRegular => "SdfRegular",
            SdfBold => "SdfBold",
            SdfItalic => "SdfItalic",
            SdfBoldItalic => "SdfBoldItalic",
            SdfOldRegular => "SdfOldRegular",
            SdfOldBold => "SdfOldBold",
            SdfOldItalic => "SdfOldItalic",
            SdfOldBoldItalic => "SdfOldBoldItalic",
            SdfRobertoSans => "SdfRobertoSans",
            SdfCaveat => "SdfCaveat",
        }
    }

    /// zombie.ui.UIFont.FromString: `valueOf` (exact name), `None` (null) otherwise.
    pub fn from_string(s: &str) -> Option<UIFont> {
        VALUES.iter().copied().find(|f| f.name() == s)
    }
}
