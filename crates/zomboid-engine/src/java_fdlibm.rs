//! java.lang.FdLibm and Math.scalb of the game's JRE 25.0.5: the StrictMath functions, which are also what
//! Math.tan/asin/acos/atan/atan2/exp/log/log10/pow/sinh/cosh/tanh return on that JRE (measured on 400000 inputs
//! each; Math.sin and Math.cos are HotSpot intrinsics there and differ, see `sin`). Ported from the JRE's own
//! class, a Java translation of fdlibm 5.3:
//!
//! Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//! Developed at SunSoft, a Sun Microsystems, Inc. business.
//! Permission to use, copy, modify, and distribute this software is freely granted, provided that this notice
//! is preserved.
//!
//! Integer arithmetic wraps and shift counts are masked, as in Java. Constants the class file holds as values a
//! float can represent exactly are written as `f32` literals widened to `f64`, like the decompiled source.

// `x - x` and `(x - x) / (x - x)` are fdlibm's way to return NaN (or a signed zero) with the IEEE result of the
// operation; the constants are fdlibm's own digits, not std's.
#![allow(
    clippy::excessive_precision,
    clippy::unreadable_literal,
    clippy::eq_op,
    clippy::approx_constant
)]

use std::f64::consts::PI;

const fn f(x: f32) -> f64 {
    x as f64
}

const TWO24: f64 = 16777216.0;
const TWON24: f64 = f(5.9604645E-8);
const TWO53: f64 = f(9.007199E15);
const TWO54: f64 = f(1.8014399E16);
const HUGE: f64 = 1.0E300;

/// FdLibm.__HI(double)
fn hi(x: f64) -> i32 {
    (x.to_bits() >> 32) as i32
}

/// FdLibm.__LO(double)
fn lo(x: f64) -> i32 {
    x.to_bits() as i32
}

/// FdLibm.__HI(double, int)
fn with_hi(x: f64, h: i32) -> f64 {
    f64::from_bits((x.to_bits() & 0xFFFF_FFFF) | (u64::from(h as u32) << 32))
}

/// FdLibm.__LO(double, int)
fn with_lo(x: f64, l: i32) -> f64 {
    f64::from_bits((x.to_bits() & 0xFFFF_FFFF_0000_0000) | u64::from(l as u32))
}

/// FdLibm.__HI_LO(int, int)
fn hi_lo(h: i32, l: i32) -> f64 {
    f64::from_bits((u64::from(h as u32) << 32) | u64::from(l as u32))
}

/// Math.primPowerOfTwoD
fn power_of_two(n: i32) -> f64 {
    f64::from_bits((((n + 1023) as i64) << 52) as u64)
}

/// java.lang.Math.scalb(double, int)
pub fn scalb(d: f64, n: i32) -> f64 {
    const UP: f64 = 8.98846567431158E307;
    const DOWN: f64 = 1.1125369292536007E-308;
    let min_value = f64::from_bits(1);
    if n > -1023 {
        if n <= 1023 {
            d * power_of_two(n)
        } else if n <= 2046 {
            d * power_of_two(n - 1023) * UP
        } else if n < 2098 {
            d * power_of_two(n - 2046) * UP * UP
        } else {
            d * UP * UP * UP
        }
    } else if n > -2046 {
        d * power_of_two(n + 1023) * DOWN
    } else if n > -2099 {
        d * power_of_two(n + 2046) * DOWN * DOWN
    } else {
        d * min_value * min_value
    }
}

/// java.lang.StrictMath.sqrt (FdLibm.Sqrt computes the correctly rounded root bit by bit, which is IEEE `sqrt`).
pub fn sqrt(x: f64) -> f64 {
    x.sqrt()
}

const PS0: f64 = 0.16666666666666666;
const PS1: f64 = -0.3255658186224009;
const PS2: f64 = 0.20121253213486293;
const PS3: f64 = -0.04005553450067941;
const PS4: f64 = 7.915349942898145E-4;
const PS5: f64 = 3.479331075960212E-5;
const QS1: f64 = -2.403394911734414;
const QS2: f64 = 2.0209457602335057;
const QS3: f64 = -0.6882839716054533;
const QS4: f64 = 0.07703815055590194;
const PIO2_HI: f64 = PI / 2.0;
const PIO2_LO: f64 = 6.123233995736766E-17;
const PIO4_HI: f64 = PI / 4.0;

fn asin_p(z: f64) -> f64 {
    z * (PS0 + z * (PS1 + z * (PS2 + z * (PS3 + z * (PS4 + z * PS5)))))
}

fn asin_q(z: f64) -> f64 {
    1.0 + z * (QS1 + z * (QS2 + z * (QS3 + z * QS4)))
}

/// java.lang.FdLibm.Acos.compute
pub fn acos(x: f64) -> f64 {
    let hx = hi(x);
    let ix = hx & 0x7fffffff;
    if ix >= 0x3ff00000 {
        if (ix.wrapping_sub(0x3ff00000) | lo(x)) == 0 {
            return if hx > 0 { 0.0 } else { PI };
        }
        return (x - x) / (x - x);
    }
    if ix < 0x3fe00000 {
        if ix <= 0x3c600000 {
            return PIO2_HI;
        }
        let z = x * x;
        let p = asin_p(z);
        let q = asin_q(z);
        let r = p / q;
        return PIO2_HI - (x - (PIO2_LO - x * r));
    }
    if hx < 0 {
        let z = (1.0 + x) * 0.5;
        let p = asin_p(z);
        let q = asin_q(z);
        let s = z.sqrt();
        let r = p / q;
        let w = r * s - PIO2_LO;
        return PI - 2.0 * (s + w);
    }
    let z = (1.0 - x) * 0.5;
    let s = z.sqrt();
    let df = with_lo(s, 0);
    let c = (z - df * df) / (s + df);
    let p = asin_p(z);
    let q = asin_q(z);
    let r = p / q;
    let w = r * s + c;
    2.0 * (df + w)
}

