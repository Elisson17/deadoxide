//! zombie.modding.ActiveModsFile: an ActiveMods list as a script block file (`mods/default.txt`). Only reading is
//! ported (`write` needs `ScriptParser.Block.prettyPrintElements`).
use super::active_mods::ActiveMods;
use crate::java;
use crate::zombie::core::math::pz_math::try_parse_int;
use crate::zombie::scripting::script_parser::{self, Block};
use crate::zombie::util::string_utils::is_null_or_whitespace;

/// zombie.modding.ActiveModsFile.read: the list cleared, then the file's lines joined without their line ends
/// (`BufferedReader.readLine`, the default charset UTF-8) and read by `fromString`. `false` when the file cannot be
/// read (FileNotFoundException, or another exception the original logs).
pub fn read(path: &str, active_mods: &mut ActiveMods) -> bool {
    active_mods.clear();
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let text = String::from_utf16_lossy(&java::utf8_decode_jdk(&bytes));
    let joined: String = java::buffered_reader_lines(&text).concat();
    from_string(&joined, active_mods);
    true
}

/// zombie.scripting.ScriptParser.Block.getBlock(type, null): the first child of that type without an id.
fn get_block<'a>(block: &'a Block, r#type: &str) -> Option<&'a Block> {
    block
        .children
        .iter()
        .find(|b| b.r#type == r#type && b.id.is_none())
}

/// zombie.modding.ActiveModsFile.fromString: comments stripped, parsed; with `VERSION = 1`, each `mod = <id>` of the
/// `mods` block (key compared ignoring case, the id trimmed and stripped of '\\') and each `map = <folder>` of the
/// `maps` block (trimmed) that is not empty or whitespace, in order.
pub fn from_string(contents: &str, active_mods: &mut ActiveMods) {
    let contents = script_parser::strip_comments(contents);
    let block = script_parser::parse(&contents);
    let version = match block.get_value("VERSION") {
        Some(value) => try_parse_int(Some(value.get_value()), -1),
        None => -1,
    };
    if version != 1 {
        return;
    }
    if let Some(block_mods) = get_block(&block, "mods") {
        for value in &block_mods.values {
            let key = java::trim(value.get_key());
            if java::equals_ignore_case(key, "mod") {
                let mod_id = java::trim(value.get_value()).replace('\\', "");
                if !is_null_or_whitespace(Some(&mod_id)) {
                    active_mods.get_mods_mut().push(mod_id);
                }
            }
        }
    }
    if let Some(block_maps) = get_block(&block, "maps") {
        for value in &block_maps.values {
            let key = java::trim(value.get_key());
            if java::equals_ignore_case(key, "map") {
                let map = java::trim(value.get_value());
                if !is_null_or_whitespace(Some(map)) {
                    active_mods.get_map_order_mut().push(map.to_string());
                }
            }
        }
    }
}
