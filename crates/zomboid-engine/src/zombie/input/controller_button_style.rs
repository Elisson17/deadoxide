//! zombie.input.ControllerButtonStyle: the controller icon sets of the `controllerButtonStyle` option.

/// zombie.input.ControllerButtonStyle
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerButtonStyle {
    Xbox,
    Ps4,
    SteamDeck,
}

impl ControllerButtonStyle {
    /// ControllerButtonStyle.values()
    pub const VALUES: [ControllerButtonStyle; 3] = [
        ControllerButtonStyle::Xbox,
        ControllerButtonStyle::Ps4,
        ControllerButtonStyle::SteamDeck,
    ];

    /// zombie.input.ControllerButtonStyle.getOptionCode
    pub fn get_option_code(self) -> i32 {
        match self {
            ControllerButtonStyle::Xbox => 1,
            ControllerButtonStyle::Ps4 => 2,
            ControllerButtonStyle::SteamDeck => 3,
        }
    }

    /// zombie.input.ControllerButtonStyle.getLabel
    pub fn get_label(self) -> &'static str {
        match self {
            ControllerButtonStyle::Xbox => "XBOX",
            ControllerButtonStyle::Ps4 => "PS4",
            ControllerButtonStyle::SteamDeck => "STEAMDECK",
        }
    }

    /// zombie.input.ControllerButtonStyle.fromOptionCode: the first style with that code, PS4 otherwise.
    pub fn from_option_code(value: i32) -> ControllerButtonStyle {
        Self::VALUES
            .into_iter()
            .find(|s| s.get_option_code() == value)
            .unwrap_or(ControllerButtonStyle::Ps4)
    }
}
