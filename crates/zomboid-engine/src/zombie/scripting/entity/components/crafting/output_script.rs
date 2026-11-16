//! zombie.scripting.entity.components.crafting.OutputScript: one output line of a craft recipe
//! (`item 1 Base.Plank`, `item 50 mapper:ammoTypes`, `fluid 1.0 Water`, a `+` line).
use super::craft_recipe::CraftRecipe;
use crate::java;
use crate::zombie::core::math::pz_math;
use crate::zombie::entity::components::crafting::fluid_match_mode::FluidMatchMode;
use crate::zombie::entity::components::crafting::item_apply_mode::ItemApplyMode;
use crate::zombie::entity::components::crafting::output_flag::{OutputFlag, OutputFlags};
use crate::zombie::entity::components::crafting::recipe::output_mapper::OutputMapper;
use crate::zombie::entity::components::resources::resource_type::ResourceType;
use crate::zombie::util::string_utils;

/// The `outputMapper` field: one of the recipe's named mappers (`mapper:name`, shared with its inputs and other
/// outputs), or a mapper of the output's own (a plain item name).
#[derive(Clone, Debug, PartialEq)]
pub enum MapperRef {
    Recipe(String),
    Own(OutputMapper),
}

/// zombie.scripting.entity.components.crafting.OutputScript
#[derive(Clone, Debug, PartialEq)]
pub struct OutputScript {
    pub r#type: ResourceType,
    pub loaded_fluid: Option<String>,
    pub loaded_energy: Option<String>,
    pub amount: f32,
    pub maxamount: f32,
    pub chance: f32,
    pub apply_on_tick: bool,
    pub shaped_index: i32,
    pub item_apply_mode: ItemApplyMode,
    pub fluid_match_mode: FluidMatchMode,
    pub original_line: String,
    pub create_to_item_script: Option<usize>,
    pub flags: OutputFlags,
    pub output_mapper: Option<MapperRef>,
}

fn nfe(s: &str) -> String {
    format!(
        "java.lang.NumberFormatException: {}",
        java::number_format_message(s)
    )
}

fn parse_float(s: &str) -> Result<f32, String> {
    java::parse_float(s).ok_or_else(|| nfe(s))
}

impl OutputScript {
    /// zombie.scripting.entity.components.crafting.OutputScript(CraftRecipe, ResourceType): the field initialisers.
    fn new(r#type: ResourceType) -> Self {
        OutputScript {
            r#type,
            loaded_fluid: None,
            loaded_energy: None,
            amount: 0.0,
            maxamount: 0.0,
            chance: 1.0,
            apply_on_tick: false,
            shaped_index: -1,
            item_apply_mode: ItemApplyMode::Normal,
            fluid_match_mode: FluidMatchMode::Exact,
            original_line: String::new(),
            create_to_item_script: None,
            flags: OutputFlags::default(),
            output_mapper: None,
        }
    }

    /// zombie.scripting.entity.components.crafting.OutputScript.isVariableAmount
    pub fn is_variable_amount(&self) -> bool {
        self.amount != self.maxamount
    }

    /// zombie.scripting.entity.components.crafting.OutputScript.getIntAmount: `(int) amount`.
    pub fn get_int_amount(&self) -> i32 {
        self.amount as i32
    }

    /// The name of the output mapper (`OutputMapper.name`).
    pub fn mapper_name(&self) -> Option<&str> {
        match &self.output_mapper {
            Some(MapperRef::Recipe(n)) => Some(n),
            Some(MapperRef::Own(m)) => Some(&m.name),
            None => None,
        }
    }

