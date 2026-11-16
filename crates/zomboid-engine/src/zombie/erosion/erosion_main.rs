//! zombie.erosion.ErosionMain: the erosion system of a dedicated server: the config (`erosion.ini`), the noise that
//! decides what grows where, the season, the erosion clock and the load of squares and chunks.
//!
//! Not ported: the client side (`GameClient.client` branches, `sendState`, `receiveState`, `DebugUpdateMapNow`,
//! `updateMapNow` over the players' chunk maps), the single-player `Rand` fallback of `ErosionData.Square.rand`,
//! `snowCheck` (empty in the original) and the `OnInitSeasons` Lua event (nothing on a server listens to it: only
//! `media/lua/client/LastStand/WinterIsComing.lua` does).
use super::erosion_config::ErosionConfig;
use super::erosion_data::{Chunk, Square};
use super::erosion_env::{Env, Pos};
use super::erosion_world::ErosionWorld;
use super::season::erosion_ice_queen::ErosionIceQueen;
use super::season::erosion_season::ErosionSeason;
use super::utils::noise2d::Noise2D;
use crate::java;
use crate::java_nio::pack_float_unit_to_byte;
use crate::zombie::core::tile_property_alias_map::TilePropertyAliasMap;
use crate::zombie::game_time::GameTime;
use crate::zombie::iso::server_map::ServerMap;
use crate::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;

/// zombie.erosion.ErosionMain.soilTable
const SOIL_TABLE: [[i32; 10]; 10] = [
    [1, 1, 1, 1, 1, 4, 4, 4, 4, 4],
    [1, 1, 1, 1, 2, 5, 4, 4, 4, 4],
    [1, 1, 1, 2, 2, 5, 5, 4, 4, 4],
    [1, 1, 2, 2, 3, 6, 5, 5, 4, 4],
    [1, 2, 2, 3, 3, 6, 6, 5, 5, 4],
    [7, 8, 8, 9, 9, 12, 12, 11, 11, 10],
    [7, 7, 8, 8, 9, 12, 11, 11, 10, 10],
    [7, 7, 7, 8, 8, 11, 11, 10, 10, 10],
    [7, 7, 7, 7, 8, 11, 10, 10, 10, 10],
    [7, 7, 7, 7, 7, 10, 10, 10, 10, 10],
];

/// The sandbox options the erosion system reads (`SandboxOptions.instance`): `getErosionSpeed()` (1 very fast to 5
/// very slow, 3 normal), `erosionDays`, `timeSinceApo` (1 to 13) and `getTemperatureModifier()` (1 very cold to 5
/// very hot).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ErosionOptions {
    pub erosion_speed: i32,
    pub erosion_days: i32,
    pub time_since_apo: i32,
    pub temperature_modifier: i32,
}

impl Default for ErosionOptions {
    /// The defaults of a new server: ErosionSpeed 3, ErosionDays 0, TimeSinceApo 1, Temperature 3.
    fn default() -> Self {
        ErosionOptions {
            erosion_speed: 3,
            erosion_days: 0,
            time_since_apo: 1,
            temperature_modifier: 3,
        }
    }
}

/// zombie.erosion.ErosionMain
pub struct ErosionMain {
    pub cfg: ErosionConfig,
    /// The text of the save's `erosion.ini` as `ErosionConfig.writeFile` last wrote it (`None` while the file does
    /// not exist).
    pub ini_file: Option<String>,
    options: ErosionOptions,
    debug: bool,
    ice_queen: ErosionIceQueen,
    pub is_snow: bool,
    noise_main: Noise2D,
    noise_moisture: Noise2D,
    noise_minerals: Noise2D,
    noise_kudzu: Noise2D,
    world: ErosionWorld,
    pub season: ErosionSeason,
    pub tick_unit: i32,
    pub ticks: i32,
    pub e_ticks: i32,
    pub day: i32,
    pub month: i32,
    pub year: i32,
    pub epoch: i32,
    rands: Vec<i32>,
}