/// java.lang.FdLibm.Asin.compute
pub fn asin(x: f64) -> f64 {
    let mut t = 0.0;
    let hx = hi(x);
    let ix = hx & 0x7fffffff;
    if ix >= 0x3ff00000 {
        return if (ix.wrapping_sub(0x3ff00000) | lo(x)) == 0 {
            x * PIO2_HI + x * PIO2_LO
        } else {
            (x - x) / (x - x)
        };
    }
    if ix < 0x3fe00000 {
        if ix < 0x3e400000 {
            if HUGE + x > 1.0 {
                return x;
            }
        } else {
            t = x * x;
        }
        let p = asin_p(t);
        let q = asin_q(t);
        let w = p / q;
        return x + x * w;
    }
    let w = 1.0 - x.abs();
    t = w * 0.5;
    let mut p = asin_p(t);
    let mut q = asin_q(t);
    let s = t.sqrt();
    if ix >= 0x3FEF3333 {
        let w = p / q;
        t = PIO2_HI - (2.0 * (s + s * w) - PIO2_LO);
    } else {
        let w = with_lo(s, 0);
        let c = (t - w * w) / (s + w);
        let r = p / q;
        p = 2.0 * s * r - (PIO2_LO - 2.0 * c);
        q = PIO4_HI - 2.0 * w;
        t = PIO4_HI - (p - q);
    }
    if hx > 0 {
        t
    } else {
        -t
    }
}

const ATANHI: [f64; 4] = [0.4636476090008061, PI / 4.0, 0.982793723247329, PI / 2.0];
const ATANLO: [f64; 4] = [
    2.2698777452961687E-17,
    3.061616997868383E-17,
    1.3903311031230998E-17,
    6.123233995736766E-17,
];
const AT: [f64; 11] = [
    0.3333333333333293,
    -0.19999999999876483,
    0.14285714272503466,
    -0.11111110405462356,
    0.09090887133436507,
    -0.0769187620504483,
    0.06661073137387531,
    -0.058335701337905735,
    0.049768779946159324,
    -0.036531572744216916,
    0.016285820115365782,
];

/// java.lang.FdLibm.Atan.compute
pub fn atan(x: f64) -> f64 {
    let mut x = x;
    let hx = hi(x);
    let ix = hx & 0x7fffffff;
    if ix >= 0x44100000 {
        if ix > 0x7ff00000 || (ix == 0x7ff00000 && lo(x) != 0) {
            return x + x;
        }
        return if hx > 0 {
            ATANHI[3] + ATANLO[3]
        } else {
            -ATANHI[3] - ATANLO[3]
        };
    }
    let id: i32;
    if ix < 0x3fdc0000 {
        if ix < 0x3e200000 && HUGE + x > 1.0 {
            return x;
        }
        id = -1;
    } else {
        x = x.abs();
        if ix < 0x3ff30000 {
            if ix < 0x3fe60000 {
                id = 0;
                x = (2.0 * x - 1.0) / (2.0 + x);
            } else {
                id = 1;
                x = (x - 1.0) / (x + 1.0);
            }
        } else if ix < 0x40038000 {
            id = 2;
            x = (x - 1.5) / (1.0 + 1.5 * x);
        } else {
            id = 3;
            x = -1.0 / x;
        }
    }
    let z = x * x;
    let w = z * z;
    let s1 = z * (AT[0] + w * (AT[2] + w * (AT[4] + w * (AT[6] + w * (AT[8] + w * AT[10])))));
    let s2 = w * (AT[1] + w * (AT[3] + w * (AT[5] + w * (AT[7] + w * AT[9]))));
    if id < 0 {
        return x - x * (s1 + s2);
    }
    let id = id as usize;
    let z = ATANHI[id] - ((x * (s1 + s2) - ATANLO[id]) - x);
    if hx < 0 {
        -z
    } else {
        z
    }
}

/// java.lang.FdLibm.Atan2.compute(y, x)
pub fn atan2(y: f64, x: f64) -> f64 {
    const PI_LO: f64 = 1.2246467991473532E-16;
    if x.is_nan() || y.is_nan() {
        return x + y;
    }
    let hx = hi(x);
    let ix = hx & 0x7fffffff;
    let lx = lo(x);
    let hy = hi(y);
    let iy = hy & 0x7fffffff;
    let ly = lo(y);
    if (hx.wrapping_sub(0x3ff00000) | lx) == 0 {
        return atan(y);
    }
    let m = ((hy >> 31) & 1) | ((hx >> 30) & 2);
    if (iy | ly) == 0 {
        match m {
            0 | 1 => return y,
            2 => return PI,
            _ => return -PI,
        }
    }
    if (ix | lx) == 0 {
        return if hy < 0 { -PI / 2.0 } else { PI / 2.0 };
    }
    if ix == 0x7ff00000 {
        if iy == 0x7ff00000 {
            return match m {
                0 => PI / 4.0,
                1 => -PI / 4.0,
                2 => PI * 3.0 / 4.0,
                _ => -PI * 3.0 / 4.0,
            };
        }
        return match m {
            0 => 0.0,
            1 => -0.0,
            2 => PI,
            _ => -PI,
        };
    }
    if iy == 0x7ff00000 {
        return if hy < 0 { -PI / 2.0 } else { PI / 2.0 };
    }
    let k = iy.wrapping_sub(ix) >> 20;
    let z = if k > 60 {
        PI / 2.0
    } else if hx < 0 && k < -60 {
        0.0
    } else {
        atan((y / x).abs())
    };
    match m {
        0 => z,
        1 => -z,
        2 => PI - (z - PI_LO),
        _ => (z - PI_LO) - PI,
    }
}

const C1: f64 = 0.0416666666666666;
const C2: f64 = -0.001388888888887411;
const C3: f64 = 2.480158728947673E-5;
const C4: f64 = -2.7557314351390663E-7;
const C5: f64 = 2.087572321298175E-9;
const C6: f64 = -1.1359647557788195E-11;

