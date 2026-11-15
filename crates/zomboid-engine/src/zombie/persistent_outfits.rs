//! zombie.PersistentOutfits: the outfit ids zombies carry (outfit index, variant, female and fallen-hat bits) and the
//! 500 seeds of `z_outfits.bin` that make each variant dress the same way every time.
use crate::java::{equals_ignore_case, string_compare_to};
use crate::java_nio::ByteBuffer;
use crate::zombie::core::skinnedmodel::population::outfit::Outfit;
use crate::zombie::core::skinnedmodel::population::outfit_manager::OutfitManager;

/// zombie.PersistentOutfits.INVALID_ID
pub const INVALID_ID: i32 = 0;
/// zombie.PersistentOutfits.FEMALE_BIT
pub const FEMALE_BIT: i32 = i32::MIN;
/// zombie.PersistentOutfits.NO_HAT_BIT
pub const NO_HAT_BIT: i32 = 32768;
/// zombie.PersistentOutfits.NUM_SEEDS
pub const NUM_SEEDS: usize = 500;
/// zombie.PersistentOutfits.FILE_MAGIC
pub const FILE_MAGIC: [u8; 4] = *b"PSTZ";

/// zombie.PersistentOutfits.IOutfitter: which function dresses a character in the outfit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outfitter {
    /// zombie.PersistentOutfits.ApplyOutfit
    ApplyOutfit,
    /// zombie.characters.ZombiesZoneDefinition.ApplyCustomOutfit
    CustomOutfit,
    /// zombie.SharedDescriptors.ApplyReanimatedPlayerOutfit
    ReanimatedPlayer,
}

/// zombie.PersistentOutfits.Data
#[derive(Clone, Debug, PartialEq)]
pub struct Data {
    /// `(short) all.size()` when made.
    pub index: i32,
    pub outfit_name: String,
    pub use_seed: bool,
    pub outfitter: Outfitter,
}

/// A `TreeMap<String, Data>(String.CASE_INSENSITIVE_ORDER)` as name → index into `all`.
fn ci_get(map: &[(String, usize)], name: &str) -> Option<usize> {
    map.iter()
        .find(|(k, _)| equals_ignore_case(k, name))
        .map(|(_, i)| *i)
}

fn ci_put(map: &mut Vec<(String, usize)>, name: &str, index: usize) {
    match map.iter_mut().find(|(k, _)| equals_ignore_case(k, name)) {
        Some(e) => e.1 = index,
        None => map.push((name.to_string(), index)),
    }
}

/// zombie.PersistentOutfits
#[derive(Clone, Debug, PartialEq)]
pub struct PersistentOutfits {
    pub seeds: Vec<i64>,
    pub outfit_names: Vec<String>,
    pub all: Vec<Data>,
    /// Indices into `all` (an outfit can be in both lists).
    pub female: Vec<usize>,
    pub male: Vec<usize>,
    outfit_to_data: Vec<(String, usize)>,
    outfit_to_female: Vec<(String, usize)>,
    outfit_to_male: Vec<(String, usize)>,
}

impl Default for PersistentOutfits {
    fn default() -> Self {
        PersistentOutfits {
            seeds: vec![0; NUM_SEEDS],
            outfit_names: Vec::new(),
            all: Vec::new(),
            female: Vec::new(),
            male: Vec::new(),
            outfit_to_data: Vec::new(),
            outfit_to_female: Vec::new(),
            outfit_to_male: Vec::new(),
        }
    }
}

impl PersistentOutfits {
    /// zombie.PersistentOutfits.init up to `registerCustomOutfits`: everything cleared, on a non-client the 500
    /// seeds drawn (`Rand.Next(Integer.MAX_VALUE)` through `rand_next`), then the female and the male outfits of the
    /// outfit manager. The original goes on with `registerCustomOutfits` (vehicle stories and the Lua
    /// `ZombiesZoneDefinition`, not ported; `register_outfitter` is), then on a non-client `load` and `save`.
    pub fn init(
        &mut self,
        om: &OutfitManager,
        client: bool,
        rand_next: &mut dyn FnMut(i32) -> i32,
    ) {
        self.all.clear();
        self.female.clear();
        self.male.clear();
        self.outfit_to_data.clear();
        self.outfit_to_female.clear();
        self.outfit_to_male.clear();
        self.outfit_names.clear();
        if !client {
            for s in self.seeds.iter_mut() {
                *s = i64::from(rand_next(i32::MAX));
            }
        }
        self.init_outfit_list(&om.female_outfits, true);
        self.init_outfit_list(&om.male_outfits, false);
    }

    /// zombie.PersistentOutfits.initOutfitList: the outfits sorted by name (`String.compareTo`, stable).
    fn init_outfit_list(&mut self, outfit_list: &[Outfit], female: bool) {
        let mut names: Vec<&str> = outfit_list.iter().map(|o| o.name.as_str()).collect();
        names.sort_by(|a, b| string_compare_to(a, b));
        for name in names {
            self.init_outfit(name, female, true, Outfitter::ApplyOutfit);
        }
    }

    /// zombie.PersistentOutfits.initOutfit: the outfit's data (made on first sight of the name, ignoring case),
    /// added to the female or male list and map.
    fn init_outfit(
        &mut self,
        outfit_name: &str,
        female: bool,
        use_seed: bool,
        outfitter: Outfitter,
    ) {
        let data = match ci_get(&self.outfit_to_data, outfit_name) {
            Some(i) => i,
            None => {
                let i = self.all.len();
                self.all.push(Data {
                    index: i32::from(i as i16),
                    outfit_name: outfit_name.to_string(),
                    use_seed,
                    outfitter,
                });
                self.outfit_names.push(outfit_name.to_string());
                ci_put(&mut self.outfit_to_data, outfit_name, i);
                i
            }
        };
        if female {
            self.female.push(data);
            ci_put(&mut self.outfit_to_female, outfit_name, data);
        } else {
            self.male.push(data);
            ci_put(&mut self.outfit_to_male, outfit_name, data);
        }
    }

