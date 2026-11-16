//! zombie.characters.traits.CharacterTraitDefinition and the script that makes them
//! (zombie.scripting.objects.CharacterTraitDefinitionScript): `characterTraitDefinitions`, a LinkedHashMap keyed by
//! the trait.
use std::cell::RefCell;

use super::super::skills::perk_factory::PerkFactory;
use crate::java;
use crate::zombie::core::math::pz_math;
use crate::zombie::core::translator::Translator;
use crate::zombie::scripting::objects::resource_location::ResourceLocation;
use crate::zombie::scripting::script_parser::{self, BlockElement};

/// A trait of the CharacterTrait registry (`None`: `Registries.CHARACTER_TRAIT.get` found nothing, a null).
pub type TraitRef = Option<ResourceLocation>;

/// zombie.characters.traits.CharacterTraitDefinition. The texture (`Texture.getSharedTexture` of
/// `media/ui/Traits/trait_<name>.png`, else `trait_generic.png`) is not kept.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterTraitDefinition {
    pub character_trait_type: ResourceLocation,
    pub display_name: String,
    pub cost: i32,
    /// `setDescription` changes it.
    pub description: RefCell<Option<String>>,
    pub is_profession_trait: bool,
    pub disabled_in_multiplayer: bool,
    pub granted_traits: Vec<TraitRef>,
    pub granted_recipes: Vec<String>,
    pub mutually_exclusive_traits: Vec<TraitRef>,
    /// The HashMap<Perk, Integer> of `addXPBoost`, as (perk id, level) in `put` order (a repeated perk keeps its
    /// place and takes the new level). The original's iteration order follows the perks' identity hashes.
    pub xp_boosts: Vec<(&'static str, i32)>,
}

/// CharacterTraitDefinition.characterTraitDefinitions, in insertion order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CharacterTraitDefinitions {
    pub definitions: Vec<CharacterTraitDefinition>,
    /// Scripts that threw while loading (name, exception), logged by the original.
    pub errors: Vec<(String, String)>,
}

/// `Boolean.parseBoolean`
fn parse_boolean(v: &str) -> bool {
    v.eq_ignore_ascii_case("true")
}

/// The `key = value` pairs of a definition block, as its loader reads them: `split("=", 2)`, both parts trimmed,
/// pairs with an empty side skipped.
pub(crate) fn block_pairs(body: &str) -> Result<Vec<(String, String)>, String> {
    let root = script_parser::parse(body);
    let block = root
        .children
        .first()
        .ok_or("java.lang.IndexOutOfBoundsException: Index 0 out of bounds for length 0")?;
    let mut pairs = Vec::new();
    for e in &block.elements {
        let BlockElement::Value(v) = e else { continue };
        let Some((k, v)) = v.string.split_once('=') else {
            continue;
        };
        let (k, v) = (java::trim(k), java::trim(v));
        if !k.is_empty() && !v.is_empty() {
            pairs.push((k.to_string(), v.to_string()));
        }
    }
    Ok(pairs)
}

/// `java.lang.String.split(";")`
pub(crate) fn split_semicolons(v: &str) -> Vec<&str> {
    java::split_char(v, ';')
}

/// XPBoosts: each `perk=level` pair whose perk `PerkFactory.Perks.FromString` knows, the level
/// `PZMath.tryParseInt(level, 1)`. `limit_two`: the profession loader's `split("=", 2)`; the trait loader's
/// `split("=")` (both read only the first two parts).
pub(crate) fn xp_boosts(v: &str, limit_two: bool) -> Vec<(&'static str, i32)> {
    let mut out = Vec::new();
    for s in split_semicolons(v) {
        let pair: Vec<&str> = if limit_two {
            s.splitn(2, '=').collect()
        } else {
            java::split_char(s, '=')
        };
        if pair.len() >= 2 {
            let perk = PerkFactory::from_string(java::trim(pair[0]));
            if perk != "MAX" {
                out.push((perk, pz_math::try_parse_int(Some(pair[1]), 1)));
            }
        }
    }
    out
}

/// `Translator.getText(name)` of a definition's UIName; a script without one is not ported.
pub(crate) fn ui_name(translator: &mut Translator, name: Option<&str>) -> Result<String, String> {
    let name = name.ok_or("not ported: Translator.getText(null) of a definition without UIName")?;
    translator.get_text(name, &[])
}

