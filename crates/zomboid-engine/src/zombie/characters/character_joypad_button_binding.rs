//! zombie.characters.CharacterJoypadButtonBinding: which controller button or axis triggers each character action.
//! The original keeps the current binding in each enum constant (a static); here one value holds all of them.
//! JoypadButton / JoypadAxis1d / JoypadAxis2d constants are kept by name (`None` is Java null).

/// zombie.characters.CharacterJoypadButtonBinding.IsDownBinding, one variant per implementation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IsDownBinding {
    /// CharacterJoypadButtonBinding.JoypadButtonBinding
    JoypadButton(Option<&'static str>),
    /// CharacterJoypadButtonBinding.Axis1dMinMaxBinding
    Axis1dMinMax {
        axis1d: Option<&'static str>,
        min: f32,
        max: f32,
    },
    /// CharacterJoypadButtonBinding.Axis2dMinMaxBinding
    Axis2dMinMax {
        axis2d: Option<&'static str>,
        min: f32,
        max: f32,
    },
}

impl IsDownBinding {
    /// IsDownBinding.getButton (the interface's default is null)
    pub fn get_button(&self) -> Option<&'static str> {
        match self {
            IsDownBinding::JoypadButton(b) => *b,
            _ => None,
        }
    }

    /// IsDownBinding.getAxis1d
    pub fn get_axis1d(&self) -> Option<&'static str> {
        match self {
            IsDownBinding::Axis1dMinMax { axis1d, .. } => *axis1d,
            _ => None,
        }
    }

    /// IsDownBinding.getAxis2d
    pub fn get_axis2d(&self) -> Option<&'static str> {
        match self {
            IsDownBinding::Axis2dMinMax { axis2d, .. } => *axis2d,
            _ => None,
        }
    }

    /// IsDownBinding.getMin (the interface's default is 0)
    pub fn get_min(&self) -> f32 {
        match self {
            IsDownBinding::JoypadButton(_) => 0.0,
            IsDownBinding::Axis1dMinMax { min, .. } | IsDownBinding::Axis2dMinMax { min, .. } => {
                *min
            }
        }
    }

    /// IsDownBinding.getMax (the interface's default is 0)
    pub fn get_max(&self) -> f32 {
        match self {
            IsDownBinding::JoypadButton(_) => 0.0,
            IsDownBinding::Axis1dMinMax { max, .. } | IsDownBinding::Axis2dMinMax { max, .. } => {
                *max
            }
        }
    }
}

const fn button(b: &'static str) -> Option<IsDownBinding> {
    Some(IsDownBinding::JoypadButton(Some(b)))
}

/// The `(JoypadAxis1d, float min)` constructor: max is Float.MAX_VALUE.
const fn axis1d(a: &'static str, min: f32) -> Option<IsDownBinding> {
    Some(IsDownBinding::Axis1dMinMax {
        axis1d: Some(a),
        min,
        max: f32::MAX,
    })
}

/// The `(JoypadAxis2d, float min)` constructor: max is Float.MAX_VALUE.
const fn axis2d(a: &'static str, min: f32) -> Option<IsDownBinding> {
    Some(IsDownBinding::Axis2dMinMax {
        axis2d: Some(a),
        min,
        max: f32::MAX,
    })
}

