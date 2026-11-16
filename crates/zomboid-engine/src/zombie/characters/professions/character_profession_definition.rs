//! zombie.characters.professions.CharacterProfessionDefinition and the script that makes them
//! (zombie.scripting.objects.CharacterProfessionDefinitionScript): `characterProfessionDefinitions`, a LinkedHashMap
//! keyed by the profession.
use std::cell::RefCell;

use super::super::traits::character_trait_definition::{
    block_pairs, put_xp_boost, split_semicolons, xp_boosts, TraitRef,
};
use crate::java;
use crate::zombie::core::translator::Translator;
use crate::zombie::scripting::objects::resource_location::ResourceLocation;

/// zombie.characters.professions.CharacterProfessionDefinition. The texture (`Texture.trygetTexture` of the icon
/// path) is not kept.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterProfessionDefinition {
    pub character_profession_type: ResourceLocation,
    pub display_name: String,
    pub cost: i32,
    /// "" unless the script names one; `setDescription` changes it.
    pub description: RefCell<String>,
    pub icon_path_name: Option<String>,
    pub granted_traits: Vec<TraitRef>,
    pub granted_recipes: Vec<String>,
    /// HashMap<Perk, Integer> as (perk id, level) in `put` order (see CharacterTraitDefinition::xp_boosts).
    pub xp_boosts: Vec<(&'static str, i32)>,
}

/// CharacterProfessionDefinition.characterProfessionDefinitions, in insertion order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CharacterProfessionDefinitions {
    pub definitions: Vec<CharacterProfessionDefinition>,
    /// Scripts that threw while loading (name, exception), logged by the original.
    pub errors: Vec<(String, String)>,
}

impl CharacterProfessionDefinitions {
    /// ScriptBucket.LoadScripts for the character_profession_definition bucket: each script (in the bucket's HashMap
    /// order of names) through CharacterProfessionDefinitionScript.Load. `profession_of` and `trait_of` are the
    /// registry lookups. A script that throws is logged and adds nothing.
    pub fn load(
        scripts: &[(String, String)],
        profession_of: &dyn Fn(&ResourceLocation) -> bool,
        trait_of: &dyn Fn(&ResourceLocation) -> bool,
        translator: &mut Translator,
    ) -> CharacterProfessionDefinitions {
        let mut defs = CharacterProfessionDefinitions::default();
        for (name, body) in scripts {
            if let Err(e) = defs.load_one(body, profession_of, trait_of, translator) {
                defs.errors.push((name.clone(), e));
            }
        }
        defs
    }

    /// CharacterProfessionDefinitionScript.loadCharacterProfessionDefinition
    fn load_one(
        &mut self,
        body: &str,
        profession_of: &dyn Fn(&ResourceLocation) -> bool,
        trait_of: &dyn Fn(&ResourceLocation) -> bool,
        translator: &mut Translator,
    ) -> Result<(), String> {
        let pairs = block_pairs(body)?;
        let mut profession: Option<ResourceLocation> = None;
        let (mut ui_name, mut ui_description, mut icon) = (None, None, None);
        let mut cost = 0;
        let mut granted = Vec::new();
        let mut recipes = Vec::new();
        let mut xp = Vec::new();
        for (k, v) in pairs {
            let is = |s: &str| k.eq_ignore_ascii_case(s);
            if is("CharacterProfession") {
                let l = ResourceLocation::of(&v)?;
                profession = profession_of(&l).then_some(l);
            } else if is("UIName") {
                ui_name = Some(v);
            } else if is("Cost") {
                cost = java::parse_int(&v).ok_or_else(|| {
                    format!("java.lang.NumberFormatException: For input string: \"{v}\"")
                })?;
            } else if is("UIDescription") {
                ui_description = Some(v);
            } else if is("GrantedRecipes") {
                recipes.extend(split_semicolons(&v).into_iter().map(str::to_string));
            } else if is("GrantedTraits") {
                for s in split_semicolons(&v) {
                    let l = ResourceLocation::of(java::trim(s))?;
                    granted.push(trait_of(&l).then_some(l));
                }
            } else if is("XPBoosts") {
                xp.extend(xp_boosts(&v, true));
            } else if is("IconPathName") {
                icon = Some(v);
            }
            // an unknown key is logged (DebugType.Script.error); Core.debug is false, so nothing is thrown
        }
        // characterProfessionDefinitions.put(null, ...) keeps a null key; the vanilla scripts all name a profession
        let profession = profession.ok_or("not ported: a profession the registry does not know")?;
        let display_name = super::super::traits::character_trait_definition::ui_name(
            translator,
            ui_name.as_deref(),
        )?;
        let description = match ui_description.filter(|d| !d.is_empty()) {
            Some(d) => translator.get_text(&d, &[])?,
            None => String::new(),
        };
        let def = CharacterProfessionDefinition {
            character_profession_type: profession.clone(),
            display_name,
            cost,
            description: RefCell::new(description),
            icon_path_name: icon,
            granted_traits: Vec::new(),
            granted_recipes: Vec::new(),
            xp_boosts: Vec::new(),
        };
        let index = match self
            .definitions
            .iter()
            .position(|d| d.character_profession_type == profession)
        {
            Some(i) => {
                self.definitions[i] = def;
                i
            }
            None => {
                self.definitions.push(def);
                self.definitions.len() - 1
            }
        };
        let d = &mut self.definitions[index];
        for (perk, level) in xp {
            put_xp_boost(&mut d.xp_boosts, perk, level);
        }
        d.granted_recipes.extend(recipes);
        d.granted_traits.extend(granted);
        Ok(())
    }
}