/// java.lang.FdLibm.Cos.__kernel_cos
fn kernel_cos(x: f64, y: f64) -> f64 {
    let ix = hi(x) & 0x7fffffff;
    if ix < 0x3e400000 && (x as i32) == 0 {
        return 1.0;
    }
    let z = x * x;
    let r = z * (C1 + z * (C2 + z * (C3 + z * (C4 + z * (C5 + z * C6)))));
    if ix < 0x3FD33333 {
        return 1.0 - (0.5 * z - (z * r - x * y));
    }
    let qx = if ix > 0x3fe90000 {
        0.28125
    } else {
        hi_lo(ix - 0x00200000, 0)
    };
    let hz = 0.5 * z - qx;
    let a = 1.0 - qx;
    a - (hz - (z * r - x * y))
}

const S1: f64 = -0.16666666666666632;
const S2: f64 = 0.00833333333332249;
const S3: f64 = -1.984126982985795E-4;
const S4: f64 = 2.7557313707070068E-6;
const S5: f64 = -2.5050760253406863E-8;
const S6: f64 = 1.58969099521155E-10;

/// java.lang.FdLibm.Sin.__kernel_sin
fn kernel_sin(x: f64, y: f64, iy: i32) -> f64 {
    let ix = hi(x) & 0x7fffffff;
    if ix < 0x3e400000 && (x as i32) == 0 {
        return x;
    }
    let z = x * x;
    let v = z * x;
    let r = S2 + z * (S3 + z * (S4 + z * (S5 + z * S6)));
    if iy == 0 {
        x + v * (S1 + z * r)
    } else {
        x - ((z * (0.5 * y - v * r) - y) - v * S1)
    }
}

/// java.lang.FdLibm.Sin.compute (StrictMath.sin; Math.sin on the game's aarch64 JRE is a HotSpot intrinsic that
/// differs from it on about 5% of inputs: UNKNOWN).
pub fn sin(x: f64) -> f64 {
    let ix = hi(x) & 0x7fffffff;
    if ix <= 0x3fe921fb {
        return kernel_sin(x, 0.0, 0);
    }
    if ix >= 0x7ff00000 {
        return x - x;
    }
    let mut y = [0.0; 2];
    let n = rem_pio2(x, &mut y);
    match n & 3 {
        0 => kernel_sin(y[0], y[1], 1),
        1 => kernel_cos(y[0], y[1]),
        2 => -kernel_sin(y[0], y[1], 1),
        _ => -kernel_cos(y[0], y[1]),
    }
}

/// java.lang.FdLibm.Cos.compute (StrictMath.cos; see `sin` for Math.cos).
pub fn cos(x: f64) -> f64 {
    let ix = hi(x) & 0x7fffffff;
    if ix <= 0x3fe921fb {
        return kernel_cos(x, 0.0);
    }
    if ix >= 0x7ff00000 {
        return x - x;
    }
    let mut y = [0.0; 2];
    let n = rem_pio2(x, &mut y);
    match n & 3 {
        0 => kernel_cos(y[0], y[1]),
        1 => -kernel_sin(y[0], y[1], 1),
        2 => -kernel_cos(y[0], y[1]),
        _ => kernel_sin(y[0], y[1], 1),
    }
}

const T: [f64; 13] = [
    0.3333333333333341,
    0.13333333333320124,
    0.05396825397622605,
    0.021869488294859542,
    0.0088632398235993,
    0.0035920791075913124,
    0.0014562094543252903,
    5.880412408202641E-4,
    2.464631348184699E-4,
    7.817944429395571E-5,
    7.140724913826082E-5,
    -1.8558637485527546E-5,
    2.590730518636337E-5,
];

/// java.lang.FdLibm.Tan.__kernel_tan
fn kernel_tan(x: f64, y: f64, iy: i32) -> f64 {
    const PIO4: f64 = PI / 4.0;
    const PIO4LO: f64 = 3.061616997868383E-17;
    let mut x = x;
    let mut y = y;
    let hx = hi(x);
    let ix = hx & 0x7fffffff;
    if ix < 0x3e300000 && (x as i32) == 0 {
        if (ix | lo(x) | iy.wrapping_add(1)) == 0 {
            return 1.0 / x.abs();
        }
        if iy == 1 {
            return x;
        }
        let w = x + y;
        let z = with_lo(w, 0);
        let v = y - (z - x);
        let a = -1.0 / w;
        let t = with_lo(a, 0);
        let s = 1.0 + t * z;
        return t + a * (s + t * v);
    }
    if ix >= 0x3FE59428 {
        if hx < 0 {
            x = -x;
            y = -y;
        }
        let z = PIO4 - x;
        let w = PIO4LO - y;
        x = z + w;
        y = 0.0;
    }
    let z = x * x;
    let w = z * z;
    let mut r = T[1] + w * (T[3] + w * (T[5] + w * (T[7] + w * (T[9] + w * T[11]))));
    let v = z * (T[2] + w * (T[4] + w * (T[6] + w * (T[8] + w * (T[10] + w * T[12])))));
    let s = z * x;
    r = y + z * (s * (r + v) + y);
    r += T[0] * s;
    let w = x + r;
    if ix >= 0x3FE59428 {
        let v = f64::from(iy);
        return f64::from(1 - ((hx >> 30) & 2)) * (v - 2.0 * (x - (w * w / (w + v) - r)));
    }
    if iy == 1 {
        return w;
    }
    let z = with_lo(w, 0);
    let v = r - (z - x);
    let a = -1.0 / w;
    let t = with_lo(a, 0);
    let s = 1.0 + t * z;
    t + a * (s + t * v)
}

/// java.lang.FdLibm.Tan.compute
pub fn tan(x: f64) -> f64 {
    let ix = hi(x) & 0x7fffffff;
    if ix <= 0x3fe921fb {
        return kernel_tan(x, 0.0, 1);
    }
    if ix >= 0x7ff00000 {
        return x - x;
    }
    let mut y = [0.0; 2];
    let n = rem_pio2(x, &mut y);
    kernel_tan(y[0], y[1], 1 - ((n & 1) << 1))
}

