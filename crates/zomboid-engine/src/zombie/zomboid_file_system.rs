//! zombie.ZomboidFileSystem: the game folder, its file map (`activeFileMap`), the file guid table, the lookups and
//! path helpers the font, XML, animation and action loaders use, and the mod side: the mod folders (the cache dir's
//! `mods`, the `42`/`42.x`/`common` version folders), `modIdToDir`/`modDirToMod`, `loadMods` (the load order with the
//! required mods first) and `loadMod` (a mod's files in the file map, shadowing the game's). One value per game
//! folder instead of the `instance` singleton; it owns the `ChooseGameInfo.Mod` objects (`modDirToMod`), which the
//! rest refers to by handle. Paths are '/'-separated; the process working directory is the game folder, as when the
//! game runs. Directory listings are in the file system's own order, like `File.list`/`File.listFiles`.
//! Not ported: Steam (the `workshop`/`steam` mod folders need Steam mode, off here as in the standalone oracle), the
//! file watchers, `loadModPackFiles`/`loadModTileDefs`, the client's translation-only mods, `ActiveModsFile.write`.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::core::translator::Translator;
use super::file_guid_table::FileGuidTable;
use super::game_states::choose_game_info::{ChooseGameInfo, Mod};
use super::modding::active_mods::{ActiveModsError, ActiveModsState};
use crate::java;

/// zombie.ZomboidFileSystem as far as the ported loaders use it.
pub struct ZomboidFileSystem {
    /// `base.canonicalFile.getPath()`: the canonical game folder, '/'-separated.
    base: String,
    /// `base.lowercaseUri`'s path: the lower-cased game folder, with a '/' when that names a folder.
    base_lower_uri: String,
    /// `activeFileMap`: lower-case path relative to the game folder ("media/fonts/en/fonts.txt") → absolute path with
    /// the file's own letter case (every file under `media`, and the directories under `media/maps/` with a trailing
    /// separator; then the loaded mods' files, relative to their common or version folder).
    pub active_file_map: HashMap<String, String>,
    /// `allAbsolutePaths`: every value put into `activeFileMap`.
    all_absolute_paths: HashSet<String>,
    /// `fileGuidTable` (`getFileGuidTable` loads it on first use; here at `init`). `None` where it failed to load.
    file_guid_table: Option<FileGuidTable>,
    /// `cacheDir` (`setCacheDir`, the `-cachedir=` option). `None` until set: the original then picks
    /// `~/Zomboid`, which the port never does.
    cache_dir: Option<String>,
    /// `modIdToDir`: mod id → mod folder.
    mod_id_to_dir: HashMap<String, String>,
    /// `modDirToMod`: mod folder (as given) → its `ChooseGameInfo.Mod`, a handle into `mod_infos`.
    mod_dir_to_mod: HashMap<String, usize>,
    /// The `ChooseGameInfo.Mod` objects `modDirToMod` holds.
    mod_infos: Vec<Mod>,
    /// `mods`: the ids `loadMods` loaded, in load order (`getModIDs`).
    mods: Vec<String>,
    /// `modFolders`: `getAllModFolders`' listing, `None` until listed (and after `resetModFolders`).
    mod_folders: Option<Vec<String>>,
    /// `modFoldersOrder` (`-modfolders`, default "workshop,steam,mods").
    mod_folders_order: Option<Vec<String>>,
}

/// zombie.core.Core.optionModsEnabled (its default; the options file is not read here).
pub const CORE_OPTION_MODS_ENABLED: bool = true;

/// zombie.ZomboidFileSystem.PZFolder: a folder as given (`absoluteFile`) and resolved (`canonicalFile`). The two
/// URIs the original also keeps (`canonicalUri`, `lowercaseUri`) follow from these.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PzFolder {
    pub absolute_file: Option<String>,
    pub canonical_file: Option<String>,
}

impl PzFolder {
    /// zombie.ZomboidFileSystem.PZFolder.setWithCatch: `file.getAbsoluteFile()`, then its `getCanonicalFile()`
    /// (`None`, with the original's logged IOException, when that fails).
    pub fn set_with_catch(&mut self, fs: &ZomboidFileSystem, file: &str) {
        let absolute = fs.absolute(&java::file_normalize(file));
        self.canonical_file = canonical_path(&absolute);
        self.absolute_file = Some(absolute);
    }
}

