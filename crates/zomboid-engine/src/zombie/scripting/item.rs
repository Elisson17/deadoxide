//! zombie.scripting.objects.Item: an `item` script. So far the parameters whose handling in `DoParam` is a plain typed
//! assignment (table generated from the original, `item_params_table.rs`); the other parameters are kept as text.
use super::item_params_table::{COMPLEX, FIELDS, PARAMS};
use super::objects::resource_location::ResourceLocation;
use super::script_parser::{self, BlockElement};
use crate::java;
use crate::zombie::util::string_utils;

#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
    F32(f32),
    I32(i32),
    Bool(bool),
    Str(Option<&'static str>),
}

/// How a `DoParam` branch assigns the (trimmed) value to its field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// `Float.parseFloat(val)`, `Integer.parseInt(val)`, `val`, `val.equalsIgnoreCase("true")` or
    /// `Boolean.parseBoolean(val)`, by the field's type.
    Set,
    /// `!val.equalsIgnoreCase("true")`
    Not,
    /// `StringUtils.discardNullOrWhitespace(val)`
    Discard,
    /// `Integer.parseInt(val)` into a float field.
    IntToFloat,
}

/// A field of the original class with its declared default.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldDef {
    pub name: &'static str,
    pub default: FieldValue,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    F32(f32),
    I32(i32),
    Bool(bool),
    Str(Option<String>),
}

/// zombie.scripting.objects.Item
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub module: String,
    pub name: String,
    pub enabled: bool,
    pub debug_only: bool,
    /// Values of `FIELDS`, same order.
    pub values: Vec<Value>,
    /// Parameters with non-trivial handling in the original (`COMPLEX`), as (lower-case name, value text), in script order.
    pub unported: Vec<(String, String)>,
    /// Parameters the original has no branch for at all.
    pub unknown: Vec<String>,
    /// `itemTags`: the locations of the `Tags` entries (`ItemTag.get(ResourceLocation.of(tag))`, a null for a
    /// location the ItemTag registry does not hold), in script order.
    pub tags: Vec<ResourceLocation>,
    /// The `component <Id> { ... }` blocks: the ids that are `ComponentType` names (GameEntityScript.LoadComponentBlock
    /// makes a component script for each; only which components the item has is kept), in script order.
    pub components: Vec<String>,
    /// The `component FluidContainer` blocks seen (a second one is not modelled: `fluid` holds the last).
    pub fluid_blocks: usize,
    /// What `FluidContainerScript` keeps of the last `component FluidContainer` block.
    pub fluid: Option<FluidScript>,
}

/// zombie.scripting.entity.components.fluids.FluidContainerScript: the fields that decide how a fresh item's
/// `FluidContainer.readFromScript` draws random numbers and how much it holds (after `OnScriptsLoaded`).
#[derive(Clone, Debug, PartialEq)]
pub struct FluidScript {
    /// `capacity`, at least `FluidUtil.getMinContainerCapacity()` (0.05).
    pub capacity: f32,
    pub initial_amount_set: bool,
    /// `initialAmountMin`/`Max`: swapped when out of order, clamped to 0..1 and scaled by `capacity`.
    pub initial_amount_min: f32,
    pub initial_amount_max: f32,
    pub initial_fluids_is_random: bool,
    /// `initialFluids.size()`: the `Fluid` values and `fluid` blocks of the `fluids` block.
    pub initial_fluids: usize,
    /// `FluidScript.percentage` of each initial fluid, in order (normalised to sum 1 unless the pick is random).
    pub initial_percentages: Vec<f32>,
    /// A `whitelist` or `blacklist` block is present.
    pub has_filter: bool,
}

impl FluidScript {
    /// FluidContainerScript's field defaults (`PreReload`).
    fn new() -> Self {
        FluidScript {
            capacity: 1.0,
            initial_amount_set: false,
            initial_amount_min: 0.0,
            initial_amount_max: 1.0,
            initial_fluids_is_random: false,
            initial_fluids: 0,
            initial_percentages: Vec::new(),
            has_filter: false,
        }
    }