const TWO_OVER_PI: [i32; 66] = [
    10680707, 7228996, 1387004, 2578385, 16069853, 12639074, 9804092, 4427841, 16666979, 11263675,
    12935607, 2387514, 4345298, 14681673, 3074569, 13734428, 16653803, 1880361, 10960616, 8533493,
    3062596, 8710556, 7349940, 6258241, 3772886, 3769171, 3798172, 8675211, 12450088, 3874808,
    9961438, 366607, 15675153, 9132554, 7151469, 3571407, 2607881, 12013382, 4155038, 6285869,
    7677882, 13102053, 15825725, 473591, 9065106, 15363067, 6271263, 9264392, 5636912, 4652155,
    7056368, 13614112, 10155062, 1944035, 9527646, 15080200, 6658437, 6231200, 6832269, 16767104,
    5075751, 3212806, 1398474, 7579849, 6349435, 12618859,
];

const NPIO2_HW: [i32; 32] = [
    1073291771, 1074340347, 1074977148, 1075388923, 1075800698, 1076025724, 1076231611, 1076437499,
    1076643386, 1076849274, 1076971356, 1077074300, 1077177244, 1077280187, 1077383131, 1077486075,
    1077589019, 1077691962, 1077794906, 1077897850, 1077968460, 1078019932, 1078071404, 1078122876,
    1078174348, 1078225820, 1078277292, 1078328763, 1078380235, 1078431707, 1078483179, 1078534651,
];

/// java.lang.FdLibm.RemPio2.__ieee754_rem_pio2: x - n*pi/2 as `y[0] + y[1]`, returns n.
fn rem_pio2(x: f64, y: &mut [f64; 2]) -> i32 {
    const INVPIO2: f64 = 0.6366197723675814;
    const PIO2_1: f64 = 1.5707963267341256;
    const PIO2_1T: f64 = 6.077100506506192E-11;
    const PIO2_2: f64 = 6.077100506303966E-11;
    const PIO2_2T: f64 = 2.0222662487959506E-21;
    const PIO2_3: f64 = 2.0222662487111665E-21;
    const PIO2_3T: f64 = 8.4784276603689E-32;
    let hx = hi(x);
    let ix = hx & 0x7fffffff;
    if ix <= 0x3fe921fb {
        y[0] = x;
        y[1] = 0.0;
        return 0;
    }
    if ix < 0x4002d97c {
        if hx > 0 {
            let mut z = x - PIO2_1;
            if ix != 0x3ff921fb {
                y[0] = z - PIO2_1T;
                y[1] = (z - y[0]) - PIO2_1T;
            } else {
                z -= PIO2_2;
                y[0] = z - PIO2_2T;
                y[1] = (z - y[0]) - PIO2_2T;
            }
            return 1;
        }
        let mut z = x + PIO2_1;
        if ix != 0x3ff921fb {
            y[0] = z + PIO2_1T;
            y[1] = (z - y[0]) + PIO2_1T;
        } else {
            z += PIO2_2;
            y[0] = z + PIO2_2T;
            y[1] = (z - y[0]) + PIO2_2T;
        }
        return -1;
    }
    if ix <= 0x413921fb {
        let mut t = x.abs();
        let n = (t * INVPIO2 + 0.5) as i32;
        let f_n = f64::from(n);
        let mut r = t - f_n * PIO2_1;
        let mut w = f_n * PIO2_1T;
        if n < 32 && ix != NPIO2_HW[(n - 1) as usize] {
            y[0] = r - w;
        } else {
            let j = ix >> 20;
            y[0] = r - w;
            let mut i = j - ((hi(y[0]) >> 20) & 0x7ff);
            if i > 16 {
                t = r;
                w = f_n * PIO2_2;
                r = t - w;
                w = f_n * PIO2_2T - ((t - r) - w);
                y[0] = r - w;
                i = j - ((hi(y[0]) >> 20) & 0x7ff);
                if i > 49 {
                    t = r;
                    w = f_n * PIO2_3;
                    r = t - w;
                    w = f_n * PIO2_3T - ((t - r) - w);
                    y[0] = r - w;
                }
            }
        }
        y[1] = (r - y[0]) - w;
        if hx < 0 {
            y[0] = -y[0];
            y[1] = -y[1];
            return -n;
        }
        return n;
    }
    if ix >= 0x7ff00000 {
        y[0] = x - x;
        y[1] = y[0];
        return 0;
    }
    let mut z = with_lo(0.0, lo(x));
    let e0 = (ix >> 20) - 1046;
    z = with_hi(z, ix - (e0 << 20));
    let mut tx = [0.0; 3];
    for v in tx.iter_mut().take(2) {
        *v = f64::from(z as i32);
        z = (z - *v) * TWO24;
    }
    tx[2] = z;
    let mut nx = 3;
    while tx[nx - 1] == 0.0 {
        nx -= 1;
    }
    let n = kernel_rem_pio2(&tx, y, e0, nx as i32, 2, &TWO_OVER_PI);
    if hx < 0 {
        y[0] = -y[0];
        y[1] = -y[1];
        return -n;
    }
    n
}