impl ErosionMain {
    /// zombie.erosion.ErosionMain.initConfig on a server: the save's `erosion.ini` when it exists and reads; else
    /// the defaults, the speed's scaling of the tick unit and the erosion that has happened since the apocalypse.
    /// (`media/data/erosion.ini`, which the original copies to the cache folder when there is one, does not exist in
    /// the game.)
    fn init_config(options: &ErosionOptions, existing_ini: Option<&str>) -> ErosionConfig {
        if let Some(text) = existing_ini {
            let mut cfg = ErosionConfig::default();
            if cfg.read_file(Some(text.as_bytes())) {
                return cfg;
            }
        }
        let mut cfg = ErosionConfig::default();
        match options.erosion_speed {
            1 => cfg.time.tickunit /= 5,
            2 => cfg.time.tickunit /= 2,
            4 => cfg.time.tickunit *= 2,
            5 => cfg.time.tickunit *= 5,
            _ => {}
        }
        let days_till_100_percent = (cfg.time.tickunit * 100) as f32 / 144.0;
        let days_since_apo = ((options.time_since_apo - 1) * 30) as f32;
        cfg.time.eticks =
            (f64::from(java::math_min_f32(1.0, days_since_apo / days_till_100_percent) * 100.0))
                .floor() as i32;
        let erosion_days = options.erosion_days;
        if erosion_days > 0 {
            cfg.time.tickunit = 144;
            cfg.time.eticks =
                (f64::from(java::math_min_f32(1.0, days_since_apo / erosion_days as f32) * 100.0))
                    .floor() as i32;
        }
        cfg
    }

    /// zombie.erosion.ErosionMain constructor and `start` on a server: reads the config, builds the four noises,
    /// the season, the ice queen and the regions with their categories.
    pub fn new(
        options: ErosionOptions,
        existing_ini: Option<&str>,
        game_time: &GameTime,
        sm: &mut IsoSpriteManager,
        alias: &TilePropertyAliasMap,
        rands: Vec<i32>,
    ) -> Self {
        let cfg = Self::init_config(&options, existing_ini);
        let mut noise_main = Noise2D::default();
        noise_main.add_layer(cfg.seeds.seed_main[0], 0.5, 3.0);
        noise_main.add_layer(cfg.seeds.seed_main[1], 2.0, 5.0);
        noise_main.add_layer(cfg.seeds.seed_main[2], 5.0, 8.0);
        let mut noise_moisture = Noise2D::default();
        noise_moisture.add_layer(cfg.seeds.seed_moisture[0], 2.0, 3.0);
        noise_moisture.add_layer(cfg.seeds.seed_moisture[1], 1.6, 5.0);
        noise_moisture.add_layer(cfg.seeds.seed_moisture[2], 0.6, 8.0);
        let mut noise_minerals = Noise2D::default();
        noise_minerals.add_layer(cfg.seeds.seed_minerals[0], 2.0, 3.0);
        noise_minerals.add_layer(cfg.seeds.seed_minerals[1], 1.6, 5.0);
        noise_minerals.add_layer(cfg.seeds.seed_minerals[2], 0.6, 8.0);
        let mut noise_kudzu = Noise2D::default();
        noise_kudzu.add_layer(cfg.seeds.seed_kudzu[0], 6.0, 3.0);
        noise_kudzu.add_layer(cfg.seeds.seed_kudzu[1], 3.0, 5.0);
        noise_kudzu.add_layer(cfg.seeds.seed_kudzu[2], 0.5, 8.0);

        let mut season = ErosionSeason::default();
        let sc = &cfg.season;
        let mut temp_min = sc.temp_min;
        let mut temp_max = sc.temp_max;
        match options.temperature_modifier {
            1 => {
                temp_min -= 10;
                temp_max -= 10;
            }
            2 => {
                temp_min -= 5;
                temp_max -= 5;
            }
            4 => {
                temp_min = (f64::from(temp_min) + 7.5) as i32;
                temp_max += 4;
            }
            5 => {
                temp_min += 15;
                temp_max += 8;
            }
            _ => {}
        }
        season.init(
            sc.lat,
            temp_max,
            temp_min,
            sc.temp_diff,
            sc.season_lag,
            sc.noon,
            sc.seed_a,
            sc.seed_b,
            sc.seed_c,
        );
        season.set_rain(sc.months);
        season.set_day(game_time.day, game_time.month, game_time.year);
        let mut ice_queen = ErosionIceQueen::new(sm, alias);
        let mut world = ErosionWorld::default();
        world.init(sm, &mut ice_queen);
        ErosionMain {
            tick_unit: cfg.time.tickunit,
            ticks: cfg.time.ticks,
            e_ticks: cfg.time.eticks,
            month: game_time.month,
            year: game_time.year,
            day: game_time.day,
            debug: false,
            options,
            ini_file: None,
            cfg,
            ice_queen,
            is_snow: false,
            noise_main,
            noise_moisture,
            noise_minerals,
            noise_kudzu,
            world,
            season,
            epoch: 0,
            rands,
        }
    }

    /// zombie.erosion.ErosionMain.isSnow
    pub fn ice_queen_is_snow(&self) -> bool {
        self.ice_queen.is_snow()
    }

