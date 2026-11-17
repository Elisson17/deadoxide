//! Named access to the fields of the body simulation, as the BodyProbe oracle names them (the Java field names): the
//! probe prints them with reflection and sets them from scenario options, the port does the same through these tables.
use super::body_damage::BodyDamage;
use super::body_part::BodyPart;
use super::body_sim::CharCtx;
use super::thermoregulator::{ThermalNode, Thermoregulator};

/// A reference to one field of a known type.
pub enum Field<'a> {
    F32(&'a mut f32),
    F64(&'a mut f64),
    Bool(&'a mut bool),
    I32(&'a mut i32),
    I8(&'a mut i8),
}

impl Field<'_> {
    /// The text the probe prints: floats and doubles as hex bits, booleans as 0/1, integers in decimal.
    pub fn show(&self) -> String {
        match self {
            Field::F32(v) => format!("{:x}", v.to_bits()),
            Field::F64(v) => format!("{:x}", v.to_bits()),
            Field::Bool(v) => u8::from(**v).to_string(),
            Field::I32(v) => v.to_string(),
            Field::I8(v) => v.to_string(),
        }
    }

    /// The probe's `setTyped`: the text is read as the field's type (a boolean is "1").
    pub fn assign(self, v: &str) -> Result<(), String> {
        match self {
            Field::F32(f) => *f = v.parse().map_err(|e| format!("float {v}: {e}"))?,
            Field::F64(f) => *f = v.parse().map_err(|e| format!("double {v}: {e}"))?,
            Field::Bool(f) => *f = v == "1",
            Field::I32(f) => *f = v.parse().map_err(|e| format!("int {v}: {e}"))?,
            Field::I8(f) => *f = v.parse().map_err(|e| format!("byte {v}: {e}"))?,
        }
        Ok(())
    }
}

/// `BodyPart`'s fields by Java name, in the order BodyProbe prints them.
pub const PART_FIELDS: [&str; 40] = [
    "health",
    "bandaged",
    "bitten",
    "bleeding",
    "isBleedingStemmed",
    "isCauterized",
    "scratched",
    "stitched",
    "deepWounded",
    "isInfected",
    "isFakeInfected",
    "bandageLife",
    "scratchTime",
    "biteTime",
    "alcoholicBandage",
    "stiffness",
    "woundInfectionLevel",
    "infectedWound",
    "bleedingTime",
    "deepWoundTime",
    "haveGlass",
    "stitchTime",
    "alcoholLevel",
    "additionalPain",
    "getBandageXp",
    "getStitchXp",
    "getSplintXp",
    "fractureTime",
    "splint",
    "splintFactor",
    "haveBullet",
    "burnTime",
    "needBurnWash",
    "lastTimeBurnWash",
    "plantainFactor",
    "comfreyFactor",
    "garlicFactor",
    "cutTime",
    "cut",
    "wetness",
];

pub fn part_field<'a>(p: &'a mut BodyPart, name: &str) -> Result<Field<'a>, String> {
    Ok(match name {
        "health" => Field::F32(&mut p.health),
        "bandaged" => Field::Bool(&mut p.bandaged),
        "bitten" => Field::Bool(&mut p.bitten),
        "bleeding" => Field::Bool(&mut p.bleeding),
        "isBleedingStemmed" => Field::Bool(&mut p.is_bleeding_stemmed),
        "isCauterized" => Field::Bool(&mut p.is_cauterized),
        "scratched" => Field::Bool(&mut p.scratched),
        "stitched" => Field::Bool(&mut p.stitched),
        "deepWounded" => Field::Bool(&mut p.deep_wounded),
        "isInfected" => Field::Bool(&mut p.is_infected),
        "isFakeInfected" => Field::Bool(&mut p.is_fake_infected),
        "bandageLife" => Field::F32(&mut p.bandage_life),
        "scratchTime" => Field::F32(&mut p.scratch_time),
        "biteTime" => Field::F32(&mut p.bite_time),
        "alcoholicBandage" => Field::Bool(&mut p.alcoholic_bandage),
        "stiffness" => Field::F32(&mut p.stiffness),
        "woundInfectionLevel" => Field::F32(&mut p.wound_infection_level),
        "infectedWound" => Field::Bool(&mut p.infected_wound),
        "bleedingTime" => Field::F32(&mut p.bleeding_time),
        "deepWoundTime" => Field::F32(&mut p.deep_wound_time),
        "haveGlass" => Field::Bool(&mut p.have_glass),
        "stitchTime" => Field::F32(&mut p.stitch_time),
        "alcoholLevel" => Field::F32(&mut p.alcohol_level),
        "additionalPain" => Field::F32(&mut p.additional_pain),
        "getBandageXp" => Field::Bool(&mut p.get_bandage_xp),
        "getStitchXp" => Field::Bool(&mut p.get_stitch_xp),
        "getSplintXp" => Field::Bool(&mut p.get_splint_xp),
        "fractureTime" => Field::F32(&mut p.fracture_time),
        "splint" => Field::Bool(&mut p.splint),
        "splintFactor" => Field::F32(&mut p.splint_factor),
        "haveBullet" => Field::Bool(&mut p.have_bullet),
        "burnTime" => Field::F32(&mut p.burn_time),
        "needBurnWash" => Field::Bool(&mut p.need_burn_wash),
        "lastTimeBurnWash" => Field::F32(&mut p.last_time_burn_wash),
        "plantainFactor" => Field::F32(&mut p.plantain_factor),
        "comfreyFactor" => Field::F32(&mut p.comfrey_factor),
        "garlicFactor" => Field::F32(&mut p.garlic_factor),
        "cutTime" => Field::F32(&mut p.cut_time),
        "cut" => Field::Bool(&mut p.cut),
        "wetness" => Field::F32(&mut p.wetness),
        _ => return Err(format!("BodyPart field {name}")),
    })
}

/// `BodyDamage`'s fields by Java name, in the order BodyProbe prints them.
pub const BODY_FIELDS: [&str; 20] = [
    "overallBodyHealth",
    "catchACold",
    "hasACold",
    "coldStrength",
    "timeToSneezeOrCough",
    "healthFromFoodTimer",
    "painReduction",
    "coldReduction",
    "infectionTime",
    "infectionMortalityDuration",
    "coldDamageStage",
    "isInfected",
    "isFakeInfected",
    "reduceFakeInfection",
    "oldNumZombiesVisible",
    "sneezeCoughActive",
    "sneezeCoughTime",
    "wasDraggingCorpse",
    "startedDraggingCorpse",
    "burntToDeath",
];

pub fn body_field<'a>(b: &'a mut BodyDamage, name: &str) -> Result<Field<'a>, String> {
    Ok(match name {
        "overallBodyHealth" => Field::F32(&mut b.overall_body_health),
        "catchACold" => Field::F32(&mut b.catch_a_cold),
        "hasACold" => Field::Bool(&mut b.has_a_cold),
        "coldStrength" => Field::F32(&mut b.cold_strength),
        "timeToSneezeOrCough" => Field::F32(&mut b.time_to_sneeze_or_cough),
        "healthFromFoodTimer" => Field::F32(&mut b.health_from_food_timer),
        "painReduction" => Field::F32(&mut b.pain_reduction),
        "coldReduction" => Field::F32(&mut b.cold_reduction),
        "infectionTime" => Field::F32(&mut b.infection_time),
        "infectionMortalityDuration" => Field::F32(&mut b.infection_mortality_duration),
        "coldDamageStage" => Field::F32(&mut b.cold_damage_stage),
        "isInfected" => Field::Bool(&mut b.is_infected),
        "isFakeInfected" => Field::Bool(&mut b.is_fake_infected),
        "reduceFakeInfection" => Field::Bool(&mut b.reduce_fake_infection),
        "oldNumZombiesVisible" => Field::I32(&mut b.old_num_zombies_visible),
        "sneezeCoughActive" => Field::I32(&mut b.sneeze_cough_active),
        "sneezeCoughTime" => Field::I32(&mut b.sneeze_cough_time),
        "wasDraggingCorpse" => Field::Bool(&mut b.was_dragging_corpse),
        "startedDraggingCorpse" => Field::Bool(&mut b.started_dragging_corpse),
        "burntToDeath" => Field::Bool(&mut b.burnt_to_death),
        _ => return Err(format!("BodyDamage field {name}")),
    })
}