/// zombie.ZomboidFileSystem.PZModFolder: a mod's folder in its common and its version folder.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PzModFolder {
    pub common: PzFolder,
    pub version: PzFolder,
}

impl PzModFolder {
    /// zombie.ZomboidFileSystem.PZModFolder.setWithCatch
    pub fn set_with_catch(&mut self, fs: &ZomboidFileSystem, common_file: &str, version_file: &str) {
        self.common.set_with_catch(fs, common_file);
        self.version.set_with_catch(fs, version_file);
    }
}

/// java.io.File.getCanonicalPath on Unix (`JDK_Canonicalize`): `realpath` of the whole path; when that fails, of the
/// longest leading part that resolves (the parts that do not exist, are not folders or may not be read are dropped
/// from the end one by one) with the rest appended; then `.` and `..` names collapsed. `None` is the IOException of
/// any other failure.
pub fn canonical_path(abs: &str) -> Option<String> {
    use std::io::ErrorKind;
    let skippable = |e: &std::io::Error| {
        matches!(
            e.kind(),
            ErrorKind::NotFound | ErrorKind::NotADirectory | ErrorKind::PermissionDenied
        )
    };
    match std::fs::canonicalize(abs) {
        Ok(p) => return Some(collapse(&slashes(&p))),
        Err(e) if !skippable(&e) => return None,
        Err(_) => {}
    }
    let mut end = abs.len();
    while let Some(p) = abs[..end].rfind('/') {
        if p == 0 {
            break;
        }
        match std::fs::canonicalize(&abs[..p]) {
            Ok(r) => {
                let mut r = slashes(&r);
                let rest = &abs[p..];
                if r.ends_with('/') {
                    r.push_str(&rest[1..]);
                } else {
                    r.push_str(rest);
                }
                return Some(collapse(&r));
            }
            Err(e) if !skippable(&e) => return None,
            Err(_) => end = p,
        }
    }
    Some(collapse(abs))
}

/// The `collapse` of `JDK_Canonicalize`: `.` names dropped, a `..` name drops itself and the name before it.
fn collapse(path: &str) -> String {
    let mut names: Vec<&str> = Vec::new();
    for n in path.split('/').filter(|n| !n.is_empty()) {
        match n {
            "." => {}
            ".." => {
                names.pop();
            }
            n => names.push(n),
        }
    }
    format!("/{}", names.join("/"))
}

