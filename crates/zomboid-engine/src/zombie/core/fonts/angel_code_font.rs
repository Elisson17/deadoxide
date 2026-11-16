//! zombie.core.fonts.AngelCodeFont: a bitmap font from an AngelCode BMFont text file (`.fnt`): its characters,
//! kerning and line height, and the text measuring the UI does with them. Text is Java's: UTF-16 code units.
//!
//! What the original reads: the first two lines (`info`, `common`) are read and dropped, so the file's `lineHeight`,
//! `base`, `padding`, `spacing` and `scaleW/H` are never used; the line height is derived from the characters.
//! Then `page` lines (each asks `Texture.getSharedTexture` for `<folder of the .fnt>/<file>`), `char` lines (fields by
//! position: id x y width height xoffset yoffset xadvance page; `chnl` and later fields ignored) and `kerning` lines.
//!
//! Not ported (needs the renderer): `Texture` loading (`fontImage`, the page textures: the port keeps the names the
//! original asks for), `CharDef.init`/`draw`/`destroy`'s texture side, `CharDefTexture`, `drawString`/`render`, the GL
//! display lists (`displayListCaching` is never set, so the original never uses its caches either),
//! `onStateChanged`, `isLoading`. The constructor `AngelCodeFont(String, Texture)` is not ported (nothing in the game
//! calls it with page lines it could resolve: its `fntFile` stays null).
use std::collections::HashMap;

use crate::java::{self, StringTokenizer};
use crate::zombie::util::string_utils;
use crate::zombie::zomboid_file_system::ZomboidFileSystem;

/// zombie.core.fonts.AngelCodeFont.CharDef (without the texture `image`, always null without a renderer).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CharDef {
    pub dl_index: i16,
    pub height: i16,
    pub id: i32,
    pub kerning_second: Option<Vec<i16>>,
    pub kerning_amount: Option<Vec<i16>>,
    pub width: i16,
    pub x: i16,
    pub xadvance: i16,
    pub xoffset: i16,
    pub y: i16,
    pub yoffset: i16,
    pub page: i16,
}

impl CharDef {
    /// zombie.core.fonts.AngelCodeFont.CharDef.getKerning: binary search over the sorted second characters (shorts,
    /// compared with the int code unit).
    pub fn get_kerning(&self, other_code_point: i32) -> i32 {
        let (Some(second), Some(amount)) = (&self.kerning_second, &self.kerning_amount) else {
            return 0;
        };
        let mut low: i32 = 0;
        let mut high: i32 = second.len() as i32 - 1;
        while low <= high {
            let mid_index = ((low.wrapping_add(high)) as u32 >> 1) as i32;
            let s = i32::from(second[mid_index as usize]);
            if s < other_code_point {
                low = mid_index + 1;
            } else {
                if s <= other_code_point {
                    return i32::from(amount[mid_index as usize]);
                }
                high = mid_index - 1;
            }
        }
        0
    }
}

/// zombie.core.fonts.AngelCodeFont, without its textures.
#[derive(Clone, Debug, Default)]
pub struct AngelCodeFont {
    /// AngelCodeFont.chars: indexed by character id, `maxChar + 1` long.
    pub chars: Vec<Option<CharDef>>,
    line_height: i32,
    /// The `imgFile` the constructor asks `Texture.getSharedTexture` for (the font image), when it is not blank.
    pub font_image: Option<String>,
    /// The page textures `parseFnt` asks `Texture.getSharedTexture` for: (page id, file name) in file order. The
    /// original's `pages` map holds the loaded textures; without a renderer it stays empty.
    pub page_textures: Vec<(i16, String)>,
    /// AngelCodeFont.fntFile (`java.io.File`'s path).
    pub fnt_file: Option<String>,
    sdf: bool,
    /// What the original logs (`DebugType.General.error`) while loading.
    pub log: Vec<String>,
}