/// java.lang.FdLibm.KernelRemPio2.__kernel_rem_pio2 (precisions 0 to 2; 3 is never asked for).
fn kernel_rem_pio2(
    x: &[f64],
    y: &mut [f64; 2],
    e0: i32,
    nx: i32,
    prec: usize,
    ipio2: &[i32],
) -> i32 {
    const INIT_JK: [i32; 4] = [2, 3, 4, 6];
    const PIO2: [f64; 8] = [
        f(1.5707963),
        f(7.5497894E-8),
        f(5.3903025E-15),
        f(3.2820034E-22),
        f(1.2706558E-29),
        f(1.2293331E-36),
        2.7337005381646456E-44,
        2.1674168387780482E-51,
    ];
    let mut iq = [0i32; 20];
    let mut fv = [0.0f64; 20];
    let mut fq = [0.0f64; 20];
    let mut q = [0.0f64; 20];
    let jk = INIT_JK[prec];
    let jp = jk;
    let jx = nx - 1;
    let mut jv = (e0 - 3) / 24;
    if jv < 0 {
        jv = 0;
    }
    let mut q0 = e0 - 24 * (jv + 1);
    let mut j = jv - jx;
    let m = jx + jk;
    let mut i = 0;
    while i <= m {
        fv[i as usize] = if j < 0 {
            0.0
        } else {
            f64::from(ipio2[j as usize])
        };
        i += 1;
        j += 1;
    }
    for i in 0..=jk {
        let mut fw = 0.0;
        for j in 0..=jx {
            fw += x[j as usize] * fv[(jx + i - j) as usize];
        }
        q[i as usize] = fw;
    }
    let mut jz = jk;
    let mut z;
    let mut n;
    let mut ih;
    loop {
        let mut i = 0usize;
        let mut j = jz;
        z = q[jz as usize];
        while j > 0 {
            let fw = f64::from((TWON24 * z) as i32);
            iq[i] = (z - TWO24 * fw) as i32;
            z = q[(j - 1) as usize] + fw;
            i += 1;
            j -= 1;
        }
        z = scalb(z, q0);
        z -= 8.0 * (z * 0.125).floor();
        n = z as i32;
        z -= f64::from(n);
        ih = 0;
        let top = (jz - 1) as usize;
        if q0 > 0 {
            let i = iq[top].wrapping_shr((24 - q0) as u32);
            n += i;
            iq[top] -= i.wrapping_shl((24 - q0) as u32);
            ih = iq[top].wrapping_shr((23 - q0) as u32);
        } else if q0 == 0 {
            ih = iq[top] >> 23;
        } else if z >= 0.5 {
            ih = 2;
        }
        if ih > 0 {
            n += 1;
            let mut carry = false;
            for v in iq.iter_mut().take(jz as usize) {
                let j = *v;
                if !carry {
                    if j != 0 {
                        carry = true;
                        *v = 0x1000000 - j;
                    }
                } else {
                    *v = 0xffffff - j;
                }
            }
            if q0 > 0 {
                match q0 {
                    1 => iq[top] &= 0x7fffff,
                    2 => iq[top] &= 0x3fffff,
                    _ => {}
                }
            }
            if ih == 2 {
                z = 1.0 - z;
                if carry {
                    z -= scalb(1.0, q0);
                }
            }
        }
        if z != 0.0 {
            break;
        }
        let mut j = 0;
        let mut i = jz - 1;
        while i >= jk {
            j |= iq[i as usize];
            i -= 1;
        }
        if j != 0 {
            break;
        }
        let mut k = 1;
        while iq[(jk - k) as usize] == 0 {
            k += 1;
        }
        for i in (jz + 1)..=(jz + k) {
            fv[(jx + i) as usize] = f64::from(ipio2[(jv + i) as usize]);
            let mut fw = 0.0;
            for j in 0..=jx {
                fw += x[j as usize] * fv[(jx + i - j) as usize];
            }
            q[i as usize] = fw;
        }
        jz += k;
    }
    if z == 0.0 {
        jz -= 1;
        q0 -= 24;
        while iq[jz as usize] == 0 {
            jz -= 1;
            q0 -= 24;
        }
    } else {
        z = scalb(z, -q0);
        if z >= TWO24 {
            let fw = f64::from((TWON24 * z) as i32);
            iq[jz as usize] = (z - TWO24 * fw) as i32;
            jz += 1;
            q0 += 24;
            iq[jz as usize] = fw as i32;
        } else {
            iq[jz as usize] = z as i32;
        }
    }
    let mut fw = scalb(1.0, q0);
    let mut i = jz;
    while i >= 0 {
        q[i as usize] = fw * f64::from(iq[i as usize]);
        fw *= TWON24;
        i -= 1;
    }
    let mut i = jz;
    while i >= 0 {
        let mut fw = 0.0;
        let mut k = 0;
        while k <= jp && k <= jz - i {
            fw += PIO2[k as usize] * q[(i + k) as usize];
            k += 1;
        }
        fq[(jz - i) as usize] = fw;
        i -= 1;
    }
    let mut fw = 0.0;
    let mut i = jz;
    while i >= 0 {
        fw += fq[i as usize];
        i -= 1;
    }
    y[0] = if ih == 0 { fw } else { -fw };
    if prec != 0 {
        fw = fq[0] - fw;
        for v in fq.iter().take(jz as usize + 1).skip(1) {
            fw += *v;
        }
        y[1] = if ih == 0 { fw } else { -fw };
    }
    n & 7
}

const LN2_HI: f64 = 0.6931471803691238;
const LN2_LO: f64 = 1.9082149292705877E-10;
const INVLN2: f64 = 1.4426950408889634;
const P1: f64 = 0.16666666666666602;
const P2: f64 = -0.0027777777777015593;
const P3: f64 = 6.613756321437934E-5;
const P4: f64 = -1.6533902205465252E-6;
const P5: f64 = 4.1381367970572385E-8;

/// java.lang.FdLibm.Exp.compute
pub fn exp(x: f64) -> f64 {
    const HALF: [f64; 2] = [0.5, -0.5];
    const TWOM1000: f64 = 9.332636185032189E-302;
    const O_THRESHOLD: f64 = 709.782712893384;
    const U_THRESHOLD: f64 = -745.1332191019411;
    const LN2HI: [f64; 2] = [LN2_HI, -LN2_HI];
    const LN2LO: [f64; 2] = [LN2_LO, -LN2_LO];
    let mut x = x;
    let mut hi_part = 0.0;
    let mut lo_part = 0.0;
    let mut k: i32 = 0;
    let mut hx = hi(x);
    let xsb = ((hx >> 31) & 1) as usize;
    hx &= 0x7fffffff;
    if hx >= 0x40862E42 {
        if hx >= 0x7ff00000 {
            if ((hx & 0xfffff) | lo(x)) != 0 {
                return x + x;
            }
            return if xsb == 0 { x } else { 0.0 };
        }
        if x > O_THRESHOLD {
            return f64::INFINITY;
        }
        if x < U_THRESHOLD {
            return 0.0;
        }
    }
    if hx > 0x3fd62e42 {
        if hx < 0x3FF0A2B2 {
            hi_part = x - LN2HI[xsb];
            lo_part = LN2LO[xsb];
            k = 1 - xsb as i32 - xsb as i32;
        } else {
            k = (INVLN2 * x + HALF[xsb]) as i32;
            let t = f64::from(k);
            hi_part = x - t * LN2HI[0];
            lo_part = t * LN2LO[0];
        }
        x = hi_part - lo_part;
    } else if hx < 0x3e300000 {
        if HUGE + x > 1.0 {
            return 1.0 + x;
        }
    } else {
        k = 0;
    }
    let t = x * x;
    let c = x - t * (P1 + t * (P2 + t * (P3 + t * (P4 + t * P5))));
    if k == 0 {
        return 1.0 - ((x * c) / (c - 2.0) - x);
    }
    let y = 1.0 - ((lo_part - (x * c) / (2.0 - c)) - hi_part);
    if k >= -1021 {
        return with_hi(y, hi(y).wrapping_add(k.wrapping_shl(20)));
    }
    let y = with_hi(y, hi(y).wrapping_add((k + 1000).wrapping_shl(20)));
    y * TWOM1000
}