fn slashes(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// java.io.File.listFiles: the entries of `dir` in the order the file system lists them; `None` when `dir` is not a
/// readable directory.
pub fn list_files(dir: &Path) -> Option<Vec<PathBuf>> {
    let rd = std::fs::read_dir(dir).ok()?;
    Some(rd.flatten().map(|e| e.path()).collect())
}

impl ZomboidFileSystem {
    /// zombie.ZomboidFileSystem.init: `base` is the canonical game folder, `searchFolders(<base>/media)` lists every
    /// file (and every folder under `media/maps/`), then each goes into `activeFileMap` under its lower-cased
    /// relative path (`getRelativeFile`), a folder's absolute path ending in '/'. The original lists folders in the file
    /// system's order; that order only matters for two names differing in case only (one overwrites the other). The
    /// file guid table (`loadFileGuidTable`, the game's own `media/fileGuidTable.xml`) is loaded here too.
    pub fn init(game_root: &Path) -> Result<ZomboidFileSystem, String> {
        let canonical = game_root
            .canonicalize()
            .map_err(|e| format!("{}: {e}", game_root.display()))?;
        let base = slashes(&canonical);
        let mut fs = ZomboidFileSystem {
            base_lower_uri: uri_path(&base.to_lowercase()),
            base,
            active_file_map: HashMap::new(),
            all_absolute_paths: HashSet::new(),
            file_guid_table: FileGuidTable::load_game(&canonical).ok(),
            cache_dir: None,
            mod_id_to_dir: HashMap::new(),
            mod_dir_to_mod: HashMap::new(),
            mod_infos: Vec::new(),
            mods: Vec::new(),
            mod_folders: None,
            mod_folders_order: None,
        };
        let mut load_list = Vec::new();
        search_folders(&canonical.join("media"), &mut load_list);
        for entry in load_list {
            let rel = fs.get_relative_file(&entry);
            let file = crate::java::file_normalize(&entry);
            let mut abs = fs.absolute(&file);
            if Path::new(&abs).is_dir() {
                abs.push('/');
            }
            fs.active_file_map.insert(rel.to_lowercase(), abs.clone());
            fs.all_absolute_paths.insert(abs);
        }
        Ok(fs)
    }

    /// zombie.ZomboidFileSystem.setCacheDir: `new File(dir).getAbsoluteFile()`, created when missing
    /// (`ensureFolderExists`, whose failure the original ignores).
    pub fn set_cache_dir(&mut self, dir: &str) {
        let abs = self.absolute(&java::file_normalize(dir));
        let _ = std::fs::create_dir_all(&abs);
        self.cache_dir = Some(abs);
    }

    /// zombie.ZomboidFileSystem.getCacheDir (`Core.getMyDocumentFolder`): `None` until `set_cache_dir`.
    pub fn get_cache_dir(&self) -> Option<&str> {
        self.cache_dir.as_deref()
    }

    /// zombie.ZomboidFileSystem.getCacheDirSub: `<cacheDir>/<sub_path>`.
    pub fn get_cache_dir_sub(&self, sub_path: &str) -> Option<String> {
        Some(format!("{}/{sub_path}", self.cache_dir.as_deref()?))
    }

    /// zombie.ZomboidFileSystem.Reset: the file map, `allAbsolutePaths`, the mod tables and folders forgotten, and
    /// `ActiveMods.Reset()`. The `Mod` objects go too, so `ChooseGameInfo.Reset()` belongs with it (the game calls
    /// both). `init` must run again before the file map is used.
    pub fn reset(&mut self, active_mods: &mut ActiveModsState) {
        self.active_file_map.clear();
        self.all_absolute_paths.clear();
        self.mod_id_to_dir.clear();
        self.mod_dir_to_mod.clear();
        self.mod_infos.clear();
        self.mods.clear();
        active_mods.reset();
        self.file_guid_table = None;
        self.reset_mod_folders();
    }

    /// `base.canonicalFile` as a path.
    pub fn base(&self) -> &Path {
        Path::new(&self.base)
    }

    /// java.io.File.getAbsolutePath with the working directory at the game folder.
    fn absolute(&self, normalized: &str) -> String {
        if normalized.starts_with('/') {
            normalized.to_string()
        } else if normalized.is_empty() {
            self.base.clone()
        } else {
            format!("{}/{normalized}", self.base)
        }
    }

    /// zombie.ZomboidFileSystem.getFilePathFromGuid
    pub fn get_file_path_from_guid(&self, guid: &str) -> Option<&str> {
        self.file_guid_table.as_ref()?.get_file_path_from_guid(guid)
    }

    /// zombie.ZomboidFileSystem.isValidFilePathGuid
    pub fn is_valid_file_path_guid(&self, source: &str) -> bool {
        self.get_file_path_from_guid(source).is_some()
    }

    /// zombie.ZomboidFileSystem.resolveFileOrGUID: the file a guid names, else `source`; then the absolute path the
    /// file map holds for it (case-insensitive), else unchanged.
    pub fn resolve_file_or_guid(&self, source: &str) -> String {
        let file_name = self.get_file_path_from_guid(source).unwrap_or(source);
        let lower = file_name.to_lowercase();
        match self.active_file_map.get(&lower) {
            Some(abs) => abs.clone(),
            None => file_name.to_string(),
        }
    }

    /// zombie.ZomboidFileSystem.getRelativeFile(URI, String) against `base.lowercaseUri`: `new File(string)`'s
    /// absolute path (File-normalized, against the game folder), lower-cased, as a URI (a directory gets a trailing
    /// '/'), relativized against the lower-cased game folder the way `URI.relativize` does (both paths normalized:
    /// `.` and `..` segments folded); `string` itself when it is not inside the game folder. The percent-encoding round
    /// trip of the URIs is not reproduced.
    pub fn get_relative_file(&self, string: &str) -> String {
        self.get_relative_file_to(&self.base_lower_uri, string)
    }

    /// zombie.ZomboidFileSystem.getRelativeFile(URI root, String string), `root_uri_path` being the root URI's path:
    /// `new File(string)`'s absolute path lower-cased as a URI (a folder gets a trailing '/'), relativized against
    /// the root the way `URI.relativize` does (both paths normalized; the root gets a '/' unless the two are equal,
    /// which gives ""), with a '/' kept from `string`; `string` itself when it is not inside the root.
    pub fn get_relative_file_to(&self, root_uri_path: &str, string: &str) -> String {
        let absolute_path = self.absolute(&java::file_normalize(string));
        let cp = normalize_uri_path(&uri_path(&absolute_path.to_lowercase()));
        let mut bp = normalize_uri_path(root_uri_path);
        if bp != cp {
            if !bp.ends_with('/') {
                bp.push('/');
            }
            if !cp.starts_with(&bp) {
                return string.to_string();
            }
        }
        let mut rel_path = cp[bp.len()..].to_string();
        if string.ends_with('/') && !rel_path.ends_with('/') {
            rel_path.push('/');
        }
        rel_path
    }

    /// zombie.ZomboidFileSystem.getString: the absolute path of a known file, looked up case-insensitively by its path
    /// relative to the game folder. `None` where the original returns its argument itself (callers compare the
    /// result by reference: `getString(path) != path` means "found"). The original's `relativeMap` cache of
    /// relative paths is not kept (a cache: same results).
    pub fn get_string(&self, s: &str) -> Option<&str> {
        let lower = self.get_relative_file(s).to_lowercase();
        self.active_file_map.get(&lower).map(String::as_str)
    }

    /// zombie.ZomboidFileSystem.getDirectoryString: `getString(s)` when found, else `getString` of `s` without its
    /// trailing '/' or with one added (that string itself when not found either).
    pub fn get_directory_string(&self, s: &str) -> String {
        if let Some(abs) = self.get_string(s) {
            return abs.to_string();
        }
        let other = match s.strip_suffix('/') {
            Some(t) => t.to_string(),
            None => format!("{s}/"),
        };
        self.get_string_or(&other).to_string()
    }

    /// zombie.ZomboidFileSystem.isKnownFile: one of the file map's absolute paths, or a path the file map knows
    /// (case-insensitively, relative to the game folder).
    pub fn is_known_file(&self, s: &str) -> bool {
        self.all_absolute_paths.contains(s) || self.get_string(s).is_some()
    }

    /// zombie.ZomboidFileSystem.getAbsolutePath: the file map's entry for `rel` lower-cased.
    pub fn get_absolute_path(&self, rel: &str) -> Option<&str> {
        self.active_file_map
            .get(&rel.to_lowercase())
            .map(String::as_str)
    }

    /// The size of `allAbsolutePaths`.
    pub fn all_absolute_paths_len(&self) -> usize {
        self.all_absolute_paths.len()
    }

    /// `ZomboidFileSystem.getString(s)` as the String the original returns: the absolute path, else `s`.
    pub fn get_string_or<'a>(&'a self, s: &'a str) -> &'a str {
        self.get_string(s).unwrap_or(s)
    }

    /// Where `new FileInputStream(name)` / `new FileReader(name)` open `name`: relative to the game folder.
    pub fn open_path(&self, name: &str) -> PathBuf {
        PathBuf::from(self.absolute(&crate::java::file_normalize(name)))
    }

    /// zombie.ZomboidFileSystem.resolveRelativePath: `relative_path` against the folder of `src_file_path`
    /// (`Path.getParent().resolve`, not normalized), then `getRelativeFile`.
    pub fn resolve_relative_path(&self, src_file_path: &str, relative_path: &str) -> String {
        let src = Path::new(src_file_path);
        let folder = src.parent().unwrap_or(Path::new(""));
        let dest = folder.join(relative_path);
        self.get_relative_file(&slashes(&dest))
    }

    /// zombie.ZomboidFileSystem.getMediaFile: `<game>/media/<sub_path>`.
    pub fn get_media_file(&self, sub_path: &str) -> PathBuf {
        Path::new(&self.base).join("media").join(sub_path)
    }

    /// zombie.ZomboidFileSystem.walkGameAndModFiles over the game folder alone: `visit(file, rel_path)` for every entry
    /// of `<game>/<rel_path>` in listing order, recursing into directories after visiting them when `recursive`.
    fn walk_game_files(&self, rel_path: &str, recursive: bool, visit: &mut dyn FnMut(&Path, &str)) {
        let fo = Path::new(&self.base).join(rel_path);
        if !fo.is_dir() {
            return;
        }
        let Some(list) = list_files(&fo) else {
            return;
        };
        for listed in list {
            visit(&listed, rel_path);
            if recursive && listed.is_dir() {
                let name = listed
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.walk_game_files(&format!("{rel_path}/{name}"), true, visit);
            }
        }
    }

    /// zombie.ZomboidFileSystem.resolveAllDirectories: `<rel_path>/<name>` of every directory the walk meets that
    /// `filter` accepts, each once, in walk order.
    pub fn resolve_all_directories(
        &self,
        rel_path: &str,
        filter: &dyn Fn(&Path) -> bool,
        recursive: bool,
    ) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        self.walk_game_files(rel_path, recursive, &mut |file, rel2| {
            if file.is_dir() && filter(file) {
                let name = file
                    .file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default();
                let rel3 = format!("{rel2}/{name}");
                if !result.contains(&rel3) {
                    result.push(rel3);
                }
            }
        });
        result
    }

    /// zombie.ZomboidFileSystem.resolveAllFiles: `<rel_path>/<name>` of every file the walk meets that `filter`
    /// accepts, each once, in walk order.
    pub fn resolve_all_files(
        &self,
        rel_path: &str,
        filter: &dyn Fn(&Path) -> bool,
        recursive: bool,
    ) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        self.walk_game_files(rel_path, recursive, &mut |file, rel2| {
            if file.is_file() && filter(file) {
                let name = file
                    .file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default();
                let rel3 = format!("{rel2}/{name}");
                if !result.contains(&rel3) {
                    result.push(rel3);
                }
            }
        });
        result
    }
}