    /// zombie.erosion.ErosionMain.mainTimer on a server (`EveryTenMinutes`): moves the erosion clock, starts a new
    /// epoch when the date changed, brings every loaded chunk whose stamp is old up to date and writes `erosion.ini`.
    pub fn main_timer(
        &mut self,
        game_time: &GameTime,
        map: &mut ServerMap,
        sm: &mut IsoSpriteManager,
        alias: &TilePropertyAliasMap,
    ) -> Result<(), String> {
        let erosion_days = self.options.erosion_days;
        if self.debug {
            self.e_ticks = self.e_ticks.wrapping_add(1);
        } else if erosion_days < 0 {
            self.e_ticks = 0;
        } else if erosion_days > 0 {
            self.ticks = self.ticks.wrapping_add(1);
            self.e_ticks = (self.ticks as f32 / 144.0 / erosion_days as f32 * 100.0) as i32;
        } else {
            self.ticks = self.ticks.wrapping_add(1);
            if self.ticks >= self.tick_unit {
                self.ticks = 0;
                self.e_ticks = self.e_ticks.wrapping_add(1);
            }
        }
        if self.e_ticks < 0 {
            self.e_ticks = i32::MAX;
        }

        if game_time.day != self.day || game_time.month != self.month || game_time.year != self.year
        {
            self.month = game_time.month;
            self.year = game_time.year;
            self.day = game_time.day;
            self.epoch = self.epoch.wrapping_add(1);
            self.season.set_day(self.day, self.month, self.year);
            // snowCheck(): empty in the original
        }

        // GameServer.server: every chunk of every loaded cell
        let cells: Vec<(i32, i32, bool)> = map
            .loaded_cells
            .iter()
            .map(|c| (c.wx, c.wy, c.is_loaded))
            .collect();
        for (cwx, cwy, is_loaded) in cells {
            if !is_loaded {
                continue;
            }
            for y in 0..8 {
                for x in 0..8 {
                    let key = (cwx * 8 + x, cwy * 8 + y);
                    let Some(chunk) = map.chunks.get(&key) else {
                        continue;
                    };
                    let stamp = &chunk.erosion;
                    if stamp.e_tick_stamp != self.e_ticks || stamp.epoch != self.epoch {
                        for yy in 0..8 {
                            for xx in 0..8 {
                                let present = map
                                    .chunks
                                    .get(&key)
                                    .and_then(|c| c.get_grid_square(xx, yy, 0))
                                    .map(|s| (s.x, s.y, s.z));
                                if let Some(pos) = present {
                                    self.load_gridsquare(game_time, map, sm, alias, pos)?;
                                }
                            }
                        }
                        if let Some(c) = map.chunks.get_mut(&key) {
                            c.erosion.e_tick_stamp = self.e_ticks;
                            c.erosion.epoch = self.epoch;
                        }
                    }
                }
            }
        }

        self.cfg.time.ticks = self.ticks;
        self.cfg.time.eticks = self.e_ticks;
        self.cfg.time.epoch = self.epoch;
        self.ini_file = Some(self.cfg.write_file());
        Ok(())
    }

    /// zombie.erosion.ErosionMain.loadGridsquare (`LoadGridsquare`): only squares of level 0 take part. The first
    /// time a square is seen its erosion data is made and its categories chosen; then, when its chunk's stamp is
    /// behind the erosion clock, the square is updated.
    pub fn load_gridsquare(
        &mut self,
        _game_time: &GameTime,
        map: &mut ServerMap,
        sm: &mut IsoSpriteManager,
        alias: &TilePropertyAliasMap,
        pos: Pos,
    ) -> Result<(), String> {
        let key = (pos.0.div_euclid(8), pos.1.div_euclid(8));
        let exists = map
            .chunks
            .get(&key)
            .and_then(|c| c.get_grid_square(pos.0.rem_euclid(8), pos.1.rem_euclid(8), pos.2))
            .is_some();
        if !exists || pos.2 != 0 {
            return Ok(());
        }
        // getChunk(square)
        if let Some(c) = map.chunks.get_mut(&key) {
            if !c.erosion.init {
                self.init_chunk(c.wx, c.wy, &mut c.erosion)?;
            }
        }
        let chunk_data: Chunk = map
            .chunks
            .get(&key)
            .map(|c| c.erosion.clone())
            .unwrap_or_default();
        let mut ed: Square = map
            .get_grid_square_mut(pos.0, pos.1, pos.2)
            .and_then(|s| s.erosion.take())
            .map(|b| *b)
            .unwrap_or_default();
        let result = self.run_square(map, sm, alias, pos, &mut ed, &chunk_data);
        if let Some(s) = map.get_grid_square_mut(pos.0, pos.1, pos.2) {
            s.erosion = Some(Box::new(ed));
        }
        result
    }

