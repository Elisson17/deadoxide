//! zombie.erosion.ErosionConfig: the seeds, the time scale, the season and the debug switches of the erosion system,
//! kept in `erosion.ini` of the save. The network `save`/`load` pair (ByteBufferWriter and ByteBufferReader) is not
//! ported; the file's text is.
use crate::java;

/// zombie.erosion.ErosionConfig.Seeds
#[derive(Clone, Debug, PartialEq)]
pub struct Seeds {
    pub seed_main0: i32,
    pub seed_main1: i32,
    pub seed_main2: i32,
    pub seed_moisture0: i32,
    pub seed_moisture1: i32,
    pub seed_moisture2: i32,
    pub seed_minerals0: i32,
    pub seed_minerals1: i32,
    pub seed_minerals2: i32,
    pub seed_kudzu0: i32,
    pub seed_kudzu1: i32,
    pub seed_kudzu2: i32,
}

/// zombie.erosion.ErosionConfig.Time
#[derive(Clone, Debug, PartialEq)]
pub struct Time {
    pub tickunit: i32,
    pub ticks: i32,
    pub eticks: i32,
    pub epoch: i32,
}

/// zombie.erosion.ErosionConfig.Debug
#[derive(Clone, Debug, PartialEq)]
pub struct Debug {
    pub enabled: bool,
    pub startday: i32,
    pub startmonth: i32,
}

/// zombie.erosion.ErosionConfig.Season
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
    /// `jan` to `dec`: the rain of each month
    pub rain: [f32; 12],
}

/// zombie.erosion.ErosionConfig
#[derive(Clone, Debug, PartialEq)]
pub struct ErosionConfig {
    pub seeds: Seeds,
    pub time: Time,
    pub debug: Debug,
    pub season: Season,
}

impl Default for ErosionConfig {
    /// The field initialisers of ErosionConfig and its four parts.
    fn default() -> Self {
        ErosionConfig {
            seeds: Seeds {
                seed_main0: 16,
                seed_main1: 32,
                seed_main2: 64,
                seed_moisture0: 96,
                seed_moisture1: 128,
                seed_moisture2: 144,
                seed_minerals0: 196,
                seed_minerals1: 255,
                seed_minerals2: 0,
                seed_kudzu0: 200,
                seed_kudzu1: 125,
                seed_kudzu2: 50,
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
                rain: [
                    0.39, 0.35, 0.39, 0.4, 0.35, 0.37, 0.29, 0.26, 0.23, 0.23, 0.3, 0.32,
                ],
            },
        }
    }
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

impl ErosionConfig {
    /// The text zombie.erosion.ErosionConfig.writeFile writes to `erosion.ini`.
    pub fn file_text(&self) -> String {
        let s = &self.seeds;
        let t = &self.time;
        let se = &self.season;
        let mut w = String::new();
        let mut line = |k: &str, v: String| {
            w.push_str(k);
            w.push_str(" = ");
            w.push_str(&v);
            w.push('\n');
        };
        line("seeds.seedMain_0", s.seed_main0.to_string());
        line("seeds.seedMain_1", s.seed_main1.to_string());
        line("seeds.seedMain_2", s.seed_main2.to_string());
        line("seeds.seedMoisture_0", s.seed_moisture0.to_string());
        line("seeds.seedMoisture_1", s.seed_moisture1.to_string());
        line("seeds.seedMoisture_2", s.seed_moisture2.to_string());
        line("seeds.seedMinerals_0", s.seed_minerals0.to_string());
        line("seeds.seedMinerals_1", s.seed_minerals1.to_string());
        line("seeds.seedMinerals_2", s.seed_minerals2.to_string());
        line("seeds.seedKudzu_0", s.seed_kudzu0.to_string());
        line("seeds.seedKudzu_1", s.seed_kudzu1.to_string());
        line("seeds.seedKudzu_2", s.seed_kudzu2.to_string());
        w.push('\n');
        let mut line = |k: &str, v: String| {
            w.push_str(k);
            w.push_str(" = ");
            w.push_str(&v);
            w.push('\n');
        };
        line("time.tickunit", t.tickunit.to_string());
        line("time.ticks", t.ticks.to_string());
        line("time.eticks", t.eticks.to_string());
        line("time.epoch", t.epoch.to_string());
        w.push('\n');
        let mut line = |k: &str, v: String| {
            w.push_str(k);
            w.push_str(" = ");
            w.push_str(&v);
            w.push('\n');
        };
        line("season.lat", se.lat.to_string());
        line("season.tempMax", se.temp_max.to_string());
        line("season.tempMin", se.temp_min.to_string());
        line("season.tempDiff", se.temp_diff.to_string());
        line("season.seasonLag", se.season_lag.to_string());
        line("season.noon", java::float_to_string(se.noon));
        line("season.seedA", se.seed_a.to_string());
        line("season.seedB", se.seed_b.to_string());
        line("season.seedC", se.seed_c.to_string());
        for (m, r) in MONTHS.iter().zip(se.rain) {
            line(&format!("season.{m}"), java::float_to_string(r));
        }
        w.push('\n');
        let d = &self.debug;
        let mut line = |k: &str, v: String| {
            w.push_str(k);
            w.push_str(" = ");
            w.push_str(&v);
            w.push('\n');
        };
        line("debug.enabled", d.enabled.to_string());
        line("debug.startday", d.startday.to_string());
        line("debug.startmonth", d.startmonth.to_string());
        w
    }

