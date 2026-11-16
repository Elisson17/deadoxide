//! The host's stand-in for the game's `media/lua/server/Climate/ClimateMain.lua`, whose `OnClimateManagerInit`
//! handler the original runs when a ClimateManager is constructed (`LuaEventManager.triggerEvent`): it sets the
//! night, fog and cloud colours and the season colours.
//!
//! The ported Lua VM does not expose ClimateManager, ClimateColorInfo or WeatherPeriod to scripts yet (UNKNOWN:
//! running the real handler there is not done). So this reads the player's own file and replays exactly the
//! statement forms that file uses, and fails on any other statement instead of guessing: the numbers come from the
//! game's file at run time, none are kept in the source. Lua numbers are doubles; Kahlua converts them to the
//! float parameters with a `(float)` cast, as here.
use super::climate_color_info::ClimateColorInfo;
use super::climate_manager::ClimateManager;

/// Which colour object a local variable (`c`) points at.
#[derive(Clone, Copy)]
enum Target {
    NightNoMoon,
    NightMoon,
    Fog,
    FogLegacy,
    FogNew,
    FogTintStorm,
    FogTintTropical,
    CloudReddish,
    CloudGreenish,
    CloudBlueish,
    CloudPurplish,
    CloudTropical,
    CloudBlizzard,
}

fn color_mut(cm: &mut ClimateManager, t: Target) -> &mut ClimateColorInfo {
    match t {
        Target::NightNoMoon => &mut cm.col_night_no_moon,
        Target::NightMoon => &mut cm.col_night_moon,
        Target::Fog => &mut cm.col_fog,
        Target::FogLegacy => &mut cm.col_fog_legacy,
        Target::FogNew => &mut cm.col_fog_new,
        Target::FogTintStorm => &mut cm.fog_tint_storm,
        Target::FogTintTropical => &mut cm.fog_tint_tropical,
        Target::CloudReddish => &mut cm.weather_period.cloud_color_reddish,
        Target::CloudGreenish => &mut cm.weather_period.cloud_color_greenish,
        Target::CloudBlueish => &mut cm.weather_period.cloud_color_blueish,
        Target::CloudPurplish => &mut cm.weather_period.cloud_color_purplish,
        Target::CloudTropical => &mut cm.weather_period.cloud_color_tropical,
        Target::CloudBlizzard => &mut cm.weather_period.cloud_color_blizzard,
    }
}

/// `_clim:getXxx()` on the manager.
fn manager_getter(name: &str) -> Option<Target> {
    Some(match name {
        "getColNightNoMoon" => Target::NightNoMoon,
        "getColNightMoon" => Target::NightMoon,
        "getColFog" => Target::Fog,
        "getColFogLegacy" => Target::FogLegacy,
        "getColFogNew" => Target::FogNew,
        "getFogTintStorm" => Target::FogTintStorm,
        "getFogTintTropical" => Target::FogTintTropical,
        _ => return None,
    })
}

/// `w:getXxx()` on the weather period.
fn period_getter(name: &str) -> Option<Target> {
    Some(match name {
        "getCloudColorReddish" => Target::CloudReddish,
        "getCloudColorGreenish" => Target::CloudGreenish,
        "getCloudColorBlueish" => Target::CloudBlueish,
        "getCloudColorPurplish" => Target::CloudPurplish,
        "getCloudColorTropical" => Target::CloudTropical,
        "getCloudColorBlizzard" => Target::CloudBlizzard,
        _ => return None,
    })
}

/// The text of a call's argument list (between the parentheses) split at commas.
fn args(s: &str) -> Result<Vec<&str>, String> {
    let open = s.find('(').ok_or("no '('")?;
    let close = s.rfind(')').ok_or("no ')'")?;
    Ok(s[open + 1..close].split(',').map(str::trim).collect())
}

/// One argument: a number, or one of the file's local constants.
fn value(s: &str, consts: &[(String, f64)]) -> Result<f64, String> {
    if let Some((_, v)) = consts.iter().find(|(n, _)| n == s) {
        return Ok(*v);
    }
    s.parse::<f64>().map_err(|e| format!("{s}: {e}"))
}

