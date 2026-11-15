//! zombie.core.skinnedmodel.population.OutfitRNG: the draws that dress a character, over one `LocationRNG` (the
//! original keeps one per thread; here the caller owns it).
use crate::zombie::core::color::Color;
use crate::zombie::core::immutable_color::ImmutableColor;
use crate::zombie::util::location_rng::LocationRng;

/// zombie.core.skinnedmodel.population.OutfitRNG
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutfitRng {
    rng: LocationRng,
}

impl OutfitRng {
    /// zombie.core.skinnedmodel.population.OutfitRNG.setSeed
    pub fn set_seed(&mut self, seed: i64) {
        self.rng.set_seed(seed);
    }

    /// zombie.core.skinnedmodel.population.OutfitRNG.getSeed
    pub fn get_seed(&self) -> i64 {
        self.rng.get_seed()
    }

    /// zombie.core.skinnedmodel.population.OutfitRNG.Next(int)
    pub fn next(&mut self, max: i32) -> i32 {
        self.rng.next_int(max)
    }

    /// zombie.core.skinnedmodel.population.OutfitRNG.Next(int, int): `min` without a draw when the bounds are
    /// equal; swapped bounds are put in order.
    pub fn next_range(&mut self, mut min: i32, mut max: i32) -> i32 {
        if max == min {
            return min;
        }
        if min > max {
            std::mem::swap(&mut min, &mut max);
        }
        self.rng.next_int(max.wrapping_sub(min)).wrapping_add(min)
    }

    /// zombie.core.skinnedmodel.population.OutfitRNG.Next(float, float): `min + nextFloat * (max - min)`.
    pub fn next_float(&mut self, mut min: f32, mut max: f32) -> f32 {
        if max == min {
            return min;
        }
        if min > max {
            std::mem::swap(&mut min, &mut max);
        }
        min + self.rng.next_float() * (max - min)
    }

    /// zombie.core.skinnedmodel.population.OutfitRNG.NextBool
    pub fn next_bool(&mut self, inv_probability: i32) -> bool {
        self.next(inv_probability) == 0
    }

    /// zombie.core.skinnedmodel.population.OutfitRNG.pickRandom: the index picked (`None` for an empty list); a
    /// list of one is picked without a draw.
    pub fn pick_random_index(&mut self, len: usize) -> Option<usize> {
        match len {
            0 => None,
            1 => Some(0),
            n => usize::try_from(self.next(i32::try_from(n).unwrap_or(i32::MAX))).ok(),
        }
    }

    /// zombie.core.skinnedmodel.population.OutfitRNG.pickRandom over a list of strings.
    pub fn pick_random(&mut self, list: &[String]) -> Option<String> {
        self.pick_random_index(list.len())
            .and_then(|i| list.get(i))
            .cloned()
    }

    /// zombie.core.skinnedmodel.population.OutfitRNG.randomImmutableColor(boolean): hue in [0, 1), saturation in
    /// [0, 0.6), brightness from 0.1 (0.2 under the sandbox option `NoBlackClothes` or `no_black`) to 0.9.
    pub fn random_immutable_color(
        &mut self,
        no_black_clothes: bool,
        no_black: bool,
    ) -> ImmutableColor {
        let color_hue = self.next_float(0.0, 1.0);
        let color_saturation = self.next_float(0.0, 0.6);
        let mut minimum_brightness = 0.1;
        if no_black_clothes || no_black {
            minimum_brightness = 0.2;
        }
        let color_brightness = self.next_float(minimum_brightness, 0.9);
        let new_c = Color::hsb_to_rgb(color_hue, color_saturation, color_brightness);
        ImmutableColor::from_color(Some(&new_c))
    }
}
