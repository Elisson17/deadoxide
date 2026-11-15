//! se.krka.kahlua.j2se.MathLib: the `math` table (`max`, `min` come from stdlib.lua; `random` is not here).
//! java.lang.Math is the ported FdLibm (`crate::java_fdlibm`), identical to the game's JRE, except `sin` and
//! `cos`: HotSpot intrinsics on that JRE that differ from StrictMath; Rust's are used, UNKNOWN.
use std::rc::Rc;

use crate::java_fdlibm as fd;

use super::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
use crate::se::krka::kahlua::vm::kahlua_util::{
    get_double_arg, is_negative, lua_assert, to_double,
};
use crate::se::krka::kahlua::vm::object::{classes, JavaClass, JavaFunction, Object, TableRef, R};

pub static CLASS: JavaClass = JavaClass {
    name: "se.krka.kahlua.j2se.MathLib",
    superclass: Some(&classes::OBJECT),
    interfaces: &[&classes::JAVA_FUNCTION],
};

const NAMES: [&str; 24] = [
    "abs", "acos", "asin", "atan", "atan2", "ceil", "cos", "cosh", "deg", "exp", "floor", "fmod",
    "frexp", "ldexp", "log", "log10", "modf", "pow", "rad", "sin", "sinh", "sqrt", "tan", "tanh",
];

pub struct MathLib {
    index: usize,
}

/// se.krka.kahlua.j2se.MathLib.register
pub fn register(env: &TableRef) {
    let math = KahluaTableImpl::new_ref();
    KahluaTableImpl::rawset(env, Object::str("math"), Object::Table(math.clone()));
    KahluaTableImpl::rawset(
        &math,
        Object::str("pi"),
        Object::Number(std::f64::consts::PI),
    );
    KahluaTableImpl::rawset(&math, Object::str("huge"), Object::Number(f64::INFINITY));
    for (i, name) in NAMES.iter().enumerate() {
        KahluaTableImpl::rawset(
            &math,
            Object::str(name),
            Object::Function(Rc::new(MathLib { index: i })),
        );
    }
}

/// `1.0 / Math.log(2.0)`
fn ln2_inv() -> f64 {
    1.0 / fd::log(2.0)
}

impl JavaFunction for MathLib {
    /// se.krka.kahlua.j2se.MathLib.call
    fn call(&self, _thread: &mut KahluaThread, cf: &FrameRef, n: i32) -> R<i32> {
        let name = NAMES[self.index];
        let two = matches!(self.index, 4 | 11 | 13 | 17);
        lua_assert(n >= if two { 2 } else { 1 }, "Not enough arguments")?;
        let x = get_double_arg(cf, 1, name)?;
        let one = |v: f64| -> R<i32> {
            cf.push(Object::Number(to_double(v)))?;
            Ok(1)
        };
        match self.index {
            0 => one(x.abs()),
            1 => one(fd::acos(x)),
            2 => one(fd::asin(x)),
            3 => one(fd::atan(x)),
            4 => {
                let x2 = get_double_arg(cf, 2, name)?;
                one(fd::atan2(x, x2))
            }
            5 => one(x.ceil()),
            6 => one(x.cos()),
            7 => one(fd::cosh(x)),
            // Math.toDegrees / toRadians: one multiplication by the JDK's constant.
            8 => one(x * 57.29577951308232),
            9 => one(fd::exp(x)),
            10 => one(x.floor()),
            11 => {
                let mut v1 = x;
                let mut v2 = get_double_arg(cf, 2, name)?;
                let res = if v1.is_infinite() || v1.is_nan() {
                    f64::NAN
                } else if v2.is_infinite() {
                    v1
                } else {
                    v2 = v2.abs();
                    let negate = is_negative(v1);
                    if negate {
                        v1 = -v1;
                    }
                    let r = v1 - (v1 / v2).floor() * v2;
                    if negate {
                        -r
                    } else {
                        r
                    }
                };
                one(res)
            }
            12 => {
                let (m, e) = if !x.is_infinite() && !x.is_nan() {
                    let e = (fd::log(x) * ln2_inv()).ceil();
                    let div = 1i32.wrapping_shl(e as i32 as u32);
                    (x / f64::from(div), e)
                } else {
                    (x, 0.0)
                };
                cf.push2(Object::Number(to_double(m)), Object::Number(to_double(e)))
            }
            13 => {
                let m = x;
                let de = get_double_arg(cf, 2, name)?;
                let tmp = m + de;
                let ret = if !tmp.is_infinite() && !tmp.is_nan() {
                    let e = de as i32;
                    m * f64::from(1i32.wrapping_shl(e as u32))
                } else {
                    m
                };
                one(ret)
            }
            14 => one(fd::log(x)),
            15 => one(fd::log10(x)),
            16 => {
                let mut x = x;
                let negate = is_negative(x);
                if negate {
                    x = -x;
                }
                let mut int_part = x.floor();
                let mut frac_part = if int_part.is_infinite() {
                    0.0
                } else {
                    x - int_part
                };
                if negate {
                    int_part = -int_part;
                    frac_part = -frac_part;
                }
                cf.push2(
                    Object::Number(to_double(int_part)),
                    Object::Number(to_double(frac_part)),
                )
            }
            17 => {
                let y = get_double_arg(cf, 2, name)?;
                one(crate::java::math_pow(x, y))
            }
            18 => one(x * 0.017453292519943295),
            19 => one(x.sin()),
            20 => one(fd::sinh(x)),
            21 => one(x.sqrt()),
            22 => one(fd::tan(x)),
            23 => one(fd::tanh(x)),
            _ => Ok(0),
        }
    }
    /// se.krka.kahlua.j2se.MathLib.toString
    fn java_to_string(&self) -> String {
        format!("math.{}", NAMES[self.index])
    }
    fn class(&self) -> &'static JavaClass {
        &CLASS
    }
}
