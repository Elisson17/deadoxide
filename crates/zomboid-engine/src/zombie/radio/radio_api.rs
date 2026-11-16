//! zombie.radio.RadioAPI: the time stamp arithmetic the radio scripts use (the Lua-facing `getChannels` is not ported).

/// zombie.radio.RadioAPI.timeToTimeStamp
pub fn time_to_time_stamp(days: i32, hours: i32, minutes: i32) -> i32 {
    days.wrapping_mul(24)
        .wrapping_add(hours.wrapping_mul(60))
        .wrapping_add(minutes)
}

/// zombie.radio.RadioAPI.timeStampToDays
pub fn time_stamp_to_days(stamp: i32) -> i32 {
    stamp / 1440
}

/// zombie.radio.RadioAPI.timeStampToHours
pub fn time_stamp_to_hours(stamp: i32) -> i32 {
    stamp / 60 % 24
}

/// zombie.radio.RadioAPI.timeStampToMinutes
pub fn time_stamp_to_minutes(stamp: i32) -> i32 {
    stamp % 60
}