/// java.lang.FdLibm.Expm1.compute
pub fn expm1(x: f64) -> f64 {
    const TINY: f64 = 1.0E-300;
    const O_THRESHOLD: f64 = 709.782712893384;
    const Q1: f64 = -0.03333333333333313;
    const Q2: f64 = 0.0015873015872548146;
    const Q3: f64 = -7.93650757867488E-5;
    const Q4: f64 = 4.008217827329362E-6;
    const Q5: f64 = -2.0109921818362437E-7;
    let mut x = x;
    let mut c = 0.0;
    let mut hx = hi(x);
    let xsb = hx & i32::MIN;
    hx &= 0x7fffffff;
    if hx >= 0x4043687A {
        if hx >= 0x40862E42 {
            if hx >= 0x7ff00000 {
                if ((hx & 0xfffff) | lo(x)) != 0 {
                    return x + x;
                }
                return if xsb == 0 { x } else { -1.0 };
            }
            if x > O_THRESHOLD {
                return f64::INFINITY;
            }
        }
        if xsb != 0 && x + TINY < 0.0 {
            return -1.0;
        }
    }
    let k: i32;
    if hx > 0x3fd62e42 {
        let hi_part;
        let lo_part;
        if hx < 0x3FF0A2B2 {
            if xsb == 0 {
                hi_part = x - LN2_HI;
                lo_part = LN2_LO;
                k = 1;
            } else {
                hi_part = x + LN2_HI;
                lo_part = -LN2_LO;
                k = -1;
            }
        } else {
            k = (INVLN2 * x + if xsb == 0 { 0.5 } else { -0.5 }) as i32;
            let t = f64::from(k);
            hi_part = x - t * LN2_HI;
            lo_part = t * LN2_LO;
        }
        x = hi_part - lo_part;
        c = (hi_part - x) - lo_part;
    } else if hx < 0x3c900000 {
        let t = HUGE + x;
        return x - (t - (HUGE + x));
    } else {
        k = 0;
    }
    let hfx = 0.5 * x;
    let hxs = x * hfx;
    let r1 = 1.0 + hxs * (Q1 + hxs * (Q2 + hxs * (Q3 + hxs * (Q4 + hxs * Q5))));
    let mut t = 3.0 - r1 * hfx;
    let mut e = hxs * ((r1 - t) / (6.0 - x * t));
    if k == 0 {
        return x - (x * e - hxs);
    }
    e = x * (e - c) - c;
    e -= hxs;
    if k == -1 {
        return 0.5 * (x - e) - 0.5;
    }
    if k == 1 {
        return if x < -0.25 {
            -2.0 * (e - (x + 0.5))
        } else {
            1.0 + 2.0 * (x - e)
        };
    }
    if k > -2 && k <= 56 {
        t = 1.0;
        let mut y;
        if k < 20 {
            t = with_hi(t, 0x3ff00000 - 0x200000_i32.wrapping_shr(k as u32));
            y = t - (e - x);
            y = with_hi(y, hi(y).wrapping_add(k.wrapping_shl(20)));
        } else {
            t = with_hi(t, (0x3ff - k).wrapping_shl(20));
            y = x - (e + t);
            y += 1.0;
            y = with_hi(y, hi(y).wrapping_add(k.wrapping_shl(20)));
        }
        return y;
    }
    let mut y = 1.0 - (e - x);
    y = with_hi(y, hi(y).wrapping_add(k.wrapping_shl(20)));
    y - 1.0
}

/// java.lang.FdLibm.Log.compute
pub fn log(x: f64) -> f64 {
    const LG1: f64 = 0.6666666666666735;
    const LG2: f64 = 0.3999999999940942;
    const LG3: f64 = 0.2857142874366239;
    const LG4: f64 = 0.22222198432149784;
    const LG5: f64 = 0.1818357216161805;
    const LG6: f64 = 0.15313837699209373;
    const LG7: f64 = 0.14798198605116586;
    let mut x = x;
    let mut hx = hi(x);
    let lx = lo(x);
    let mut k = 0;
    if hx < 0x00100000 {
        if ((hx & 0x7fffffff) | lx) == 0 {
            return f64::NEG_INFINITY;
        }
        if hx < 0 {
            return (x - x) / 0.0;
        }
        k -= 54;
        x *= TWO54;
        hx = hi(x);
    }
    if hx >= 0x7ff00000 {
        return x + x;
    }
    k += (hx >> 20) - 1023;
    hx &= 0x000fffff;
    let i = (hx + 0x95f64) & 0x100000;
    x = with_hi(x, hx | (i ^ 0x3ff00000));
    k += i >> 20;
    let f = x - 1.0;
    if (0x000fffff & (2 + hx)) < 3 {
        if f == 0.0 {
            if k == 0 {
                return 0.0;
            }
            let dk = f64::from(k);
            return dk * LN2_HI + dk * LN2_LO;
        }
        let r = f * f * (0.5 - 0.3333333333333333 * f);
        if k == 0 {
            return f - r;
        }
        let dk = f64::from(k);
        return dk * LN2_HI - ((r - dk * LN2_LO) - f);
    }
    let s = f / (2.0 + f);
    let dk = f64::from(k);
    let z = s * s;
    let mut i = hx - 0x6147a;
    let w = z * z;
    let j = 0x6b851 - hx;
    let t1 = w * (LG2 + w * (LG4 + w * LG6));
    let t2 = z * (LG1 + w * (LG3 + w * (LG5 + w * LG7)));
    i |= j;
    let r = t2 + t1;
    if i > 0 {
        let hfsq = 0.5 * f * f;
        if k == 0 {
            return f - (hfsq - s * (hfsq + r));
        }
        return dk * LN2_HI - ((hfsq - (s * (hfsq + r) + dk * LN2_LO)) - f);
    }
    if k == 0 {
        return f - s * (f - r);
    }
    dk * LN2_HI - ((s * (f - r) - dk * LN2_LO) - f)
}

