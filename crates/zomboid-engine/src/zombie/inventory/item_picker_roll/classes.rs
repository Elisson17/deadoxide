//! The item classes the loot rolls create beyond the plain ones: what `Item.InstanceItem` draws for a `Key` (a padlock's
//! key id), `Radio` (the device's channel), `Moveable`, `AlarmClock` / `AlarmClockClothing` (the alarm) and `MapItem`
//! (the map id), and what `ItemPickerJava` does with a key it spawned.
use super::*;

/// What an `IAlarmClock` keeps that the oracle reads: `alarmHour`, `alarmMinutes` and `alarmSet`.
#[derive(Clone, Copy, Debug)]
pub struct AlarmState {
    pub hour: i32,
    pub minute: i32,
    pub set: bool,
}

impl LootRoller<'_> {
    /// `AlarmClock.randomizeAlarm`: `Rand.Next(0, 23)` hours, `Rand.Next(0, 59) / 10 * 10` minutes and
    /// `Rand.Next(6) == 1` for the alarm being set.
    pub(super) fn randomize_alarm_clock(&mut self) -> AlarmState {
        let hour = self.rand.next_range(0, 23);
        let minute = self.rand.next_range(0, 59) / 10 * 10;
        let set = self.rand.next(6) == 1;
        AlarmState { hour, minute, set }
    }

    /// `AlarmClockClothing.randomizeAlarm`: nothing outside `Core.lastStand` unless `isDigital()` (a full type with
    /// "Classic" in it is not); then the hours, minutes and `Rand.Next(15) == 1`.
    pub(super) fn randomize_alarm_clock_clothing(&mut self, full_type: &str) -> AlarmState {
        if full_type.contains("Classic") {
            // `alarmHour` and `alarmMinutes` start at -1, `alarmSet` false
            return AlarmState {
                hour: -1,
                minute: -1,
                set: false,
            };
        }
        let hour = self.rand.next_range(0, 23);
        let minute = self.rand.next_range(0, 59) / 10 * 10;
        let set = self.rand.next(15) == 1;
        AlarmState { hour, minute, set }
    }

    /// The `Radio` branch of `Item.InstanceItem`: `DeviceData.setMinChannelRange` / `setMaxChannelRange` (each outside
    /// 200..=1000000 falls back to its bound), `generatePresets` (a channel list per category from
    /// `ZomboidRadio.GetChannelList`; none is registered headless, so no preset and no draw) and `setRandomChannel`
    /// with no preset: `Rand.Next(min, max)` rounded down to a multiple of 200. UNKNOWN: the presets and the channel
    /// of a running game (its radio scripts register the channels).
    pub(super) fn radio_random_channel(&mut self, script: &Item) -> i32 {
        let bound = |v: i32, fallback: i32| {
            if (200..=1_000_000).contains(&v) {
                v
            } else {
                fallback
            }
        };
        let min = bound(script_int(script, "minChannel"), 200);
        let max = bound(script_int(script, "maxChannel"), 1_000_000);
        let channel = self.rand.next_range(min, max);
        channel - channel % 200
    }

    /// `Item.InstanceItem` for an `ItemType.MAP` script without a `Map` parameter:
    /// `MapDefinitions.getInstance().pickRandom()` is `PZArrayUtil.pickRandom` of the `LootMaps.Init` keys (a draw), or
    /// "Default" without a draw when the Lua environment defines none (the client Lua that defines `LootMaps` is not
    /// loaded headless).
    pub(super) fn map_pick_random(&mut self, script: &Item) {
        if script_str(script, "map").is_none_or(|m| java::trim(m).is_empty())
            && !self.map_definitions.is_empty()
        {
            self.rand.next(self.map_definitions.len() as i32);
        }
    }

    /// ItemPickerJava's handling of a key it spawned (`key.takeKeyId()` and the building's key bookkeeping for a
    /// `BuildingKey`, `addVehicleKeyAsLoot` for a car key), for the key at `idx` of `container`. Headless, no
    /// building has a `BuildingDef` and no vehicle exists, so a building key takes a random key id (`Rand.Next
    /// (10000000)`; `outContainer.getSourceGrid().getBuilding().def` is null) and is removed, and a car key is
    /// removed without a draw (`getVehicles()` is empty, or the container holds two car keys). True when the key
    /// is removed from the container.
    pub(super) fn spawned_key_removed(
        &mut self,
        script: &Item,
        full_type: &str,
        corpse: bool,
        it: &mut LootItem,
    ) -> bool {
        let mut removed = false;
        if has_tag(script, "BuildingKey") {
            if !corpse {
                it.key_id = self.rand.next(10_000_000);
            }
            removed = true;
        }
        if full_type == "Base.CarKey" || has_tag(script, "CarKey") {
            removed = true;
        }
        removed
    }
}
