//! zombie.ui.FontsFile: `media/fonts/<lang>/fonts.txt` (and `fontsDyslexic.txt`), the `UIFont` name → font file
//! table `TextManager.Init` reads.
use std::collections::HashMap;

use super::fonts_file_font::FontsFileFont;
use crate::java;
use crate::zombie::core::math::pz_math;
use crate::zombie::scripting::script_parser;
use crate::zombie::util::string_utils;
use crate::zombie::zomboid_file_system::ZomboidFileSystem;

/// zombie.ui.FontsFile.read: the file's lines joined WITHOUT separators (`BufferedReader.readLine` appended to one
/// `StringBuilder`), then `fromString`. `false` for a missing file (silently) or any other failure (logged into
/// `log`); fonts read before a failure stay in `fonts`. `path` is opened as `new FileReader(path)` opens it (relative to
/// the game folder).
pub fn read(
    fs: &ZomboidFileSystem,
    path: &str,
    fonts: &mut HashMap<String, FontsFileFont>,
    log: &mut Vec<String>,
) -> bool {
    let file = fs.open_path(path);
    let bytes = match std::fs::read(&file) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let text = String::from_utf16_lossy(&java::utf8_decode_jdk(&bytes));
    let contents: String = java::buffered_reader_lines(&text).concat();
    match from_string(&contents, fonts, log) {
        Ok(()) => true,
        Err(e) => {
            log.push(e);
            false
        }
    }
}

/// zombie.ui.FontsFile.fromString: comments stripped, `ScriptParser.parse`, `VERSION` must be 1; every child block
/// must be a `font` (else "unknown block type"); a font without an id or without `fnt` is skipped with a warning;
/// `fnt`/`img` are trimmed, `type` trimmed and dropped when blank. A later id replaces an earlier one.
fn from_string(
    contents: &str,
    fonts: &mut HashMap<String, FontsFileFont>,
    log: &mut Vec<String>,
) -> Result<(), String> {
    let contents = script_parser::strip_comments(contents);
    let block = script_parser::parse(&contents);
    let mut version = -1;
    if let Some(value) = block.get_value("VERSION") {
        version = pz_math::try_parse_int(Some(value.get_value()), -1);
    }
    if !(1..=1).contains(&version) {
        return Err("java.lang.RuntimeException: invalid or missing VERSION".into());
    }
    for block1 in &block.children {
        if !java::equals_ignore_case(&block1.r#type, "font") {
            return Err(format!(
                "java.lang.RuntimeException: unknown block type \"{}\"",
                block1.r#type
            ));
        }
        if string_utils::is_null_or_whitespace(block1.id.as_deref()) {
            log.push("missing or empty font id".into());
            continue;
        }
        let fnt = block1.get_value("fnt");
        let img = block1.get_value("img");
        let ty = block1.get_value("type");
        match fnt {
            Some(fnt) if !string_utils::is_null_or_whitespace(Some(fnt.get_value())) => {
                let mut font = FontsFileFont {
                    id: block1.id.clone().unwrap_or_default(),
                    fnt: java::trim(fnt.get_value()).to_string(),
                    ..FontsFileFont::default()
                };
                if let Some(img) = img {
                    if !string_utils::is_null_or_whitespace(Some(img.get_value())) {
                        font.img = Some(java::trim(img.get_value()).to_string());
                    }
                }
                if let Some(ty) = ty {
                    let t = java::trim(ty.get_value());
                    // StringUtils.discardNullOrWhitespace
                    if !string_utils::is_null_or_whitespace(Some(t)) {
                        font.r#type = Some(t.to_string());
                    }
                }
                fonts.insert(font.id.clone(), font);
            }
            _ => log.push("missing or empty value \"fnt\"".into()),
        }
    }
    Ok(())
}