/// The mod side of zombie.ZomboidFileSystem.
impl ZomboidFileSystem {
    /// java.nio.file.Path.toAbsolutePath().toString() of a normalized path, against the game folder.
    pub fn to_absolute_path(&self, normalized: &str) -> String {
        self.absolute(normalized)
    }

    /// zombie.ZomboidFileSystem.resetModFolders: the mod folders are listed again on next use.
    pub fn reset_mod_folders(&mut self) {
        self.mod_folders = None;
    }

    /// zombie.ZomboidFileSystem.getModVersionDirName: of the names `File.list` gives in `mod_dir` (folders or not),
    /// in listing order, the last whose `getGameVersionIntFromName` is the highest of those from 42.0 up to the game's
    /// version; "42.0" (`ChooseGameInfo.getMinRequiredVersion()`) when none is.
    pub fn get_mod_version_dir_name(&self, mod_dir: &str) -> String {
        use crate::zombie::core::game_version::core_game_version;
        use crate::zombie::game_states::choose_game_info::get_min_required_version;
        let game_version = core_game_version().get_int();
        let min = get_min_required_version();
        let mut version_dir_name = min.to_string();
        let mut best_version = min.get_int();
        if let Some(names) = file_list(Path::new(mod_dir)) {
            for name in names {
                let version = self.get_game_version_int_from_name(&name);
                if version >= best_version && version <= game_version {
                    version_dir_name = name;
                    best_version = version;
                }
            }
        }
        version_dir_name
    }