impl AngelCodeFont {
    /// zombie.core.fonts.AngelCodeFont.<init>(String fntFile, String imgFile): a leading '/' is dropped and
    /// backslashes become slashes, the path goes through `ZomboidFileSystem.getString`, then `parseFnt`. `Err` is the
    /// exception the original throws out of the constructor (FileNotFoundException, or a RuntimeException of
    /// `parseFnt`).
    pub fn new(
        fs: &ZomboidFileSystem,
        fnt_file: &str,
        img_file: Option<&str>,
    ) -> Result<AngelCodeFont, String> {
        let mut font = AngelCodeFont::default();
        if !string_utils::is_null_or_whitespace(img_file) {
            font.font_image = img_file.map(str::to_string);
        }
        let path = fnt_file.strip_prefix('/').unwrap_or(fnt_file);
        let path = path.replace('\\', "/");
        let resolved = fs.get_string_or(&path);
        font.fnt_file = Some(java::file_normalize(resolved));
        let open = fs.open_path(resolved);
        let bytes = std::fs::read(&open).map_err(|e| {
            format!(
                "java.io.FileNotFoundException: {resolved} ({})",
                if e.kind() == std::io::ErrorKind::NotFound {
                    "No such file or directory".to_string()
                } else {
                    e.to_string()
                }
            )
        })?;
        font.parse_fnt(&bytes)?;
        Ok(font)
    }

    /// zombie.core.fonts.AngelCodeFont.getHeight(String)
    pub fn get_height(&self, text: &[u16]) -> i32 {
        self.get_height_ext(text, false, false)
    }

    /// zombie.core.fonts.AngelCodeFont.getHeight(String, boolean, boolean): the number of lines times the line
    /// height; or the tallest `height + yoffset` of the last line minus the smallest `yoffset` of all lines
    /// (`returnActualHeight`); or that smallest `yoffset` (`returnOffset`). Spaces and characters the font lacks are
    /// skipped.
    pub fn get_height_ext(
        &self,
        text: &[u16],
        return_actual_height: bool,
        return_offset: bool,
    ) -> i32 {
        let mut lines: i32 = 1;
        let mut max_height: i32 = 0;
        let mut min_offset: i32 = 1_000_000;
        for &unit in text {
            let id = i32::from(unit);
            if id == 10 {
                lines = lines.wrapping_add(1);
                max_height = 0;
            } else if id != 32 && (id as usize) < self.chars.len() {
                if let Some(char_def) = &self.chars[id as usize] {
                    max_height = (i32::from(char_def.height) + i32::from(char_def.yoffset))
                        .max(max_height);
                    min_offset = i32::from(char_def.yoffset).min(min_offset);
                }
            }
        }
        if return_actual_height {
            max_height.wrapping_sub(min_offset)
        } else if return_offset {
            min_offset
        } else {
            lines.wrapping_mul(self.get_line_height())
        }
    }

    /// zombie.core.fonts.AngelCodeFont.getLineHeight
    pub fn get_line_height(&self) -> i32 {
        self.line_height
    }

    /// zombie.core.fonts.AngelCodeFont.getWidth(String)
    pub fn get_width(&self, text: &[u16]) -> i32 {
        self.get_width_xadvance(text, false)
    }

    /// zombie.core.fonts.AngelCodeFont.getWidth(String, boolean)
    pub fn get_width_xadvance(&self, text: &[u16], xadvance: bool) -> i32 {
        self.width_of(text, 0, text.len() as i32 - 1, xadvance)
    }

    /// zombie.core.fonts.AngelCodeFont.getWidth(String, int, int, boolean): the widest line of `text[start..=end]`:
    /// each character adds the kerning from the previous one (also across a line break) and its `xadvance`, the last
    /// one its `width` unless `xadvance`. `Err` is the StringIndexOutOfBoundsException of `charAt`.
    pub fn get_width_range(
        &self,
        text: &[u16],
        start: i32,
        end: i32,
        xadvance: bool,
    ) -> Result<i32, String> {
        let num_chars = end.wrapping_sub(start).wrapping_add(1);
        if num_chars > 0 && (start < 0 || end >= text.len() as i32) {
            let index = if start < 0 { start } else { text.len() as i32 };
            return Err(format!(
                "java.lang.StringIndexOutOfBoundsException: Index {index} out of bounds for length {}",
                text.len()
            ));
        }
        Ok(self.width_of(text, start, end, xadvance))
    }

    fn width_of(&self, text: &[u16], start: i32, end: i32, xadvance: bool) -> i32 {
        let num_chars = end.wrapping_sub(start).wrapping_add(1);
        let mut max_width: i32 = 0;
        let mut width: i32 = 0;
        let mut last_char_def: Option<&CharDef> = None;
        for i in 0..num_chars.max(0) {
            let id = i32::from(text[(start + i) as usize]);
            if id == 10 {
                width = 0;
            } else if (id as usize) < self.chars.len() {
                if let Some(char_def) = &self.chars[id as usize] {
                    if let Some(last) = last_char_def {
                        width = width.wrapping_add(last.get_kerning(id));
                    }
                    last_char_def = Some(char_def);
                    if !xadvance && i >= num_chars - 1 {
                        width = width.wrapping_add(i32::from(char_def.width));
                    } else {
                        width = width.wrapping_add(i32::from(char_def.xadvance));
                    }
                    max_width = max_width.max(width);
                }
            }
        }
        max_width
    }

