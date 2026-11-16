//! zombie.characters.skills.PerkFactory: the perks (PerkFactory.Perks' constants, by id) and what `init()` gives
//! them: translation key, translated name, parent, the ten xp thresholds (times 1.5) and `passiv`; PerkList in
//! `AddPerk` order.
use crate::zombie::core::translator::Translator;

/// The ids of PerkFactory.Perks' constants (`new Perk(id)`), in declaration order: PerkById's keys.
pub const PERK_IDS: [&str; 47] = [
    "None",
    "Agility",
    "Cooking",
    "Melee",
    "Crafting",
    "Fitness",
    "Strength",
    "Blunt",
    "Axe",
    "Lightfoot",
    "Nimble",
    "Sprinting",
    "Sneak",
    "Woodwork",
    "Aiming",
    "Reloading",
    "Farming",
    "Survivalist",
    "Fishing",
    "Trapping",
    "Passiv",
    "Firearm",
    "PlantScavenging",
    "Doctor",
    "Electricity",
    "Blacksmith",
    "MetalWelding",
    "Melting",
    "Mechanics",
    "Spear",
    "Maintenance",
    "SmallBlade",
    "LongBlade",
    "SmallBlunt",
    "Combat",
    "Tailoring",
    "Tracking",
    "Husbandry",
    "FlintKnapping",
    "Masonry",
    "Pottery",
    "Carving",
    "Butchering",
    "Glassmaking",
    "FarmingCategory",
    "PhysicalCategory",
    "MAX",
];

/// One `AddPerk(perk, translation, [parent,] xp1..xp10[, passiv])` call of PerkFactory.init.
pub struct AddPerk {
    pub perk: &'static str,
    pub translation: &'static str,
    pub parent: &'static str,
    pub xp: [i32; 10],
    pub passiv: bool,
}

/// PerkFactory.init's calls, in order.
pub const INIT: [AddPerk; 42] = [
    AddPerk {
        perk: "Combat",
        translation: "CombatMelee",
        parent: "None",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Axe",
        translation: "Axe",
        parent: "Combat",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Blunt",
        translation: "Blunt",
        parent: "Combat",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "SmallBlunt",
        translation: "SmallBlunt",
        parent: "Combat",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "LongBlade",
        translation: "LongBlade",
        parent: "Combat",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "SmallBlade",
        translation: "SmallBlade",
        parent: "Combat",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Spear",
        translation: "Spear",
        parent: "Combat",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Maintenance",
        translation: "Maintenance",
        parent: "Combat",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Firearm",
        translation: "CombatFirearms",
        parent: "None",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Aiming",
        translation: "Aiming",
        parent: "Firearm",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Reloading",
        translation: "Reloading",
        parent: "Firearm",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Crafting",
        translation: "Crafting",
        parent: "None",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Woodwork",
        translation: "Carpentry",
        parent: "Crafting",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Carving",
        translation: "Carving",
        parent: "Crafting",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Cooking",
        translation: "Cooking",
        parent: "Crafting",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Electricity",
        translation: "Electricity",
        parent: "Crafting",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Doctor",
        translation: "Doctor",
        parent: "Survivalist",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Glassmaking",
        translation: "Glassmaking",
        parent: "Crafting",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "FlintKnapping",
        translation: "FlintKnapping",
        parent: "Crafting",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Masonry",
        translation: "Masonry",
        parent: "Crafting",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Blacksmith",
        translation: "Blacksmith",
        parent: "Crafting",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Mechanics",
        translation: "Mechanics",
        parent: "Crafting",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Pottery",
        translation: "Pottery",
        parent: "Crafting",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Tailoring",
        translation: "Tailoring",
        parent: "Crafting",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "MetalWelding",
        translation: "MetalWelding",
        parent: "Crafting",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Survivalist",
        translation: "Survivalist",
        parent: "None",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Fishing",
        translation: "Fishing",
        parent: "Survivalist",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "PlantScavenging",
        translation: "Foraging",
        parent: "Survivalist",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Tracking",
        translation: "Tracking",
        parent: "Survivalist",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Trapping",
        translation: "Trapping",
        parent: "Survivalist",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "PhysicalCategory",
        translation: "PhysicalCategory",
        parent: "None",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Fitness",
        translation: "Fitness",
        parent: "PhysicalCategory",
        xp: [
            1000, 2000, 4000, 6000, 12000, 20000, 40000, 60000, 80000, 100000,
        ],
        passiv: true,
    },
    AddPerk {
        perk: "Strength",
        translation: "Strength",
        parent: "PhysicalCategory",
        xp: [
            1000, 2000, 4000, 6000, 12000, 20000, 40000, 60000, 80000, 100000,
        ],
        passiv: true,
    },
    AddPerk {
        perk: "Agility",
        translation: "Agility",
        parent: "None",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Lightfoot",
        translation: "Lightfooted",
        parent: "PhysicalCategory",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Nimble",
        translation: "Nimble",
        parent: "PhysicalCategory",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Sprinting",
        translation: "Sprinting",
        parent: "PhysicalCategory",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Sneak",
        translation: "Sneaking",
        parent: "PhysicalCategory",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "FarmingCategory",
        translation: "FarmingCategory",
        parent: "None",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Farming",
        translation: "Farming",
        parent: "FarmingCategory",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Husbandry",
        translation: "Husbandry",
        parent: "FarmingCategory",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
    AddPerk {
        perk: "Butchering",
        translation: "Butchering",
        parent: "FarmingCategory",
        xp: [50, 100, 200, 500, 1000, 2000, 3000, 4000, 5000, 6000],
        passiv: false,
    },
];

/// A perk after `init()` (PerkFactory.Perk's fields that AddPerk sets).
#[derive(Clone, Debug, PartialEq)]
pub struct Perk {
    pub id: &'static str,
    pub translation: &'static str,
    pub name: String,
    pub parent: &'static str,
    pub passiv: bool,
    pub xp: [i32; 10],
}

/// PerkFactory's statics after `init()`: PerkList.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PerkFactory {
    pub perk_list: Vec<Perk>,
}

impl PerkFactory {
    /// zombie.characters.skills.PerkFactory.init: AddPerk for each perk, its name `Translator.getText("IGUI_perks_"
    /// + translation)` and each xp threshold `(int)(xp * 1.5F)`; PerkList in call order.
    pub fn init(translator: &mut Translator) -> Result<PerkFactory, String> {
        let mut perk_list = Vec::with_capacity(INIT.len());
        for a in INIT.iter() {
            let name = translator.get_text(&format!("IGUI_perks_{}", a.translation), &[])?;
            perk_list.push(Perk {
                id: a.perk,
                translation: a.translation,
                name,
                parent: a.parent,
                passiv: a.passiv,
                xp: a.xp.map(|x| (x as f32 * 1.5f32) as i32),
            });
        }
        Ok(PerkFactory { perk_list })
    }

    /// PerkFactory.Perk.getName: the translated name after `init()`, else the id (the constructor's `name = id`).
    pub fn get_name<'a>(&'a self, id: &'a str) -> &'a str {
        self.perk_list
            .iter()
            .find(|p| p.id == id)
            .map_or(id, |p| p.name.as_str())
    }

    /// PerkFactory.Perks.FromString: `PerkById.getOrDefault(id, MAX)`.
    pub fn from_string(id: &str) -> &'static str {
        PERK_IDS
            .iter()
            .find(|p| **p == id)
            .copied()
            .unwrap_or("MAX")
    }
}
