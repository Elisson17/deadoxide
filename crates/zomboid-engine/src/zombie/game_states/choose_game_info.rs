//! zombie.gameStates.ChooseGameInfo: `map.info` (`getMapDetails`) and `mod.info` (`Mod`, `readModInfo`,
//! `getModDetails`, availability). The class's statics (`Mods`, `MissingMods`, `tempStrings`) are one
//! `ChooseGameInfo` value; the `Mod` objects belong to `ZomboidFileSystem.modDirToMod` and are referred to by handle.
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::java;
use crate::zombie::core::game_version::{core_game_version, GameVersion};
use crate::zombie::core::translator::Translator;
use crate::zombie::util::string_utils::is_null_or_whitespace;
use crate::zombie::zomboid_file_system::{PzModFolder, ZomboidFileSystem};

/// zombie.gameStates.ChooseGameInfo.Map: the fields of `media/maps/<dir>/map.info` the engine reads so far.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Map {
    pub title: String,
    pub lots_dir: Vec<String>,
    pub fixed2x: bool,
}

/// zombie.gameStates.ChooseGameInfo.getMapDetails: `None` when the folder has no map.info. Vanilla lookup only
/// (the original resolves the path through the mod-aware file system and caches the result).
pub fn get_map_details(media: &Path, dir: &str) -> Option<Map> {
    let text = crate::vfs::read(&media.join("maps").join(dir).join("map.info")).ok()?;
    let text = String::from_utf8_lossy(&text);
    let mut map = Map {
        title: dir.to_string(),
        ..Map::default()
    };
    for line in text.lines() {
        let line = java::trim(line);
        if line.starts_with("title=") {
            map.title = line.replace("title=", "");
        } else if line.starts_with("lots=") {
            map.lots_dir
                .push(java::trim(&line.replace("lots=", "")).to_string());
        } else if line.starts_with("fixed2x=") {
            // java.lang.Boolean.parseBoolean
            map.fixed2x = java::trim(&line.replace("fixed2x=", "")).eq_ignore_ascii_case("true");
        }
    }
    Some(map)
}

/// zombie.gameStates.ChooseGameInfo.getMinRequiredVersion: 42.0, the oldest version folder a mod may have.
pub fn get_min_required_version() -> GameVersion {
    GameVersion::plain(42, 0)
}

/// zombie.core.textures.TextureID.useCompressionOption (its default; the options file is not read here).
pub const TEXTURE_ID_USE_COMPRESSION_OPTION: bool = true;

/// zombie.gameStates.ChooseGameInfo.PackFile: a `pack=` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackFile {
    pub name: String,
    pub flags: i32,
}

/// zombie.gameStates.ChooseGameInfo.TileDef: a `tiledef=` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TileDef {
    pub name: String,
    pub file_number: i32,
}

/// zombie.gameStates.ChooseGameInfo.Mod: a mod folder and what its `mod.info` says. `tex` (the poster texture) is not
/// ported.
#[derive(Clone, Debug, PartialEq)]
pub struct Mod {
    /// the folder as given
    pub dir: String,
    /// `<dir>/common`, absolute
    pub common_dir: String,
    /// `<dir>/<getModVersionDirName>`, absolute
    pub version_dir: String,
    pub base_file: PzModFolder,
    pub media_file: PzModFolder,
    pub action_groups_file: PzModFolder,
    pub anim_sets_file: PzModFolder,
    pub anims_x_file: PzModFolder,
    pub posters: Vec<String>,
    pub require: Option<Vec<String>>,
    pub incompatible: Option<Vec<String>>,
    pub load_after: Option<Vec<String>>,
    pub load_before: Option<Vec<String>>,
    pub name: String,
    pub desc: String,
    pub id: String,
    pub url: Option<String>,
    pub author: Option<String>,
    pub mod_version: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    pub workshop_id: String,
    pub source: String,
    pub available_done: bool,
    pub available: bool,
    pub version_min: Option<GameVersion>,
    pub version_max: Option<GameVersion>,
    pub packs: Vec<PackFile>,
    pub tile_defs: Vec<TileDef>,
    pub read: bool,
    pub valid: bool,
}