    /// zombie.ZomboidFileSystem.getGameVersionIntFromName: `name.split("\\.")`; one piece `p0 * 1000`, two or more
    /// `p0 * 1000 + min(p1, 999)` (int arithmetic), each through `PZMath.tryParseInt(s, 0)`; none 0.
    pub fn get_game_version_int_from_name(&self, name: &str) -> i32 {
        use crate::zombie::core::math::pz_math::try_parse_int;
        let parts = java::split_char(name, '.');
        match parts.len() {
            1 => try_parse_int(Some(parts[0]), 0).wrapping_mul(1000),
            n if n >= 2 => try_parse_int(Some(parts[0]), 0)
                .wrapping_mul(1000)
                .wrapping_add(try_parse_int(Some(parts[1]), 0).min(999)),
            _ => 0,
        }
    }

    /// zombie.ZomboidFileSystem.getAllModFoldersAux: the folders of `folder` (directory stream order) other than
    /// "examplemod" (any letter case) that hold `common/mod.info` or `<version folder>/mod.info`, as absolute paths.
    fn get_all_mod_folders_aux(&self, folder: &str, out: &mut Vec<String>) {
        let dir = Path::new(folder);
        if !dir.exists() {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let path = dir.join(e.file_name());
            if !path.is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            if java::equals_ignore_case(&name, "examplemod") {
                // DebugType.Mod.println("refusing to list " + name)
                continue;
            }
            let path_str = format!("{}/{name}", java::file_normalize(folder));
            let version_dir_name = self.get_mod_version_dir_name(&path_str);
            if path.join("common").join("mod.info").exists()
                || path.join(&version_dir_name).join("mod.info").exists()
            {
                out.push(self.absolute(&path_str));
            }
        }
    }

