//! zombie.characters.SurvivorFactory: the name lists and the descriptor of a random survivor.
use super::survivor_desc::SurvivorDesc;
use crate::java::JavaRandom;
use crate::zombie::core::random::rand_abstract::RandAbstract;

/// zombie.characters.SurvivorFactory.SurvivorType
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SurvivorType {
    Friendly,
    #[default]
    Neutral,
    Aggressive,
}

impl SurvivorType {
    /// `Enum.name()`
    pub fn name(self) -> &'static str {
        match self {
            SurvivorType::Friendly => "Friendly",
            SurvivorType::Neutral => "Neutral",
            SurvivorType::Aggressive => "Aggressive",
        }
    }
}

/// The three static lists of zombie.characters.SurvivorFactory (`FemaleForenames`, `MaleForenames`, `Surnames`),
/// which the game's Lua fills on `OnGameBoot` through `addFemaleForename`, `addMaleForename` and `addSurname`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SurvivorFactory {
    pub female_forenames: Vec<String>,
    pub male_forenames: Vec<String>,
    pub surnames: Vec<String>,
}

impl SurvivorFactory {
    /// zombie.characters.SurvivorFactory.CreateSurvivor(): `switch (Rand.Next(3))` over the three types.
    pub fn create_survivor<R: JavaRandom>(
        &self,
        rand: &mut RandAbstract<R>,
        id_count: &mut i32,
    ) -> Result<SurvivorDesc, String> {
        let survivor_type = match rand.next(3) {
            0 => SurvivorType::Friendly,
            1 => SurvivorType::Neutral,
            2 => SurvivorType::Aggressive,
            _ => return Err("SurvivorFactory.CreateSurvivor: unreachable switch".into()),
        };
        self.create_survivor_of_type(survivor_type, rand, id_count)
    }

    /// zombie.characters.SurvivorFactory.CreateSurvivor(SurvivorType): the gender is `Rand.Next(2) == 0` (female),
    /// drawn before the descriptor is made.
    pub fn create_survivor_of_type<R: JavaRandom>(
        &self,
        survivor_type: SurvivorType,
        rand: &mut RandAbstract<R>,
        id_count: &mut i32,
    ) -> Result<SurvivorDesc, String> {
        let female = rand.next(2) == 0;
        self.create_survivor_with_gender(survivor_type, female, rand, id_count)
    }

    /// zombie.characters.SurvivorFactory.CreateSurvivor(SurvivorType, boolean). `IsoGameCharacter.getSurvivorMap().put`
    /// is the caller's registry, not modelled.
    pub fn create_survivor_with_gender<R: JavaRandom>(
        &self,
        survivor_type: SurvivorType,
        female: bool,
        rand: &mut RandAbstract<R>,
        id_count: &mut i32,
    ) -> Result<SurvivorDesc, String> {
        let mut survivor = SurvivorDesc::new(id_count, rand);
        survivor.survivor_type = survivor_type;
        survivor.female = female;
        self.random_name(&mut survivor, rand)?;
        // setTorso: both branches of the original's `if (survivor.isFemale())` set the torso by gender
        survivor.torso = if survivor.female { "Kate" } else { "Male" }.to_string();
        Ok(survivor)
    }

    /// zombie.characters.SurvivorFactory.randomName: the forename (from the gender's list) first, then the surname.
    /// `Err` where a list is empty (the original's `ArrayList.get(Rand.Next(0))` throws).
    pub fn random_name<R: JavaRandom>(
        &self,
        desc: &mut SurvivorDesc,
        rand: &mut RandAbstract<R>,
    ) -> Result<(), String> {
        desc.forename = self.get_random_forename(desc.female, rand)?;
        let i = rand.next(i32::try_from(self.surnames.len()).unwrap_or(i32::MAX));
        desc.surname = pick(&self.surnames, i, "Surnames")?;
        Ok(())
    }

    /// zombie.characters.SurvivorFactory.getRandomForename
    pub fn get_random_forename<R: JavaRandom>(
        &self,
        female: bool,
        rand: &mut RandAbstract<R>,
    ) -> Result<String, String> {
        let (list, what) = if female {
            (&self.female_forenames, "FemaleForenames")
        } else {
            (&self.male_forenames, "MaleForenames")
        };
        let i = rand.next(i32::try_from(list.len()).unwrap_or(i32::MAX));
        pick(list, i, what)
    }

    /// The effect of the game's `MainCreationMethods.lua` (`BaseGameCharacterDetails.DoSurname`,
    /// `DoFemaleForename`, `DoMaleForename`, run on `OnGameBoot`): every `SurvivorFactory.addSurname("X")`,
    /// `addFemaleForename("X")` and `addMaleForename("X")` call of the file, in file order, adds
    /// `Translator.getText("SurvivorSurname_X")` / `"SurvivorName_X"` to its list. STAND-IN for running that Lua on
    /// the ported VM: the calls are found by reading the file's lines; `translate` is `Translator.getText`.
    pub fn from_main_creation_methods(
        lua: &str,
        translate: &mut dyn FnMut(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        let mut f = SurvivorFactory::default();
        for line in lua.lines() {
            let t = line.trim();
            for (call, prefix, which) in [
                ("SurvivorFactory.addSurname(\"", "SurvivorSurname_", 0),
                ("SurvivorFactory.addFemaleForename(\"", "SurvivorName_", 1),
                ("SurvivorFactory.addMaleForename(\"", "SurvivorName_", 2),
            ] {
                let Some(rest) = t.strip_prefix(call) else {
                    continue;
                };
                let Some(end) = rest.find("\")") else {
                    return Err(format!("unterminated name call: {t}"));
                };
                let text = translate(&format!("{prefix}{}", &rest[..end]))?;
                match which {
                    0 => f.surnames.push(text),
                    1 => f.female_forenames.push(text),
                    _ => f.male_forenames.push(text),
                }
            }
        }
        Ok(f)
    }
}

fn pick(list: &[String], i: i32, what: &str) -> Result<String, String> {
    usize::try_from(i)
        .ok()
        .and_then(|i| list.get(i))
        .cloned()
        .ok_or_else(|| {
            format!(
                "{what}.get({i}): index out of range for size {}",
                list.len()
            )
        })
}
