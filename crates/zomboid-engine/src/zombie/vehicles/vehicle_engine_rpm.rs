//! zombie.vehicles.VehicleEngineRPM: a `vehicleEngineRPM` script, the RPM table of an engine type by gear.
use super::engine_rpm_data::EngineRPMData;
use crate::java;
use crate::zombie::core::math::pz_math;
use crate::zombie::core::translator::Translator;
use crate::zombie::scripting::objects::base_script_object::{
    first_child, BaseScriptObject, ScriptObject,
};
use crate::zombie::scripting::script_parser::{self, Block};

/// zombie.vehicles.VehicleEngineRPM.MAX_GEARS
pub const MAX_GEARS: usize = 8;

/// zombie.vehicles.VehicleEngineRPM
#[derive(Clone, Debug, PartialEq)]
pub struct VehicleEngineRPM {
    pub base: BaseScriptObject,
    /// VehicleEngineRPM.name
    pub name: Option<String>,
    /// VehicleEngineRPM.rpmData
    pub rpm_data: [Option<EngineRPMData>; MAX_GEARS],
}

/// `new RuntimeException(message)` as `toString()` gives it.
fn runtime_exception(message: String) -> String {
    format!("java.lang.RuntimeException: {message}")
}

impl VehicleEngineRPM {
    /// zombie.vehicles.VehicleEngineRPM()
    pub fn new(module: Option<&str>) -> VehicleEngineRPM {
        VehicleEngineRPM {
            base: BaseScriptObject::new(module),
            name: None,
            rpm_data: [None; MAX_GEARS],
        }
    }

    /// zombie.vehicles.VehicleEngineRPM.LoadData: `gearChange` and `afterGearChange` (`tryParseFloat(v, 0)`); any
    /// other value and any child block other than `xxx` throws.
    fn load_data(block: &Block, rpm_data: &mut EngineRPMData) -> Result<(), String> {
        for value in &block.values {
            let k = java::trim(value.get_key());
            let v = java::trim(value.get_value());
            if k == "afterGearChange" {
                rpm_data.after_gear_change = pz_math::try_parse_float(Some(v), 0.0);
            } else if k == "gearChange" {
                rpm_data.gear_change = pz_math::try_parse_float(Some(v), 0.0);
            } else {
                return Err(runtime_exception(format!(
                    "unknown value vehicleEngineRPM.data.{}",
                    value.string
                )));
            }
        }
        for child in &block.children {
            if child.r#type != "xxx" {
                return Err(runtime_exception(format!(
                    "unknown block vehicleEngineRPM.data.{}",
                    child.r#type
                )));
            }
        }
        Ok(())
    }
}

impl ScriptObject for VehicleEngineRPM {
    fn base(&self) -> &BaseScriptObject {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseScriptObject {
        &mut self.base
    }

    /// zombie.vehicles.VehicleEngineRPM.reset: the data already loaded is zeroed, not removed.
    fn reset(&mut self) {
        for data in self.rpm_data.iter_mut().flatten() {
            data.reset();
        }
    }

    /// zombie.vehicles.VehicleEngineRPM.Load: the name, the common block, the `VERSION` value (0 or 1; any other,
    /// or none, throws), then every child block must be a `data` block (at most eight) and is loaded into the next
    /// gear's slot. A throw leaves what was read so far.
    fn load(&mut self, name: &str, body: &str, _translator: &Translator) -> Result<(), String> {
        self.name = Some(name.to_string());
        let mut version = -1;
        let parsed = script_parser::parse(body);
        let block = first_child(&parsed)?;
        self.base.load_common_block(block);
        for value in &block.values {
            let k = java::trim(value.get_key());
            let v = java::trim(value.get_value());
            if k == "VERSION" {
                version = pz_math::try_parse_int(Some(v), -1);
                if !(0..=1).contains(&version) {
                    return Err(runtime_exception(format!(
                        "unknown vehicleEngineRPM VERSION \"{v}\""
                    )));
                }
            }
        }
        if version == -1 {
            return Err(runtime_exception(format!(
                "unknown vehicleEngineRPM VERSION \"{}\"",
                block.r#type
            )));
        }
        for (data_index, child) in block.children.iter().enumerate() {
            if child.r#type != "data" {
                return Err(runtime_exception(format!(
                    "unknown block vehicleEngineRPM.{}",
                    child.r#type
                )));
            }
            if data_index == MAX_GEARS {
                return Err(runtime_exception(format!(
                    "too many vehicleEngineRPM.data blocks, max is {MAX_GEARS}"
                )));
            }
            self.rpm_data[data_index] = Some(EngineRPMData::default());
            if let Some(data) = self.rpm_data[data_index].as_mut() {
                Self::load_data(child, data)?;
            }
        }
        Ok(())
    }
}