/// java.lang.FdLibm.Log10.compute
pub fn log10(x: f64) -> f64 {
    const IVLN10: f64 = 0.4342944819032518;
    const LOG10_2HI: f64 = 0.30102999566361177;
    const LOG10_2LO: f64 = 3.694239077158931E-13;
    let mut x = x;
    let mut hx = hi(x);
    let lx = lo(x);
    let mut k = 0;
    if hx < 0x00100000 {
        if ((hx & 0x7fffffff) | lx) == 0 {
            return f64::NEG_INFINITY;
        }
        if hx < 0 {
            return (x - x) / 0.0;
        }
        k -= 54;
        x *= TWO54;
        hx = hi(x);
    }
    if hx >= 0x7ff00000 {
        return x + x;
    }
    k += (hx >> 20) - 1023;
    let i = ((k as u32 & 0x80000000) >> 31) as i32;
    hx = (hx & 0x000fffff) | ((0x3ff - i) << 20);
    let y = f64::from(k + i);
    x = with_hi(x, hx);
    let z = y * LOG10_2LO + IVLN10 * log(x);
    z + y * LOG10_2HI
}

/// java.lang.FdLibm.Pow.compute
pub fn pow(x: f64, y: f64) -> f64 {
    if y == 0.0 {
        return 1.0;
    }
    if x.is_nan() || y.is_nan() {
        return x + y;
    }
    let y_abs = y.abs();
    let mut x_abs = x.abs();
    if y == 2.0 {
        return x * x;
    }
    if y == 0.5 {
        if x >= -f64::MAX {
            return (x + 0.0).sqrt();
        }
    } else {
        if y_abs == 1.0 {
            return if y == 1.0 { x } else { 1.0 / x };
        }
        if y_abs == f64::INFINITY {
            if x_abs == 1.0 {
                return y - y;
            }
            if x_abs > 1.0 {
                return if y >= 0.0 { y } else { 0.0 };
            }
            return if y < 0.0 { -y } else { 0.0 };
        }
    }
    let hx = hi(x);
    let mut ix = hx & 0x7fffffff;
    let mut y_is_int = 0;
    if hx < 0 {
        if y_abs >= TWO53 {
            y_is_int = 2;
        } else if y_abs >= 1.0 {
            let y_abs_as_long = y_abs as i64;
            if y_abs_as_long as f64 == y_abs {
                y_is_int = 2 - (y_abs_as_long & 1) as i32;
            }
        }
    }
    if x_abs == 0.0 || x_abs == f64::INFINITY || x_abs == 1.0 {
        let mut z = x_abs;
        if y < 0.0 {
            z = 1.0 / x_abs;
        }
        if hx < 0 {
            if (ix.wrapping_sub(0x3ff00000) | y_is_int) == 0 {
                z = (z - z) / (z - z);
            } else if y_is_int == 1 {
                z *= -1.0;
            }
        }
        return z;
    }
    let mut n = (hx >> 31) + 1;
    if (n | y_is_int) == 0 {
        return (x - x) / (x - x);
    }
    let mut s = 1.0;
    if (n | (y_is_int - 1)) == 0 {
        s = -1.0;
    }
    let t1;
    let t2;
    if y_abs > 2.1474856959999995E9 {
        const INV_LN2: f64 = 1.4426950408889634;
        const INV_LN2_H: f64 = f(1.442695);
        const INV_LN2_L: f64 = 1.9259629911266175E-8;
        if x_abs < f(0.9999995) {
            return if y < 0.0 { s * f64::INFINITY } else { s * 0.0 };
        }
        if x_abs > 1.0000009536743162 {
            return if y > 0.0 { s * f64::INFINITY } else { s * 0.0 };
        }
        let t = x_abs - 1.0;
        let w = t * t * (0.5 - t * (0.3333333333333333 - t * 0.25));
        let u = INV_LN2_H * t;
        let v = t * INV_LN2_L - w * INV_LN2;
        t1 = with_lo(u + v, 0);
        t2 = v - (t1 - u);
    } else {
        const CP: f64 = 0.9617966939259756;
        const CP_H: f64 = f(0.9617967);
        const CP_L: f64 = -7.028461650952758E-9;
        const BP: [f64; 2] = [1.0, 1.5];
        const DP_H: [f64; 2] = [0.0, f(0.5849625)];
        const DP_L: [f64; 2] = [0.0, 1.350039202129749E-8];
        const L1: f64 = 0.5999999999999946;
        const L2: f64 = 0.4285714285785502;
        const L3: f64 = 0.33333332981837743;
        const L4: f64 = 0.272728123808534;
        const L5: f64 = 0.23066074577556175;
        const L6: f64 = 0.20697501780033842;
        let mut n2 = 0;
        if ix < 0x00100000 {
            x_abs *= TWO53;
            n2 -= 53;
            ix = hi(x_abs);
        }
        n2 += (ix >> 20) - 0x3ff;
        let j = ix & 0x000fffff;
        ix = j | 0x3ff00000;
        let k: usize;
        if j <= 0x3988E {
            k = 0;
        } else if j < 0xBB67A {
            k = 1;
        } else {
            k = 0;
            n2 += 1;
            ix -= 0x00100000;
        }
        x_abs = with_hi(x_abs, ix);
        let mut u = x_abs - BP[k];
        let mut v = 1.0 / (x_abs + BP[k]);
        let ss = u * v;
        let s_h = with_lo(ss, 0);
        let mut t_h = with_hi(
            0.0,
            ((ix >> 1) | 0x20000000) + 0x00080000 + ((k as i32) << 18),
        );
        let mut t_l = x_abs - (t_h - BP[k]);
        let s_l = v * ((u - s_h * t_h) - s_h * t_l);
        let mut s2 = ss * ss;
        let mut r = s2 * s2 * (L1 + s2 * (L2 + s2 * (L3 + s2 * (L4 + s2 * (L5 + s2 * L6)))));
        r += s_l * (s_h + ss);
        s2 = s_h * s_h;
        t_h = 3.0 + s2 + r;
        t_h = with_lo(t_h, 0);
        t_l = r - ((t_h - 3.0) - s2);
        u = s_h * t_h;
        v = s_l * t_h + t_l * ss;
        let mut p_h = u + v;
        p_h = with_lo(p_h, 0);
        let p_l = v - (p_h - u);
        let z_h = CP_H * p_h;
        let z_l = CP_L * p_h + p_l * CP + DP_L[k];
        let t = f64::from(n2);
        t1 = with_lo(z_h + z_l + DP_H[k] + t, 0);
        t2 = z_l - (t1 - t - DP_H[k] - z_h);
    }
    let y1 = with_lo(y, 0);
    let p_l = (y - y1) * t1 + y * t2;
    let mut p_h = y1 * t1;
    let mut z = p_l + p_h;
    let mut j = hi(z);
    let i = lo(z);
    if j >= 0x40900000 {
        if (j.wrapping_sub(0x40900000) | i) != 0 {
            return s * f64::INFINITY;
        }
        const OVT: f64 = 8.008566259537294E-17;
        if p_l + OVT > z - p_h {
            return s * f64::INFINITY;
        }
    } else if (j & 0x7fffffff) >= 0x4090cc00 {
        if (j.wrapping_sub(0xc090cc00_u32 as i32) | i) != 0 {
            return s * 0.0;
        }
        if p_l <= z - p_h {
            return s * 0.0;
        }
    }
    const LG2: f64 = 0.6931471805599453;
    const LG2_H: f64 = f(0.6931472);
    const LG2_L: f64 = -1.904654299957768E-9;
    let i = j & 0x7fffffff;
    let mut k = (i >> 20) - 0x3ff;
    n = 0;
    if i > 0x3fe00000 {
        n = j.wrapping_add(0x00100000_i32.wrapping_shr((k + 1) as u32));
        k = ((n & 0x7fffffff) >> 20) - 0x3ff;
        let t = with_hi(0.0, n & !(0x000fffff_i32.wrapping_shr(k as u32)));
        n = ((n & 0x000fffff) | 0x00100000).wrapping_shr((20 - k) as u32);
        if j < 0 {
            n = -n;
        }
        p_h -= t;
    }
    let mut t = p_l + p_h;
    t = with_lo(t, 0);
    let u = t * LG2_H;
    let v = (p_l - (t - p_h)) * LG2 + t * LG2_L;
    z = u + v;
    let w = v - (z - u);
    t = z * z;
    let t1 = z - t * (P1 + t * (P2 + t * (P3 + t * (P4 + t * P5))));
    let r = (z * t1) / (t1 - 2.0) - (w + z * w);
    z = 1.0 - (r - z);
    j = hi(z);
    j = j.wrapping_add(n.wrapping_shl(20));
    if (j >> 20) <= 0 {
        z = scalb(z, n);
    } else {
        z = with_hi(z, hi(z).wrapping_add(n.wrapping_shl(20)));
    }
    s * z
}

