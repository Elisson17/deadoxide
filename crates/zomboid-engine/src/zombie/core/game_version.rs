//! zombie.core.GameVersion: a `major.minor[suffix]` game version, compared by `major * 1000 + minor`.
use std::fmt;

use crate::zombie::core::math::pz_math;

/// zombie.core.Core.gameVersion: the version of the game this port follows (42.21).
pub fn core_game_version() -> GameVersion {
    GameVersion::plain(42, 21)
}

/// zombie.core.GameVersion
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameVersion {
    major: i32,
    minor: i32,
    /// `String.format("%d.%d%s", major, minor, suffix)`
    string: String,
}

impl GameVersion {
    /// zombie.core.GameVersion.<init>: `Err` is the IllegalArgumentException's message (a negative major, a minor
    /// outside 0..999).
    pub fn new(major: i32, minor: i32, suffix: &str) -> Result<GameVersion, String> {
        if major < 0 {
            return Err("major version must be greater than zero".into());
        }
        if !(0..=999).contains(&minor) {
            return Err("minor version must be from 0 to 999".into());
        }
        Ok(GameVersion {
            major,
            minor,
            string: format!("{major}.{minor}{suffix}"),
        })
    }

    /// `new GameVersion(major, minor, "")` for the original's constants (`minor` within 0..999; not checked).
    pub fn plain(major: u16, minor: u16) -> GameVersion {
        GameVersion {
            major: i32::from(major),
            minor: i32::from(minor),
            string: format!("{major}.{minor}"),
        }
    }

    /// zombie.core.GameVersion.getMajor
    pub fn get_major(&self) -> i32 {
        self.major
    }

    /// zombie.core.GameVersion.getMinor
    pub fn get_minor(&self) -> i32 {
        self.minor
    }

    /// zombie.core.GameVersion.getInt: `major * 1000 + minor` (int arithmetic).
    pub fn get_int(&self) -> i32 {
        self.major.wrapping_mul(1000).wrapping_add(self.minor)
    }

    /// zombie.core.GameVersion.isGreaterThan
    pub fn is_greater_than(&self, rhs: &GameVersion) -> bool {
        self.get_int() > rhs.get_int()
    }

    /// zombie.core.GameVersion.isLessThan
    pub fn is_less_than(&self, rhs: &GameVersion) -> bool {
        self.get_int() < rhs.get_int()
    }

    /// zombie.core.GameVersion.parse: `([0-9]+)\.([0-9]+)(.*)` over the whole string (`.` matches no line
    /// terminator: `\n \r \u{85} \u{2028} \u{2029}`), each number through `PZMath.tryParseInt(s, 0)` (too large is 0),
    /// the rest the suffix. `Err` is the IllegalArgumentException's message.
    pub fn parse(s: &str) -> Result<GameVersion, String> {
        let invalid = || format!("invalid game version \"{s}\"");
        let digits = |t: &str| t.bytes().take_while(u8::is_ascii_digit).count();
        let n1 = digits(s);
        if n1 == 0 || !s[n1..].starts_with('.') {
            return Err(invalid());
        }
        let after = &s[n1 + 1..];
        let n2 = digits(after);
        if n2 == 0 {
            return Err(invalid());
        }
        let suffix = &after[n2..];
        if suffix.contains(['\n', '\r', '\u{85}', '\u{2028}', '\u{2029}']) {
            return Err(invalid());
        }
        let major = pz_math::try_parse_int(Some(&s[..n1]), 0);
        let minor = pz_math::try_parse_int(Some(&after[..n2]), 0);
        GameVersion::new(major, minor, suffix)
    }
}

impl fmt::Display for GameVersion {
    /// zombie.core.GameVersion.toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.string)
    }
}
