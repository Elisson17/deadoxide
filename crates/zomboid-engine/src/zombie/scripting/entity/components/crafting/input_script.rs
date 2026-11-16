//! zombie.scripting.entity.components.crafting.InputScript: one input line of a craft recipe
//! (`item 2 [Base.Nails;Base.Screws] mode:keep flags[Prop1]`, `fluid 0.5 [Water]`, a `-` line).
//! Other io lines are referred to by their index in the recipe's `ioLines`.
use super::craft_recipe::CraftRecipe;
use crate::java;
use crate::zombie::core::math::pz_math;
use crate::zombie::entity::components::crafting::fluid_match_mode::FluidMatchMode;
use crate::zombie::entity::components::crafting::input_flag::{InputFlag, InputFlags};
use crate::zombie::entity::components::crafting::item_apply_mode::ItemApplyMode;
use crate::zombie::entity::components::resources::resource_type::ResourceType;
use crate::zombie::scripting::objects::item_tag;
use crate::zombie::scripting::objects::resource_location::ResourceLocation;
use crate::zombie::util::string_utils;

/// zombie.scripting.entity.components.crafting.InputScript
#[derive(Clone, Debug, PartialEq)]
pub struct InputScript {
    pub loaded_items: Vec<String>,
    pub loaded_fluids: Vec<String>,
    pub loaded_energies: Vec<String>,
    /// `itemTags` (HashSet<ItemTag>): each registered tag once, `None` for the null of an unregistered one; insertion
    /// order (the original's iteration order follows identity hashes).
    pub item_tags: Vec<Option<ResourceLocation>>,
    pub categories: Vec<String>,
    pub accepts_any_item: bool,
    pub accepts_any_fluid: bool,
    pub accepts_any_energy: bool,
    pub r#type: ResourceType,
    pub item_apply_mode: ItemApplyMode,
    pub fluid_match_mode: FluidMatchMode,
    pub amount: f32,
    pub maxamount: f32,
    pub amounts: Vec<f32>,
    pub maxamounts: Vec<f32>,
    pub apply_on_tick: bool,
    pub shaped_index: i32,
    pub original_line: String,
    pub create_to_item_script: Option<usize>,
    pub consume_from_item_script: Option<usize>,
    pub parent_script: Option<usize>,
    pub flags: InputFlags,
}

fn nfe(s: &str) -> String {
    format!(
        "java.lang.NumberFormatException: {}",
        java::number_format_message(s)
    )
}

/// `s.substring(s.indexOf("[") + 1, s.indexOf("]"))`
fn bracketed(s: &str) -> Result<String, String> {
    java::string_substring(
        s,
        java::string_index_of(s, "[") + 1,
        java::string_index_of(s, "]"),
    )
}

/// `s.substring(s.indexOf(":") + 1)`
fn after_colon(s: &str) -> Result<String, String> {
    java::string_substring_from(s, java::string_index_of(s, ":") + 1)
}

/// `Float.parseFloat(s)`
fn parse_float(s: &str) -> Result<f32, String> {
    java::parse_float(s).ok_or_else(|| nfe(s))
}

impl InputScript {
    /// zombie.scripting.entity.components.crafting.InputScript(CraftRecipe, ResourceType): the field initialisers.
    fn new(r#type: ResourceType) -> Self {
        InputScript {
            loaded_items: Vec::new(),
            loaded_fluids: Vec::new(),
            loaded_energies: Vec::new(),
            item_tags: Vec::new(),
            categories: Vec::new(),
            accepts_any_item: false,
            accepts_any_fluid: false,
            accepts_any_energy: false,
            r#type,
            item_apply_mode: ItemApplyMode::Normal,
            fluid_match_mode: FluidMatchMode::Exact,
            amount: 0.0,
            maxamount: 0.0,
            amounts: Vec::new(),
            maxamounts: Vec::new(),
            apply_on_tick: false,
            shaped_index: -1,
            original_line: String::new(),
            create_to_item_script: None,
            consume_from_item_script: None,
            parent_script: None,
            flags: InputFlags::default(),
        }
    }

    /// zombie.scripting.entity.components.crafting.InputScript.isVariableAmount
    pub fn is_variable_amount(&self) -> bool {
        self.amount != self.maxamount
    }

    /// zombie.scripting.entity.components.crafting.InputScript.getIntAmount: `(int) amount`.
    pub fn get_int_amount(&self) -> i32 {
        self.amount as i32
    }