    /// zombie.ZomboidFileSystem.setModFoldersOrder: `s.split(",")`.
    pub fn set_mod_folders_order(&mut self, s: &str) {
        self.mod_folders_order = Some(
            java::split_char(s, ',')
                .into_iter()
                .map(str::to_string)
                .collect(),
        );
    }

    /// zombie.ZomboidFileSystem.getAllModFolders: the mod folders, listed once until `resetModFolders`: for each
    /// entry of `modFoldersOrder` ("workshop,steam,mods" when unset) the folders that hold mods, then every mod in
    /// them. "workshop" and "steam" are Steam's folders (none without Steam mode); "mods" is the cache dir's
    /// `mods`, created when missing.
    pub fn get_all_mod_folders(&mut self) -> Vec<String> {
        if self.mod_folders.is_none() {
            let mut result = Vec::new();
            if self.mod_folders_order.is_none() {
                self.set_mod_folders_order("workshop,steam,mods");
            }
            let mut mods_folders: Vec<String> = Vec::new();
            for s in self.mod_folders_order.clone().unwrap_or_default() {
                // "workshop": getStagedItemModsFolders, "steam": getInstalledItemModsFolders (Steam mode only)
                if s == "mods" {
                    if let Some(sub) = self.get_cache_dir_sub("mods") {
                        let file = self.absolute(&java::file_normalize(&sub));
                        if !Path::new(&file).exists() {
                            let _ = std::fs::create_dir_all(&file);
                        }
                        mods_folders.push(file);
                    }
                }
            }
            for folder in &mods_folders {
                self.get_all_mod_folders_aux(folder, &mut result);
            }
            self.mod_folders = Some(result);
        }
        self.mod_folders.clone().unwrap_or_default()
    }

    /// `modFolders` as last listed (`None` before).
    pub fn mod_folders(&self) -> Option<&[String]> {
        self.mod_folders.as_deref()
    }

    /// `modFoldersOrder`
    pub fn mod_folders_order(&self) -> Option<&[String]> {
        self.mod_folders_order.as_deref()
    }

    /// zombie.ZomboidFileSystem.setModIdToDir: the first folder seen for an id stays.
    pub fn set_mod_id_to_dir(&mut self, id: &str, dir: &str) {
        self.mod_id_to_dir
            .entry(id.to_string())
            .or_insert_with(|| dir.to_string());
    }

    /// zombie.ZomboidFileSystem.getModDir
    pub fn get_mod_dir(&self, mod_id: &str) -> Option<&str> {
        self.mod_id_to_dir.get(mod_id).map(String::as_str)
    }

    /// `modIdToDir`
    pub fn mod_id_to_dir(&self) -> &HashMap<String, String> {
        &self.mod_id_to_dir
    }

    /// `modDirToMod`: folder → handle.
    pub fn mod_dir_to_mod(&self) -> &HashMap<String, usize> {
        &self.mod_dir_to_mod
    }

    /// zombie.ZomboidFileSystem.getModInfoForDir: the folder's `Mod`, made (unread) on first use.
    pub fn get_mod_info_for_dir(&mut self, mod_dir: &str) -> usize {
        if let Some(&i) = self.mod_dir_to_mod.get(mod_dir) {
            return i;
        }
        let m = Mod::new(mod_dir, self);
        self.mod_infos.push(m);
        let i = self.mod_infos.len() - 1;
        self.mod_dir_to_mod.insert(mod_dir.to_string(), i);
        i
    }

    /// The `Mod` behind a handle.
    pub fn mod_info(&self, handle: usize) -> &Mod {
        &self.mod_infos[handle]
    }

    /// The `Mod` behind a handle, to change.
    pub fn mod_info_mut(&mut self, handle: usize) -> &mut Mod {
        &mut self.mod_infos[handle]
    }

    /// zombie.ZomboidFileSystem.getModIDs: the ids the last `loadMods` loaded, in order.
    pub fn get_mod_ids(&self) -> &[String] {
        &self.mods
    }