/// java.lang.FdLibm.Sinh.compute
pub fn sinh(x: f64) -> f64 {
    const SHUGE: f64 = 1.0E307;
    let jx = hi(x);
    let ix = jx & 0x7fffffff;
    if ix >= 0x7ff00000 {
        return x + x;
    }
    let h = if jx < 0 { -0.5 } else { 0.5 };
    if ix < 0x40360000 {
        if ix < 0x3e300000 && SHUGE + x > 1.0 {
            return x;
        }
        let t = expm1(x.abs());
        if ix < 0x3ff00000 {
            return h * (2.0 * t - t * t / (t + 1.0));
        }
        return h * (t + t / (t + 1.0));
    }
    if ix < 0x40862E42 {
        return h * exp(x.abs());
    }
    let lx = lo(x);
    if ix < 0x408633CE || (ix == 0x408633CE && (lx as u32) <= 0x8fb9f87d) {
        let w = exp(0.5 * x.abs());
        let t = h * w;
        return t * w;
    }
    x * SHUGE
}

/// java.lang.FdLibm.Cosh.compute
pub fn cosh(x: f64) -> f64 {
    let ix = hi(x) & 0x7fffffff;
    if ix >= 0x7ff00000 {
        return x * x;
    }
    if ix < 0x3fd62e43 {
        let t = expm1(x.abs());
        let w = 1.0 + t;
        return if ix < 0x3c800000 {
            w
        } else {
            1.0 + t * t / (w + w)
        };
    }
    if ix < 0x40360000 {
        let t = exp(x.abs());
        return 0.5 * t + 0.5 / t;
    }
    if ix < 0x40862E42 {
        return 0.5 * exp(x.abs());
    }
    let lx = lo(x);
    if ix < 0x408633CE || (ix == 0x408633CE && (lx as u32) <= 0x8fb9f87d) {
        let w = exp(0.5 * x.abs());
        let t = 0.5 * w;
        return t * w;
    }
    f64::INFINITY
}

/// java.lang.FdLibm.Tanh.compute
pub fn tanh(x: f64) -> f64 {
    let jx = hi(x);
    let ix = jx & 0x7fffffff;
    if ix >= 0x7ff00000 {
        return if jx >= 0 {
            1.0 / x + 1.0
        } else {
            1.0 / x - 1.0
        };
    }
    let z;
    if ix < 0x40360000 {
        if ix < 0x3c800000 {
            return x * (1.0 + x);
        }
        if ix >= 0x3ff00000 {
            let t = expm1(2.0 * x.abs());
            z = 1.0 - 2.0 / (t + 2.0);
        } else {
            let t = expm1(-2.0 * x.abs());
            z = -t / (t + 2.0);
        }
    } else {
        z = 1.0;
    }
    if jx >= 0 {
        z
    } else {
        -z
    }
}
