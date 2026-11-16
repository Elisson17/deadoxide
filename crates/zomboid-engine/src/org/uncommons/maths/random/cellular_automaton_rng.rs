//! org.uncommons.maths.random.CellularAutomatonRNG (Uncommons Maths, in the game jar; not decompiled, read from its
//! bytecode): the generator behind RandStandard and RandLua.
use crate::java::JavaRandom;

const AUTOMATON_LENGTH: usize = 2056;

/// RNG_RULE, as the class's static initialiser fills it.
const RNG_RULE: [i32; 511] = [
    100, 75, 16, 3, 229, 51, 197, 118, 24, 62, 198, 11, 141, 152, 241, 188, 2, 17, 71, 47, 179,
    177, 126, 231, 202, 243, 59, 25, 77, 196, 30, 134, 199, 163, 34, 216, 21, 84, 37, 182, 224,
    186, 64, 79, 225, 45, 143, 20, 48, 147, 209, 221, 125, 29, 99, 12, 46, 190, 102, 220, 80, 215,
    242, 105, 15, 53, 0, 67, 68, 69, 70, 89, 109, 195, 170, 78, 210, 131, 42, 110, 181, 145, 40,
    114, 254, 85, 107, 87, 72, 192, 90, 201, 162, 122, 86, 252, 94, 129, 98, 132, 193, 249, 156,
    172, 219, 230, 153, 54, 180, 151, 83, 214, 123, 88, 164, 167, 116, 117, 7, 27, 23, 213, 235, 5,
    65, 124, 60, 127, 236, 149, 44, 28, 58, 121, 191, 13, 250, 10, 232, 112, 101, 217, 183, 239, 8,
    32, 228, 174, 49, 113, 247, 158, 106, 218, 154, 66, 226, 157, 50, 26, 253, 93, 205, 41, 133,
    165, 61, 161, 187, 169, 6, 171, 81, 248, 56, 175, 246, 36, 178, 52, 57, 212, 39, 176, 184, 185,
    245, 63, 35, 189, 206, 76, 104, 233, 194, 19, 43, 159, 108, 55, 200, 155, 14, 74, 244, 255,
    222, 207, 208, 137, 128, 135, 96, 144, 18, 95, 234, 139, 173, 92, 1, 203, 115, 223, 130, 97,
    91, 227, 146, 4, 31, 120, 211, 38, 22, 138, 140, 237, 238, 251, 240, 160, 142, 119, 73, 103,
    166, 33, 148, 9, 111, 136, 168, 150, 82, 204, 100, 75, 16, 3, 229, 51, 197, 118, 24, 62, 198,
    11, 141, 152, 241, 188, 2, 17, 71, 47, 179, 177, 126, 231, 202, 243, 59, 25, 77, 196, 30, 134,
    199, 163, 34, 216, 21, 84, 37, 182, 224, 186, 64, 79, 225, 45, 143, 20, 48, 147, 209, 221, 125,
    29, 99, 12, 46, 190, 102, 220, 80, 215, 242, 105, 15, 53, 0, 67, 68, 69, 70, 89, 109, 195, 170,
    78, 210, 131, 42, 110, 181, 145, 40, 114, 254, 85, 107, 87, 72, 192, 90, 201, 162, 122, 86,
    252, 94, 129, 98, 132, 193, 249, 156, 172, 219, 230, 153, 54, 180, 151, 83, 214, 123, 88, 164,
    167, 116, 117, 7, 27, 23, 213, 235, 5, 65, 124, 60, 127, 236, 149, 44, 28, 58, 121, 191, 13,
    250, 10, 232, 112, 101, 217, 183, 239, 8, 32, 228, 174, 49, 113, 247, 158, 106, 218, 154, 66,
    226, 157, 50, 26, 253, 93, 205, 41, 133, 165, 61, 161, 187, 169, 6, 171, 81, 248, 56, 175, 246,
    36, 178, 52, 57, 212, 39, 176, 184, 185, 245, 63, 35, 189, 206, 76, 104, 233, 194, 19, 43, 159,
    108, 55, 200, 155, 14, 74, 244, 255, 222, 207, 208, 137, 128, 135, 96, 144, 18, 95, 234, 139,
    173, 92, 1, 203, 115, 223, 130, 97, 91, 227, 146, 4, 31, 120, 211, 38, 22, 138, 140, 237, 238,
    251, 240, 160, 142, 119, 73, 103, 166, 33, 148, 9, 111, 136, 168, 150, 82,
];