    /// zombie.core.fonts.AngelCodeFont.getYOffset: the smallest `yoffset` on the first line (10000 when none).
    /// `Err` is the ArrayIndexOutOfBoundsException the original throws for a character past the font's last id.
    pub fn get_y_offset(&self, text: &[u16]) -> Result<i32, String> {
        let stop_index = text.iter().position(|&u| u == 10).unwrap_or(text.len());
        let mut min_y_offset: i32 = 10000;
        for &unit in &text[..stop_index] {
            let id = usize::from(unit);
            let Some(slot) = self.chars.get(id) else {
                return Err(format!(
                    "java.lang.ArrayIndexOutOfBoundsException: Index {id} out of bounds for length {}",
                    self.chars.len()
                ));
            };
            if let Some(char_def) = slot {
                min_y_offset = i32::from(char_def.yoffset).min(min_y_offset);
            }
        }
        Ok(min_y_offset)
    }

    /// zombie.core.fonts.AngelCodeFont.parseChar: `None` for a negative id (skipped). The line height grows to the
    /// character's `height + yoffset` unless it is the space. (`CharDef.init` would run if the page texture were
    /// ready: never without a renderer.)
    fn parse_char(&mut self, line: &str) -> Result<Option<CharDef>, String> {
        let mut tokens = StringTokenizer::new(line, " =");
        let mut def = CharDef::default();
        tokens.next_token()?;
        tokens.next_token()?;
        def.id = parse_int(tokens.next_token()?)?;
        if def.id < 0 {
            return Ok(None);
        }
        tokens.next_token()?;
        def.x = parse_short(tokens.next_token()?)?;
        tokens.next_token()?;
        def.y = parse_short(tokens.next_token()?)?;
        tokens.next_token()?;
        def.width = parse_short(tokens.next_token()?)?;
        tokens.next_token()?;
        def.height = parse_short(tokens.next_token()?)?;
        tokens.next_token()?;
        def.xoffset = parse_short(tokens.next_token()?)?;
        tokens.next_token()?;
        def.yoffset = parse_short(tokens.next_token()?)?;
        tokens.next_token()?;
        def.xadvance = parse_short(tokens.next_token()?)?;
        tokens.next_token()?;
        def.page = parse_short(tokens.next_token()?)?;
        if def.id != 32 {
            self.line_height =
                (i32::from(def.height) + i32::from(def.yoffset)).max(self.line_height);
        }
        Ok(Some(def))
    }

