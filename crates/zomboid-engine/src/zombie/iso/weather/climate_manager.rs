//! zombie.iso.weather.ClimateManager: the weather state a save stores in `map_t.bin` after the game time.
use super::climate_color_info::ClimateColorInfo;
use super::thunder_storm::ThunderStorm;
use super::weather_period::WeatherPeriod;
use crate::java_nio::ByteBuffer;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTable;
use crate::zombie::iso::map_meta::MetaSide;

/// zombie.iso.weather.ClimateManager.AirFront
#[derive(Clone, Debug, PartialEq)]
pub struct AirFront {
    pub days: f32,
    pub max_noise: f32,
    pub total_noise: f32,
    pub type_: i32,
    pub strength: f32,
    pub tmp_noise_abs: f32,
    pub noise_cache: [f32; 2],
    pub noise_cache_value: f32,
    pub front_wind_angle_degrees: f32,
}

impl Default for AirFront {
    /// zombie.iso.weather.ClimateManager.AirFront(): `reset()`, so the noise cache is -1.
    fn default() -> Self {
        AirFront {
            days: 0.0,
            max_noise: 0.0,
            total_noise: 0.0,
            type_: 0,
            strength: 0.0,
            tmp_noise_abs: 0.0,
            noise_cache: [-1.0; 2],
            noise_cache_value: 0.0,
            front_wind_angle_degrees: 0.0,
        }
    }
}

impl AirFront {
    /// zombie.iso.weather.ClimateManager.AirFront.load(DataInputStream): days, noise, type, strength and wind angle,
    /// then the saved noise cache: values past the two it holds are read and dropped, missing ones become -1.
    pub fn load(&mut self, input: &mut ByteBuffer) -> Result<(), String> {
        self.days = input.get_float()?;
        self.max_noise = input.get_float()?;
        self.total_noise = input.get_float()?;
        self.type_ = input.get_int()?;
        self.strength = input.get_float()?;
        self.front_wind_angle_degrees = input.get_float()?;
        let len = input.get_int()?;
        let cache_len = self.noise_cache.len() as i32;
        let max = if len > cache_len { len } else { cache_len };
        for i in 0..max {
            if i < len {
                let val = input.get_float()?;
                if i < cache_len {
                    self.noise_cache[i as usize] = val;
                }
            } else if i < cache_len {
                self.noise_cache[i as usize] = -1.0;
            }
        }
        Ok(())
    }
}

/// zombie.iso.weather.ClimateManager.ClimateFloat: only the admin override a server's save stores is ported.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClimateFloat {
    pub is_admin_override: bool,
    pub admin_value: f32,
}

impl ClimateFloat {
    /// zombie.iso.weather.ClimateManager.ClimateFloat.loadAdmin
    pub fn load_admin(
        &mut self,
        input: &mut ByteBuffer,
        _world_version: i32,
    ) -> Result<(), String> {
        self.is_admin_override = input.get()? != 0;
        self.admin_value = input.get_float()?;
        Ok(())
    }
}

/// zombie.iso.weather.ClimateManager.ClimateColor: only the admin override is ported.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClimateColor {
    pub is_admin_override: bool,
    pub admin_value: ClimateColorInfo,
}

impl ClimateColor {
    /// zombie.iso.weather.ClimateManager.ClimateColor.loadAdmin
    pub fn load_admin(&mut self, input: &mut ByteBuffer, world_version: i32) -> Result<(), String> {
        self.is_admin_override = input.get()? != 0;
        self.admin_value.load(input, world_version)
    }
}

/// zombie.iso.weather.ClimateManager.ClimateBool: only the admin override is ported.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClimateBool {
    pub is_admin_override: bool,
    pub admin_value: bool,
}

impl ClimateBool {
    /// zombie.iso.weather.ClimateManager.ClimateBool.loadAdmin
    pub fn load_admin(
        &mut self,
        input: &mut ByteBuffer,
        _world_version: i32,
    ) -> Result<(), String> {
        self.is_admin_override = input.get()? != 0;
        self.admin_value = input.get()? != 0;
        Ok(())
    }
}

