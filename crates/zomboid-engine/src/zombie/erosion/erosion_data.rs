//! zombie.erosion.ErosionData: the erosion state a chunk save carries per chunk and per square, with the category
//! data of the erosion category classes (`ErosionRegions` decides which category a (region, category) pair is).
use crate::java_nio::{unpack_byte_to_float_unit, ByteBuffer};

/// zombie.erosion.ErosionData.Chunk
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Chunk {
    pub init: bool,
    pub e_tick_stamp: i32,
    pub epoch: i32,
    pub x: i32,
    pub y: i32,
    pub moisture: f32,
    pub minerals: f32,
    pub soil: i32,
}

impl Chunk {
    /// The field initialisers of zombie.erosion.ErosionData.Chunk (`eTickStamp` and `epoch` start at -1).
    pub fn new() -> Self {
        Chunk {
            e_tick_stamp: -1,
            epoch: -1,
            ..Chunk::default()
        }
    }

    /// zombie.erosion.ErosionData.Chunk.set (IsoChunk)
    pub fn set(&mut self, wx: i32, wy: i32) {
        self.x = wx;
        self.y = wy;
    }

    /// zombie.erosion.ErosionData.Chunk.save
    pub fn save(&self, out: &mut Vec<u8>) {
        if self.init {
            out.push(1);
            out.extend_from_slice(&self.e_tick_stamp.to_be_bytes());
            out.extend_from_slice(&self.epoch.to_be_bytes());
            out.extend_from_slice(&self.moisture.to_be_bytes());
            out.extend_from_slice(&self.minerals.to_be_bytes());
            out.push(self.soil as u8);
        } else {
            out.push(0);
        }
    }

    /// zombie.erosion.ErosionData.Chunk.load
    pub fn load(b: &mut ByteBuffer) -> Result<Self, String> {
        let mut c = Chunk {
            e_tick_stamp: -1,
            epoch: -1,
            ..Chunk::default()
        };
        c.init = b.get()? != 0;
        if c.init {
            c.e_tick_stamp = b.get_int()?;
            c.epoch = b.get_int()?;
            c.moisture = b.get_float()?;
            c.minerals = b.get_float()?;
            c.soil = i32::from(b.get()?);
        }
        Ok(c)
    }
}

/// The category class `ErosionRegions.getCategory(regionID, categoryID)` returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    NatureTrees,
    NatureBush,
    NaturePlants,
    NatureGeneric,
    StreetCracks,
    WallVines,
    WallCracks,
    Flowerbed,
}

/// zombie.erosion.ErosionRegions static init: region 0 (blends_natural_01) trees, bushes, plants, generic; region 1
/// street cracks; region 2 wall vines, wall cracks; region 3 flower beds.
pub fn get_category(region_id: i32, category_id: i32) -> Option<Category> {
    use Category::*;
    Some(match (region_id, category_id) {
        (0, 0) => NatureTrees,
        (0, 1) => NatureBush,
        (0, 2) => NaturePlants,
        (0, 3) => NatureGeneric,
        (1, 0) => StreetCracks,
        (2, 0) => WallVines,
        (2, 1) => WallCracks,
        (3, 0) => Flowerbed,
        _ => return None,
    })
}

/// The `curId` initialiser of StreetCracks, WallVines and WallCracks `CategoryData`.
pub const NO_CUR_ID: i32 = -999999;

/// zombie.erosion.categories.ErosionCategory.Data and the subclass fields (unused ones stay at their defaults).
#[derive(Clone, Debug, PartialEq)]
pub struct CategoryData {
    pub category: Category,
    pub region_id: i32,
    pub category_id: i32,
    pub do_nothing: bool,
    pub has_spawned: bool,
    pub stage: i32,
    pub disp_season: i32,
    pub disp_bloom: bool,
    pub game_obj: i32,
    pub max_stage: i32,
    pub spawn_time: i32,
    pub not_grass: bool,
    pub cur_id: i32,
    pub has_grass: bool,
    pub alpha: f32,
    /// `hasTop` of WallVines.CategoryData and WallCracks.CategoryData: another data object of the same category
    /// (its `region_id` and `category_id` stay 0 and only the fields the saves carry are set by a load).
    pub has_top: Option<Box<CategoryData>>,
}

