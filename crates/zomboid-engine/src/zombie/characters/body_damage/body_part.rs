//! zombie.characters.BodyDamage.BodyPart: the wound, bandage and infection state of one body part, with the setters
//! `BodyDamage.load` calls in their original order and side effects. Setters that draw from the global `Rand` (a new
//! cut, scratch or bite, removing stitches or a bullet) return `not ported:`; the saved values overwrite what they
//! would draw, but the draws themselves cannot be reproduced.
use super::body_part_type;
use crate::java;
use crate::zombie::core::math::pz_math;
use crate::zombie::inventory::inventory_item::not_ported;

/// The sandbox options the setters read (`SandboxOptions.instance`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodySandbox {
    /// ZombieLore.Transmission
    pub transmission: i32,
    /// ZombieLore.Mortality
    pub mortality: i32,
    /// InjurySeverity
    pub injury_severity: i32,
}

/// zombie.characters.BodyDamage.BodyPart, the fields `load` reads or its setters change.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyPart {
    /// `BodyPartType` ordinal.
    pub index: i32,
    pub health: f32,
    pub bandaged: bool,
    pub bitten: bool,
    pub bleeding: bool,
    pub is_bleeding_stemmed: bool,
    pub is_cauterized: bool,
    pub scratched: bool,
    pub stitched: bool,
    pub deep_wounded: bool,
    pub is_infected: bool,
    pub is_fake_infected: bool,
    pub bandage_life: f32,
    pub scratch_time: f32,
    pub bite_time: f32,
    pub alcoholic_bandage: bool,
    pub stiffness: f32,
    pub wound_infection_level: f32,
    pub infected_wound: bool,
    pub bleeding_time: f32,
    pub deep_wound_time: f32,
    pub have_glass: bool,
    pub stitch_time: f32,
    pub alcohol_level: f32,
    pub additional_pain: f32,
    pub bandage_type: Option<String>,
    pub get_bandage_xp: bool,
    pub get_stitch_xp: bool,
    pub get_splint_xp: bool,
    pub fracture_time: f32,
    pub splint: bool,
    pub splint_factor: f32,
    pub have_bullet: bool,
    pub burn_time: f32,
    pub need_burn_wash: bool,
    pub last_time_burn_wash: f32,
    pub splint_item: Option<String>,
    pub plantain_factor: f32,
    pub comfrey_factor: f32,
    pub garlic_factor: f32,
    pub cut_time: f32,
    pub cut: bool,
    pub wetness: f32,
}

type R = Result<(), String>;

impl BodyPart {
    /// zombie.characters.BodyDamage.BodyPart(BodyPartType, IsoGameCharacter): the field initialisers.
    pub fn new(index: i32) -> Self {
        BodyPart {
            index,
            health: 0.0,
            bandaged: false,
            bitten: false,
            bleeding: false,
            is_bleeding_stemmed: false,
            is_cauterized: false,
            scratched: false,
            stitched: false,
            deep_wounded: false,
            is_infected: false,
            is_fake_infected: false,
            bandage_life: 0.0,
            scratch_time: 0.0,
            bite_time: 0.0,
            alcoholic_bandage: false,
            stiffness: 0.0,
            wound_infection_level: 0.0,
            infected_wound: false,
            bleeding_time: 0.0,
            deep_wound_time: 0.0,
            have_glass: false,
            stitch_time: 0.0,
            alcohol_level: 0.0,
            additional_pain: 0.0,
            bandage_type: None,
            get_bandage_xp: true,
            get_stitch_xp: true,
            get_splint_xp: true,
            fracture_time: 0.0,
            splint: false,
            splint_factor: 0.0,
            have_bullet: false,
            burn_time: 0.0,
            need_burn_wash: false,
            last_time_burn_wash: 0.0,
            splint_item: None,
            plantain_factor: 0.0,
            comfrey_factor: 0.0,
            garlic_factor: 0.0,
            cut_time: 0.0,
            cut: false,
            wetness: 0.0,
        }
    }

    /// zombie.characters.BodyDamage.BodyPart.setCut (boolean, boolean)
    pub fn set_cut(&mut self, cut: bool) -> R {
        self.cut = cut;
        if !cut {
            self.set_bleeding(false);
            Ok(())
        } else {
            Err(not_ported("BodyPart.setCut(true): Rand cut time"))
        }
    }

