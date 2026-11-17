//! zombie.iso.weather.Temperature: the body temperature constants and the conversions the body simulation reads.
use crate::java;

pub const SKIN_CELCIUS_MIN: f32 = 20.0;
pub const SKIN_CELCIUS_FAVORABLE: f32 = 33.0;
pub const SKIN_CELCIUS_MAX: f32 = 42.0;
pub const HOMEOSTASIS_DEFAULT: f32 = 37.0;
pub const FAVORABLE_NAKED_TEMP: f32 = 27.0;
pub const FAVORABLE_ROOM_TEMP: f32 = 22.0;
pub const CORE_CELCIUS_MIN: f32 = 20.0;
pub const CORE_CELCIUS_MAX: f32 = 42.0;
pub const NEUTRAL_ZONE: f32 = 27.0;
pub const HYPOTHERMIA_1: f32 = 36.5;
pub const HYPOTHERMIA_2: f32 = 35.0;
pub const HYPOTHERMIA_3: f32 = 30.0;
pub const HYPOTHERMIA_4: f32 = 25.0;
pub const HYPERTHERMIA_1: f32 = 37.5;
pub const HYPERTHERMIA_2: f32 = 39.0;
pub const HYPERTHERMIA_3: f32 = 40.0;
pub const HYPERTHERMIA_4: f32 = 41.0;
pub const TRUE_INSULATION_MULTIPLIER: f32 = 2.0;
pub const TRUE_WINDRESIST_MULTIPLIER: f32 = 1.0;
pub const BODY_MIN_TEMP: f32 = 20.0;
pub const BODY_MAX_TEMP: f32 = 42.0;

/// zombie.iso.weather.Temperature.WindchillCelsiusKph: `Math.pow(v, 0.16F)` is the double pow of the widened floats.
pub fn windchill_celsius_kph(t: f32, v: f32) -> f32 {
    let p = java::math_pow(f64::from(v), f64::from(0.16f32)) as f32;
    let w = 13.12f32 + 0.6215f32 * t - 11.37f32 * p + 0.3965f32 * t * p;
    if w < t {
        w
    } else {
        t
    }
}

/// zombie.iso.weather.Temperature.getTrueInsulationValue
pub fn get_true_insulation_value(insulation: f32) -> f32 {
    insulation * 2.0 + 0.5 * insulation * insulation * insulation
}

/// zombie.iso.weather.Temperature.getTrueWindresistanceValue
pub fn get_true_windresistance_value(windresist: f32) -> f32 {
    windresist * 1.0 + 0.5 * windresist * windresist
}

/// zombie.iso.weather.Temperature.getWindChillAmountForPlayer: what the wind takes off the air temperature of a
/// player outside any room and vehicle. `air_temperature` is `getAirTemperatureForCharacter(player, true)`.
pub fn get_wind_chill_amount_for_player(
    in_vehicle_or_room: bool,
    air_temperature: f32,
    climate_temperature: f32,
) -> f32 {
    if in_vehicle_or_room {
        return 0.0;
    }
    let mut wind_chill_amount = 0.0;
    if air_temperature < climate_temperature {
        wind_chill_amount = climate_temperature - air_temperature;
    }
    wind_chill_amount
}