    /// zombie.scripting.entity.components.crafting.OutputScript.Load(CraftRecipe, String, boolean). `Ok(None)` for a
    /// blank line; `Err` is the exception that drops the recipe. Core.debug is false (a dedicated server), so an
    /// internal item line whose result is not `Uses` and a mode on an energy line pass.
    pub fn load(
        parent: &CraftRecipe,
        line: &str,
        is_internal: bool,
    ) -> Result<Option<OutputScript>, String> {
        if string_utils::is_null_or_whitespace(Some(line)) {
            return Ok(None);
        }
        let elems: Vec<String> = java::split_whitespace_regex(java::trim(line))
            .into_iter()
            .map(|e| java::trim(e).to_string())
            .collect();
        let aioobe = |i: usize| {
            format!(
                "java.lang.ArrayIndexOutOfBoundsException: Index {i} out of bounds for length {}",
                elems.len()
            )
        };
        let eq = java::equals_ignore_case;
        let e0 = elems.first().ok_or_else(|| aioobe(0))?;
        let r#type = if eq(e0, "item") {
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
        let mut output = OutputScript::new(r#type);
        output.original_line = java::trim(line).to_string();
        let output_amount = elems.get(1).ok_or_else(|| aioobe(1))?;
        let (baseamount, upperamount);
        if output_amount.contains("variable") {
            let value_string = java::string_substring(
                output_amount,
                java::string_index_of(output_amount, "[") + 1,
                java::string_index_of(output_amount, "]"),
            )?;
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
            baseamount = pz_math::max(0.0, parse_float(output_amount)?);
            upperamount = baseamount;
        }
        output.amount = baseamount;
        output.maxamount = upperamount;
        let result = elems.get(2).ok_or_else(|| aioobe(2))?;
        match r#type {
            ResourceType::Item => {
                if !is_internal {
                    if result.starts_with("mapper:") {
                        let result = java::string_substring_from(
                            result,
                            java::string_index_of(result, ":") + 1,
                        )?;
                        if parent.get_output_mapper(&result).is_none() {
                            return Err(format!(
                                "java.lang.Exception: Could not find output mapper: {result}"
                            ));
                        }
                        output.output_mapper = Some(MapperRef::Recipe(result));
                    } else {
                        let mut m = OutputMapper::new(result);
                        m.set_default_output_entree(result);
                        output.output_mapper = Some(MapperRef::Own(m));
                    }
                }
            }
            ResourceType::Fluid => output.loaded_fluid = Some(result.clone()),
            ResourceType::Energy => output.loaded_energy = Some(result.clone()),
            ResourceType::Any => {}
        }
        for s in elems.iter().skip(3) {
            let after_colon =
                |s: &str| java::string_substring_from(s, java::string_index_of(s, ":") + 1);
            if s.starts_with("chance:") {
                let s = after_colon(s)?;
                output.chance = pz_math::clamp(parse_float(&s)?, 0.0, 1.0);
            } else if s.starts_with("shapedIndex:") {
                let s = after_colon(s)?;
                output.shaped_index = java::parse_int(&s).ok_or_else(|| nfe(&s))?;
            } else if s.starts_with("apply:") {
                if is_internal {
                    return Err(
                        "java.lang.Exception: Cannot apply 'onTick' on 'itemCreate' ('+' lines)."
                            .into(),
                    );
                }
                let s = after_colon(s)?;
                if !eq(&s, "onTick") {
                    return Err("java.lang.Exception: Apply Error".into());
                }
                if r#type == ResourceType::Item {
                    return Err("java.lang.Exception: Cannot apply 'onTick' on item.".into());
                }
                if r#type == ResourceType::Fluid {
                    return Err("java.lang.Exception: Cannot apply 'onTick' on fluid.".into());
                }
                output.apply_on_tick = true;
            } else if s.starts_with("mode:") {
                let s = after_colon(s)?;
                if r#type == ResourceType::Fluid {
                    output.fluid_match_mode = if eq(&s, "exact") {
                        FluidMatchMode::Exact
                    } else if eq(&s, "primary") {
                        FluidMatchMode::Primary
                    } else if eq(&s, "anything") {
                        FluidMatchMode::Anything
                    } else {
                        return Err("java.lang.Exception: Invalid fluid mode Error".into());
                    };
                }
            } else if s.starts_with("flags") {
                let s = java::string_substring(
                    s,
                    java::string_index_of(s, "[") + 1,
                    java::string_index_of(s, "]"),
                )?;
                for entry in java::split_char(&s, ';') {
                    output.flags.add(OutputFlag::value_of(entry)?);
                }
            } else {
                return Err(format!("java.lang.Exception: unknown recipe param: {s}"));
            }
        }
        Ok(Some(output))
    }
}
