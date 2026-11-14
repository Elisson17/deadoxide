//! zombie.characters.BodyDamage.BodyPartType: the 17 body parts (and MAX) with the modifiers health uses.

/// zombie.characters.BodyDamage.BodyPartType, the fields loading needs: (name, damageModifier, bleedingTimeModifier).
pub const BODY_PART_TYPES: [(&str, f32, f32); 18] = [
    ("Hand_L", 0.1, 0.2),
    ("Hand_R", 0.1, 0.2),
    ("ForeArm_L", 0.2, 0.3),
    ("ForeArm_R", 0.2, 0.3),
    ("UpperArm_L", 0.3, 0.4),
    ("UpperArm_R", 0.3, 0.4),
    ("Torso_Upper", 0.35, 0.5),
    ("Torso_Lower", 0.4, 0.9),
    ("Head", 0.6, 1.0),
    ("Neck", 0.7, 1.5),
    ("Groin", 0.4, 0.5),
    ("UpperLeg_L", 0.3, 0.4),
    ("UpperLeg_R", 0.3, 0.4),
    ("LowerLeg_L", 0.2, 0.3),
    ("LowerLeg_R", 0.2, 0.3),
    ("Foot_L", 0.2, 0.2),
    ("Foot_R", 0.2, 0.2),
    ("Unknown Body Part", 1.0, 1.0),
];

/// `BodyPartType.MAX.ordinal()`: the number of real body parts.
pub const MAX: usize = 17;

/// zombie.characters.BodyDamage.BodyPartType.FromIndex: out of range is MAX.
pub fn from_index(index: i32) -> usize {
    usize::try_from(index)
        .ok()
        .filter(|&i| i < BODY_PART_TYPES.len())
        .unwrap_or(MAX)
}

/// zombie.characters.BodyDamage.BodyPartType.getDamageModifyer
pub fn get_damage_modifyer(index: i32) -> f32 {
    BODY_PART_TYPES[from_index(index)].1
}

/// zombie.characters.BodyDamage.BodyPartType.getBleedingTimeModifyer
pub fn get_bleeding_time_modifyer(index: i32) -> f32 {
    BODY_PART_TYPES[from_index(index)].2
}
