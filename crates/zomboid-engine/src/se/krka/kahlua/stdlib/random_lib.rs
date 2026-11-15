//! se.krka.kahlua.stdlib.RandomLib: `newrandom()` and the `random`/`seed` methods of a java.util.Random.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::java::Random;
use crate::se::krka::kahlua::j2se::j2se_platform::J2SEPlatform;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
use crate::se::krka::kahlua::vm::kahlua_util::{
    fail, get_arg, get_class_metatables, get_optional_number_arg, to_double,
};
use crate::se::krka::kahlua::vm::object::{
    classes, identity_hash_code, JavaClass, JavaFunction, Object, TableRef, Throwable, Userdata, R,
};

pub static CLASS: JavaClass = JavaClass {
    name: "se.krka.kahlua.stdlib.RandomLib",
    superclass: Some(&classes::OBJECT),
    interfaces: &[&classes::JAVA_FUNCTION],
};
pub static RANDOM_CLASS: JavaClass = JavaClass {
    name: "java.util.Random",
    superclass: Some(&classes::OBJECT),
    interfaces: &[&classes::SERIALIZABLE],
};

/// A java.util.Random as a Lua value.
pub struct RandomObject(pub RefCell<Random>);

impl Userdata for RandomObject {
    fn class(&self) -> &'static JavaClass {
        &RANDOM_CLASS
    }
    /// Object.toString: the class name and the identity hash (the JVM's differs from run to run).
    fn java_to_string(&self) -> String {
        format!(
            "java.util.Random@{:x}",
            (std::ptr::from_ref(self) as usize >> 4) & 0x7fff_ffff
        )
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub struct RandomLib {
    index: usize,
    /// java.util.Random.seedUniquifier, for `newrandom` (static in the JDK, per function object here).
    seed_uniquifier: Cell<i64>,
}

impl RandomLib {
    fn new(index: usize) -> RandomLib {
        RandomLib {
            index,
            seed_uniquifier: Cell::new(8_682_522_807_148_012),
        }
    }
}

/// se.krka.kahlua.stdlib.RandomLib.register
pub fn register(platform: &J2SEPlatform, env: &TableRef) -> R<()> {
    let t = KahluaTableImpl::new_ref();
    for (i, name) in ["random", "seed"].iter().enumerate() {
        KahluaTableImpl::rawset(
            &t,
            Object::str(name),
            Object::Function(Rc::new(RandomLib::new(i))),
        );
    }
    KahluaTableImpl::rawset(&t, Object::str("__index"), Object::Table(t.clone()));
    let metatables = get_class_metatables(env)?;
    KahluaTableImpl::rawset(
        &metatables,
        platform.class_object(&RANDOM_CLASS),
        Object::Table(t),
    );
    KahluaTableImpl::rawset(
        env,
        Object::str("newrandom"),
        Object::Function(Rc::new(RandomLib::new(2))),
    );
    Ok(())
}

impl JavaFunction for RandomLib {
    /// se.krka.kahlua.stdlib.RandomLib.call
    fn call(&self, _thread: &mut KahluaThread, cf: &FrameRef, _n: i32) -> R<i32> {
        match self.index {
            0 => random(cf),
            1 => random_seed(cf),
            _ => self.new_random(cf),
        }
    }
    fn java_to_string(&self) -> String {
        format!("{}@0", CLASS.name)
    }
    fn class(&self) -> &'static JavaClass {
        &CLASS
    }
}

/// se.krka.kahlua.stdlib.RandomLib.getRandom
fn get_random(cf: &FrameRef, name: &str) -> R<Rc<dyn Userdata>> {
    let obj = get_arg(cf, 1, name)?;
    match obj {
        Object::Userdata(u) if u.as_any().is::<RandomObject>() => Ok(u),
        _ => Err(fail(&format!(
            "First argument to {name} must be an object of type random."
        ))),
    }
}

fn with_random<T>(u: &Rc<dyn Userdata>, f: impl FnOnce(&mut Random) -> T) -> Option<T> {
    u.as_any()
        .downcast_ref::<RandomObject>()
        .map(|r| f(&mut r.0.borrow_mut()))
}

/// `o.hashCode()` of a Lua value: Double, String and Boolean by value; the rest is the identity hash, which in the
/// original differs from run to run (UNKNOWN).
fn hash_code(o: &Object) -> i32 {
    match o {
        Object::Nil => 0,
        Object::Boolean(b) => {
            if *b {
                1231
            } else {
                1237
            }
        }
        Object::Number(d) => {
            let bits = if d.is_nan() {
                0x7ff8_0000_0000_0000
            } else {
                d.to_bits()
            };
            (bits ^ (bits >> 32)) as i32
        }
        Object::String(s) => s
            .iter()
            .fold(0i32, |h, &c| h.wrapping_mul(31).wrapping_add(i32::from(c))),
        Object::Table(t) => identity_hash_code(t),
        Object::Closure(c) => identity_hash_code(c),
        Object::Function(f) => identity_hash_code(f),
        Object::Coroutine(c) => identity_hash_code(c),
        Object::Userdata(u) => identity_hash_code(u),
    }
}

/// se.krka.kahlua.stdlib.RandomLib.randomSeed: `setSeed(o == null ? 0 : o.hashCode())`.
fn random_seed(cf: &FrameRef) -> R<i32> {
    let random = get_random(cf, "seed")?;
    let hash = hash_code(&cf.get(1));
    with_random(&random, |r| *r = Random::new(i64::from(hash)));
    Ok(0)
}

/// se.krka.kahlua.stdlib.RandomLib.random: `nextDouble()`, or an integer of [min, max] (min 1 without a max) by
/// `nextInt(n - m + 1)`, which throws for a bound below 1.
fn random(cf: &FrameRef) -> R<i32> {
    let random = get_random(cf, "random")?;
    let min = get_optional_number_arg(cf, 2)?;
    let max = get_optional_number_arg(cf, 3)?;
    let v = match min {
        None => with_random(&random, |r| r.next_double()).unwrap_or(0.0),
        Some(min) => {
            let mut m = min as i32;
            let n = match max {
                None => {
                    let n = m;
                    m = 1;
                    n
                }
                Some(max) => max as i32,
            };
            let bound = n.wrapping_sub(m).wrapping_add(1);
            if bound <= 0 {
                return Err(Throwable::Java {
                    class: "java.lang.IllegalArgumentException",
                    message: Some("bound must be positive".into()),
                });
            }
            let r = with_random(&random, |r| r.next_int_bound(bound)).unwrap_or(0);
            f64::from(m.wrapping_add(r))
        }
    };
    cf.push(Object::Number(to_double(v)))?;
    Ok(1)
}

impl RandomLib {
    /// se.krka.kahlua.stdlib.RandomLib.newRandom: `new Random()`. The JDK seeds it from `seedUniquifier()` XOR
    /// `System.nanoTime()`; here the uniquifier sequence alone (PLACEHOLDER: the original's seed differs each run).
    fn new_random(&self, cf: &FrameRef) -> R<i32> {
        let next = self
            .seed_uniquifier
            .get()
            .wrapping_mul(1_181_783_497_276_652_981);
        self.seed_uniquifier.set(next);
        cf.push(Object::Userdata(Rc::new(RandomObject(RefCell::new(
            Random::new(next),
        )))))?;
        Ok(1)
    }
}
