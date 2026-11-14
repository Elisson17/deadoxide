//! zombie.characters.BodyDamage.Fitness: muscle stiffness and exercise regularity, as a save stores them.
use std::collections::BTreeMap;

use crate::java_nio::{read_string, ByteBuffer};

/// zombie.characters.BodyDamage.Fitness, the saved maps (HashMaps in the original; ordered here). `load` adds to the
/// maps of a fresh Fitness.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fitness {
    pub stiffness_inc_map: BTreeMap<String, f32>,
    pub stiffness_timer_map: BTreeMap<String, i32>,
    pub regularity_map: BTreeMap<String, f32>,
    pub bodypart_to_inc_stiffness: Vec<String>,
    pub exe_timer: BTreeMap<String, i64>,
}

impl Fitness {
    /// zombie.characters.BodyDamage.Fitness.load
    pub fn load(b: &mut ByteBuffer) -> Result<Self, String> {
        let mut f = Fitness::default();
        for _ in 0..b.get_int()? {
            let k = read_string(b)?;
            f.stiffness_inc_map.insert(k, b.get_float()?);
        }
        for _ in 0..b.get_int()? {
            let k = read_string(b)?;
            f.stiffness_timer_map.insert(k, b.get_int()?);
        }
        for _ in 0..b.get_int()? {
            let k = read_string(b)?;
            f.regularity_map.insert(k, b.get_float()?);
        }
        for _ in 0..b.get_int()? {
            f.bodypart_to_inc_stiffness.push(read_string(b)?);
        }
        for _ in 0..b.get_int()? {
            let k = read_string(b)?;
            f.exe_timer.insert(k, b.get_long()?);
        }
        Ok(f)
    }
}
