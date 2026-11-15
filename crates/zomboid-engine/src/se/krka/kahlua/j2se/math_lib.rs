//! se.krka.kahlua.j2se.MathLib: the `math` table (`max`, `min` come from stdlib.lua; `random` is not here).
use std::rc::Rc;

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
    1.0 / 2.0f64.ln()
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
            1 => one(x.acos()),
            2 => one(x.asin()),
            3 => one(x.atan()),
            4 => {
                let x2 = get_double_arg(cf, 2, name)?;
                one(x.atan2(x2))
            }
            5 => one(x.ceil()),
            6 => one(x.cos()),
            7 => one(x.cosh()),
            // Math.toDegrees / toRadians: one multiplication by the JDK's constant.
            8 => one(x * 57.29577951308232),
            9 => one(x.exp()),
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
                    let e = (x.ln() * ln2_inv()).ceil();
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
            14 => one(x.ln()),
            15 => one(x.log10()),
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
            20 => one(x.sinh()),
            21 => one(x.sqrt()),
            22 => one(x.tan()),
            23 => one(x.tanh()),
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