/// zombie.iso.weather.ClimateManager: the saved state. `Default` has the simplex offsets at 0; the original's
/// constructor draws them from `Rand` (not ported), and a load replaces them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClimateManager {
    pub simplex_offset_a: f64,
    pub simplex_offset_b: f64,
    pub simplex_offset_c: f64,
    pub simplex_offset_d: f64,
    pub current_front: AirFront,
    pub snow_frac_now: f32,
    pub snow_strength: f32,
    pub can_do_winter_sprites: bool,
    pub day_do_fog: bool,
    pub day_fog_strength: f32,
    pub weather_period: WeatherPeriod,
    pub thunder_storm: ThunderStorm,
    pub desaturation: ClimateFloat,
    pub global_light_intensity: ClimateFloat,
    pub night_strength: ClimateFloat,
    pub precipitation_intensity: ClimateFloat,
    pub temperature: ClimateFloat,
    pub fog_intensity: ClimateFloat,
    pub wind_intensity: ClimateFloat,
    pub wind_angle_intensity: ClimateFloat,
    pub cloud_intensity: ClimateFloat,
    pub ambient: ClimateFloat,
    pub view_distance: ClimateFloat,
    pub day_light_strength: ClimateFloat,
    pub humidity: ClimateFloat,
    pub global_light: ClimateColor,
    pub precipitation_is_snow: ClimateBool,
    pub mod_data_table: Option<KahluaTable>,
}

impl ClimateManager {
    /// zombie.iso.weather.ClimateManager.load(DataInputStream, int): when the first byte is 1, the simplex offsets,
    /// the current air front, snow, winter sprites and fog; then the weather period and the thunder storm; on a
    /// server the admin overrides (a single-player or client save has none); the mod data table when its byte is 1
    /// (loaded into the existing table without wiping it); on a server the humidity override. The `ClimateValues`
    /// the original rebuilds at the end are not ported.
    pub fn load(
        &mut self,
        input: &mut ByteBuffer,
        world_version: i32,
        side: MetaSide,
        log: &mut Vec<String>,
    ) -> Result<(), String> {
        let server = side == MetaSide::Server;
        if input.get()? == 1 {
            self.simplex_offset_a = input.get_double()?;
            self.simplex_offset_b = input.get_double()?;
            self.simplex_offset_c = input.get_double()?;
            self.simplex_offset_d = input.get_double()?;
            self.current_front.load(input)?;
            self.snow_frac_now = input.get_float()?;
            self.snow_strength = input.get_float()?;
            self.can_do_winter_sprites = input.get()? != 0;
            self.day_do_fog = input.get()? != 0;
            self.day_fog_strength = input.get_float()?;
        }
        self.weather_period.load(input, world_version, log)?;
        self.thunder_storm.load(input)?;
        if server {
            for f in [
                &mut self.desaturation,
                &mut self.global_light_intensity,
                &mut self.night_strength,
                &mut self.precipitation_intensity,
                &mut self.temperature,
                &mut self.fog_intensity,
                &mut self.wind_intensity,
                &mut self.wind_angle_intensity,
                &mut self.cloud_intensity,
                &mut self.ambient,
                &mut self.view_distance,
                &mut self.day_light_strength,
            ] {
                f.load_admin(input, world_version)?;
            }
            self.global_light.load_admin(input, world_version)?;
            self.precipitation_is_snow
                .load_admin(input, world_version)?;
        }
        if input.get()? == 1 {
            self.mod_data_table
                .get_or_insert_with(KahluaTable::default)
                .load_stream(input, world_version)?;
        }
        if server {
            self.humidity.load_admin(input, world_version)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A saved noise cache of three values keeps the first two; one of a single value leaves the second at -1.
    #[test]
    fn air_front_noise_cache_keeps_two_values() {
        let front = |cache: &[f32]| {
            let mut d = Vec::new();
            for v in [1.0f32, 2.0, 3.0] {
                d.extend(v.to_be_bytes());
            }
            d.extend(1i32.to_be_bytes());
            for v in [4.0f32, 5.0] {
                d.extend(v.to_be_bytes());
            }
            d.extend((cache.len() as i32).to_be_bytes());
            for v in cache {
                d.extend(v.to_be_bytes());
            }
            d.push(9);
            let mut f = AirFront {
                noise_cache: [7.0, 7.0],
                ..AirFront::default()
            };
            let mut b = ByteBuffer::wrap(&d);
            f.load(&mut b).unwrap();
            (f, b.get().unwrap())
        };
        let (f, next) = front(&[0.5, 0.25, 0.125]);
        assert_eq!(
            (f.noise_cache, f.type_, f.front_wind_angle_degrees, next),
            ([0.5, 0.25], 1, 5.0, 9)
        );
        assert_eq!(front(&[0.5]).0.noise_cache, [0.5, -1.0]);
    }
}
