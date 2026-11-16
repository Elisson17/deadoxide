//! zombie.iso.weather.SimplexNoise: Stefan Gustavson's simplex noise; the climate uses the 2D form.

/// zombie.iso.weather.SimplexNoise.p: Ken Perlin's reference permutation of 0..=255 (zombie.erosion.utils.Noise2D
/// holds the same 256 values).
pub const P: [u8; 256] = [
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225,
    140, 36, 103, 30, 69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148,
    247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32,
    57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175,
    74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122,
    60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54,
    65, 25, 63, 161, 1, 216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169,
    200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173, 186, 3, 64,
    52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212,
    207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213,
    119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9,
    129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104,
    218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162, 241,
    81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157,
    184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93,
    222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
];

/// zombie.iso.weather.SimplexNoise.grad3 (x, y; the z of each gradient is unused in 2D).
const GRAD3: [(f64, f64); 12] = [
    (1.0, 1.0),
    (-1.0, 1.0),
    (1.0, -1.0),
    (-1.0, -1.0),
    (1.0, 0.0),
    (-1.0, 0.0),
    (1.0, 0.0),
    (-1.0, 0.0),
    (0.0, 1.0),
    (0.0, -1.0),
    (0.0, 1.0),
    (0.0, -1.0),
];

/// zombie.iso.weather.SimplexNoise.perm[i] = p[i & 0xFF].
fn perm(i: i32) -> i32 {
    i32::from(P[(i & 0xFF) as usize])
}

/// zombie.iso.weather.SimplexNoise.permMod12[i] = perm[i] % 12.
fn perm_mod12(i: i32) -> usize {
    (perm(i) % 12) as usize
}

/// zombie.iso.weather.SimplexNoise.fastfloor(double)
fn fastfloor(x: f64) -> i32 {
    let xi = x as i32;
    if x < f64::from(xi) {
        xi - 1
    } else {
        xi
    }
}

/// zombie.iso.weather.SimplexNoise.noise(double, double): 2D simplex noise in about [-1, 1]. F2 and G2 come from
/// `Math.sqrt(3.0)` as in the class initializer.
pub fn noise(xin: f64, yin: f64) -> f64 {
    let sqrt3 = 3.0f64.sqrt();
    let f2 = 0.5 * (sqrt3 - 1.0);
    let g2 = (3.0 - sqrt3) / 6.0;
    let s = (xin + yin) * f2;
    let i = fastfloor(xin + s);
    let j = fastfloor(yin + s);
    let t = f64::from(i.wrapping_add(j)) * g2;
    let origin_x0 = f64::from(i) - t;
    let origin_y0 = f64::from(j) - t;
    let x0 = xin - origin_x0;
    let y0 = yin - origin_y0;
    let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };
    let x1 = x0 - f64::from(i1) + g2;
    let y1 = y0 - f64::from(j1) + g2;
    let x2 = x0 - 1.0 + 2.0 * g2;
    let y2 = y0 - 1.0 + 2.0 * g2;
    let ii = i & 0xFF;
    let jj = j & 0xFF;
    let gi0 = perm_mod12(ii + perm(jj));
    let gi1 = perm_mod12(ii + i1 + perm(jj + j1));
    let gi2 = perm_mod12(ii + 1 + perm(jj + 1));
    let dot = |g: (f64, f64), x: f64, y: f64| g.0 * x + g.1 * y;
    let mut t0 = 0.5 - x0 * x0 - y0 * y0;
    let n0 = if t0 < 0.0 {
        0.0
    } else {
        t0 *= t0;
        t0 * t0 * dot(GRAD3[gi0], x0, y0)
    };
    let mut t1 = 0.5 - x1 * x1 - y1 * y1;
    let n1 = if t1 < 0.0 {
        0.0
    } else {
        t1 *= t1;
        t1 * t1 * dot(GRAD3[gi1], x1, y1)
    };
    let mut t2 = 0.5 - x2 * x2 - y2 * y2;
    let n2 = if t2 < 0.0 {
        0.0
    } else {
        t2 *= t2;
        t2 * t2 * dot(GRAD3[gi2], x2, y2)
    };
    70.0 * (n0 + n1 + n2)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table is a permutation of 0..=255; the noise is 0 on every simplex corner (integer lattice points of
    /// the skewed grid, here the origin).
    #[test]
    fn permutation_and_lattice_zero() {
        let mut seen = [false; 256];
        for &v in P.iter() {
            seen[usize::from(v)] = true;
        }
        assert!(seen.iter().all(|s| *s));
        assert_eq!(noise(0.0, 0.0), 0.0);
    }
}
