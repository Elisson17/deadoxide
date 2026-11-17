//! zombie.characters.BodyDamage.Metabolics: the metabolic equivalents (MET) of activities, in `ordinal` order.

/// zombie.characters.BodyDamage.Metabolics
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metabolics {
    Sleeping,
    SeatedResting,
    StandingAtRest,
    SedentaryActivity,
    Default,
    DrivingCar,
    LightDomestic,
    HeavyDomestic,
    DefaultExercise,
    UsingTools,
    LightWork,
    MediumWork,
    DiggingSpade,
    HeavyWork,
    ForestryAxe,
    Walking2kmh,
    Walking5kmh,
    Running10kmh,
    Running15kmh,
    JumpFence,
    ClimbRope,
    Fitness,
    FitnessHeavy,
    Max,
}

impl Metabolics {
    /// `getMet`
    pub fn get_met(self) -> f32 {
        match self {
            Metabolics::Sleeping => 0.8,
            Metabolics::SeatedResting => 1.0,
            Metabolics::StandingAtRest => 1.1,
            Metabolics::SedentaryActivity => 1.2,
            Metabolics::Default => 1.5,
            Metabolics::DrivingCar => 1.4,
            Metabolics::LightDomestic => 1.6,
            Metabolics::HeavyDomestic => 2.0,
            Metabolics::DefaultExercise => 3.0,
            Metabolics::UsingTools => 2.5,
            Metabolics::LightWork => 3.2,
            Metabolics::MediumWork => 3.9,
            Metabolics::DiggingSpade => 5.5,
            Metabolics::HeavyWork => 6.0,
            Metabolics::ForestryAxe => 8.0,
            Metabolics::Walking2kmh => 1.9,
            Metabolics::Walking5kmh => 3.1,
            Metabolics::Running10kmh => 6.9,
            Metabolics::Running15kmh => 9.5,
            Metabolics::JumpFence => 4.0,
            Metabolics::ClimbRope => 8.0,
            Metabolics::Fitness => 6.0,
            Metabolics::FitnessHeavy => 9.0,
            Metabolics::Max => 10.3,
        }
    }

    /// `getWm2`
    pub fn get_wm2(self) -> f32 {
        Self::met_to_wm2(self.get_met())
    }

    /// `getW`
    pub fn get_w(self) -> f32 {
        Self::met_to_w(self.get_met())
    }

    /// `getBtuHr`
    pub fn get_btu_hr(self) -> f32 {
        Self::met_to_btu_hr(self.get_met())
    }

    /// `MetToWm2`
    pub fn met_to_wm2(met: f32) -> f32 {
        58.0 * met
    }

    /// `MetToW`
    pub fn met_to_w(met: f32) -> f32 {
        Self::met_to_wm2(met) * 1.8
    }

    /// `MetToBtuHr`
    pub fn met_to_btu_hr(met: f32) -> f32 {
        356.0 * met
    }
}