impl CategoryData {
    /// allocData of every erosion category class (zombie.erosion.categories): a data object of that category with the field initialisers.
    pub fn alloc(category: Category, region_id: i32, category_id: i32) -> Self {
        let has_cur_id = matches!(
            category,
            Category::StreetCracks | Category::WallVines | Category::WallCracks
        );
        CategoryData {
            category,
            region_id,
            category_id,
            do_nothing: false,
            has_spawned: false,
            stage: 0,
            disp_season: 0,
            disp_bloom: false,
            game_obj: 0,
            max_stage: 0,
            spawn_time: 0,
            not_grass: false,
            cur_id: if has_cur_id { NO_CUR_ID } else { 0 },
            has_grass: false,
            alpha: 0.0,
            has_top: None,
        }
    }

    /// zombie.erosion.categories.ErosionCategory.Data.save and the subclass saves.
    pub fn save(&self, out: &mut Vec<u8>) {
        let mut flags: u8 = 0;
        if self.do_nothing {
            flags |= 1;
        }
        if self.has_spawned {
            flags |= 2;
        }
        if self.disp_bloom {
            flags |= 4;
        }
        if self.stage == 1 {
            flags |= 8;
        } else if self.stage == 2 {
            flags |= 16;
        } else if self.stage == 3 {
            flags |= 32;
        } else if self.stage == 4 {
            flags |= 64;
        } else if self.stage > 4 {
            flags |= 128;
        }
        out.push(self.region_id as u8);
        out.push(self.category_id as u8);
        out.push(self.disp_season as u8);
        out.push(flags);
        if self.stage > 4 {
            out.push(self.stage as u8);
        }
        use Category::*;
        out.push(self.game_obj as u8);
        match self.category {
            Flowerbed => {}
            NatureBush | NatureTrees => {
                out.push(self.max_stage as u8);
                out.extend_from_slice(&(self.spawn_time as i16).to_be_bytes());
            }
            NatureGeneric => {
                out.push(self.max_stage as u8);
                out.extend_from_slice(&(self.spawn_time as i16).to_be_bytes());
                out.push(u8::from(self.not_grass));
            }
            NaturePlants => out.extend_from_slice(&(self.spawn_time as i16).to_be_bytes()),
            StreetCracks => {
                out.push(self.max_stage as u8);
                out.extend_from_slice(&(self.spawn_time as i16).to_be_bytes());
                out.extend_from_slice(&self.cur_id.to_be_bytes());
                out.push(u8::from(self.has_grass));
            }
            WallCracks => {
                out.extend_from_slice(&(self.spawn_time as i16).to_be_bytes());
                out.extend_from_slice(&self.cur_id.to_be_bytes());
                out.extend_from_slice(&self.alpha.to_be_bytes());
                match &self.has_top {
                    Some(t) => {
                        out.push(1);
                        out.push(t.game_obj as u8);
                        out.extend_from_slice(&(t.spawn_time as i16).to_be_bytes());
                        out.extend_from_slice(&t.cur_id.to_be_bytes());
                        out.extend_from_slice(&t.alpha.to_be_bytes());
                    }
                    None => out.push(0),
                }
            }
            WallVines => {
                out.push(self.max_stage as u8);
                out.extend_from_slice(&(self.spawn_time as i16).to_be_bytes());
                out.extend_from_slice(&self.cur_id.to_be_bytes());
                match &self.has_top {
                    Some(t) => {
                        out.push(1);
                        out.push(t.game_obj as u8);
                        out.extend_from_slice(&(t.spawn_time as i16).to_be_bytes());
                        out.extend_from_slice(&t.cur_id.to_be_bytes());
                    }
                    None => out.push(0),
                }
            }
        }
    }
}