/// java.nio.file.Path.resolve(String) of a normalized Unix path.
fn path_resolve(base: &str, other: &str) -> String {
    if other.starts_with('/') || base.is_empty() {
        other.to_string()
    } else if other.is_empty() {
        base.to_string()
    } else if base.ends_with('/') {
        format!("{base}{other}")
    } else {
        format!("{base}/{other}")
    }
}

/// `new File(parent, child)` where `parent` may be null (the canonical file of a folder whose canonicalization
/// failed): the child alone then.
fn file_child(parent: Option<&str>, child: &str) -> String {
    match parent {
        Some(p) => path_resolve(p, child),
        None => child.to_string(),
    }
}

/// `Paths.get(dir, "..", "..").normalize().getFileName()`: the name two levels up from `dir` (syntactically: a `..`
/// drops the name before it, and is dropped itself right after the root).
fn grandparent_name(dir_path: &str) -> Option<String> {
    let absolute = dir_path.starts_with('/');
    let mut names: Vec<&str> = dir_path.split('/').filter(|n| !n.is_empty()).collect();
    names.extend(["..", ".."]);
    let mut out: Vec<&str> = Vec::new();
    for n in names {
        match n {
            "." => {}
            ".." => {
                if matches!(out.last(), Some(l) if *l != "..") {
                    out.pop();
                } else if !absolute {
                    out.push("..");
                }
            }
            n => out.push(n),
        }
    }
    out.last().map(|s| s.to_string())
}

/// zombie.core.znet.SteamUtils.isValidSteamID: `new BigInteger(s)` from 0 to 2^64 - 1. Only ASCII digits are taken
/// as digits (BigInteger also takes other Unicode decimal digits).
fn is_valid_steam_id(s: &str) -> bool {
    let (negative, digits) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let significant = digits.trim_start_matches('0');
    if negative {
        return significant.is_empty();
    }
    significant.is_empty() || significant.parse::<u64>().is_ok()
}

/// zombie.Lua.LuaManager.isIndieStoneUrl: Steam's, the game's, The Indie Stone's or the wiki's site.
fn is_indie_stone_url(url: &str) -> bool {
    [
        "https://steamcommunity.com",
        "https://projectzomboid.com",
        "https://theindiestone.com",
        "https://pzwiki.net",
    ]
    .iter()
    .any(|site| url == *site || url.starts_with(&format!("{site}/")))
}

/// `new ArrayList<>(Arrays.asList(line.replace(key, "").replace("\\", "").split(",")))`
fn id_list(line: &str, key: &str) -> Vec<String> {
    java::split_char(&line.replace(key, "").replace('\\', ""), ',')
        .into_iter()
        .map(str::to_string)
        .collect()
}

impl Mod {
    /// zombie.gameStates.ChooseGameInfo.Mod.<init>: the common and version folders (`getModVersionDirName`), the
    /// folders the mod's `media`, `actiongroups`, `AnimSets` and `anims_X` would be in, and the source: "Workshop"
    /// when the folder two levels up is named `Contents` (any letter case), "Steam" (with that name as the workshop
    /// id) when it is a Steam id, else "Mods".
    pub fn new(dir: &str, fs: &ZomboidFileSystem) -> Mod {
        let dir_path = java::file_normalize(dir);
        let common_path = path_resolve(&dir_path, "common");
        let version_path = path_resolve(&dir_path, &fs.get_mod_version_dir_name(&dir_path));
        let mut base_file = PzModFolder::default();
        base_file.set_with_catch(fs, &common_path, &version_path);
        let sub = |parent: &PzModFolder, name: &str| {
            let mut f = PzModFolder::default();
            f.set_with_catch(
                fs,
                &file_child(parent.common.canonical_file.as_deref(), name),
                &file_child(parent.version.canonical_file.as_deref(), name),
            );
            f
        };
        let media_file = sub(&base_file, "media");
        let action_groups_file = sub(&media_file, "actiongroups");
        let anim_sets_file = sub(&media_file, "AnimSets");
        let anims_x_file = sub(&media_file, "anims_X");
        let mut source = "Mods".to_string();
        let mut workshop_id = String::new();
        if let Some(dir_name) = grandparent_name(&dir_path) {
            if java::equals_ignore_case(&dir_name, "Contents") {
                source = "Workshop".into();
            } else if is_valid_steam_id(&dir_name) {
                source = "Steam".into();
                workshop_id = dir_name;
            }
        }
        Mod {
            dir: dir.to_string(),
            common_dir: fs.to_absolute_path(&common_path),
            version_dir: fs.to_absolute_path(&version_path),
            base_file,
            media_file,
            action_groups_file,
            anim_sets_file,
            anims_x_file,
            posters: Vec::new(),
            require: None,
            incompatible: None,
            load_after: None,
            load_before: None,
            name: "Unnamed Mod".into(),
            desc: String::new(),
            id: "undefined_id".into(),
            url: None,
            author: None,
            mod_version: None,
            icon: None,
            category: None,
            workshop_id,
            source,
            available_done: false,
            available: true,
            version_min: None,
            version_max: None,
            packs: Vec::new(),
            tile_defs: Vec::new(),
            read: false,
            valid: false,
        }
    }

