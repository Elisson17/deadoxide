//! `zombie.inventory.types.Clothing.randomizeCondition` as the loot rolls run it headless.
//!
//! With no game window the `ClothingItem` assets are never ready, so `InventoryItem.getVisual()` returns null and
//! the first `getVisual().setDirt/setBlood/getHole(...)` throws a `NullPointerException` after its arguments (the
//! part draw and the level roll) were evaluated. The original's probe shows that exception, so the port reproduces
//! it: the error value `EXC java.lang.NullPointerException` ends the fill exactly where Java's does. A running game
//! has the visuals and does not throw: UNKNOWN what the dirt, blood and holes it then sets are (not compared).
use super::*;

/// The error a fill ends with where the original throws `java.lang.NullPointerException`.
pub(super) const NPE: &str = "EXC java.lang.NullPointerException";

/// `BloodClothingType.coveredParts.size()` of each constant (`Apron`, `ShirtNoSleeves`, ... as the enum builds them).
fn covered_part_count(name: &str) -> Option<usize> {
    Some(match name {
        "Apron" => 4,
        "ShirtNoSleeves" | "JumperNoSleeves" => 3,
        "Shirt" => 5,
        "ShirtLongSleeves" | "Jumper" => 7,
        "Jacket" => 8,
        "LongJacket" => 11,
        "ShortsShort" => 3,
        "Trousers" => 5,
        "Shoes" | "Hands" | "LowerLegs" | "UpperLegs" | "LowerArms" | "UpperArms" => 2,
        "FullHelmet" | "Bag" | "Head" | "Neck" | "Groin" | "UpperBody" | "LowerBody"
        | "Hand_L" | "Hand_R" | "ForeArm_L" | "ForeArm_R" | "UpperArm_L" | "UpperArm_R"
        | "UpperLeg_L" | "UpperLeg_R" | "LowerLeg_L" | "LowerLeg_R" | "Foot_L" | "Foot_R" => 1,
        _ => return None,
    })
}

impl LootRoller<'_> {
    /// `Clothing.getNbrOfCoveredParts` = `BloodClothingType.getCoveredPartCount(scriptItem.getBloodClothingType())`:
    /// the sum over the `BloodLocation` entries (none without the parameter).
    fn nbr_of_covered_parts(script: &Item) -> Result<i32, String> {
        let Some(v) = script_complex(script, "bloodlocation") else {
            return Ok(0);
        };
        let mut count = 0usize;
        for name in java::split_char(v, ';') {
            // a name `BloodClothingType.fromString` does not know is a null entry: getCoveredPartCount throws
            let Some(n) = covered_part_count(java::trim(&name)) else {
                return Err(NPE.to_string());
            };
            count += n;
        }
        Ok(count as i32)
    }

    /// Clothing.randomizeCondition(wetChance, dirtChance, bloodChance, holeChance)
    pub(super) fn clothing_randomize_condition(
        &mut self,
        it: &mut LootItem,
        wet_chance: i32,
        dirt_chance: i32,
        blood_chance: i32,
        hole_chance: i32,
    ) -> Result<(), String> {
        let script = self.script(it.script)?;
        // `if (!this.isCosmetic())`
        if script_bool(script, "cosmetic") {
            return Ok(());
        }
        if self.rand.next(100) < wet_chance {
            // setWetness(Rand.Next(0, 100)): the wetness is not compared
            self.rand.next_range(0, 100);
        }
        let parts = Self::nbr_of_covered_parts(script)?;
        if parts >= 1 {
            // addRandomDirt / addRandomBlood: `parts.get(Rand.Next(parts.size()))`, `Rand.Next(100) + 1`, then
            // `getVisual().setDirt(...)` with no visual
            if self.rand.next(100) < dirt_chance {
                self.rand.next(parts);
                self.rand.next(100);
                return Err(NPE.to_string());
            }
            if self.rand.next(100) < blood_chance {
                self.rand.next(parts);
                self.rand.next(100);
                return Err(NPE.to_string());
            }
            if self.rand.next(100) < hole_chance {
                if script_bool(script, "canHaveHoles") {
                    let rolls = self.rand.next(parts + 1);
                    if rolls > 0 {
                        // addRandomHole: `Rand.Next(parts.size())`, then `getVisual().getHole(part)`
                        self.rand.next(parts);
                        return Err(NPE.to_string());
                    }
                } else {
                    // setCondition(Rand.Next(getCondition()) + 1, false)
                    let max = script_int(script, "conditionMax");
                    let v = self.rand.next(it.condition) + 1;
                    it.condition = v.clamp(0, max);
                }
            }
        }
        Ok(())
    }
}