/// zombie.erosion.categories.ErosionCategory.loadCategoryData: region and category bytes, then the subclass load.
pub fn load_category_data(b: &mut ByteBuffer) -> Result<CategoryData, String> {
    let region_id = i32::from(b.get()?);
    let category_id = i32::from(b.get()?);
    let category = get_category(region_id, category_id).ok_or_else(|| {
        format!("java.lang.IndexOutOfBoundsException: erosion category {region_id}/{category_id}")
    })?;
    // ErosionCategory.Data.load
    let disp_season = i32::from(b.get()?);
    let flags = i32::from(b.get()?);
    let stage = if flags & 8 != 0 {
        1
    } else if flags & 16 != 0 {
        2
    } else if flags & 32 != 0 {
        3
    } else if flags & 64 != 0 {
        4
    } else if flags & 128 != 0 {
        i32::from(b.get()?)
    } else {
        0
    };
    let mut d = CategoryData {
        category,
        region_id,
        category_id,
        do_nothing: flags & 1 != 0,
        has_spawned: flags & 2 != 0,
        stage,
        disp_season,
        disp_bloom: flags & 4 != 0,
        game_obj: 0,
        max_stage: 0,
        spawn_time: 0,
        not_grass: false,
        cur_id: 0,
        has_grass: false,
        alpha: 0.0,
        has_top: None,
    };
    use Category::*;
    d.game_obj = i32::from(b.get()?);
    match category {
        Flowerbed => {}
        NatureBush | NatureTrees => {
            d.max_stage = i32::from(b.get()?);
            d.spawn_time = i32::from(b.get_short()?);
        }
        NatureGeneric => {
            d.max_stage = i32::from(b.get()?);
            d.spawn_time = i32::from(b.get_short()?);
            d.not_grass = b.get()? != 0;
        }
        NaturePlants => d.spawn_time = i32::from(b.get_short()?),
        StreetCracks => {
            d.max_stage = i32::from(b.get()?);
            d.spawn_time = i32::from(b.get_short()?);
            d.cur_id = b.get_int()?;
            d.has_grass = b.get()? != 0;
        }
        WallCracks => {
            d.spawn_time = i32::from(b.get_short()?);
            d.cur_id = b.get_int()?;
            d.alpha = b.get_float()?;
            if b.get()? != 0 {
                let mut t = CategoryData::alloc(category, 0, 0);
                t.game_obj = i32::from(b.get()?);
                t.spawn_time = i32::from(b.get_short()?);
                t.cur_id = b.get_int()?;
                t.alpha = b.get_float()?;
                d.has_top = Some(Box::new(t));
            }
        }
        WallVines => {
            d.max_stage = i32::from(b.get()?);
            d.spawn_time = i32::from(b.get_short()?);
            d.cur_id = b.get_int()?;
            if b.get()? != 0 {
                let mut t = CategoryData::alloc(category, 0, 0);
                t.game_obj = i32::from(b.get()?);
                t.spawn_time = i32::from(b.get_short()?);
                t.cur_id = b.get_int()?;
                d.has_top = Some(Box::new(t));
            }
        }
    }
    Ok(d)
}

/// zombie.erosion.ErosionData.Square, the saved part.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Square {
    pub init: bool,
    pub do_nothing: bool,
    pub noise_main_byte: i8,
    pub noise_main: f32,
    pub noise_main_int: i32,
    pub soil: i32,
    pub magic_num_byte: i8,
    pub magic_num: f32,
    /// zombie.erosion.ErosionData.Square.noiseKudzu: not saved.
    pub noise_kudzu: f32,
    pub regions: Vec<CategoryData>,
}

/// zombie.erosion.ErosionData.Square.rands: the 90 by 90 table of percentages `rand` reads on a server.
pub const RANDS_LEN: usize = 90 * 90;

/// The class holding the table (`private static final int[] rands` of `ErosionData.Square`).
pub const RANDS_CLASS: &str = "zombie/erosion/ErosionData$Square.class";

/// Read a JVM small integer constant from an array-initializer instruction (iconst, bipush, sipush).
fn jvm_int_constant(code: &[u8], pos: &mut usize) -> Option<i32> {
    let op = *code.get(*pos)?;
    *pos += 1;
    match op {
        0x02..=0x08 => Some(i32::from(op) - 3),
        0x10 => {
            let n = *code.get(*pos)? as i8;
            *pos += 1;
            Some(i32::from(n))
        }
        0x11 => {
            let n = i16::from_be_bytes(code.get(*pos..*pos + 2)?.try_into().ok()?);
            *pos += 2;
            Some(i32::from(n))
        }
        _ => None,
    }
}

