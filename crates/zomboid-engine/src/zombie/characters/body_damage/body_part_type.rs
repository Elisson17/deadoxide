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

/// zombie.characters.BodyDamage.BodyPartType, the per-part tables the simulation reads: (painModifier, skinSurface,
/// distToCore) in constructor order, one row per `BODY_PART_TYPES` row (MAX last).
const BODY_PART_SIM: [(f32, f32, f32); 18] = [
    (0.5, 0.01, 0.8),
    (0.5, 0.01, 0.8),
    (0.6, 0.035, 0.6),
    (0.6, 0.035, 0.6),
    (0.6, 0.045, 0.3),
    (0.6, 0.045, 0.3),
    (0.7, 0.18, 0.0),
    (0.78, 0.12, 0.05),
    (0.8, 0.08, 0.05),
    (0.8, 0.02, 0.02),
    (0.7, 0.06, 0.3),
    (0.7, 0.09, 0.45),
    (0.7, 0.09, 0.45),
    (0.6, 0.07, 0.75),
    (0.6, 0.07, 0.75),
    (0.5, 0.02, 1.0),
    (0.5, 0.02, 1.0),
    (1.0, 0.001, 0.0),
];

/// zombie.characters.BodyDamage.BodyPartType.getPainModifyer
pub fn get_pain_modifyer(index: i32) -> f32 {
    BODY_PART_SIM[from_index(index)].0
}

/// zombie.characters.BodyDamage.BodyPartType.GetSkinSurface
pub fn get_skin_surface(index: usize) -> f32 {
    BODY_PART_SIM[index.min(MAX)].1
}

/// zombie.characters.BodyDamage.BodyPartType.GetDistToCore
pub fn get_dist_to_core(index: usize) -> f32 {
    BODY_PART_SIM[index.min(MAX)].2
}

/// `BodyPartType.ToIndex` of the named part (`Neck`, `Torso_Upper`, ...).
pub const NECK: usize = 9;
pub const TORSO_UPPER: usize = 6;
pub const TORSO_LOWER: usize = 7;
pub const HEAD: usize = 8;
pub const HAND_L: usize = 0;
pub const HAND_R: usize = 1;
pub const FORE_ARM_L: usize = 2;
pub const FORE_ARM_R: usize = 3;
pub const UPPER_ARM_L: usize = 4;
pub const UPPER_ARM_R: usize = 5;