/// The `IsoGameCharacter`/`IsoPlayer` fields BodyProbe prints, in its order.
pub const CHAR_FIELDS: [&str; 10] = [
    "bleedingLevel",
    "painEffect",
    "painDelta",
    "reduceInfectionPower",
    "timeSinceLastSmoke",
    "forceWakeUp",
    "moveSpeed",
    "health",
    "maxWeight",
    "corpseSicknessRate",
];

pub fn char_field<'a>(c: &'a mut CharCtx, name: &str) -> Result<Field<'a>, String> {
    Ok(match name {
        "bleedingLevel" => Field::I8(&mut c.bleeding_level),
        "painEffect" => Field::F32(&mut c.pain_effect),
        "painDelta" => Field::F32(&mut c.pain_delta),
        "reduceInfectionPower" => Field::F32(&mut c.reduce_infection_power),
        "timeSinceLastSmoke" => Field::F32(&mut c.time_since_last_smoke),
        "forceWakeUp" => Field::Bool(&mut c.force_wake_up),
        "moveSpeed" => Field::F32(&mut c.move_speed),
        "health" => Field::F32(&mut c.health),
        "maxWeight" => Field::I32(&mut c.max_weight),
        "corpseSicknessRate" => Field::F32(&mut c.corpse_sickness_rate),
        _ => return Err(format!("character field {name}")),
    })
}