    /// zombie.PersistentOutfits.registerOutfitter: the outfit for both sexes.
    pub fn register_outfitter(&mut self, id: &str, use_seed: bool, outfitter: Outfitter) {
        self.init_outfit(id, true, use_seed, outfitter);
        self.init_outfit(id, false, use_seed, outfitter);
    }

    /// zombie.PersistentOutfits.pickOutfitFemale: 0 for an unknown name; else the female bit, the outfit index and
    /// variant + 1 (`Rand.Next(500)` through `rand_next` when the outfit uses seeds, else 0).
    pub fn pick_outfit_female(
        &self,
        outfit_name: &str,
        rand_next: &mut dyn FnMut(i32) -> i32,
    ) -> i32 {
        self.pick(&self.outfit_to_female, outfit_name, rand_next)
            .map_or(0, |id| i32::MIN | id)
    }

    /// zombie.PersistentOutfits.pickOutfitMale
    pub fn pick_outfit_male(
        &self,
        outfit_name: &str,
        rand_next: &mut dyn FnMut(i32) -> i32,
    ) -> i32 {
        self.pick(&self.outfit_to_male, outfit_name, rand_next)
            .unwrap_or(0)
    }

    fn pick(
        &self,
        map: &[(String, usize)],
        outfit_name: &str,
        rand_next: &mut dyn FnMut(i32) -> i32,
    ) -> Option<i32> {
        let data = &self.all[ci_get(map, outfit_name)?];
        let outfit_index = data.index as i16;
        let variant = if data.use_seed {
            rand_next(NUM_SEEDS as i32) as i16
        } else {
            0
        };
        Some((i32::from(outfit_index) << 16) | (i32::from(variant) + 1))
    }

    /// zombie.PersistentOutfits.getOutfit: 0 for 0 or an outfit index outside `all`; a variant outside 1..=500 of
    /// an outfit that uses seeds is redrawn (`Rand.Next(500) + 1` through `rand_next`).
    pub fn get_outfit(&self, id: i32, rand_next: &mut dyn FnMut(i32) -> i32) -> i32 {
        if id == 0 {
            return 0;
        }
        let female_bit = id & i32::MIN;
        let mut id = id & i32::MAX;
        let no_hat_bit = id & NO_HAT_BIT;
        id &= -32769;
        let outfit_index = (id >> 16) as i16;
        let mut variant = (id & 65535) as i16;
        if outfit_index >= 0 && (outfit_index as usize) < self.all.len() {
            let data = &self.all[outfit_index as usize];
            if data.use_seed && !(1..=500).contains(&variant) {
                variant = (rand_next(NUM_SEEDS as i32) + 1) as i16;
            }
            female_bit | no_hat_bit | (i32::from(outfit_index) << 16) | i32::from(variant)
        } else {
            0
        }
    }

    /// zombie.PersistentOutfits.isHatFallen(int)
    pub fn is_hat_fallen(&self, outfit_id: i32) -> bool {
        outfit_id & NO_HAT_BIT != 0
    }

    /// zombie.PersistentOutfits.load(ByteBuffer) over the bytes of `z_outfits.bin`: the "PSTZ" magic (else the
    /// original's `IOException("not magic")`), the version, and for version 1 the seed count and the seeds (only
    /// the first 500 are read; a count past them reads nothing more).
    pub fn load(&mut self, data: &[u8]) -> Result<(), String> {
        let mut input = ByteBuffer::wrap(data);
        if input.get_bytes(4)? != FILE_MAGIC {
            return Err("java.io.IOException: not magic".to_string());
        }
        let version = input.get_int()?;
        if version == 1 {
            let num_variants = i32::from(input.get_short()?);
            for i in 0..num_variants {
                if (i as usize) < NUM_SEEDS {
                    self.seeds[i as usize] = input.get_long()?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Outfit ids: the female bit, index << 16, variant; an unknown name is 0; a variant of 0 is redrawn.
    #[test]
    fn outfit_ids() {
        let mut p = PersistentOutfits::default();
        p.init_outfit("Police", true, true, Outfitter::ApplyOutfit);
        p.init_outfit("Generic01", false, true, Outfitter::ApplyOutfit);
        p.init_outfit("police", false, false, Outfitter::ApplyOutfit);
        assert_eq!(p.all.len(), 2);
        let mut seven = |_| 7;
        assert_eq!(p.pick_outfit_female("POLICE", &mut seven), i32::MIN | 8);
        assert_eq!(p.pick_outfit_male("Generic01", &mut seven), (1 << 16) | 8);
        assert_eq!(p.pick_outfit_male("Nope", &mut seven), 0);
        assert_eq!(
            p.get_outfit((1 << 16) | NO_HAT_BIT, &mut seven),
            (1 << 16) | NO_HAT_BIT | 8
        );
        assert_eq!(p.get_outfit(5 << 16 | 3, &mut seven), 0);
        let mut data = b"PSTZ".to_vec();
        data.extend(1i32.to_be_bytes());
        data.extend(2i16.to_be_bytes());
        data.extend(11i64.to_be_bytes());
        data.extend((-12i64).to_be_bytes());
        p.load(&data).unwrap();
        assert_eq!(&p.seeds[..3], &[11, -12, 0]);
        assert!(p.load(b"XXXX").is_err());
    }
}