    fn run_square(
        &mut self,
        map: &mut ServerMap,
        sm: &mut IsoSpriteManager,
        alias: &TilePropertyAliasMap,
        pos: Pos,
        ed: &mut Square,
        chunk_data: &Chunk,
    ) -> Result<(), String> {
        let mut env = Env {
            map,
            sm,
            alias,
            season: &self.season,
            rands: &self.rands,
        };
        if !ed.init {
            Self::init_grid_square(
                &self.noise_main,
                &self.noise_kudzu,
                env.rands,
                pos,
                ed,
                chunk_data,
            )?;
            self.world.validate_spawn(&mut env, pos, ed, chunk_data)?;
        }
        if ed.do_nothing {
            return Ok(());
        }
        if chunk_data.e_tick_stamp >= self.e_ticks && chunk_data.epoch == self.epoch {
            return Ok(());
        }
        self.world
            .update(&mut env, pos, ed, chunk_data, self.e_ticks)
    }

    /// zombie.erosion.ErosionMain.initGridSquare: the noise values, soil and "magic number" of a square.
    fn init_grid_square(
        noise_main: &Noise2D,
        noise_kudzu: &Noise2D,
        rands: &[i32],
        pos: Pos,
        ed: &mut Square,
        chunk_mod_data: &Chunk,
    ) -> Result<(), String> {
        let (sqx, sqy) = (pos.0, pos.1);
        let noise = noise_main.layered_noise(sqx as f32 / 10.0, sqy as f32 / 10.0);
        ed.noise_main_byte = pack_float_unit_to_byte(noise);
        ed.noise_main = noise;
        ed.noise_main_int = (f64::from(ed.noise_main * 100.0)).floor() as i32;
        ed.noise_kudzu = noise_kudzu.layered_noise(sqx as f32 / 10.0, sqy as f32 / 10.0);
        ed.soil = chunk_mod_data.soil;
        // erosionModData.rand = new RandLocation(sqx, sqy): the server's rand() reads the table instead
        let magic = ed.rand(rands, sqx, sqy, 100) as f32 / 100.0;
        ed.magic_num_byte = pack_float_unit_to_byte(magic);
        ed.magic_num = magic;
        ed.regions.clear();
        ed.init = true;
        Ok(())
    }

    /// zombie.erosion.ErosionMain.initChunk: the moisture and minerals of a chunk and from them its soil.
    fn init_chunk(&self, wx: i32, wy: i32, chunk_mod_data: &mut Chunk) -> Result<(), String> {
        chunk_mod_data.set(wx, wy);
        let nx = chunk_mod_data.x as f32 / 5.0;
        let ny = chunk_mod_data.y as f32 / 5.0;
        let moisture = self.noise_moisture.layered_noise(nx, ny);
        let minerals = self.noise_minerals.layered_noise(nx, ny);
        let moi = if moisture < 1.0 {
            (f64::from(moisture * 10.0)).floor() as i32
        } else {
            9
        };
        let min = if minerals < 1.0 {
            (f64::from(minerals * 10.0)).floor() as i32
        } else {
            9
        };
        chunk_mod_data.init = true;
        chunk_mod_data.e_tick_stamp = -1;
        chunk_mod_data.epoch = -1;
        chunk_mod_data.moisture = moisture;
        chunk_mod_data.minerals = minerals;
        if !(0..10).contains(&moi) {
            return Err(crate::zombie::erosion::erosion_env::aioobe(moi, 10));
        }
        if !(0..10).contains(&min) {
            return Err(crate::zombie::erosion::erosion_env::aioobe(min, 10));
        }
        let (row, col) = (moi as usize, min as usize);
        chunk_mod_data.soil = SOIL_TABLE[row][col] - 1;
        Ok(())
    }

    /// zombie.erosion.ErosionMain.loadChunk (`ChunkLoaded`): the chunk's erosion data is made if needed and stamped
    /// with the current erosion tick and epoch.
    pub fn load_chunk(&self, map: &mut ServerMap, wx: i32, wy: i32) -> Result<(), String> {
        let Some(c) = map.chunks.get_mut(&(wx, wy)) else {
            return Ok(());
        };
        if !c.erosion.init {
            self.init_chunk(wx, wy, &mut c.erosion)?;
        }
        c.erosion.e_tick_stamp = self.e_ticks;
        c.erosion.epoch = self.epoch;
        Ok(())
    }
}