    /// zombie.gameStates.ChooseGameInfo.Mod.getId
    pub fn get_id(&self) -> &str {
        &self.id
    }

    /// zombie.gameStates.ChooseGameInfo.Mod.getDir
    pub fn get_dir(&self) -> &str {
        &self.dir
    }

    /// zombie.gameStates.ChooseGameInfo.Mod.getRequire
    pub fn get_require(&self) -> Option<&[String]> {
        self.require.as_deref()
    }

    /// zombie.gameStates.ChooseGameInfo.Mod.getUrl: "" when unset.
    pub fn get_url(&self) -> &str {
        self.url.as_deref().unwrap_or("")
    }

    /// zombie.gameStates.ChooseGameInfo.Mod.setUrl: kept only for The Indie Stone's sites (`isIndieStoneUrl`).
    pub fn set_url(&mut self, url: &str) {
        if is_indie_stone_url(url) {
            self.url = Some(url.to_string());
        }
    }

    /// zombie.gameStates.ChooseGameInfo.Mod.isAvailableSelf: the game's version is within `versionMin`..`versionMax`
    /// (either may be unset).
    pub fn is_available_self(&self) -> bool {
        let game_version = core_game_version();
        if let Some(min) = &self.version_min {
            if min.is_greater_than(&game_version) {
                return false;
            }
        }
        match &self.version_max {
            Some(max) => !max.is_less_than(&game_version),
            None => true,
        }
    }

