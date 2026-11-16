//! zombie.ui.TextManager: which font file each `UIFont` uses (per language and font size option) and the text
//! measuring the UI does. Text is Java's: UTF-16 code units. The original's `TextManager.instance` is a value here;
//! fonts are shared objects (`Rc`), as several `UIFont`s may hold the same `AngelCodeFont`.
//!
//! Not ported (drawing): `DrawString*`, `DrawTextFromGameWorld`/`DeferedTextDraw`/`todoTextList`, `GetDrawTextObject`,
//! `DrawTextObject`, `DrawStringBBcode`, `isAnyFontLoading`, the SDF shader (`sdfShader`) `Init` creates last.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::fonts_file::{self};
use super::fonts_file_font::FontsFileFont;
use super::ui_font::UIFont;
use crate::java;
use crate::zombie::core::fonts::angel_code_font::AngelCodeFont;
use crate::zombie::zomboid_file_system::ZomboidFileSystem;

/// A font as the original shares it (one object behind several `UIFont`s and fields).
pub type FontRef = Rc<RefCell<AngelCodeFont>>;

const NPE: &str = "java.lang.NullPointerException";

/// zombie.ui.TextManager (the font fields and tables).
pub struct TextManager {
    pub font: Option<FontRef>,
    pub font2: Option<FontRef>,
    pub font3: Option<FontRef>,
    pub font4: Option<FontRef>,
    pub main1: Option<FontRef>,
    pub main2: Option<FontRef>,
    pub zombiefontcredits1: Option<FontRef>,
    pub zombiefontcredits2: Option<FontRef>,
    pub zombienew1: Option<FontRef>,
    pub zombienew2: Option<FontRef>,
    pub zombienew3: Option<FontRef>,
    pub zomboid_dialogue: Option<FontRef>,
    pub codetext: Option<FontRef>,
    pub code_small: Option<FontRef>,
    pub code_medium: Option<FontRef>,
    pub code_large: Option<FontRef>,
    pub debug_console: Option<FontRef>,
    pub intro: Option<FontRef>,
    pub handwritten: Option<FontRef>,
    /// TextManager.normal: `zomboidNormal11` .. `zomboidNormal24`.
    pub normal: Vec<Option<FontRef>>,
    /// TextManager.enumToFont, by `UIFont.ordinal()`.
    pub enum_to_font: Vec<Option<FontRef>>,
    pub current_code_font: UIFont,
    /// `GameServer.server && !ServerGUI.isCreated()`: the measuring methods then return 0.
    pub server_without_gui: bool,
    /// What the original logs while reading the fonts files.
    pub log: Vec<String>,
}

impl Default for TextManager {
    fn default() -> Self {
        TextManager::new()
    }
}

impl TextManager {
    /// zombie.ui.TextManager.<init>: no fonts until `Init`.
    pub fn new() -> TextManager {
        TextManager {
            font: None,
            font2: None,
            font3: None,
            font4: None,
            main1: None,
            main2: None,
            zombiefontcredits1: None,
            zombiefontcredits2: None,
            zombienew1: None,
            zombienew2: None,
            zombienew3: None,
            zomboid_dialogue: None,
            codetext: None,
            code_small: None,
            code_medium: None,
            code_large: None,
            debug_console: None,
            intro: None,
            handwritten: None,
            normal: vec![None; 14],
            enum_to_font: vec![None; UIFont::values().len()],
            current_code_font: UIFont::CodeSmall,
            server_without_gui: false,
            log: Vec::new(),
        }
    }

    /// zombie.ui.TextManager.getNormalFromFontSize: `normal[points - 11]`. `Err` is the
    /// ArrayIndexOutOfBoundsException.
    pub fn get_normal_from_font_size(&self, points: i32) -> Result<Option<FontRef>, String> {
        let i = points.wrapping_sub(11);
        usize::try_from(i)
            .ok()
            .and_then(|i| self.normal.get(i))
            .cloned()
            .ok_or_else(|| {
                format!(
                    "java.lang.ArrayIndexOutOfBoundsException: Index {i} out of bounds for length {}",
                    self.normal.len()
                )
            })
    }

    /// zombie.ui.TextManager.getFontFromEnum: the font of `font`, else `this.font` (also for a null `font`).
    pub fn get_font_from_enum(&self, font: Option<UIFont>) -> Option<FontRef> {
        match font {
            None => self.font.clone(),
            Some(f) => self.enum_to_font[f.ordinal()]
                .clone()
                .or_else(|| self.font.clone()),
        }
    }