/// `HashMap.put` into the (perk, level) list.
pub(crate) fn put_xp_boost(map: &mut Vec<(&'static str, i32)>, perk: &'static str, level: i32) {
    match map.iter_mut().find(|(p, _)| *p == perk) {
        Some(e) => e.1 = level,
        None => map.push((perk, level)),
    }
}

impl CharacterTraitDefinitions {
    /// ScriptBucket.LoadScripts for the character_trait_definition bucket: each script (in the bucket's HashMap
    /// order of names) through CharacterTraitDefinitionScript.Load. `trait_of` is the registry lookup
    /// `Registries.CHARACTER_TRAIT.get(ResourceLocation.of(id))`. A script that throws is logged and adds nothing.
    pub fn load(
        scripts: &[(String, String)],
        trait_of: &dyn Fn(&ResourceLocation) -> bool,
        translator: &mut Translator,
    ) -> CharacterTraitDefinitions {
        let mut defs = CharacterTraitDefinitions::default();
        for (name, body) in scripts {
            if let Err(e) = defs.load_one(body, trait_of, translator) {
                defs.errors.push((name.clone(), e));
            }
        }
        defs
    }

    /// CharacterTraitDefinitionScript.loadCharacterTraitDefinition (after LoadCommonBlock, which reads nothing these
    /// scripts set).
    fn load_one(
        &mut self,
        body: &str,
        trait_of: &dyn Fn(&ResourceLocation) -> bool,
        translator: &mut Translator,
    ) -> Result<(), String> {
        let lookup = |v: &str| -> Result<TraitRef, String> {
            let l = ResourceLocation::of(v)?;
            Ok(trait_of(&l).then_some(l))
        };
        let pairs = block_pairs(body)?;
        let mut trait_type: TraitRef = None;
        let (mut ui_name, mut ui_description) = (None, None);
        let mut cost = 0;
        let (mut profession, mut disabled_in_multiplayer) = (false, false);
        let mut xp = Vec::new();
        let mut exclusive = Vec::new();
        let mut recipes = Vec::new();
        let mut granted = Vec::new();
        for (k, v) in pairs {
            let is = |s: &str| k.eq_ignore_ascii_case(s);
            if is("CharacterTrait") {
                trait_type = lookup(&v)?;
            } else if is("UIName") {
                ui_name = Some(v);
            } else if is("Cost") {
                cost = java::parse_int(&v).ok_or_else(|| {
                    format!("java.lang.NumberFormatException: For input string: \"{v}\"")
                })?;
            } else if is("UIDescription") {
                ui_description = Some(v);
            } else if is("IsProfessionTrait") {
                profession = parse_boolean(&v);
            } else if is("DisabledInMultiplayer") {
                disabled_in_multiplayer = parse_boolean(&v);
            } else if is("GrantedRecipes") {
                recipes.extend(split_semicolons(&v).into_iter().map(str::to_string));
            } else if is("GrantedTraits") {
                for s in split_semicolons(&v) {
                    granted.push(lookup(java::trim(s))?);
                }
            } else if is("XPBoosts") {
                xp.extend(xp_boosts(&v, false));
            } else if is("MutuallyExclusiveTraits") {
                for s in split_semicolons(&v) {
                    exclusive.push(lookup(java::trim(s))?);
                }
            }
            // Texture: the texture is not kept; an unknown key is logged (DebugType.Script.error), and with
            // Core.debug false nothing is thrown
        }
        // new CharacterTraitDefinition: characterTraitType.getName() on a null trait is a NullPointerException
        let trait_type = trait_type.ok_or("java.lang.NullPointerException")?;
        let display_name = self::ui_name(translator, ui_name.as_deref())?;
        let description = match ui_description.filter(|d| !d.is_empty()) {
            Some(d) => Some(translator.get_text(&d, &[])?),
            None => None,
        };
        let def = CharacterTraitDefinition {
            character_trait_type: trait_type.clone(),
            display_name,
            cost,
            description: RefCell::new(description),
            is_profession_trait: profession,
            disabled_in_multiplayer,
            granted_traits: Vec::new(),
            granted_recipes: Vec::new(),
            mutually_exclusive_traits: Vec::new(),
            xp_boosts: Vec::new(),
        };
        // characterTraitDefinitions.put: a known trait keeps its place
        let index = match self
            .definitions
            .iter()
            .position(|d| d.character_trait_type == trait_type)
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
        for (perk, level) in xp {
            put_xp_boost(&mut self.definitions[index].xp_boosts, perk, level);
        }
        self.definitions[index]
            .mutually_exclusive_traits
            .extend(exclusive);
        self.definitions[index].granted_recipes.extend(recipes);
        for t in granted {
            self.definitions[index].granted_traits.push(t);
            self.update_mutual_exclusive_traits(index);
        }
        Ok(())
    }

    /// CharacterTraitDefinition.getCharacterTraitDefinition
    pub fn get(&self, t: &ResourceLocation) -> Option<usize> {
        self.definitions
            .iter()
            .position(|d| &d.character_trait_type == t)
    }

    /// CharacterTraitDefinition.isMutuallyExclusive(other) of definition `a`.
    fn is_mutually_exclusive(&self, a: usize, other: usize) -> bool {
        let other_type = Some(self.definitions[other].character_trait_type.clone());
        let d = &self.definitions[a];
        if d.mutually_exclusive_traits.contains(&other_type) {
            return true;
        }
        d.granted_traits.iter().any(|g| {
            g.as_ref().and_then(|g| self.get(g)).is_some_and(|g| {
                self.definitions[g]
                    .mutually_exclusive_traits
                    .contains(&other_type)
            })
        })
    }

    /// CharacterTraitDefinition.updateMutualExclusiveTraits of definition `a`.
    fn update_mutual_exclusive_traits(&mut self, a: usize) {
        for b in 0..self.definitions.len() {
            if a != b && (self.is_mutually_exclusive(a, b) || self.is_mutually_exclusive(b, a)) {
                let bt = Some(self.definitions[b].character_trait_type.clone());
                if !self.definitions[a].mutually_exclusive_traits.contains(&bt) {
                    self.definitions[a].mutually_exclusive_traits.push(bt);
                }
                let at = Some(self.definitions[a].character_trait_type.clone());
                if !self.definitions[b].mutually_exclusive_traits.contains(&at) {
                    self.definitions[b].mutually_exclusive_traits.push(at);
                }
            }
        }
    }
}
