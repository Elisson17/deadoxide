//! zombie.radio.ChannelCategory and zombie.radio.GameMode.

/// zombie.radio.ChannelCategory
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelCategory {
    Undefined,
    Radio,
    Television,
    Military,
    Amateur,
    Bandit,
    Emergency,
    Other,
}

impl ChannelCategory {
    const ALL: [ChannelCategory; 8] = [
        ChannelCategory::Undefined,
        ChannelCategory::Radio,
        ChannelCategory::Television,
        ChannelCategory::Military,
        ChannelCategory::Amateur,
        ChannelCategory::Bandit,
        ChannelCategory::Emergency,
        ChannelCategory::Other,
    ];

    /// java.lang.Enum.name
    pub fn name(self) -> &'static str {
        match self {
            ChannelCategory::Undefined => "Undefined",
            ChannelCategory::Radio => "Radio",
            ChannelCategory::Television => "Television",
            ChannelCategory::Military => "Military",
            ChannelCategory::Amateur => "Amateur",
            ChannelCategory::Bandit => "Bandit",
            ChannelCategory::Emergency => "Emergency",
            ChannelCategory::Other => "Other",
        }
    }

    /// java.lang.Enum.valueOf for ChannelCategory: the exact name, else the original's `IllegalArgumentException`.
    pub fn value_of(name: &str) -> Result<Self, String> {
        Self::ALL
            .iter()
            .copied()
            .find(|c| c.name() == name)
            .ok_or_else(|| {
                format!(
                    "java.lang.IllegalArgumentException: No enum constant zombie.radio.ChannelCategory.{name}"
                )
            })
    }
}

/// zombie.radio.GameMode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameMode {
    SinglePlayer,
    Server,
    Client,
}
