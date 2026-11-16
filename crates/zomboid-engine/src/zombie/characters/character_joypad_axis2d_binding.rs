//! zombie.characters.CharacterJoypadAxis2dBinding: which stick moves and which aims. The original keeps the current
//! binding in each enum constant (a static); here one value holds both. JoypadAxis2d constants are kept by name.

/// The enum constants in declaration order with their `defaultBinding` (a JoypadAxis2d).
pub const CONSTANTS: [(&str, &str); 2] = [("Movement", "LeftStick"), ("Aiming", "RightStick")];

/// The `binding` field of every CharacterJoypadAxis2dBinding constant, by ordinal (`None` is null).
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterJoypadAxis2dBindings {
    pub binding: Vec<Option<&'static str>>,
}

impl Default for CharacterJoypadAxis2dBindings {
    /// The enum's class initialisation: the constructor ends with setDefault().
    fn default() -> Self {
        CharacterJoypadAxis2dBindings {
            binding: CONSTANTS.iter().map(|(_, d)| Some(*d)).collect(),
        }
    }
}

impl CharacterJoypadAxis2dBindings {
    /// zombie.characters.CharacterJoypadAxis2dBinding.setDefault
    pub fn set_default(&mut self, ordinal: usize) {
        self.binding[ordinal] = Some(CONSTANTS[ordinal].1);
    }

    /// zombie.characters.CharacterJoypadAxis2dBinding.setAllToDefault
    pub fn set_all_to_default(&mut self) {
        for i in 0..CONSTANTS.len() {
            self.set_default(i);
        }
    }
}