/// zombie.erosion.ErosionData.Square.rands, read from the compiled `ErosionData$Square` class of the player's jar: the
/// straight-line initializer `sipush 8100; newarray int; (dup; index; value; iastore)*; putstatic`. Anything else
/// (another game version) is refused instead of guessed.
pub fn rands_from_class(class: &[u8]) -> Result<Vec<i32>, String> {
    if !class.starts_with(&[0xca, 0xfe, 0xba, 0xbe]) {
        return Err("not a Java class".into());
    }
    let len = RANDS_LEN as i16;
    let b = len.to_be_bytes();
    let marker = [0x11, b[0], b[1], 0xbc, 0x0a];
    let mut found: Vec<Vec<i32>> = Vec::new();
    for start in 0..class.len().saturating_sub(marker.len()) {
        if !class[start..].starts_with(&marker) {
            continue;
        }
        let mut pos = start + marker.len();
        let mut values = Vec::with_capacity(RANDS_LEN);
        let mut valid = true;
        for index in 0..RANDS_LEN {
            if class.get(pos) != Some(&0x59) {
                valid = false;
                break;
            }
            pos += 1;
            let idx = jvm_int_constant(class, &mut pos);
            let value = jvm_int_constant(class, &mut pos);
            if idx != Some(index as i32) || class.get(pos) != Some(&0x4f) || value.is_none() {
                valid = false;
                break;
            }
            values.push(value.unwrap_or_default());
            pos += 1;
        }
        if valid && class.get(pos) == Some(&0xb3) {
            found.push(values);
        }
    }
    if found.len() != 1 {
        return Err("unsupported or ambiguous ErosionData.Square.rands initializer".into());
    }
    Ok(found.remove(0))
}

/// zombie.erosion.ErosionData.Square.rands from the installed game's `projectzomboid.jar`.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_rands(game_root: &std::path::Path) -> Result<Vec<i32>, String> {
    use std::io::Read;
    let jar = game_root.join("projectzomboid.jar");
    let file = std::fs::File::open(&jar).map_err(|e| format!("{}: {e}", jar.display()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("{}: {e}", jar.display()))?;
    let entry = zip.by_name(RANDS_CLASS).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    entry
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("ErosionData$Square is too large".into());
    }
    rands_from_class(&bytes)
}

impl Square {
    /// zombie.erosion.ErosionData.Square.reset
    pub fn reset(&mut self) {
        self.init = false;
        self.do_nothing = false;
        self.regions.clear();
    }

    /// zombie.erosion.ErosionData.Square.rand on a server (`GameServer.server`): the table entry of the square, as a
    /// share of `max` clamped to `max - 1`. The single-player branch (the square's `RandLocation`, else the global
    /// `Rand`) is not ported.
    pub fn rand(&self, rands: &[i32], x: i32, y: i32, max: i32) -> i32 {
        let x = if x < 0 { x.wrapping_neg() } else { x };
        let y = if y < 0 { y.wrapping_neg() } else { y };
        let rand = rands[(x % 90 + y % 90 * 90) as usize] as f32 / 100.0;
        ((rand * max as f32) as i32).min(max - 1)
    }

    /// zombie.erosion.ErosionData.Square.save
    pub fn save(&self, out: &mut Vec<u8>) {
        if !self.init {
            out.push(0);
            return;
        }
        let mut flags: u8 = 1;
        if self.do_nothing {
            flags |= 2;
        }
        let size = self.regions.len();
        // the flags byte is written last, at the position the first byte was reserved at
        let mut body = vec![
            self.noise_main_byte as u8,
            self.soil as u8,
            self.magic_num_byte as u8,
        ];
        if size == 1 {
            flags |= 4;
        } else if size == 2 {
            flags |= 8;
        } else if size == 3 {
            flags |= 16;
        } else if size == 4 {
            flags |= 32;
        } else if size > 4 {
            flags |= 64;
            body.push(size as u8);
        }
        for r in &self.regions {
            r.save(&mut body);
        }
        out.push(flags);
        out.extend_from_slice(&body);
    }

    /// zombie.erosion.ErosionData.Square.load
    pub fn load(b: &mut ByteBuffer) -> Result<Self, String> {
        let flags = i32::from(b.get()?);
        let mut s = Square {
            init: flags & 1 != 0,
            ..Square::default()
        };
        if s.init {
            s.do_nothing = flags & 2 != 0;
            s.noise_main_byte = b.get()?;
            s.noise_main = unpack_byte_to_float_unit(s.noise_main_byte);
            s.noise_main_int = (s.noise_main * 100.0).floor() as i32;
            s.soil = i32::from(b.get()?);
            s.magic_num_byte = b.get()?;
            s.magic_num = unpack_byte_to_float_unit(s.magic_num_byte);
            let count = if flags & 4 != 0 {
                1
            } else if flags & 8 != 0 {
                2
            } else if flags & 16 != 0 {
                3
            } else if flags & 32 != 0 {
                4
            } else if flags & 64 != 0 {
                i32::from(b.get()?)
            } else {
                0
            };
            for _ in 0..count {
                s.regions.push(load_category_data(b)?);
            }
        }
        Ok(s)
    }
}