    /// zombie.ui.TextManager.getFontHeight: the font's line height. `Err` is the NullPointerException of a missing font
    /// (before `Init`).
    pub fn get_font_height(&self, font_id: Option<UIFont>) -> Result<i32, String> {
        let font = self.get_font_from_enum(font_id).ok_or(NPE)?;
        let h = font.borrow().get_line_height();
        Ok(h)
    }

    /// zombie.ui.TextManager.isSdf
    pub fn is_sdf(&self, font: Option<UIFont>) -> Result<bool, String> {
        let f = self.get_font_from_enum(font).ok_or(NPE)?;
        let b = f.borrow().is_sdf();
        Ok(b)
    }

    /// zombie.ui.TextManager.getAllFonts: every `UIFont`, sorted by name with `compareToIgnoreCase` (stable).
    pub fn get_all_fonts(&self) -> Vec<UIFont> {
        let mut result = UIFont::values().to_vec();
        result.sort_by(|a, b| java::compare_to_ignore_case(a.name(), b.name()));
        result
    }

    /// zombie.ui.TextManager.getFontFilePath: the first existing of `media/fonts/<lang>/<sizeDir>/<file>`,
    /// `media/fonts/<lang>/<file>`, then for a language other than EN the same under `EN`, then `media/fonts/<file>`;
    /// else `media/<file>` unchecked. "Existing" is `ZomboidFileSystem.getString` finding it (any letter case).
    pub fn get_font_file_path(
        fs: &ZomboidFileSystem,
        lang: &str,
        size_dir: Option<&str>,
        file_name: &str,
    ) -> String {
        if let Some(size_dir) = size_dir {
            let path = format!("media/fonts/{lang}/{size_dir}/{file_name}");
            if fs.get_string(&path).is_some() {
                return path;
            }
        }
        let path = format!("media/fonts/{lang}/{file_name}");
        if fs.get_string(&path).is_some() {
            return path;
        }
        if lang != "EN" {
            if let Some(size_dir) = size_dir {
                let path = format!("media/fonts/EN/{size_dir}/{file_name}");
                if fs.get_string(&path).is_some() {
                    return path;
                }
            }
            let path = format!("media/fonts/EN/{file_name}");
            if fs.get_string(&path).is_some() {
                return path;
            }
        }
        let path = format!("media/fonts/{file_name}");
        if fs.get_string(&path).is_some() {
            path
        } else {
            format!("media/{file_name}")
        }
    }

