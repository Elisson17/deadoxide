//! zombie.erosion.ErosionConfig: the erosion settings a save keeps in `erosion.ini` (a text file of `key = value`
//! lines) and writes back as that file. Ported: the field defaults, `readFile` and `writeFile`'s text. Not ported: the
//! network `save`/`load` (ByteBufferWriter/Reader) and the speed scaling `ErosionMain.initConfig` applies afterwards.
use crate::java;

/// zombie.erosion.ErosionConfig.Seeds
#[derive(Clone, Debug, PartialEq)]
pub struct Seeds {
    pub seed_main: [i32; 3],
    pub seed_moisture: [i32; 3],
    pub seed_minerals: [i32; 3],
    pub seed_kudzu: [i32; 3],
}

/// zombie.erosion.ErosionConfig.Time
#[derive(Clone, Debug, PartialEq)]
pub struct Time {
    pub tickunit: i32,
    pub ticks: i32,
    pub eticks: i32,
    pub epoch: i32,
}

/// zombie.erosion.ErosionConfig.Season: `months` is jan..dec.
#[derive(Clone, Debug, PartialEq)]
pub struct Season {
    pub lat: i32,
    pub temp_max: i32,
    pub temp_min: i32,
    pub temp_diff: i32,
    pub season_lag: i32,
    pub noon: f32,
    pub seed_a: i32,
    pub seed_b: i32,
    pub seed_c: i32,
    pub months: [f32; 12],
}

/// zombie.erosion.ErosionConfig.Debug
#[derive(Clone, Debug, PartialEq)]
pub struct Debug {
    pub enabled: bool,
    pub startday: i32,
    pub startmonth: i32,
}

/// zombie.erosion.ErosionConfig
#[derive(Clone, Debug, PartialEq)]
pub struct ErosionConfig {
    pub seeds: Seeds,
    pub time: Time,
    pub debug: Debug,
    pub season: Season,
}

/// The month keys of `season.*` in the order of `Season.months`.
const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

impl Default for ErosionConfig {
    /// The field initializers of the original's classes.
    fn default() -> Self {
        ErosionConfig {
            seeds: Seeds {
                seed_main: [16, 32, 64],
                seed_moisture: [96, 128, 144],
                seed_minerals: [196, 255, 0],
                seed_kudzu: [200, 125, 50],
            },
            time: Time {
                tickunit: 144,
                ticks: 0,
                eticks: 0,
                epoch: 0,
            },
            debug: Debug {
                enabled: false,
                startday: 26,
                startmonth: 11,
            },
            season: Season {
                lat: 38,
                temp_max: 25,
                temp_min: 0,
                temp_diff: 7,
                season_lag: 31,
                noon: 12.5,
                seed_a: 64,
                seed_b: 128,
                seed_c: 255,
                months: [
                    0.39, 0.35, 0.39, 0.4, 0.35, 0.37, 0.29, 0.26, 0.23, 0.23, 0.3, 0.32,
                ],
            },
        }
    }
}

/// `Integer.parseInt` as a `Result` carrying the NumberFormatException text.
fn int(v: &str) -> Result<i32, String> {
    java::parse_int(v).ok_or_else(|| {
        format!(
            "java.lang.NumberFormatException: {}",
            java::number_format_message(v)
        )
    })
}

/// `Float.parseFloat` as a `Result` carrying the NumberFormatException text.
fn float(v: &str) -> Result<f32, String> {
    java::parse_float(v).ok_or_else(|| {
        format!(
            "java.lang.NumberFormatException: {}",
            if java::trim(v).is_empty() {
                "empty String".to_string()
            } else {
                java::number_format_message(java::trim(v))
            }
        )
    })
}

impl ErosionConfig {
    /// zombie.erosion.ErosionConfig.readFile: `text` is the file (`None` when it does not exist: false). Each line
    /// that is not a `--` comment is split at `=` and must give exactly a key and a value; a value that does not parse
    /// ends the read with false, keeping every setting read before it. Unknown keys are logged and skipped.
    pub fn read_file(&mut self, text: Option<&[u8]>) -> bool {
        let Some(text) = text else {
            return false;
        };
        let text = String::from_utf8_lossy(text);
        for line in java::buffered_reader_lines(&text) {
            if java::trim(line).starts_with("--") {
                continue;
            }
            if !line.contains('=') {
                continue;
            }
            let ss = java::split_char(line, '=');
            if ss.len() != 2 {
                continue;
            }
            let k = java::trim(ss[0]);
            let v = java::trim(ss[1]);
            if self.set(k, v).is_err() {
                return false;
            }
        }
        true
    }