/// Replays the `OnClimateManagerInit` handler of `ClimateMain.lua` (`text`) on `cm`.
pub fn apply_climate_main(cm: &mut ClimateManager, text: &str) -> Result<(), String> {
    let mut consts: Vec<(String, f64)> = Vec::new();
    let mut c: Option<Target> = None;
    for raw in text.lines() {
        let line = match raw.find("--") {
            Some(i) if !raw.trim_start().starts_with("--[[") => &raw[..i],
            _ => raw,
        };
        let line = line.trim().trim_end_matches(';').trim();
        if line.is_empty() || line.starts_with("--") || line == "end" || line == "local c" {
            continue;
        }
        if line.starts_with("function ClimateMain.onClimateManagerInit")
            || line.starts_with("ClimateMain")
            || line.starts_with("Events.OnClimateManagerInit.Add")
        {
            continue;
        }
        if let Some(rest) = line.strip_prefix("local ") {
            if let Some((names, vals)) = rest.split_once('=') {
                if names.contains(',') {
                    // local A,B,C = 0,1,2
                    let names: Vec<&str> = names.split(',').map(str::trim).collect();
                    let vals: Vec<&str> = vals.split(',').map(str::trim).collect();
                    if names.len() != vals.len() {
                        return Err(format!("ClimateMain.lua: {line}"));
                    }
                    for (n, v) in names.iter().zip(vals) {
                        consts.push(((*n).to_string(), value(v, &consts)?));
                    }
                    continue;
                }
                if names.trim() == "w" && vals.trim() == "_clim:getWeatherPeriod()" {
                    continue;
                }
            }
            return Err(format!("ClimateMain.lua: unknown statement: {line}"));
        }
        if let Some((lhs, rhs)) = line.split_once('=') {
            let (lhs, rhs) = (lhs.trim(), rhs.trim());
            if lhs == "c" {
                if let Some(name) = rhs.strip_prefix("_clim:") {
                    let name = name.trim_end_matches("()");
                    c = Some(
                        manager_getter(name)
                            .ok_or(format!("ClimateMain.lua: unknown getter: {line}"))?,
                    );
                    continue;
                }
                if let Some(name) = rhs.strip_prefix("w:") {
                    let name = name.trim_end_matches("()");
                    c = Some(
                        period_getter(name)
                            .ok_or(format!("ClimateMain.lua: unknown getter: {line}"))?,
                    );
                    continue;
                }
            }
            return Err(format!("ClimateMain.lua: unknown statement: {line}"));
        }
        if line.starts_with("c:setExterior(") || line.starts_with("c:setInterior(") {
            let a = args(line)?;
            if a.len() != 4 {
                return Err(format!("ClimateMain.lua: {line}"));
            }
            let mut v = [0f32; 4];
            for (i, s) in a.iter().enumerate() {
                v[i] = value(s, &consts)? as f32;
            }
            let t = c.ok_or("ClimateMain.lua: colour used before it was fetched")?;
            let col = color_mut(cm, t);
            if line.starts_with("c:setExterior(") {
                col.set_exterior(v[0], v[1], v[2], v[3]);
            } else {
                col.set_interior(v[0], v[1], v[2], v[3]);
            }
            continue;
        }
        let season_setter = line
            .strip_prefix("_clim:setSeasonColorDawn(")
            .map(|_| 0)
            .or_else(|| line.strip_prefix("_clim:setSeasonColorDay(").map(|_| 1))
            .or_else(|| line.strip_prefix("_clim:setSeasonColorDusk(").map(|_| 2));
        if let Some(seg) = season_setter {
            let a = args(line)?;
            if a.len() != 7 {
                return Err(format!("ClimateMain.lua: {line}"));
            }
            let temperature = value(a[0], &consts)? as i32 as usize;
            let season = value(a[1], &consts)? as i32 as usize;
            let mut v = [0f32; 4];
            for i in 0..4 {
                v[i] = value(a[2 + i], &consts)? as f32;
            }
            let exterior = match a[6] {
                "true" => true,
                "false" => false,
                other => return Err(format!("ClimateMain.lua: {other}")),
            };
            match seg {
                0 => {
                    cm.set_season_color_dawn(temperature, season, v[0], v[1], v[2], v[3], exterior)
                }
                1 => cm.set_season_color_day(temperature, season, v[0], v[1], v[2], v[3], exterior),
                _ => {
                    cm.set_season_color_dusk(temperature, season, v[0], v[1], v[2], v[3], exterior)
                }
            }
            continue;
        }
        return Err(format!("ClimateMain.lua: unknown statement: {line}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The colours and season colours a handler of the file's shape sets land on the manager; a statement of
    /// another shape is an error rather than a silent skip.
    #[test]
    fn replays_calls_and_refuses_unknown_statements() {
        let mut cm = ClimateManager::default();
        let lua = "local WARM,CLOUDY = 0,2;\nlocal SUMMER,FALL = 0,1;\nfunction ClimateMain.onClimateManagerInit(_clim)\n\
            local c;\nc = _clim:getColFog();\nc:setExterior(0.25,0.5,0.75,1.0);\nc:setInterior(0.1,0.2,0.3,0.4); -- x\n\
            local w = _clim:getWeatherPeriod();\nc = w:getCloudColorBlizzard();\nc:setExterior(0.5,0.5,0.5,0.5);\n\
            _clim:setSeasonColorDay(CLOUDY,FALL,0.9,0.8,0.7,0.6,true);\nend";
        apply_climate_main(&mut cm, lua).unwrap();
        assert_eq!(cm.col_fog.exterior, [0.25, 0.5, 0.75, 1.0]);
        assert_eq!(cm.col_fog.interior, [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(cm.weather_period.cloud_color_blizzard.exterior, [0.5; 4]);
        let day = cm.get_season_color(1, 2, 1).unwrap();
        assert_eq!(day.exterior, [0.9, 0.8, 0.7, 0.6]);
        assert!(apply_climate_main(&mut cm, "os.execute('x')").is_err());
        assert!(apply_climate_main(&mut cm, "c:setExterior(1,1,1,1)").is_err());
    }
}