    /// zombie.ui.TextManager.Init. `lang` is `Translator.getLanguage().name()`, `option_enable_dyslexic_font`
    /// `Core.getOptionEnableDyslexicFont()`, `option_font_size_real` `Core.getOptionFontSizeReal()`
    /// (`zombie::core::core::get_option_font_size_real`). The fonts table: `fontsDyslexic.txt` of the language when
    /// the dyslexic font is on, else (or when that gives nothing) EN's `fonts.txt` overlaid with the language's. Font
    /// size 2..5 picks the `1x`..`4x` folders. Old fonts are destroyed; each `UIFont` of the table gets its font
    /// (one object per `fnt|img` pair), DebugConsole falls back to Small, the 14 `zomboidNormal` fonts load, the
    /// fields are set, and MediumNew and the AutoNorm fonts end up null. `Err` is the exception of a font that
    /// fails to load (the state then is what the original leaves).
    pub fn init(
        &mut self,
        fs: &ZomboidFileSystem,
        lang: &str,
        option_enable_dyslexic_font: bool,
        option_font_size_real: i32,
    ) -> Result<(), String> {
        let mut fonts_file_font_map: HashMap<String, FontsFileFont> = HashMap::new();
        if option_enable_dyslexic_font {
            let path = format!("media/fonts/{lang}/fontsDyslexic.txt");
            let fonts_file_path = fs.get_string_or(&path);
            fonts_file::read(fs, fonts_file_path, &mut fonts_file_font_map, &mut self.log);
        }
        if fonts_file_font_map.is_empty() {
            let fonts_file_path = fs.get_string_or("media/fonts/EN/fonts.txt");
            fonts_file::read(fs, fonts_file_path, &mut fonts_file_font_map, &mut self.log);
            if lang != "EN" {
                let path = format!("media/fonts/{lang}/fonts.txt");
                let fonts_file_path = fs.get_string_or(&path);
                fonts_file::read(fs, fonts_file_path, &mut fonts_file_font_map, &mut self.log);
            }
        }
        let mut loaded: HashMap<String, FontRef> = HashMap::new();
        let size_dir = match option_font_size_real {
            2 => Some("1x"),
            3 => Some("2x"),
            4 => Some("3x"),
            5 => Some("4x"),
            _ => None,
        };
        for ac_font in self.enum_to_font.iter().flatten() {
            ac_font.borrow_mut().destroy();
        }
        self.enum_to_font.iter_mut().for_each(|f| *f = None);
        for ac_font in self.normal.iter().flatten() {
            ac_font.borrow_mut().destroy();
        }
        self.normal.iter_mut().for_each(|f| *f = None);
        for &ui_font in UIFont::values() {
            let Some(fonts_file_font) = fonts_file_font_map.get(ui_font.name()) else {
                continue;
            };
            let fnt = Self::get_font_file_path(fs, lang, size_dir, &fonts_file_font.fnt);
            let img = fonts_file_font
                .img
                .as_ref()
                .map(|img| Self::get_font_file_path(fs, lang, size_dir, img));
            let key = format!("{fnt}|{}", img.as_deref().unwrap_or("null"));
            if let Some(f) = loaded.get(&key) {
                self.enum_to_font[ui_font.ordinal()] = Some(f.clone());
            } else {
                let mut angel_code_font = AngelCodeFont::new(fs, &fnt, img.as_deref())?;
                angel_code_font.set_sdf(
                    fonts_file_font
                        .r#type
                        .as_deref()
                        .is_some_and(|t| java::equals_ignore_case("sdf", t)),
                );
                let f = Rc::new(RefCell::new(angel_code_font));
                self.enum_to_font[ui_font.ordinal()] = Some(f.clone());
                loaded.insert(key, f);
            }
        }
        if self.enum_to_font[UIFont::DebugConsole.ordinal()].is_none() {
            self.enum_to_font[UIFont::DebugConsole.ordinal()] =
                self.enum_to_font[UIFont::Small.ordinal()].clone();
        }
        for n in 0..self.normal.len() {
            let f = AngelCodeFont::new(
                fs,
                &format!("media/fonts/zomboidNormal{}.fnt", n + 11),
                Some(&format!("media/fonts/zomboidNormal{}_0", n + 11)),
            )?;
            self.normal[n] = Some(Rc::new(RefCell::new(f)));
        }
        let e = |f: UIFont| self.enum_to_font[f.ordinal()].clone();
        self.font = e(UIFont::Small);
        self.font2 = e(UIFont::Medium);
        self.font3 = e(UIFont::Large);
        self.font4 = e(UIFont::Massive);
        self.main1 = e(UIFont::MainMenu1);
        self.main2 = e(UIFont::MainMenu2);
        self.zombiefontcredits1 = e(UIFont::Cred1);
        self.zombiefontcredits2 = e(UIFont::Cred2);
        self.zombienew1 = e(UIFont::NewSmall);
        self.zombienew2 = e(UIFont::NewMedium);
        self.zombienew3 = e(UIFont::NewLarge);
        self.codetext = e(UIFont::Code);
        self.code_small = e(UIFont::CodeSmall);
        self.code_medium = e(UIFont::CodeMedium);
        self.code_large = e(UIFont::CodeLarge);
        for f in [
            UIFont::MediumNew,
            UIFont::AutoNormSmall,
            UIFont::AutoNormMedium,
            UIFont::AutoNormLarge,
        ] {
            self.enum_to_font[f.ordinal()] = None;
        }
        let e = |f: UIFont| self.enum_to_font[f.ordinal()].clone();
        self.zomboid_dialogue = e(UIFont::Dialogue);
        self.intro = e(UIFont::Intro);
        self.handwritten = e(UIFont::Handwritten);
        self.debug_console = e(UIFont::DebugConsole);
        Ok(())
    }

    /// zombie.ui.TextManager.isUsingNonEnglishFonts: the language is not EN and has its own `fonts.txt`.
    pub fn is_using_non_english_fonts(fs: &ZomboidFileSystem, lang: &str) -> bool {
        if java::equals_ignore_case("EN", lang) {
            false
        } else {
            fs.get_string(&format!("media/fonts/{lang}/fonts.txt"))
                .is_some()
        }
    }

    /// zombie.ui.TextManager.MeasureStringX: the font's `getWidth`; 0 on a server without GUI or for a null string.
    pub fn measure_string_x(
        &self,
        font: Option<UIFont>,
        str: Option<&[u16]>,
    ) -> Result<i32, String> {
        if self.server_without_gui {
            return Ok(0);
        }
        let Some(str) = str else {
            return Ok(0);
        };
        let to_use = self.get_font_from_enum(font).ok_or(NPE)?;
        let w = to_use.borrow().get_width(str);
        Ok(w)
    }