    /// The lines of zombie.gameStates.ChooseGameInfo.readModInfoAux's loop: each line is matched by the first key
    /// it contains (`id=`, `versionMax=`, `versionMin=` only at its start), in the original's order. `Err` is a line
    /// that makes the mod invalid (the fields set before it stay).
    fn read_lines(
        &mut self,
        text: &str,
        version_subdir: &str,
        common_subdir: &str,
    ) -> Result<(), ()> {
        let in_mod = |tex: &str| {
            let name = format!("{version_subdir}/{tex}");
            if Path::new(&name).exists() {
                name
            } else {
                format!("{common_subdir}/{tex}")
            }
        };
        for input_line in java::buffered_reader_lines(text) {
            if input_line.contains("name=") {
                let name_str = input_line.replace("name=", "");
                if !is_null_or_whitespace(Some(&name_str)) {
                    self.name = name_str;
                }
            } else if input_line.contains("poster=") {
                let tex_name = input_line.replace("poster=", "");
                if !is_null_or_whitespace(Some(&tex_name)) {
                    self.posters.push(in_mod(&tex_name));
                }
            } else if input_line.contains("description=") {
                self.desc.push_str(&input_line.replace("description=", ""));
            } else if input_line.contains("require=") {
                self.require = Some(id_list(input_line, "require="));
            } else if input_line.contains("incompatible=") {
                self.incompatible = Some(id_list(input_line, "incompatible="));
            } else if input_line.contains("loadModAfter=") {
                self.load_after = Some(id_list(input_line, "loadModAfter="));
            } else if input_line.contains("loadModBefore=") {
                self.load_before = Some(id_list(input_line, "loadModBefore="));
            } else if input_line.starts_with("id=") {
                let id_str = input_line.replace("id=", "");
                let id_str = java::trim(&id_str);
                if !is_null_or_whitespace(Some(id_str)) {
                    self.id = id_str.to_string();
                }
            } else if input_line.contains("author=") {
                self.author = Some(input_line.replace("author=", ""));
            } else if input_line.contains("modversion=") {
                self.mod_version = Some(input_line.replace("modversion=", ""));
            } else if input_line.contains("icon=") {
                let tex_name = input_line.replace("icon=", "");
                if !is_null_or_whitespace(Some(&tex_name)) {
                    self.icon = Some(in_mod(&tex_name));
                }
            } else if input_line.contains("category=") {
                self.category = Some(input_line.replace("category=", ""));
            } else if input_line.contains("url=") {
                self.set_url(&input_line.replace("url=", ""));
            } else if input_line.contains("pack=") {
                self.read_pack(input_line)?;
            } else if input_line.contains("tiledef=") {
                let s = input_line.replace("tiledef=", "");
                let ss = java::split_whitespace_regex(java::trim(&s));
                if ss.len() != 2 {
                    // DebugType.Mod.error("tiledef= line requires file name and file number")
                    return Err(());
                }
                let Some(file_number) = java::parse_int(ss[1]) else {
                    return Err(());
                };
                if !(100..=8189).contains(&file_number) {
                    // DebugType.Mod.error("tiledef=%s %d file number must be from %d to %d", ...)
                    return Err(());
                }
                self.tile_defs.push(TileDef {
                    name: ss[0].to_string(),
                    file_number,
                });
            } else if input_line.starts_with("versionMax=") {
                let version_str = input_line.replace("versionMax=", "");
                let version_str = java::trim(&version_str);
                if !version_str.is_empty() {
                    // DebugType.Mod.error("invalid versionMax: " + message)
                    self.version_max = Some(GameVersion::parse(version_str).map_err(|_| ())?);
                }
            } else if input_line.starts_with("versionMin=") {
                let version_str = input_line.replace("versionMin=", "");
                let version_str = java::trim(&version_str);
                if !version_str.is_empty() {
                    self.version_min = Some(GameVersion::parse(version_str).map_err(|_| ())?);
                }
            }
        }
        Ok(())
    }

    /// A `pack=` line of readModInfoAux: the trimmed name (none: invalid), flags 4 (texture compression option) | 64;
    /// with `type=`: `ui` makes the flags 2 (another type is logged), and the name is cut at its first space (none:
    /// the StringIndexOutOfBoundsException that makes the mod invalid); a name ending in ".floor" loses flag 4.
    fn read_pack(&mut self, input_line: &str) -> Result<(), ()> {
        let mut pack = java::trim(&input_line.replace("pack=", "")).to_string();
        if pack.is_empty() {
            // DebugType.Mod.error("pack= line requires a file name")
            return Err(());
        }
        let mut flags = if TEXTURE_ID_USE_COMPRESSION_OPTION {
            4
        } else {
            0
        };
        flags |= 64;
        let p = java::string_index_of(&pack, "type=");
        if p != -1 {
            let r#type = java::string_substring_from(&pack, p + "type=".len() as i32).map_err(|_| ())?;
            if r#type == "ui" {
                flags = 2;
            }
            // else DebugType.Mod.error("unknown pack type=" + type)
            let p1 = java::string_index_of(&pack, " ");
            pack = java::trim(&java::string_substring(&pack, 0, p1).map_err(|_| ())?).to_string();
        }
        if pack.ends_with(".floor") {
            flags &= -5;
        }
        self.packs.push(PackFile { name: pack, flags });
        Ok(())
    }
}