    /// zombie.ZomboidFileSystem.loadMod: the mod's common folder, then its version folder, through `searchFolders`;
    /// each entry goes into the file map under its lower-cased path relative to that folder (shadowing the game's
    /// file or an earlier mod's of the same path) and into `allAbsolutePaths`. A folder that does not exist is itself
    /// an entry, of relative path "". Nothing for an id without a known folder.
    pub fn load_mod(&mut self, mod_id: &str) {
        let Some(dir) = self.get_mod_dir(mod_id).map(str::to_string) else {
            return;
        };
        // CoopMaster.instance.update(): no coop server messages here
        let handle = self.get_mod_info_for_dir(&dir);
        let subdirs = [
            self.mod_infos[handle].common_dir.clone(),
            self.mod_infos[handle].version_dir.clone(),
        ];
        for sub in subdirs {
            // new File(dir.toLowerCase(Locale.ENGLISH)).toURI()
            let mod_pathbase_uri =
                uri_path(&self.absolute(&java::file_normalize(&sub.to_lowercase())));
            let mut load_list = Vec::new();
            search_folders(Path::new(&sub), &mut load_list);
            for entry in load_list {
                let rel = self
                    .get_relative_file_to(&mod_pathbase_uri, &entry)
                    .to_lowercase();
                // DebugType.Mod.println("mod \"<id>\" overrides <rel>") for a known path other than mod.info/poster.png
                let abs_path = self.absolute(&java::file_normalize(&entry));
                self.active_file_map.insert(rel, abs_path.clone());
                self.all_absolute_paths.insert(abs_path);
            }
        }
    }

    /// zombie.ZomboidFileSystem.loadModAndRequired: puts `mod_id` into `ordered` after its required mods (each
    /// once); "" and "examplemod" are skipped. `Some(id)` names the first mod that is not available (missing, outside
    /// its game versions, or with a required mod that is not); nothing after it is ordered. A require cycle recurses
    /// without end, as in the original (StackOverflowError there; not measured).
    pub fn load_mod_and_required(
        &mut self,
        cgi: &mut ChooseGameInfo,
        tr: &mut Translator,
        mod_id: &str,
        ordered: &mut Vec<String>,
    ) -> Option<String> {
        if mod_id.is_empty() {
            return None;
        }
        if java::equals_ignore_case(mod_id, "examplemod") {
            // DebugType.Mod.warn("refusing to load " + modId)
            return None;
        }
        if ordered.iter().any(|o| o == mod_id) {
            return None;
        }
        let Some(info) = cgi.get_available_mod_details(self, tr, mod_id) else {
            // GameServer.ServerMods.remove(modId) on a server; DebugType.Mod.warn("required mod ... not found")
            return Some(mod_id.to_string());
        };
        if let Some(require) = self.mod_infos[info].get_require().map(<[String]>::to_vec) {
            if let Some(req_id) = self.load_mods_aux(cgi, tr, &require, ordered) {
                return Some(req_id);
            }
        }
        ordered.push(mod_id.to_string());
        None
    }

    /// zombie.ZomboidFileSystem.loadModsAux: `loadModAndRequired` for each id until one fails (its id returned).
    pub fn load_mods_aux(
        &mut self,
        cgi: &mut ChooseGameInfo,
        tr: &mut Translator,
        to_load: &[String],
        ordered: &mut Vec<String>,
    ) -> Option<String> {
        for mod_id in to_load {
            if let Some(fail_id) = self.load_mod_and_required(cgi, tr, mod_id, ordered) {
                return Some(fail_id);
            }
        }
        None
    }

    /// zombie.ZomboidFileSystem.loadMods(List): the load order (`loadModAndRequired` of each id; a failing one is
    /// left out with what it would have needed), kept as `getModIDs`, then `loadMod` of each in that order.
    pub fn load_mods(&mut self, cgi: &mut ChooseGameInfo, tr: &mut Translator, to_load: &[String]) {
        let mut mods = std::mem::take(&mut self.mods);
        mods.clear();
        for mod_id in to_load {
            self.load_mod_and_required(cgi, tr, mod_id, &mut mods);
        }
        self.mods = mods;
        for mod_id in self.mods.clone() {
            self.load_mod(&mod_id);
        }
    }