    /// zombie.ui.TextManager.CentreStringYOffset
    pub fn centre_string_y_offset(
        &self,
        font: Option<UIFont>,
        str: Option<&[u16]>,
    ) -> Result<i32, String> {
        let offset = self.measure_string_y_offset(font, str)?;
        let y = self.measure_string_y(font, str)?;
        let real = self.measure_string_y_real(font, str)?;
        Ok(offset.wrapping_sub(y.wrapping_sub(real) / 2))
    }

    /// zombie.ui.TextManager.MeasureStringY(UIFont, String)
    pub fn measure_string_y(
        &self,
        font: Option<UIFont>,
        str: Option<&[u16]>,
    ) -> Result<i32, String> {
        self.measure_string_y_ext(font, str, false, false)
    }

    /// zombie.ui.TextManager.MeasureStringYReal
    pub fn measure_string_y_real(
        &self,
        font: Option<UIFont>,
        str: Option<&[u16]>,
    ) -> Result<i32, String> {
        self.measure_string_y_ext(font, str, true, false)
    }

    /// zombie.ui.TextManager.MeasureStringYOffset
    pub fn measure_string_y_offset(
        &self,
        font: Option<UIFont>,
        str: Option<&[u16]>,
    ) -> Result<i32, String> {
        self.measure_string_y_ext(font, str, false, true)
    }

    /// zombie.ui.TextManager.MeasureStringY(UIFont, String, boolean, boolean): the font's `getHeight`; 0 for a null
    /// font or string, or on a server without GUI.
    pub fn measure_string_y_ext(
        &self,
        font: Option<UIFont>,
        str: Option<&[u16]>,
        return_actual_height: bool,
        return_offset: bool,
    ) -> Result<i32, String> {
        let (Some(_), Some(str)) = (font, str) else {
            return Ok(0);
        };
        if self.server_without_gui {
            return Ok(0);
        }
        let to_use = self.get_font_from_enum(font).ok_or(NPE)?;
        let h = to_use
            .borrow()
            .get_height_ext(str, return_actual_height, return_offset);
        Ok(h)
    }

    /// zombie.ui.TextManager.MeasureFont: fixed numbers for Small (10), Dialogue and Medium (20), Large (24),
    /// Massive and the main menu fonts (30), else the font's line height.
    pub fn measure_font(&self, font: Option<UIFont>) -> Result<i32, String> {
        Ok(match font {
            Some(UIFont::Small) => 10,
            Some(UIFont::Dialogue) => 20,
            Some(UIFont::Medium) => 20,
            Some(UIFont::Large) => 24,
            Some(UIFont::Massive) => 30,
            Some(UIFont::MainMenu1) => 30,
            Some(UIFont::MainMenu2) => 30,
            _ => {
                let f = self.get_font_from_enum(font).ok_or(NPE)?;
                let h = f.borrow().get_line_height();
                h
            }
        })
    }

    /// zombie.ui.TextManager.WrapText(UIFont, String, int): no line limit.
    pub fn wrap_text(
        &self,
        font: Option<UIFont>,
        str: &[u16],
        max_width: i32,
    ) -> Result<Vec<u16>, String> {
        self.wrap_text_ext(font, str, max_width, -1, &[])
    }

