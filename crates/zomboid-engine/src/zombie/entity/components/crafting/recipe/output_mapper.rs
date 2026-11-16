//! zombie.entity.components.crafting.recipe.OutputMapper: the item an output line produces, by default or by the
//! items the registered input lines used. Items are held by their full names (`Item.getFullName()`), input scripts by
//! their index in the recipe's `ioLines`.
use crate::zombie::util::string_utils;

/// zombie.entity.components.crafting.recipe.OutputMapper.OutputEntree
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OutputEntree {
    pub full_type: String,
    /// `result`: the item `OnPostWorldDictionaryInit` resolved.
    pub result: Option<String>,
    pub pattern_full_types: Vec<String>,
    pub pattern: Vec<String>,
}

/// zombie.entity.components.crafting.recipe.OutputMapper
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OutputMapper {
    pub result_items: Vec<String>,
    pub entrees: Vec<OutputEntree>,
    /// `entreeMap` (HashMap<Item, OutputEntree>): result item -> index into `entrees`, `put` replacing.
    pub entree_map: Vec<(String, usize)>,
    pub default_output_entree: Option<OutputEntree>,
    pub input_scripts: Vec<usize>,
    pub name: String,
}

/// What `OutputMapper.getItem` asks of the ScriptManager.
pub struct Lookups<'a> {
    /// ScriptManager.getItem: the item's full name.
    pub get_item: &'a dyn Fn(&str) -> Option<String>,
    /// `ScriptManager.getCraftRecipe(recipe) != null`
    pub has_craft_recipe: &'a dyn Fn(&str) -> bool,
}

impl OutputMapper {
    /// zombie.entity.components.crafting.recipe.OutputMapper(String)
    pub fn new(name: &str) -> Self {
        OutputMapper {
            name: name.to_string(),
            ..OutputMapper::default()
        }
    }

    /// zombie.entity.components.crafting.recipe.OutputMapper.isEmpty
    pub fn is_empty(&self) -> bool {
        self.default_output_entree.is_none() && self.entrees.is_empty()
    }

    /// zombie.entity.components.crafting.recipe.OutputMapper.registerInputScript
    pub fn register_input_script(&mut self, input_script: usize) {
        self.input_scripts.push(input_script);
    }

    /// zombie.entity.components.crafting.recipe.OutputMapper.setDefaultOutputEntree
    pub fn set_default_output_entree(&mut self, item: &str) {
        self.default_output_entree = Some(OutputEntree {
            full_type: item.to_string(),
            ..OutputEntree::default()
        });
    }

    /// zombie.entity.components.crafting.recipe.OutputMapper.addOutputEntree(String, String[])
    pub fn add_output_entree(&mut self, result: &str, items: &[String]) {
        self.entrees.push(OutputEntree {
            full_type: result.to_string(),
            pattern_full_types: items.to_vec(),
            ..OutputEntree::default()
        });
    }

    /// zombie.entity.components.crafting.recipe.OutputMapper.getItem(String, String): the item, or the exception.
    /// With a recipe that `getCraftRecipe` finds, the original also marks the item as a craft product and, when the
    /// recipe `canBeResearched()`, adds the recipe to the item's researchable recipes; those item fields are not
    /// ported, but `canBeResearched()` itself changes the recipe (`getResearchSkillLevel`), so the recipe's name is
    /// pushed to `research` for the caller to run it.
    fn get_item(
        full_type: &str,
        recipe: Option<&str>,
        lookups: &Lookups,
        research: &mut Vec<String>,
    ) -> Result<String, String> {
        let recipe_text = recipe.unwrap_or("null");
        if string_utils::is_null_or_whitespace(Some(full_type)) {
            return Err(format!(
                "java.lang.Exception: {recipe_text} item not found, full type invalid: {full_type}"
            ));
        }
        let Some(item) = (lookups.get_item)(full_type) else {
            return Err(format!(
                "java.lang.Exception: {recipe_text} item not found: {full_type}"
            ));
        };
        if let Some(r) = recipe {
            if (lookups.has_craft_recipe)(r) {
                research.push(r.to_string());
            }
        }
        Ok(item)
    }

    /// zombie.entity.components.crafting.recipe.OutputMapper.OnPostWorldDictionaryInit(String): resolves the default
    /// entree, then every entree and its pattern (pattern items without the recipe), in order; the first item that is
    /// not found throws, leaving what was resolved before it.
    pub fn on_post_world_dictionary_init(
        &mut self,
        recipe: Option<&str>,
        lookups: &Lookups,
        research: &mut Vec<String>,
    ) -> Result<(), String> {
        if let Some(d) = &mut self.default_output_entree {
            let item = Self::get_item(&d.full_type, recipe, lookups, research)?;
            d.result = Some(item.clone());
            self.result_items.push(item);
        }
        for i in 0..self.entrees.len() {
            let item = Self::get_item(&self.entrees[i].full_type, recipe, lookups, research)?;
            self.entrees[i].result = Some(item.clone());
            self.result_items.push(item.clone());
            match self.entree_map.iter_mut().find(|(k, _)| *k == item) {
                Some(e) => e.1 = i,
                None => self.entree_map.push((item, i)),
            }
            for j in 0..self.entrees[i].pattern_full_types.len() {
                let p = Self::get_item(
                    &self.entrees[i].pattern_full_types[j],
                    None,
                    lookups,
                    research,
                )?;
                self.entrees[i].pattern.push(p);
            }
        }
        Ok(())
    }
}