    /// zombie.core.fonts.AngelCodeFont.parseFnt: the file read as lines (`InputStreamReader` in the default charset,
    /// UTF-8); `info` and `common` dropped; `page`, `char` (not `chars c...`) and `kerning` (not `kernings c...`)
    /// lines in order. Then `chars` is filled (a repeated id keeps the last line) and each first character's kerning
    /// pairs become two arrays: seconds sorted ascending as shorts, each amount taken from the first pair in the
    /// file with that second. `Err` is the RuntimeException the original lets out (NumberFormatException,
    /// NoSuchElementException, and NullPointerException / ArrayIndexOutOfBoundsException for kerning of a missing
    /// character).
    fn parse_fnt(&mut self, bytes: &[u8]) -> Result<(), String> {
        let text = String::from_utf16_lossy(&java::utf8_decode_jdk(bytes));
        let mut lines = java::buffered_reader_lines(&text).into_iter();
        let _info = lines.next();
        let _common = lines.next();
        let mut kerning: HashMap<i16, Vec<i16>> = HashMap::new();
        let mut kerning_order: Vec<i16> = Vec::new();
        let mut char_defs: Vec<CharDef> = Vec::new();
        let mut max_char: i32 = 0;
        for line in lines {
            if line.starts_with("page") {
                let mut tokens = StringTokenizer::new(line, " =");
                tokens.next_token()?;
                tokens.next_token()?;
                let id = parse_short(tokens.next_token()?)?;
                tokens.next_token()?;
                let file = tokens.next_token()?.replace('"', "");
                let parent = self
                    .fnt_file
                    .as_deref()
                    .and_then(java::file_parent)
                    .unwrap_or("null");
                let file = format!("{parent}/{file}").replace('\\', "/");
                // Texture.getSharedTexture(file, flags): no renderer, so null; the original logs it and keeps no page.
                self.log.push(format!(
                    "AngelCodeFont failed to load page {id} texture {file}"
                ));
                self.page_textures.push((id, file));
            }
            if !line.starts_with("chars c") && line.starts_with("char") {
                if let Some(def) = self.parse_char(line)? {
                    max_char = max_char.max(def.id);
                    char_defs.push(def);
                }
            }
            if !line.starts_with("kernings c") && line.starts_with("kerning") {
                let mut tokens = StringTokenizer::new(line, " =");
                tokens.next_token()?;
                tokens.next_token()?;
                let first = parse_short(tokens.next_token()?)?;
                tokens.next_token()?;
                let second = parse_int(tokens.next_token()?)?;
                tokens.next_token()?;
                let offset = parse_int(tokens.next_token()?)?;
                let values = kerning.entry(first).or_insert_with(|| {
                    kerning_order.push(first);
                    Vec::new()
                });
                values.push(second as i16);
                values.push(offset as i16);
            }
        }
        self.chars = vec![None; max_char as usize + 1];
        for def in char_defs {
            let id = def.id as usize;
            self.chars[id] = Some(def);
        }
        // TShortObjectHashMap.forEachEntry: each entry is independent; only which failure comes first could differ.
        for key in kerning_order {
            let value = &kerning[&key];
            let len = self.chars.len();
            let char_def = self
                .chars
                .get_mut(key as i32 as usize)
                .filter(|_| key >= 0)
                .ok_or_else(|| {
                    format!(
                        "java.lang.ArrayIndexOutOfBoundsException: Index {key} out of bounds for length {len}"
                    )
                })?
                .as_mut()
                .ok_or_else(|| "java.lang.NullPointerException".to_string())?;
            let n = value.len() / 2;
            let mut kerning_second = vec![0i16; n];
            let mut kerning_amount = vec![0i16; n];
            let mut j = 0;
            let mut i = 0;
            while i < value.len() {
                kerning_second[j] = value[i];
                kerning_amount[j] = value[i + 1];
                j += 1;
                i += 2;
            }
            let mut sorted_second = kerning_second.clone();
            let copy_amount = kerning_amount.clone();
            sorted_second.sort_unstable();
            for (amount, sorted) in kerning_amount.iter_mut().zip(&sorted_second) {
                for (second, copy) in kerning_second.iter().zip(&copy_amount) {
                    if second == sorted {
                        *amount = *copy;
                        break;
                    }
                }
            }
            char_def.kerning_second = Some(sorted_second);
            char_def.kerning_amount = Some(kerning_amount);
        }
        Ok(())
    }

    /// zombie.core.fonts.AngelCodeFont.isSdf
    pub fn is_sdf(&self) -> bool {
        self.sdf
    }

    /// zombie.core.fonts.AngelCodeFont.setSdf
    pub fn set_sdf(&mut self, b: bool) {
        self.sdf = b;
    }

    /// zombie.core.fonts.AngelCodeFont.destroy: every character gone (the array keeps its length), pages cleared.
    pub fn destroy(&mut self) {
        for c in &mut self.chars {
            *c = None;
        }
        self.page_textures.clear();
    }

    /// The texture names the original asks `Texture.getSharedTexture` for while loading, in order: the font image,
    /// then every page.
    pub fn texture_requests(&self) -> Vec<&str> {
        self.font_image
            .iter()
            .map(String::as_str)
            .chain(self.page_textures.iter().map(|(_, f)| f.as_str()))
            .collect()
    }

    /// `getWidth` of a Rust string (its UTF-16 units).
    pub fn get_width_str(&self, text: &str) -> i32 {
        self.get_width(&text.encode_utf16().collect::<Vec<u16>>())
    }
}

/// Integer.parseInt as the original calls it: `Err` is the NumberFormatException.
fn parse_int(s: &str) -> Result<i32, String> {
    java::parse_int(s)
        .ok_or_else(|| format!("java.lang.NumberFormatException: {}", java::number_format_message(s)))
}

/// Short.parseShort as the original calls it: `Err` is the NumberFormatException.
fn parse_short(s: &str) -> Result<i16, String> {
    match java::parse_int(s) {
        None => Err(format!(
            "java.lang.NumberFormatException: {}",
            java::number_format_message(s)
        )),
        Some(v) => i16::try_from(v).map_err(|_| {
            format!("java.lang.NumberFormatException: Value out of range. Value:\"{s}\" Radix:10")
        }),
    }
}