    /// zombie.ZomboidFileSystem.loadMods(String) outside a client: with mods enabled, the ActiveMods list of that id
    /// becomes the loaded list and is loaded; for "default" (any letter case) the list is first read from the cache
    /// dir's `mods/default.txt` and its unavailable mods dropped (`checkMissingMods`; `checkMissingMaps` is not
    /// ported: a list with maps is an `Err`). `Err` is also the exception of an invalid id; the original logs and
    /// skips the default list's other failures.
    pub fn load_mods_by_id(
        &mut self,
        cgi: &mut ChooseGameInfo,
        tr: &mut Translator,
        active_mods: &mut ActiveModsState,
        id: &str,
    ) -> Result<(), ActiveModsError> {
        if !CORE_OPTION_MODS_ENABLED {
            return Ok(());
        }
        let list = active_mods.get_by_id(id)?;
        if !java::equals_ignore_case("default", id) {
            active_mods.set_loaded_mods(list);
            let mods = active_mods.get(list).get_mods().to_vec();
            self.load_mods(cgi, tr, &mods);
            return Ok(());
        }
        let list = self.read_default_mods_txt(active_mods)?;
        active_mods.check_missing_mods(list, cgi, self, tr);
        if !active_mods.get(list).get_map_order().is_empty() {
            return Err(ActiveModsError::not_ported(
                "ActiveMods.checkMissingMaps (MapGroups)",
            ));
        }
        active_mods.set_loaded_mods(list);
        let mods = active_mods.get(list).get_mods().to_vec();
        self.load_mods(cgi, tr, &mods);
        Ok(())
    }

    /// zombie.ZomboidFileSystem.readDefaultModsTxt: the "default" list, cleared and read from
    /// `<cacheDir>/mods/default.txt` (`ActiveModsFile.read`). A `mods/loaded.txt` (an older game's list, merged in and
    /// saved back) is not ported: `Err`.
    fn read_default_mods_txt(
        &mut self,
        active_mods: &mut ActiveModsState,
    ) -> Result<usize, ActiveModsError> {
        let list = active_mods.get_by_id("default")?;
        let Some(dir) = self.get_cache_dir_sub("mods") else {
            return Err(ActiveModsError::not_ported(
                "the default cache dir (~/Zomboid)",
            ));
        };
        if Path::new(&format!("{dir}/loaded.txt")).exists() {
            return Err(ActiveModsError::not_ported(
                "readLoadedDotTxt/saveModsFile (ActiveModsFile.write)",
            ));
        }
        active_mods.get_mut(list).clear();
        crate::zombie::modding::active_mods_file::read(
            &format!("{dir}/default.txt"),
            active_mods.get_mut(list),
        );
        Ok(list)
    }
}

/// java.io.File.list: the names in `dir` in the file system's order; `None` when it is not a readable folder.
fn file_list(dir: &Path) -> Option<Vec<String>> {
    let rd = std::fs::read_dir(dir).ok()?;
    Some(
        rd.flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect(),
    )
}

/// java.io.File.toURI's path for an absolute, normalized path: a '/' added when it names a folder.
fn uri_path(abs: &str) -> String {
    let mut p = abs.to_string();
    if !p.ends_with('/') && Path::new(abs).is_dir() {
        p.push('/');
    }
    p
}

/// zombie.ZomboidFileSystem.searchFolders: every file under `fo`, and the folders whose path contains `media/maps/`,
/// as `getAbsolutePath()` with "./" removed, in `File.list` order.
fn search_folders(fo: &Path, load_list: &mut Vec<String>) {
    let path = slashes(fo).replace("./", "");
    if fo.is_dir() {
        if path.contains("media/maps/") {
            load_list.push(path);
        }
        if let Ok(rd) = std::fs::read_dir(fo) {
            for e in rd.flatten() {
                search_folders(&fo.join(e.file_name()), load_list);
            }
        }
    } else {
        load_list.push(path);
    }
}

/// java.net.URI.normalize of a path: `.` segments dropped, `seg/..` pairs folded (a leading `..` stays).
fn normalize_uri_path(path: &str) -> String {
    let absolute = path.starts_with('/');
    let trailing = path.ends_with('/');
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if matches!(out.last(), Some(s) if *s != "..") {
                    out.pop();
                } else if !absolute {
                    out.push("..");
                }
            }
            s => out.push(s),
        }
    }
    let mut r = String::new();
    if absolute {
        r.push('/');
    }
    r.push_str(&out.join("/"));
    if trailing && !out.is_empty() {
        r.push('/');
    }
    r
}

#[cfg(test)]
mod tests {
    use super::normalize_uri_path;

    #[test]
    fn uri_normalize_folds_dot_segments() {
        assert_eq!(normalize_uri_path("/a/b/./c/../../d.xml"), "/a/d.xml");
        assert_eq!(normalize_uri_path("/a/b/"), "/a/b/");
    }
}
