//! zombie.characters.BodyDamage.Nutrition: calories, macronutrients and body weight, as a save stores them.
use crate::java_nio::ByteBuffer;
use crate::zombie::inventory::inventory_item::not_ported;

/// zombie.characters.BodyDamage.Nutrition, the saved fields.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Nutrition {
    pub calories: f32,
    pub proteins: f32,
    pub lipids: f32,
    pub carbohydrates: f32,
    pub weight: f64,
}

impl Nutrition {
    /// zombie.characters.BodyDamage.Nutrition.load through the clamping setters. A weight below 35 also damages the
    /// character (`setWeight`), which is not ported.
    pub fn load(b: &mut ByteBuffer) -> Result<Self, String> {
        let clamp = |v: f32, lo: f32, hi: f32| {
            let v = if v < lo { lo } else { v };
            if v > hi {
                hi
            } else {
                v
            }
        };
        let calories = clamp(b.get_float()?, -2200.0, 3700.0);
        let proteins = clamp(b.get_float()?, -500.0, 1000.0);
        let lipids = clamp(b.get_float()?, -500.0, 1000.0);
        let carbohydrates = clamp(b.get_float()?, -500.0, 1000.0);
        let weight = f64::from(b.get_float()?);
        if weight < 35.0 {
            return Err(not_ported("Nutrition.setWeight below 35: health damage"));
        }
        Ok(Nutrition {
            calories,
            proteins,
            lipids,
            carbohydrates,
            weight,
        })
    }
}