    /// zombie.characters.BodyDamage.BodyPart.SetBitten
    pub fn set_bitten(&mut self, bitten: bool, sb: &BodySandbox) -> R {
        self.bitten = bitten;
        if bitten {
            return Err(not_ported("BodyPart.SetBitten(true): Rand bite time"));
        }
        if sb.transmission != 4 {
            self.is_infected = true;
            self.is_fake_infected = false;
        }
        if self.is_infected && sb.mortality == 7 {
            self.is_infected = false;
            self.is_fake_infected = true;
        }
        self.generate_bleeding(sb)
    }

    /// zombie.characters.BodyDamage.BodyPart.setScratched (boolean, boolean)
    pub fn set_scratched(&mut self, scratched: bool) -> R {
        self.scratched = scratched;
        if !scratched {
            self.set_bleeding(false);
            Ok(())
        } else {
            Err(not_ported("BodyPart.setScratched(true): Rand scratch time"))
        }
    }

    /// zombie.characters.BodyDamage.BodyPart.setBandaged (boolean, float) = (bandaged, life, false, null)
    pub fn set_bandaged(&mut self, bandaged: bool, bandage_life: f32) {
        if bandaged {
            if self.bleeding {
                self.bleeding = false;
            }
            self.bitten = false;
            self.scratched = false;
            self.cut = false;
            self.alcoholic_bandage = false;
            self.stitched = false;
            self.deep_wounded = false;
            self.bandage_type = None;
            self.get_bandage_xp = false;
        } else {
            if self.scratch_time > 0.0 {
                self.scratched = true;
            }
            if self.cut_time > 0.0 {
                self.cut = true;
            }
            if self.bleeding_time > 0.0 {
                self.bleeding = true;
            }
            if self.bite_time > 0.0 {
                self.bitten = true;
            }
            if self.stitch_time > 0.0 {
                self.stitched = true;
            }
            if self.deep_wound_time > 0.0 {
                self.deep_wounded = true;
            }
            self.bandage_type = None;
            // resetPoulticeFactors
            self.set_comfrey_factor(0.0);
            self.set_garlic_factor(0.0);
            self.set_plantain_factor(0.0);
        }
        self.set_bandage_life(bandage_life);
        self.bandaged = bandaged;
    }

    /// zombie.characters.BodyDamage.BodyPart.setBleeding
    pub fn set_bleeding(&mut self, bleeding: bool) {
        self.bleeding = bleeding;
    }

    /// zombie.characters.BodyDamage.BodyPart.setDeepWounded
    pub fn set_deep_wounded(&mut self, wounded: bool) {
        self.deep_wounded = wounded;
        if wounded {
            self.bleeding = true;
            self.is_bleeding_stemmed = false;
            self.is_cauterized = false;
            self.bandaged = false;
            self.stitched = false;
        }
    }

    /// zombie.characters.BodyDamage.BodyPart.setBandageLife
    pub fn set_bandage_life(&mut self, bandage_life: f32) {
        self.bandage_life = bandage_life;
        if self.bandage_life <= 0.0 {
            self.alcoholic_bandage = false;
        }
    }

    /// zombie.characters.BodyDamage.BodyPart.setWoundInfectionLevel
    pub fn set_wound_infection_level(&mut self, level: f32) {
        self.wound_infection_level = level;
        if self.wound_infection_level <= 0.0 {
            self.infected_wound = false;
            if self.wound_infection_level < -2.0 {
                self.wound_infection_level = -2.0;
            }
        } else {
            self.infected_wound = true;
        }
    }

    /// zombie.characters.BodyDamage.BodyPart.setCutTime: at most 100.
    pub fn set_cut_time(&mut self, cut_time: f32) {
        self.cut_time = java::math_min_f32(100.0, cut_time);
    }

    /// zombie.characters.BodyDamage.BodyPart.setScratchTime: at most 100.
    pub fn set_scratch_time(&mut self, scratch_time: f32) {
        self.scratch_time = java::math_min_f32(100.0, scratch_time);
    }