/// The enum constants in declaration order with their `defaultBinding`.
pub const CONSTANTS: [(&str, Option<IsDownBinding>); 28] = [
    ("Aim", axis2d("RightStick", 0.1)),
    ("PrecisionAim", axis1d("RightTrigger", -0.8)),
    ("Melee", axis1d("LeftTrigger", -0.8)),
    ("Attack", axis1d("RightTrigger", 0.93)),
    ("Run", axis2d("LeftStick", 0.975)),
    ("Interact", button("A")),
    ("WalkTo", None),
    ("Crouch", button("LeftStick")),
    ("ReloadWeapon", button("RightBump")),
    ("RackFirearm", button("LeftBump")),
    ("Sprint", button("RightStick")),
    ("CancelAction", button("B")),
    ("ManualFloorAtk", button("LeftBump")),
    ("Inventory", button("Y")),
    ("Loot", button("X")),
    ("ClosePanel", button("B")),
    ("CycleLoot", button("RightBump")),
    ("CycleInventory", button("LeftBump")),
    ("CycleTabsLeft", button("LeftBump")),
    ("CycleTabsRight", button("RightBump")),
    ("TransferItem", button("X")),
    ("InteractOptions", button("X")),
    ("ClimbThrough", button("B")),
    ("SmashWindow", button("B")),
    ("Brakes", button("B")),
    ("CruiseControl", button("X")),
    ("ZoomIn", None),
    ("ZoomOut", None),
];

/// The `binding` field of every CharacterJoypadButtonBinding constant, by ordinal.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterJoypadButtonBindings {
    pub binding: Vec<Option<IsDownBinding>>,
}

impl Default for CharacterJoypadButtonBindings {
    /// The enum's class initialisation: every constructor ends with setDefault().
    fn default() -> Self {
        CharacterJoypadButtonBindings {
            binding: CONSTANTS.iter().map(|(_, d)| *d).collect(),
        }
    }
}

impl CharacterJoypadButtonBindings {
    /// The ordinal of the constant called `name`.
    pub fn ordinal(name: &str) -> Option<usize> {
        CONSTANTS.iter().position(|(n, _)| *n == name)
    }

    /// zombie.characters.CharacterJoypadButtonBinding.setDefault
    pub fn set_default(&mut self, ordinal: usize) {
        self.binding[ordinal] = CONSTANTS[ordinal].1;
    }

    /// zombie.characters.CharacterJoypadButtonBinding.setAllToDefault
    pub fn set_all_to_default(&mut self) {
        for i in 0..CONSTANTS.len() {
            self.set_default(i);
        }
    }

    /// zombie.characters.CharacterJoypadButtonBinding.setBinding(JoypadButton)
    pub fn set_binding_button(&mut self, ordinal: usize, new_binding: Option<&'static str>) {
        self.binding[ordinal] = Some(IsDownBinding::JoypadButton(new_binding));
    }

    /// zombie.characters.CharacterJoypadButtonBinding.setBinding(JoypadAxis1d, float, float); the two-argument
    /// overload passes Float.MAX_VALUE as max.
    pub fn set_binding_axis1d(
        &mut self,
        ordinal: usize,
        axis1d: Option<&'static str>,
        min: f32,
        max: f32,
    ) {
        self.binding[ordinal] = Some(IsDownBinding::Axis1dMinMax { axis1d, min, max });
    }

    /// zombie.characters.CharacterJoypadButtonBinding.setBinding(JoypadAxis2d, float, float); the two-argument
    /// overload passes Float.MAX_VALUE as max.
    pub fn set_binding_axis2d(
        &mut self,
        ordinal: usize,
        axis2d: Option<&'static str>,
        min: f32,
        max: f32,
    ) {
        self.binding[ordinal] = Some(IsDownBinding::Axis2dMinMax { axis2d, min, max });
    }

    /// zombie.characters.CharacterJoypadButtonBinding.getJoypadButton
    pub fn get_joypad_button(&self, ordinal: usize) -> Option<&'static str> {
        self.binding[ordinal].and_then(|b| b.get_button())
    }

    /// zombie.characters.CharacterJoypadButtonBinding.getJoypadAxis1d
    pub fn get_joypad_axis1d(&self, ordinal: usize) -> Option<&'static str> {
        self.binding[ordinal].and_then(|b| b.get_axis1d())
    }

    /// zombie.characters.CharacterJoypadButtonBinding.getJoypadAxis2d
    pub fn get_joypad_axis2d(&self, ordinal: usize) -> Option<&'static str> {
        self.binding[ordinal].and_then(|b| b.get_axis2d())
    }
}