/// org.uncommons.maths.random.CellularAutomatonRNG
#[derive(Clone)]
pub struct CellularAutomatonRng {
    cells: Box<[i32; AUTOMATON_LENGTH]>,
    current_cell_index: usize,
}

/// org.uncommons.maths.binary.BinaryUtils.convertBytesToInt: big-endian.
fn convert_bytes_to_int(b: &[u8; 4]) -> i32 {
    i32::from_be_bytes(*b)
}

/// CellularAutomatonRNG.convertCellsToInt
fn convert_cells_to_int(cells: &[i32], offset: usize) -> i32 {
    cells[offset]
        .wrapping_add(cells[offset + 1] << 8)
        .wrapping_add(cells[offset + 2] << 16)
        .wrapping_add(cells[offset + 3] << 24)
}

impl CellularAutomatonRng {
    /// CellularAutomatonRNG(byte[] seed): the four seed bytes (Java bytes, so -128..127, plus 128) in the last
    /// cells, the rest from the seed's bits, then 1056784 draws.
    pub fn new(seed: [u8; 4]) -> Self {
        let mut cells = Box::new([0i32; AUTOMATON_LENGTH]);
        for (i, b) in seed.iter().enumerate() {
            cells[AUTOMATON_LENGTH - 1 - i] = i32::from(*b as i8) + 128;
        }
        let mut seed_as_int = convert_bytes_to_int(&seed);
        if seed_as_int != -1 {
            seed_as_int += 1;
        }
        for (i, c) in cells.iter_mut().take(AUTOMATON_LENGTH - 4).enumerate() {
            *c = 255 & (seed_as_int >> (i % 32));
        }
        let mut r = CellularAutomatonRng {
            cells,
            current_cell_index: AUTOMATON_LENGTH - 1,
        };
        for _ in 0..1_056_784 {
            r.next(32);
        }
        r
    }

    /// CellularAutomatonRNG.next(int bits)
    pub fn next(&mut self, bits: u32) -> i32 {
        let c = &mut self.cells;
        let cur = self.current_cell_index;
        let cell_c = cur - 1;
        let cell_b = cell_c - 1;
        let cell_a = cell_b - 1;
        c[cur] = RNG_RULE[(c[cell_c] + c[cur]) as usize];
        c[cell_c] = RNG_RULE[(c[cell_b] + c[cell_c]) as usize];
        c[cell_b] = RNG_RULE[(c[cell_a] + c[cell_b]) as usize];
        if cell_a == 0 {
            c[cell_a] = RNG_RULE[c[cell_a] as usize];
            self.current_cell_index = AUTOMATON_LENGTH - 1;
        } else {
            c[cell_a] = RNG_RULE[(c[cell_a - 1] + c[cell_a]) as usize];
            self.current_cell_index -= 4;
        }
        let result = convert_cells_to_int(&c[..], cell_a);
        ((result as u32) >> (32 - bits)) as i32
    }

    /// java.util.Random.nextInt(int bound) over this generator's `next`. `bound` must be positive.
    pub fn next_int_bound(&mut self, bound: i32) -> i32 {
        let mut r = self.next(31);
        let m = bound.wrapping_sub(1);
        if bound & m == 0 {
            ((bound as i64).wrapping_mul(r as i64) >> 31) as i32
        } else {
            let mut u = r;
            loop {
                r = u % bound;
                if u.wrapping_sub(r).wrapping_add(m) >= 0 {
                    break;
                }
                u = self.next(31);
            }
            r
        }
    }
}

impl JavaRandom for CellularAutomatonRng {
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        CellularAutomatonRng::next_int_bound(self, bound)
    }
    /// java.util.Random.nextFloat()
    fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Draws of the original class from seed {1, 2, 3, 4} (java, the game jar on the classpath):
    /// nextInt(1000000000), nextInt(Integer.MAX_VALUE), nextInt(16), nextInt(7).
    #[test]
    fn draws_match_the_original() {
        let mut r = CellularAutomatonRng::new([1, 2, 3, 4]);
        let got = [
            r.next_int_bound(1_000_000_000),
            r.next_int_bound(i32::MAX),
            r.next_int_bound(16),
            r.next_int_bound(7),
        ];
        assert_eq!(got, [263154371, 1085768463, 6, 1]);
    }
}