    /// zombie.characters.BodyDamage.BodyPart.setBleedingTime: an unbandaged part bleeds while the time is positive.
    pub fn set_bleeding_time(&mut self, bleeding_time: f32) {
        self.bleeding_time = bleeding_time;
        if !self.bandaged {
            self.set_bleeding(bleeding_time > 0.0);
        }
    }

    /// zombie.characters.BodyDamage.BodyPart.setStitched
    pub fn set_stitched(&mut self, stitched: bool) -> R {
        if stitched {
            self.set_bleeding_time(0.0);
            self.set_bleeding(false);
            self.deep_wound_time = 0.0;
            self.set_deep_wounded(false);
            self.get_stitch_xp = false;
        } else if self.stitched {
            return Err(not_ported(
                "BodyPart.setStitched(false) of a stitched part: Rand times",
            ));
        }
        self.stitched = stitched;
        Ok(())
    }

    /// zombie.characters.BodyDamage.BodyPart.setSplint
    pub fn set_splint(&mut self, splint: bool, splint_factor: f32) {
        self.splint = splint;
        self.splint_factor = splint_factor;
        if splint {
            self.get_splint_xp = false;
        }
    }

    /// zombie.characters.BodyDamage.BodyPart.setHaveBullet (with doctor level 0)
    pub fn set_have_bullet(&mut self, have_bullet: bool, sb: &BodySandbox) -> R {
        if self.have_bullet && !have_bullet {
            return Err(not_ported(
                "BodyPart.setHaveBullet(false) of a part with a bullet: Rand time",
            ));
        } else if have_bullet {
            self.have_bullet = true;
            self.generate_bleeding(sb)?;
        }
        self.have_bullet = have_bullet;
        Ok(())
    }

    /// zombie.characters.BodyDamage.BodyPart.setNeedBurnWash
    pub fn set_need_burn_wash(&mut self, need_burn_wash: bool) {
        if self.need_burn_wash && !need_burn_wash {
            self.last_time_burn_wash = self.burn_time;
        }
        self.need_burn_wash = need_burn_wash;
    }

    /// zombie.characters.BodyDamage.BodyPart.setWetness: clamped to [0, 100].
    pub fn set_wetness(&mut self, wetness: f32) {
        self.wetness = pz_math::clamp(wetness, 0.0, 100.0);
    }

    /// zombie.characters.BodyDamage.BodyPart.setStiffness: clamped to [0, 100].
    pub fn set_stiffness(&mut self, stiffness: f32) {
        self.stiffness = pz_math::clamp(stiffness, 0.0, 100.0);
    }

    /// zombie.characters.BodyDamage.BodyPart.setComfreyFactor: clamped to [0, 100].
    pub fn set_comfrey_factor(&mut self, f: f32) {
        self.comfrey_factor = pz_math::clamp(f, 0.0, 100.0);
    }

    /// zombie.characters.BodyDamage.BodyPart.setGarlicFactor: clamped to [0, 100].
    pub fn set_garlic_factor(&mut self, f: f32) {
        self.garlic_factor = pz_math::clamp(f, 0.0, 100.0);
    }

    /// zombie.characters.BodyDamage.BodyPart.setPlantainFactor: clamped to [0, 100].
    pub fn set_plantain_factor(&mut self, f: f32) {
        self.plantain_factor = pz_math::clamp(f, 0.0, 100.0);
    }

    /// zombie.characters.BodyDamage.BodyPart.generateBleeding: every wound adds a random share of its time; with no
    /// wound the time is 0 and no draw happens.
    pub fn generate_bleeding(&mut self, sb: &BodySandbox) -> R {
        if self.scratched
            || self.cut
            || self.burn_time > 0.0
            || self.deep_wounded
            || self.have_glass
            || self.have_bullet
            || self.bitten
        {
            return Err(not_ported(
                "BodyPart.generateBleeding of a wounded part: Rand times",
            ));
        }
        let mut bleeding_time = 0.0f32;
        match sb.injury_severity {
            1 => bleeding_time *= 0.5,
            3 => bleeding_time *= 1.5,
            _ => {}
        }
        bleeding_time *= body_part_type::get_bleeding_time_modifyer(self.index);
        self.set_bleeding_time(bleeding_time);
        Ok(())
    }
}
