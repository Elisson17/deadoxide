//! zombie.ZomboidGlobals: the character tuning numbers the game reads from the Lua table `ZomboidGlobals`
//! (`media/lua/shared/defines.lua`) once the scripts are loaded (GameWindow.init, after LuaManager.LoadDirBase).
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::object::{Object, TableRef, Throwable, R};

/// The fields Load reads: (Java field, key in the Lua table), in Load's order.
pub const LOADED: [(&str, &str); 36] = [
    ("sprintingEnduranceReduce", "SprintingEnduranceReduce"),
    ("runningEnduranceReduce", "RunningEnduranceReduce"),
    ("imobileEnduranceReduce", "ImobileEnduranceIncrease"),
    ("thirstIncrease", "ThirstIncrease"),
    ("thirstSleepingIncrease", "ThirstSleepingIncrease"),
    ("thirstLevelToAutoDrink", "ThirstLevelToAutoDrink"),
    (
        "thirstLevelReductionOnAutoDrink",
        "ThirstLevelReductionOnAutoDrink",
    ),
    ("hungerIncrease", "HungerIncrease"),
    ("hungerIncreaseWhenWellFed", "HungerIncreaseWhenWellFed"),
    ("hungerIncreaseWhileAsleep", "HungerIncreaseWhileAsleep"),
    ("hungerIncreaseWhenExercise", "HungerIncreaseWhenExercise"),
    ("fatigueIncrease", "FatigueIncrease"),
    ("stressReduction", "StressDecrease"),
    ("boredomIncreaseRate", "BoredomIncrease"),
    ("boredomDecreaseRate", "BoredomDecrease"),
    ("unhappinessIncrease", "UnhappinessIncrease"),
    ("stressFromSoundsMultiplier", "StressFromSoundsMultiplier"),
    ("stressFromBiteOrScratch", "StressFromBiteOrScratch"),
    ("stressFromHemophobic", "StressFromHemophobic"),
    ("stressFromDiscomfort", "StressFromDiscomfort"),
    ("angerDecrease", "AngerDecrease"),
    ("sleepFatigueReduction", "SleepFatigueReduction"),
    ("wetnessIncrease", "WetnessIncrease"),
    ("wetnessDecrease", "WetnessDecrease"),
    ("catchAColdIncreaseRate", "CatchAColdIncreaseRate"),
    ("catchAColdDecreaseRate", "CatchAColdDecreaseRate"),
    ("poisonLevelDecrease", "PoisonLevelDecrease"),
    ("poisonHealthReduction", "PoisonHealthReduction"),
    ("foodSicknessDecrease", "FoodSicknessDecrease"),
    ("idleIncreaseRate", "IdleIncrease"),
    ("idleDecreaseRate", "IdleDecrease"),
    ("cleanBloodBleachAmount", "CleanBloodBleachAmount"),
    (
        "cleanStainCleaningFluidAmount",
        "CleanStainCleaningFluidAmount",
    ),
    (
        "vehicleDiscomfortWhenOverEncumbered",
        "VehicleDiscomfortWhenOverEncumbered",
    ),
    (
        "equippedOrWornEncumbranceMultiplier",
        "EquippedOrWornEncumbranceMultiplier",
    ),
    (
        "corpseSicknessFilterDrainMultiplier",
        "CorpseSicknessFilterDrainMultiplier",
    ),
];

/// zombie.ZomboidGlobals' static fields: the doubles Load sets (by Java field name, 0.0 until then) and the
/// constants it does not touch.
#[derive(Clone, Debug, PartialEq)]
pub struct ZomboidGlobals {
    /// LOADED's fields, in its order.
    pub loaded: [f64; 36],
    /// sittingEnduranceMultiplier = 5.0
    pub sitting_endurance_multiplier: f64,
    /// refillBlowtorchPropaneAmount = 70.0
    pub refill_blowtorch_propane_amount: f64,
    /// basicMaskDefense = 25.0F
    pub basic_mask_defense: f32,
    /// improvisedGasMaskDefense = 85.0F
    pub improvised_gas_mask_defense: f32,
    /// standardGasMaskDefense = 100.0F
    pub standard_gas_mask_defense: f32,
}

impl Default for ZomboidGlobals {
    /// The static initialisers of zombie.ZomboidGlobals.
    fn default() -> Self {
        ZomboidGlobals {
            loaded: [0.0; 36],
            sitting_endurance_multiplier: 5.0,
            refill_blowtorch_propane_amount: 70.0,
            basic_mask_defense: 25.0,
            improvised_gas_mask_defense: 85.0,
            standard_gas_mask_defense: 100.0,
        }
    }
}

impl ZomboidGlobals {
    /// zombie.ZomboidGlobals.Load: `(Double)globals.rawget(key)` for each field, in order. A missing table or
    /// value is a NullPointerException (the cast table, the unboxing), a value of another type a
    /// ClassCastException (their JDK messages are not reproduced: GameWindow.init stops the start there); the
    /// fields read before stay set.
    pub fn load(&mut self, env: &TableRef) -> R<()> {
        let globals = match KahluaTableImpl::rawget(env, &Object::str("ZomboidGlobals"))? {
            Object::Table(t) => t,
            Object::Nil => return Err(exception("java.lang.NullPointerException")),
            _ => return Err(exception("java.lang.ClassCastException")),
        };
        for (i, (_, key)) in LOADED.iter().enumerate() {
            self.loaded[i] = match KahluaTableImpl::rawget(&globals, &Object::str(key))? {
                Object::Number(d) => d,
                Object::Nil => return Err(exception("java.lang.NullPointerException")),
                _ => return Err(exception("java.lang.ClassCastException")),
            };
        }
        Ok(())
    }

    /// The loaded double named `java_field` (`ZomboidGlobals.thirstIncrease`, ...). A name `Load` does not set is a
    /// programming error: it answers NaN so the oracle diff shows it.
    pub fn field(&self, java_field: &str) -> f64 {
        LOADED
            .iter()
            .position(|(f, _)| *f == java_field)
            .map_or(f64::NAN, |i| self.loaded[i])
    }

    /// Every static double/float field by Java name with its raw bits as hex (`Double.doubleToRawLongBits`,
    /// `Float.floatToRawIntBits`), sorted by name (String order; the names are ASCII).
    pub fn fields(&self) -> Vec<(&'static str, String)> {
        let mut r: Vec<(&'static str, String)> = LOADED
            .iter()
            .zip(self.loaded.iter())
            .map(|((f, _), v)| (*f, format!("{:x}", v.to_bits())))
            .collect();
        r.push((
            "sittingEnduranceMultiplier",
            format!("{:x}", self.sitting_endurance_multiplier.to_bits()),
        ));
        r.push((
            "refillBlowtorchPropaneAmount",
            format!("{:x}", self.refill_blowtorch_propane_amount.to_bits()),
        ));
        r.push((
            "basicMaskDefense",
            format!("{:x}", self.basic_mask_defense.to_bits()),
        ));
        r.push((
            "improvisedGasMaskDefense",
            format!("{:x}", self.improvised_gas_mask_defense.to_bits()),
        ));
        r.push((
            "standardGasMaskDefense",
            format!("{:x}", self.standard_gas_mask_defense.to_bits()),
        ));
        r.sort_by(|a, b| a.0.cmp(b.0));
        r
    }
}

fn exception(class: &'static str) -> Throwable {
    Throwable::Java {
        class,
        message: None,
    }
}