    /// One `key = value` line of `readFile`: an unknown key changes nothing.
    fn set(&mut self, k: &str, v: &str) -> Result<(), String> {
        let triple = |name: &str, a: &mut [i32; 3], v: &str| -> Result<(), String> {
            for (i, slot) in a.iter_mut().enumerate() {
                if k == format!("seeds.{name}_{i}") {
                    *slot = int(v)?;
                }
            }
            Ok(())
        };
        if k.starts_with("seeds.") {
            triple("seedMain", &mut self.seeds.seed_main, v)?;
            triple("seedMoisture", &mut self.seeds.seed_moisture, v)?;
            triple("seedMinerals", &mut self.seeds.seed_minerals, v)?;
            triple("seedKudzu", &mut self.seeds.seed_kudzu, v)?;
        } else if k.starts_with("time.") {
            match k {
                "time.tickunit" => self.time.tickunit = int(v)?,
                "time.ticks" => self.time.ticks = int(v)?,
                "time.eticks" => self.time.eticks = int(v)?,
                "time.epoch" => self.time.epoch = int(v)?,
                _ => {}
            }
        } else if k.starts_with("season.") {
            match k {
                "season.lat" => self.season.lat = int(v)?,
                "season.tempMax" => self.season.temp_max = int(v)?,
                "season.tempMin" => self.season.temp_min = int(v)?,
                "season.tempDiff" => self.season.temp_diff = int(v)?,
                "season.seasonLag" => self.season.season_lag = int(v)?,
                "season.noon" => self.season.noon = float(v)?,
                "season.seedA" => self.season.seed_a = int(v)?,
                "season.seedB" => self.season.seed_b = int(v)?,
                "season.seedC" => self.season.seed_c = int(v)?,
                _ => {
                    for (i, m) in MONTHS.iter().enumerate() {
                        if k == format!("season.{m}") {
                            self.season.months[i] = float(v)?;
                        }
                    }
                }
            }
        } else if k.starts_with("debug.") {
            match k {
                // Boolean.parseBoolean: "true" in any case, anything else false
                "debug.enabled" => self.debug.enabled = v.eq_ignore_ascii_case("true"),
                "debug.startday" => self.debug.startday = int(v)?,
                "debug.startmonth" => self.debug.startmonth = int(v)?,
                _ => {}
            }
        }
        Ok(())
    }

    /// The text zombie.erosion.ErosionConfig.writeFile writes (`"key = value\n"` lines in four blocks).
    pub fn write_file(&self) -> String {
        let mut s = String::new();
        let mut put = |k: String, v: String| {
            s.push_str(&format!("{k} = {v}\n"));
        };
        for (name, a) in [
            ("seedMain", &self.seeds.seed_main),
            ("seedMoisture", &self.seeds.seed_moisture),
            ("seedMinerals", &self.seeds.seed_minerals),
            ("seedKudzu", &self.seeds.seed_kudzu),
        ] {
            for (i, v) in a.iter().enumerate() {
                put(format!("seeds.{name}_{i}"), v.to_string());
            }
        }
        s.push('\n');
        let mut put = |k: &str, v: String| s.push_str(&format!("{k} = {v}\n"));
        put("time.tickunit", self.time.tickunit.to_string());
        put("time.ticks", self.time.ticks.to_string());
        put("time.eticks", self.time.eticks.to_string());
        put("time.epoch", self.time.epoch.to_string());
        s.push('\n');
        let mut put = |k: &str, v: String| s.push_str(&format!("{k} = {v}\n"));
        put("season.lat", self.season.lat.to_string());
        put("season.tempMax", self.season.temp_max.to_string());
        put("season.tempMin", self.season.temp_min.to_string());
        put("season.tempDiff", self.season.temp_diff.to_string());
        put("season.seasonLag", self.season.season_lag.to_string());
        put("season.noon", java::float_to_string(self.season.noon));
        put("season.seedA", self.season.seed_a.to_string());
        put("season.seedB", self.season.seed_b.to_string());
        put("season.seedC", self.season.seed_c.to_string());
        for (i, m) in MONTHS.iter().enumerate() {
            put(
                &format!("season.{m}"),
                java::float_to_string(self.season.months[i]),
            );
        }
        s.push('\n');
        let mut put = |k: &str, v: String| s.push_str(&format!("{k} = {v}\n"));
        put("debug.enabled", self.debug.enabled.to_string());
        put("debug.startday", self.debug.startday.to_string());
        put("debug.startmonth", self.debug.startmonth.to_string());
        s
    }
}