/// zombie.core.IndieFileLoader.getStreamReader then `BufferedReader`: the file the file map gives for `path` (or
/// `path`), UTF-8 with the JDK's replacement of malformed bytes; failing that `<cacheDir>/mods/<path>`. `None` is the
/// exception readModInfoAux logs.
fn read_mod_info_text(fs: &ZomboidFileSystem, path: &str) -> Option<String> {
    let bytes = match std::fs::read(fs.get_string_or(path)) {
        Ok(b) => b,
        Err(_) => std::fs::read(format!("{}/mods/{path}", fs.get_cache_dir()?)).ok()?,
    };
    Some(String::from_utf16_lossy(&java::utf8_decode_jdk(&bytes)))
}

/// The statics of zombie.gameStates.ChooseGameInfo (the mod side).
#[derive(Clone, Debug, Default)]
pub struct ChooseGameInfo {
    /// `Mods`: mod id → its `Mod` (a handle into the file system's mods).
    pub mods: HashMap<String, usize>,
    /// `MissingMods`: ids looked up and not found.
    pub missing_mods: HashSet<String>,
    /// `tempStrings`: a scratch list the original shares between `getModDetails`/`readModInfo` (the mod folders) and
    /// `Mod.isAvailable` (the ids seen), so one overwrites the other mid-check; kept shared here.
    pub temp_strings: Vec<String>,
}

impl ChooseGameInfo {
    /// zombie.gameStates.ChooseGameInfo.Reset (the mod side).
    pub fn reset(&mut self) {
        self.mods.clear();
        self.missing_mods.clear();
    }

    /// zombie.gameStates.ChooseGameInfo.getModDetails: the mod of that id: `None` once missing; the known one; else
    /// the folder `modIdToDir` has, or (none) every mod folder read in order (each id remembered with its folder)
    /// until one has the id, which is returned as read. Not found: remembered as missing.
    pub fn get_mod_details(
        &mut self,
        fs: &mut ZomboidFileSystem,
        tr: &mut Translator,
        mod_id: &str,
    ) -> Option<usize> {
        if self.missing_mods.contains(mod_id) {
            return None;
        }
        if let Some(&m) = self.mods.get(mod_id) {
            return Some(m);
        }
        let mod_dir = fs.get_mod_dir(mod_id).map(str::to_string);
        if mod_dir.is_none() {
            self.temp_strings = fs.get_all_mod_folders();
            let mut i = 0;
            while i < self.temp_strings.len() {
                let folder = self.temp_strings[i].clone();
                if let Some(m) = self.read_mod_info(fs, tr, Some(&folder)) {
                    let id = fs.mod_info(m).id.clone();
                    let folder = self.temp_strings.get(i).cloned().unwrap_or(folder);
                    fs.set_mod_id_to_dir(&id, &folder);
                    if id == mod_id {
                        return Some(m);
                    }
                }
                i += 1;
            }
        }
        let m = self.read_mod_info(fs, tr, mod_dir.as_deref());
        if m.is_none() {
            self.missing_mods.insert(mod_id.to_string());
        }
        m
    }

    /// zombie.gameStates.ChooseGameInfo.getAvailableModDetails: `getModDetails` when `isAvailable`.
    pub fn get_available_mod_details(
        &mut self,
        fs: &mut ZomboidFileSystem,
        tr: &mut Translator,
        mod_id: &str,
    ) -> Option<usize> {
        let m = self.get_mod_details(fs, tr, mod_id)?;
        self.is_available(fs, tr, m).then_some(m)
    }

    /// zombie.gameStates.ChooseGameInfo.readModInfo: `readModInfoAux`; a valid mod becomes `Mods`' entry for its id
    /// unless another mod has it whose folder comes earlier in `getAllModFolders` (a folder outside them comes
    /// first).
    pub fn read_mod_info(
        &mut self,
        fs: &mut ZomboidFileSystem,
        tr: &mut Translator,
        mod_dir: Option<&str>,
    ) -> Option<usize> {
        let m = self.read_mod_info_aux(fs, tr, mod_dir)?;
        let id = fs.mod_info(m).id.clone();
        match self.mods.get(&id).copied() {
            None => {
                self.mods.insert(id, m);
            }
            Some(exists) if exists != m => {
                self.temp_strings = fs.get_all_mod_folders();
                let index_of = |dir: &str, list: &[String]| {
                    list.iter()
                        .position(|s| s == dir)
                        .map_or(-1, |i| i as i64)
                };
                let index1 = index_of(&fs.mod_info(m).dir, &self.temp_strings);
                let index2 = index_of(&fs.mod_info(exists).dir, &self.temp_strings);
                if index1 < index2 {
                    self.mods.insert(id, m);
                }
            }
            Some(_) => {}
        }
        Some(m)
    }