    /// zombie.erosion.ErosionConfig.readFile on the file's text: `--` lines are comments, every other line is
    /// `key = value` (anything else is only logged). `false` where the original's `NumberFormatException` makes it
    /// return false (the values read before stay set).
    pub fn read_text(&mut self, text: &str) -> bool {
        for line in java::buffered_reader_lines(text) {
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
            let int = |v: &str| java::parse_int(v);
            let float = |v: &str| java::parse_float(v);
            let ok = (|| -> Option<()> {
                let s = &mut self.seeds;
                let t = &mut self.time;
                let se = &mut self.season;
                match k {
                    "seeds.seedMain_0" => s.seed_main0 = int(v)?,
                    "seeds.seedMain_1" => s.seed_main1 = int(v)?,
                    "seeds.seedMain_2" => s.seed_main2 = int(v)?,
                    "seeds.seedMoisture_0" => s.seed_moisture0 = int(v)?,
                    "seeds.seedMoisture_1" => s.seed_moisture1 = int(v)?,
                    "seeds.seedMoisture_2" => s.seed_moisture2 = int(v)?,
                    "seeds.seedMinerals_0" => s.seed_minerals0 = int(v)?,
                    "seeds.seedMinerals_1" => s.seed_minerals1 = int(v)?,
                    "seeds.seedMinerals_2" => s.seed_minerals2 = int(v)?,
                    "seeds.seedKudzu_0" => s.seed_kudzu0 = int(v)?,
                    "seeds.seedKudzu_1" => s.seed_kudzu1 = int(v)?,
                    "seeds.seedKudzu_2" => s.seed_kudzu2 = int(v)?,
                    "time.tickunit" => t.tickunit = int(v)?,
                    "time.ticks" => t.ticks = int(v)?,
                    "time.eticks" => t.eticks = int(v)?,
                    "time.epoch" => t.epoch = int(v)?,
                    "season.lat" => se.lat = int(v)?,
                    "season.tempMax" => se.temp_max = int(v)?,
                    "season.tempMin" => se.temp_min = int(v)?,
                    "season.tempDiff" => se.temp_diff = int(v)?,
                    "season.seasonLag" => se.season_lag = int(v)?,
                    "season.noon" => se.noon = float(v)?,
                    "season.seedA" => se.seed_a = int(v)?,
                    "season.seedB" => se.seed_b = int(v)?,
                    "season.seedC" => se.seed_c = int(v)?,
                    "debug.enabled" => {
                        self.debug.enabled = java::trim(v).eq_ignore_ascii_case("true")
                    }
                    "debug.startday" => self.debug.startday = int(v)?,
                    "debug.startmonth" => self.debug.startmonth = int(v)?,
                    _ => {
                        if let Some(m) = k.strip_prefix("season.") {
                            if let Some(i) = MONTHS.iter().position(|n| *n == m) {
                                se.rain[i] = float(v)?;
                            }
                        }
                    }
                }
                Some(())
            })();
            if ok.is_none() {
                return false;
            }
        }
        true
    }
}