    /// zombie.scripting.entity.components.crafting.InputScript.isTool
    pub fn is_tool(&self) -> bool {
        self.flags.contains(InputFlag::TOOL_RIGHT) || self.flags.contains(InputFlag::TOOL_LEFT)
    }

    /// zombie.scripting.entity.components.crafting.InputScript.Load(CraftRecipe, String, boolean): parses one line
    /// into a new input of `parent`, registering it (as the io line it is about to become, `index`) with the output
    /// mappers and the overlay mapper it names. `Ok(None)` for a blank line; `Err` is the exception that drops the
    /// recipe.
    pub fn load(
        parent: &mut CraftRecipe,
        line: &str,
        is_internal: bool,
        index: usize,
    ) -> Result<Option<InputScript>, String> {
        if string_utils::is_null_or_whitespace(Some(line)) {
            return Ok(None);
        }
        let elems: Vec<String> = java::split_whitespace_regex(java::trim(line))
            .into_iter()
            .map(|e| java::trim(e).to_string())
            .collect();
        let eq = java::equals_ignore_case;
        let e0 = elems.first().ok_or(
            "java.lang.ArrayIndexOutOfBoundsException: Index 0 out of bounds for length 0",
        )?;
        let r#type = if eq(e0, "item") || (is_internal && eq(e0, "uses")) {
            ResourceType::Item
        } else if eq(e0, "fluid") {
            ResourceType::Fluid
        } else if eq(e0, "energy") {
            ResourceType::Energy
        } else {
            return Err(format!(
                "java.lang.Exception: unknown type in craftrecipe: {e0}"
            ));
        };
        let mut input = InputScript::new(r#type);
        input.original_line = java::trim(line).to_string();
        let input_amount = elems.get(1).ok_or(
            "java.lang.ArrayIndexOutOfBoundsException: Index 1 out of bounds for length 1",
        )?;
        let (baseamount, upperamount);
        if input_amount.contains("variable") {
            let value_string = bracketed(input_amount)?;
            let values = java::split_char(&value_string, ':');
            let base = values.first().ok_or(
                "java.lang.ArrayIndexOutOfBoundsException: Index 0 out of bounds for length 0",
            )?;
            baseamount = pz_math::max(0.0, parse_float(base)?);
            let upper = values.get(1).ok_or_else(|| {
                format!(
                    "java.lang.ArrayIndexOutOfBoundsException: Index 1 out of bounds for length {}",
                    values.len()
                )
            })?;
            upperamount = pz_math::max(0.0, parse_float(upper)?);
        } else {
            baseamount = pz_math::max(0.0, parse_float(input_amount)?);
            upperamount = baseamount;
        }
        input.amount = baseamount;
        input.maxamount = upperamount;
        if is_internal {
            // all three branches of the original set acceptsAnyItem
            input.accepts_any_item = true;
        }
        for s in elems.iter().skip(2) {
            if s.starts_with('[') {
                let s = bracketed(s)?;
                for entry in java::split_char(&s, ';') {
                    match r#type {
                        ResourceType::Item => {
                            if entry == "*" {
                                input.accepts_any_item = true;
                            } else if entry.contains(':') {
                                let split2 = java::split_char(entry, ':');
                                let item = split2.get(1).ok_or_else(|| {
                                    format!("java.lang.ArrayIndexOutOfBoundsException: Index 1 out of bounds for length {}", split2.len())
                                })?;
                                input.loaded_items.push(item.to_string());
                                let itembaseamount = pz_math::max(0.0, parse_float(split2[0])?);
                                input.amounts.push(itembaseamount);
                                input
                                    .maxamounts
                                    .push(itembaseamount * (upperamount / baseamount));
                            } else {
                                input.loaded_items.push(entry.to_string());
                                input.amounts.push(baseamount);
                                input.maxamounts.push(upperamount);
                            }
                        }
                        ResourceType::Fluid => {
                            if entry == "*" {
                                input.accepts_any_fluid = true;
                            } else {
                                input.loaded_fluids.push(entry.to_string());
                                input.amounts.push(baseamount);
                                input.maxamounts.push(upperamount);
                            }
                        }
                        ResourceType::Energy => {
                            if entry == "*" {
                                input.accepts_any_energy = true;
                            } else {
                                input.loaded_energies.push(entry.to_string());
                                input.amounts.push(baseamount);
                                input.maxamounts.push(upperamount);
                            }
                        }
                        ResourceType::Any => {}
                    }
                }
            } else if s.starts_with("shapedIndex:") {
                let s = after_colon(s)?;
                input.shaped_index = java::parse_int(&s).ok_or_else(|| nfe(&s))?;
            } else if s.starts_with("apply:") {
                after_colon(s)?;
                return Err("java.lang.Exception: OnTick currently disabled for inputs.".into());
            } else if s.starts_with("mode:") {
                let s = after_colon(s)?;
                match r#type {
                    ResourceType::Item => {
                        if eq(&s, "use") {
                        } else if eq(&s, "keep") {
                            input.item_apply_mode = ItemApplyMode::Keep;
                        } else if eq(&s, "destroy") {
                            input.item_apply_mode = ItemApplyMode::Destroy;
                        } else if [
                            "useprop1",
                            "useprop2",
                            "keepprop1",
                            "keepprop2",
                            "prop1",
                            "prop2",
                        ]
                        .iter()
                        .any(|d| eq(&s, d))
                        {
                            // "Deprecated parameter in inputscript!" is only logged
                        } else {
                            return Err(format!(
                                "java.lang.Exception: Invalid item mode Error - {s}"
                            ));
                        }
                    }
                    ResourceType::Fluid => {
                        input.fluid_match_mode = if eq(&s, "exact") {
                            FluidMatchMode::Exact
                        } else if eq(&s, "primary") {
                            FluidMatchMode::Primary
                        } else if eq(&s, "mixture") {
                            FluidMatchMode::Mixture
                        } else if eq(&s, "anything") {
                            FluidMatchMode::Anything
                        } else {
                            return Err("java.lang.Exception: Invalid fluid mode Error".into());
                        };
                    }
                    ResourceType::Energy => {
                        if !eq(&s, "keep") {
                            return Err("java.lang.Exception: Invalid energy mode Error".into());
                        }
                        input.item_apply_mode = ItemApplyMode::Keep;
                    }
                    // "Cannot set mode" is only warned about (Core.debug is false)
                    ResourceType::Any => {}
                }
            } else if s.starts_with("tags") {
                if r#type != ResourceType::Item {
                    return Err(format!(
                        "java.lang.Exception: cannot set tags on non-item: {s}"
                    ));
                }
                let s = bracketed(s)?;
                for entry in java::split_char(&s, ';') {
                    let tag = item_tag::get(&ResourceLocation::of(entry)?);
                    if !input.item_tags.contains(&tag) {
                        input.item_tags.push(tag);
                    }
                }
            } else if s.starts_with("categories") {
                if r#type != ResourceType::Fluid {
                    return Err(format!(
                        "java.lang.Exception: cannot set category on non-fluid: {s}"
                    ));
                }
                let s = bracketed(s)?;
                input
                    .categories
                    .extend(java::split_char(&s, ';').into_iter().map(str::to_string));
            } else if s.starts_with("flags") {
                let s = bracketed(s)?;
                for entry in java::split_char(&s, ';') {
                    input.flags.add(InputFlag::value_of(entry)?);
                }
            } else if s.starts_with("mappers") {
                let s = bracketed(s)?;
                for entry in java::split_char(&s, ';') {
                    parent
                        .get_or_create_output_mapper(entry)
                        .register_input_script(index);
                }
            } else if s.starts_with("overlayMapper") {
                parent.overlay_mapper.register_input_script(index);
            } else {
                return Err(format!("java.lang.Exception: unknown recipe param: {s}"));
            }
        }
        if input.amounts.len() > 1
            && !input.is_variable_amount()
            && input.loaded_fluids.is_empty()
            && distinct_count(&input.amounts) > 1
        {
            input.amount = 1.0;
            input.maxamount = 1.0;
        }
        if input.accepts_any_item {
            input.loaded_items.clear();
            input.item_tags.clear();
        }
        if input.accepts_any_fluid {
            input.loaded_fluids.clear();
            input.categories.clear();
        }
        if input.accepts_any_energy {
            input.loaded_energies.clear();
        }
        Ok(Some(input))
    }
}

/// `amounts.stream().distinct().count()`: Float.equals compares `floatToIntBits` (one NaN, -0.0 apart from 0.0).
fn distinct_count(amounts: &[f32]) -> usize {
    let bits = |f: f32| if f.is_nan() { 0x7fc0_0000 } else { f.to_bits() };
    let mut seen: Vec<u32> = Vec::new();
    for a in amounts {
        if !seen.contains(&bits(*a)) {
            seen.push(bits(*a));
        }
    }
    seen.len()
}