    /// zombie.gameStates.ChooseGameInfo.readModInfoAux: the folder's `Mod` (`getModInfoForDir`), read once: its
    /// `mod.info` from the version folder, else the common one (none: `None`), line by line (`Mod::read_lines`), then
    /// `Translator.readModTranslation`; valid unless a line or the translation fails.
    fn read_mod_info_aux(
        &mut self,
        fs: &mut ZomboidFileSystem,
        tr: &mut Translator,
        mod_dir: Option<&str>,
    ) -> Option<usize> {
        let mod_dir = mod_dir?;
        let m = fs.get_mod_info_for_dir(mod_dir);
        if fs.mod_info(m).read {
            return fs.mod_info(m).valid.then_some(m);
        }
        fs.mod_info_mut(m).read = true;
        let version_subdir = fs.mod_info(m).version_dir.clone();
        let common_subdir = fs.mod_info(m).common_dir.clone();
        let mut file_name = format!("{version_subdir}/mod.info");
        if !Path::new(&file_name).exists() {
            file_name = format!("{common_subdir}/mod.info");
            if !Path::new(&file_name).exists() {
                // DebugType.Mod.warn("can't find mod.info in mod dir: \"" + modDir + "\"")
                return None;
            }
        }
        let text = read_mod_info_text(fs, &file_name)?;
        let info = fs.mod_info_mut(m);
        info.read_lines(&text, &version_subdir, &common_subdir)
            .ok()?;
        let (mut name, mut desc) = (info.name.clone(), info.desc.clone());
        tr.read_mod_translation(&common_subdir, &version_subdir, &mut name, &mut desc)
            .ok()?;
        let info = fs.mod_info_mut(m);
        info.name = name;
        info.desc = desc;
        info.valid = true;
        Some(m)
    }

    /// zombie.gameStates.ChooseGameInfo.Mod.isAvailable: once per mod, `isAvailableSelf` and every required mod
    /// (found, available itself, and its own required mods, each id checked once).
    pub fn is_available(&mut self, fs: &mut ZomboidFileSystem, tr: &mut Translator, m: usize) -> bool {
        if fs.mod_info(m).available_done {
            return fs.mod_info(m).available;
        }
        fs.mod_info_mut(m).available_done = true;
        if !fs.mod_info(m).is_available_self() {
            fs.mod_info_mut(m).available = false;
            return false;
        }
        self.temp_strings.clear();
        self.temp_strings.push(fs.mod_info(m).id.clone());
        let available = self.is_available_required(fs, tr, m);
        fs.mod_info_mut(m).available = available;
        available
    }

    /// zombie.gameStates.ChooseGameInfo.Mod.isAvailableRequired: the "seen" list is `tempStrings`, which a
    /// `getModDetails` that lists the mod folders replaces with them.
    fn is_available_required(
        &mut self,
        fs: &mut ZomboidFileSystem,
        tr: &mut Translator,
        m: usize,
    ) -> bool {
        let Some(require) = fs.mod_info(m).get_require().map(<[String]>::to_vec) else {
            return true;
        };
        for r in &require {
            let mod_id = java::trim(r);
            if !self.temp_strings.iter().any(|s| s == mod_id) {
                self.temp_strings.push(mod_id.to_string());
                let Some(required) = self.get_mod_details(fs, tr, mod_id) else {
                    return false;
                };
                if !fs.mod_info(required).is_available_self() {
                    return false;
                }
                if !self.is_available_required(fs, tr, required) {
                    return false;
                }
            }
        }
        true
    }
}