    /// zombie.ui.TextManager.WrapText(UIFont, String, int, int, String): lines split at `\r?\n` (trailing empty lines
    /// dropped); a line wider than `maxWidth` is split at single spaces into words, packed greedily; a word that
    /// does not fit on an empty line is cut into `floor(maxWidth / width * length) - 1` (at least 5) character
    /// pieces ending in "-", the last piece starting the next line. With `maxLines`, the result is cut to that many
    /// lines and the suffix appended to the last (replacing its last characters when there is no room). `Err` is
    /// the StringIndexOutOfBoundsException the original throws for an empty word wider than a negative width.
    pub fn wrap_text_ext(
        &self,
        font: Option<UIFont>,
        str: &[u16],
        max_width: i32,
        max_lines: i32,
        max_lines_suffix: &[u16],
    ) -> Result<Vec<u16>, String> {
        let mut out_lines: Vec<Vec<u16>> = Vec::new();
        let in_lines = split_line_breaks(str);
        let space_width = self.measure_string_x(font, Some(&[32u16][..]))?;
        let min_chars_per_line = 5;
        for line in &in_lines {
            let line_width = self.measure_string_x(font, Some(line))?;
            if line_width <= max_width {
                out_lines.push(line.to_vec());
            } else {
                let words = split_unit(line, 32);
                let mut new_line: Vec<Vec<u16>> = Vec::new();
                let mut current_line_length: i32 = 0;
                for (word_idx, &word) in words.iter().enumerate() {
                    let new_line_length =
                        current_line_length.wrapping_add(self.measure_string_x(font, Some(word))?);
                    if new_line_length <= max_width {
                        new_line.push(word.to_vec());
                        current_line_length = current_line_length
                            .wrapping_add(self.measure_string_x(font, Some(word))?)
                            .wrapping_add(space_width);
                    } else if !new_line.is_empty() {
                        out_lines.push(join(&[32], &new_line));
                        new_line.clear();
                        new_line.push(word.to_vec());
                        current_line_length = self
                            .measure_string_x(font, Some(word))?
                            .wrapping_add(space_width);
                    } else {
                        let len = word.len() as i32;
                        let mut chars_per_line_approx =
                            (f64::from(max_width) / f64::from(new_line_length) * f64::from(len))
                                .floor() as i32
                                - 1;
                        chars_per_line_approx = chars_per_line_approx.max(min_chars_per_line);
                        let lines =
                            (f64::from(len) / f64::from(chars_per_line_approx)).ceil() as i32;
                        for line in 0..lines - 1 {
                            let mut piece = substring(
                                word,
                                line * chars_per_line_approx,
                                (line + 1) * chars_per_line_approx,
                            )?
                            .to_vec();
                            piece.push(u16::from(b'-'));
                            out_lines.push(piece);
                        }
                        let final_part = substring(word, (lines - 1) * chars_per_line_approx, len)?;
                        new_line.push(final_part.to_vec());
                        current_line_length = self
                            .measure_string_x(font, Some(final_part))?
                            .wrapping_add(space_width);
                    }
                    if word_idx == words.len() - 1 {
                        out_lines.push(join(&[32], &new_line));
                        // (the original also zeroes currentLineLength here; the loop ends)
                        new_line.clear();
                    }
                }
            }
        }
        if max_lines > 0 && out_lines.len() > max_lines as usize {
            let last = max_lines as usize - 1;
            let suffix_length = self.measure_string_x(font, Some(max_lines_suffix))?;
            let last_width = self.measure_string_x(font, Some(&out_lines[last]))?;
            if max_width.wrapping_sub(last_width) < suffix_length {
                let final_line = &out_lines[last];
                let final_index = (final_line.len() as i32 - max_lines_suffix.len() as i32).max(0);
                let mut new_line = substring(final_line, 0, final_index)?.to_vec();
                new_line.extend_from_slice(max_lines_suffix);
                out_lines[last] = new_line;
            } else {
                out_lines[last].extend_from_slice(max_lines_suffix);
            }
            Ok(join(&[10], &out_lines[..max_lines as usize]))
        } else {
            Ok(join(&[10], &out_lines))
        }
    }

    /// zombie.ui.TextManager.getCurrentCodeFont
    pub fn get_current_code_font(&self) -> UIFont {
        self.current_code_font
    }
}

/// `String.split("\\r?\\n")`: pieces between line breaks ("\r\n" or "\n"; a lone "\r" stays in the text), trailing
/// empty pieces dropped; a text without a break is one piece.
fn split_line_breaks(s: &[u16]) -> Vec<&[u16]> {
    let mut parts = Vec::new();
    let mut index = 0usize;
    let mut found = false;
    for p in 0..s.len() {
        if s[p] == 10 {
            let start = if p > index && s[p - 1] == 13 {
                p - 1
            } else {
                p
            };
            parts.push(&s[index..start]);
            index = p + 1;
            found = true;
        }
    }
    if !found {
        return vec![s];
    }
    parts.push(&s[index..]);
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

/// `String.split(" ")` (one plain character): the pieces between separators, trailing empty pieces dropped; a text
/// without the separator is one piece.
fn split_unit(s: &[u16], sep: u16) -> Vec<&[u16]> {
    let mut parts: Vec<&[u16]> = s.split(|&u| u == sep).collect();
    if parts.len() == 1 {
        return parts;
    }
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

/// `String.join`
fn join(sep: &[u16], parts: &[Vec<u16>]) -> Vec<u16> {
    let mut out = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            out.extend_from_slice(sep);
        }
        out.extend_from_slice(p);
    }
    out
}

/// `String.substring(begin, end)`; `Err` is the StringIndexOutOfBoundsException.
fn substring(s: &[u16], begin: i32, end: i32) -> Result<&[u16], String> {
    if begin < 0 || end > s.len() as i32 || begin > end {
        return Err(format!(
            "java.lang.StringIndexOutOfBoundsException: begin {begin}, end {end}, length {}",
            s.len()
        ));
    }
    Ok(&s[begin as usize..end as usize])
}
