//! zombie.util.LocationRNG: the small seeded generator (xoroshiro128+ seeded through SplitMix64) behind
//! `OutfitRNG`, so an outfit variant's seed always dresses a character the same way.

/// `Float.intBitsToFloat(864026624)`: 2^-24.
const INT_TO_FLOAT: f32 = 5.960_464_5e-8;

/// zombie.util.LocationRNG
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocationRng {
    s0: i64,
    s1: i64,
    state: i64,
}

impl LocationRng {
    /// zombie.util.LocationRNG.setSeed: the state is the seed, then two SplitMix64 outputs are s0 and s1.
    pub fn set_seed(&mut self, seed: i64) {
        self.state = seed;
        self.s0 = self.next_split_mix64();
        self.s1 = self.next_split_mix64();
    }

    /// zombie.util.LocationRNG.getSeed: the SplitMix64 state (not the seed once `set_seed` has run).
    pub fn get_seed(&self) -> i64 {
        self.state
    }

    fn next_split_mix64(&mut self) -> i64 {
        self.state = self.state.wrapping_add(-7_046_029_254_386_353_131);
        let mut z = self.state;
        z = (z ^ ((z as u64) >> 30) as i64).wrapping_mul(-4_658_895_280_553_007_687);
        z = (z ^ ((z as u64) >> 27) as i64).wrapping_mul(-7_723_592_293_110_705_685);
        z ^ ((z as u64) >> 31) as i64
    }

    /// zombie.util.LocationRNG.nextFloat: the top 24 bits of the next int, times 2^-24.
    pub fn next_float(&mut self) -> f32 {
        ((self.next_int_raw() as u32) >> 8) as f32 * INT_TO_FLOAT
    }

    /// zombie.util.LocationRNG.nextInt(): one xoroshiro128+ step, the low 32 bits of s0 + s1.
    fn next_int_raw(&mut self) -> i32 {
        let s0 = self.s0;
        let mut s1 = self.s1;
        let result = s0.wrapping_add(s1);
        s1 ^= s0;
        self.s0 = s0.rotate_left(55) ^ s1 ^ (s1 << 14);
        self.s1 = s1.rotate_left(36);
        result as i32
    }

    /// zombie.util.LocationRNG.nextInt(int): the next int shifted right unsigned by one (an int operation, so the
    /// result is below 2^31), as a long times `n`, shifted right by 31.
    pub fn next_int(&mut self, n: i32) -> i32 {
        let mut r = i64::from((self.next_int_raw() as u32) >> 1);
        r = r.wrapping_mul(i64::from(n)) >> 31;
        r as i32
    }

    /// zombie.util.LocationRNG.nextInt(int, int, int, int): seeded from the square, then `next_int(n)`.
    pub fn next_int_at(&mut self, n: i32, x: i32, y: i32, z: i32) -> i32 {
        self.set_seed((i64::from(z) << 16) | (i64::from(y) << 32) | i64::from(x));
        self.next_int(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The constant is 2^-24, and floats stay in [0, 1).
    #[test]
    fn float_scale_and_range() {
        assert_eq!(INT_TO_FLOAT.to_bits(), 864_026_624);
        let mut r = LocationRng::default();
        r.set_seed(1234);
        for _ in 0..1000 {
            let f = r.next_float();
            assert!((0.0..1.0).contains(&f));
            let i = r.next_int(10);
            assert!((0..10).contains(&i));
        }
    }
}