/// The `Thermoregulator` fields BodyProbe prints, in its order.
pub const THERMO_FIELDS: [&str; 23] = [
    "setPoint",
    "metabolicRate",
    "metabolicRateReal",
    "metabolicTarget",
    "bodyHeatDelta",
    "coreHeatDelta",
    "thermalDamage",
    "damageCounter",
    "thermalDuration",
    "totalHeatRaw",
    "totalHeat",
    "primTotal",
    "secTotal",
    "externalAirTemperature",
    "airTemperature",
    "airAndWindTemp",
    "rateOfChangeCounter",
    "coreCelciusCache",
    "coreRateOfChange",
    "fluidsMultiplier",
    "energyMultiplier",
    "fatigueMultiplier",
    "thermalChevronUp",
];

pub fn thermo_field<'a>(t: &'a mut Thermoregulator, name: &str) -> Result<Field<'a>, String> {
    Ok(match name {
        "setPoint" => Field::F32(&mut t.set_point),
        "metabolicRate" => Field::F32(&mut t.metabolic_rate),
        "metabolicRateReal" => Field::F32(&mut t.metabolic_rate_real),
        "metabolicTarget" => Field::F32(&mut t.metabolic_target),
        "bodyHeatDelta" => Field::F32(&mut t.body_heat_delta),
        "coreHeatDelta" => Field::F32(&mut t.core_heat_delta),
        "thermalDamage" => Field::F32(&mut t.thermal_damage),
        "damageCounter" => Field::F32(&mut t.damage_counter),
        "thermalDuration" => Field::F32(&mut t.thermal_duration),
        "totalHeatRaw" => Field::F32(&mut t.total_heat_raw),
        "totalHeat" => Field::F32(&mut t.total_heat),
        "primTotal" => Field::F32(&mut t.prim_total),
        "secTotal" => Field::F32(&mut t.sec_total),
        "externalAirTemperature" => Field::F32(&mut t.external_air_temperature),
        "airTemperature" => Field::F32(&mut t.air_temperature),
        "airAndWindTemp" => Field::F32(&mut t.air_and_wind_temp),
        "rateOfChangeCounter" => Field::F32(&mut t.rate_of_change_counter),
        "coreCelciusCache" => Field::F32(&mut t.core_celcius_cache),
        "coreRateOfChange" => Field::F32(&mut t.core_rate_of_change),
        "fluidsMultiplier" => Field::F64(&mut t.fluids_multiplier),
        "energyMultiplier" => Field::F64(&mut t.energy_multiplier),
        "fatigueMultiplier" => Field::F64(&mut t.fatigue_multiplier),
        "thermalChevronUp" => Field::Bool(&mut t.thermal_chevron_up),
        _ => return Err(format!("Thermoregulator field {name}")),
    })
}

/// The `ThermalNode` fields BodyProbe prints, in its order.
pub const NODE_FIELDS: [&str; 9] = [
    "celcius",
    "skinCelcius",
    "heatDelta",
    "primaryDelta",
    "secondaryDelta",
    "insulation",
    "windresist",
    "bodyWetness",
    "clothingWetness",
];

pub fn node_field<'a>(n: &'a mut ThermalNode, name: &str) -> Result<Field<'a>, String> {
    Ok(match name {
        "celcius" => Field::F32(&mut n.celcius),
        "skinCelcius" => Field::F32(&mut n.skin_celcius),
        "heatDelta" => Field::F32(&mut n.heat_delta),
        "primaryDelta" => Field::F32(&mut n.primary_delta),
        "secondaryDelta" => Field::F32(&mut n.secondary_delta),
        "insulation" => Field::F32(&mut n.insulation),
        "windresist" => Field::F32(&mut n.windresist),
        "bodyWetness" => Field::F32(&mut n.body_wetness),
        "clothingWetness" => Field::F32(&mut n.clothing_wetness),
        _ => return Err(format!("ThermalNode field {name}")),
    })
}