    /// FluidContainerScript.load for the keys that matter here, then `OnScriptsLoaded`.
    fn load(block: &script_parser::Block) -> Result<Self, String> {
        let mut f = FluidScript::new();
        let float = |v: &str| {
            parse_float(v).ok_or_else(|| {
                format!("java.lang.NumberFormatException: For input string: \"{v}\"")
            })
        };
        for element in &block.elements {
            match element {
                BlockElement::Value(v) => {
                    if !java::trim(&v.string).is_empty() && v.string.contains('=') {
                        let (k, val) = (java::trim(v.get_key()), java::trim(v.get_value()));
                        if java::equals_ignore_case(k, "Capacity") {
                            f.capacity = float(val)?;
                        } else if java::equals_ignore_case(k, "InitialPercent") {
                            f.initial_amount_min = float(val)?;
                            f.initial_amount_max = f.initial_amount_min;
                            f.initial_amount_set = true;
                        } else if java::equals_ignore_case(k, "InitialPercentMin") {
                            f.initial_amount_min = float(val)?;
                            f.initial_amount_set = true;
                        } else if java::equals_ignore_case(k, "InitialPercentMax") {
                            f.initial_amount_max = float(val)?;
                            f.initial_amount_set = true;
                        } else if java::equals_ignore_case(k, "PickRandomFluid") {
                            f.initial_fluids_is_random = java::equals_ignore_case(val, "true");
                        }
                    }
                }
                BlockElement::Block(i) => {
                    let child = &block.children[*i];
                    if java::equals_ignore_case(&child.r#type, "fluids") {
                        // loadBlockFluids: each `Fluid` value, then each `fluid` block that has a `type`
                        for value in &child.values {
                            if java::equals_ignore_case(java::trim(value.get_key()), "Fluid") {
                                f.initial_fluids += 1;
                                // readFluid: `Type:percentage[:r:g:b]`
                                let parts = java::split_char(java::trim(value.get_value()), ':');
                                f.initial_percentages.push(if parts.len() > 1 {
                                    float(parts[1])?
                                } else {
                                    1.0
                                });
                            }
                        }
                        for fb in &child.children {
                            if java::equals_ignore_case(&fb.r#type, "Fluid")
                                && fb.values.iter().any(|v| {
                                    java::equals_ignore_case(java::trim(v.get_key()), "type")
                                })
                            {
                                f.initial_fluids += 1;
                                // readFluidAsBlock: the last `percentage` value
                                let mut pct = 1.0f32;
                                for v in &fb.values {
                                    if java::equals_ignore_case(
                                        java::trim(v.get_key()),
                                        "percentage",
                                    ) {
                                        pct = float(java::trim(v.get_value()))?;
                                    }
                                }
                                f.initial_percentages.push(pct);
                            }
                        }
                    } else if java::equals_ignore_case(&child.r#type, "whitelist")
                        || java::equals_ignore_case(&child.r#type, "blacklist")
                    {
                        f.has_filter = true;
                    }
                }
            }
        }
        // OnScriptsLoaded
        if !f.initial_fluids_is_random && !f.initial_percentages.is_empty() {
            // PZMath.normalize: the total summed from the last to the first, then each divided by it
            let mut total = 0.0f32;
            for p in f.initial_percentages.iter().rev() {
                total += *p;
            }
            if total != 0.0 {
                for p in f.initial_percentages.iter_mut().rev() {
                    *p /= total;
                }
            }
        }
        f.capacity = f.capacity.max(0.05);
        if f.initial_amount_min > f.initial_amount_max {
            std::mem::swap(&mut f.initial_amount_min, &mut f.initial_amount_max);
        }
        f.initial_amount_min = f.initial_amount_min.clamp(0.0, 1.0) * f.capacity;
        f.initial_amount_max = f.initial_amount_max.clamp(0.0, 1.0) * f.capacity;
        Ok(f)
    }
}

/// zombie.entity.ComponentType: the names `ComponentType.valueOf` accepts, without `Undefined` (a component block
/// of that type is warned about and skipped).
const COMPONENT_TYPES: [&str; 23] = [
    "Attributes",
    "FluidContainer",
    "SpriteConfig",
    "Lua",
    "Parts",
    "Signals",
    "Script",
    "UiConfig",
    "CraftLogic",
    "FurnaceLogic",
    "TestComponent",
    "MashingLogic",
    "DryingLogic",
    "MetaTag",
    "Resources",
    "CraftBench",
    "CraftRecipe",
    "Durability",
    "DryingCraftLogic",
    "ContextMenuConfig",
    "SpriteOverlayConfig",
    "CraftBenchSounds",
    "WallCoveringConfig",
];

/// java.lang.Float.parseFloat for decimal text (surrounding blanks and an f/d suffix allowed).
pub(crate) fn parse_float(s: &str) -> Option<f32> {
    let t = java::trim(s);
    let t = t.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(t);
    if t.is_empty()
        || !t
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.' | b'e' | b'E'))
    {
        return None;
    }
    t.parse::<f32>().ok()
}

/// Attribute names (`zombie.entity.components.attributes.Attribute`), which `DoParam` routes to the attribute
/// component before anything else. Lower case.
const ATTRIBUTES: [&str; 17] = [
    "testquality",
    "testuses",
    "testcondition",
    "testbool",
    "teststring",
    "teststring2",
    "testitemtype",
    "testcategories",
    "testtags",
    "sharpness",
    "headcondition",
    "headconditionmax",
    "timesheadrepaired",
    "quality",
    "originx",
    "originy",
    "originz",
];

impl Item {
    pub fn new(module: &str, name: &str) -> Self {
        let values = FIELDS
            .iter()
            .map(|f| match &f.default {
                FieldValue::F32(v) => Value::F32(*v),
                FieldValue::I32(v) => Value::I32(*v),
                FieldValue::Bool(v) => Value::Bool(*v),
                FieldValue::Str(v) => Value::Str(v.map(str::to_string)),
            })
            .collect();
        Item {
            module: module.to_string(),
            name: name.to_string(),
            enabled: true,
            debug_only: false,
            values,
            unported: Vec::new(),
            unknown: Vec::new(),
            tags: Vec::new(),
            components: Vec::new(),
            fluid_blocks: 0,
            fluid: None,
        }
    }

    /// zombie.scripting.entity.GameEntityScript.containsComponent for the component named `id`.
    pub fn contains_component(&self, id: &str) -> bool {
        self.components.iter().any(|c| c == id)
    }

    /// zombie.scripting.entity.GameEntityScript.hasComponents: a `component` block, or an attribute parameter (`Sharpness`,
    /// `HeadCondition`, ...), which `DoParam` routes to `LoadAttribute` and so to an Attributes component script.
    pub fn has_components(&self) -> bool {
        !self.components.is_empty()
            || self
                .unported
                .iter()
                .any(|(k, _)| ATTRIBUTES.contains(&k.as_str()))
    }

    /// `module.name`, the original's `moduleDotType`.
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.module, self.name)
    }

    pub fn get(&self, field: &str) -> Option<&Value> {
        FIELDS
            .iter()
            .position(|f| f.name == field)
            .map(|i| &self.values[i])
    }

    pub fn set(&mut self, field: &str, value: Value) {
        if let Some(i) = FIELDS.iter().position(|f| f.name == field) {
            self.values[i] = value;
        }
    }

    /// zombie.scripting.objects.Item.Load: every `param = value,` of the script body goes through `do_param`.
    /// `Err` is the exception that makes the original drop the whole item (a value without `=`, an unparsable number).
    pub fn load(&mut self, body: &str) -> Result<(), String> {
        let parsed = script_parser::parse(body);
        let block = parsed
            .children
            .first()
            .ok_or("java.lang.IndexOutOfBoundsException: no item block")?;
        for value in &block.values {
            let (key, val) = (java::trim(value.get_key()), java::trim(value.get_value()));
            if !key.is_empty() && !val.is_empty() {
                if java::equals_ignore_case(key, "enabled") {
                    self.enabled = java::equals_ignore_case(val, "true");
                } else if java::equals_ignore_case(key, "debugOnly") {
                    self.debug_only = java::equals_ignore_case(val, "true");
                }
            }
        }
        for element in &block.elements {
            let v = match element {
                BlockElement::Value(v) => v,
                // Item.Load: `child.type.equalsIgnoreCase("component")` -> LoadComponentBlock(child)
                BlockElement::Block(i) => {
                    let child = &block.children[*i];
                    if java::equals_ignore_case(&child.r#type, "component") {
                        if let Some(id) = child.id.as_deref() {
                            if COMPONENT_TYPES.contains(&id) {
                                self.components.push(id.to_string());
                                if id == "FluidContainer" {
                                    self.fluid_blocks += 1;
                                    self.fluid = Some(FluidScript::load(child)?);
                                }
                            }
                        }
                    }
                    continue;
                }
            };
            if java::trim(&v.string).is_empty() {
                continue;
            }
            // java.lang.String.split("="): pieces between '=' signs, trailing empty pieces dropped.
            let mut p: Vec<&str> = v.string.split('=').collect();
            while p.last().is_some_and(|x| x.is_empty()) {
                p.pop();
            }
            if p.len() < 2 {
                return Err(format!(
                    "java.lang.ArrayIndexOutOfBoundsException: \"{}\"",
                    java::trim(&v.string)
                ));
            }
            self.do_param(java::trim(p[0]), java::trim(p[1]))?;
        }
        Ok(())
    }

    /// zombie.scripting.objects.Item.DoParam (String, String) for the tabulated parameters.
    pub fn do_param(&mut self, param: &str, val: &str) -> Result<(), String> {
        let lower = param.to_lowercase();
        if ATTRIBUTES.contains(&lower.as_str()) {
            self.unported.push((lower, val.to_string()));
            return Ok(());
        }
        if lower == "tags" {
            // `val.split(";")`, each trimmed: a bad location throws (the item fails to load)
            for t in java::split_char(val, ';') {
                self.tags.push(ResourceLocation::of(java::trim(t))?);
            }
        }
        let Some(&(_, i, op)) = PARAMS.iter().find(|p| p.0 == lower) else {
            if COMPLEX.contains(&lower.as_str()) {
                self.unported.push((lower, val.to_string()));
            } else {
                self.unknown.push(param.to_string());
            }
            return Ok(());
        };
        let nfe =
            || format!("java.lang.NumberFormatException: For input string: \"{val}\" ({param})");
        self.values[i] = match (&FIELDS[i].default, op) {
            (FieldValue::F32(_), Op::IntToFloat) => {
                Value::F32(java::parse_int(val).ok_or_else(nfe)? as f32)
            }
            (FieldValue::F32(_), _) => Value::F32(parse_float(val).ok_or_else(nfe)?),
            (FieldValue::I32(_), _) => Value::I32(java::parse_int(val).ok_or_else(nfe)?),
            (FieldValue::Bool(_), Op::Not) => Value::Bool(!java::equals_ignore_case(val, "true")),
            (FieldValue::Bool(_), _) => Value::Bool(java::equals_ignore_case(val, "true")),
            (FieldValue::Str(_), Op::Discard) if string_utils::is_null_or_whitespace(Some(val)) => {
                Value::Str(None)
            }
            (FieldValue::Str(_), _) => Value::Str(Some(val.to_string())),
        };
        Ok(())
    }
}
