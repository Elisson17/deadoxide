//! zombie.ZomboidFileSystem: the game folder, its file map (`activeFileMap`), the file guid table, and the lookups and
//! path helpers the font, XML, animation and action loaders use. Only the game itself (no mods: `getModIDs` is empty,
//! as in the oracles); one value per game folder instead of the `instance` singleton. Paths are '/'-separated; the
//! process working directory is the game folder, as when the game runs. Directory listings are in the file system's own
//! order, like `File.list`/`File.listFiles`.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::file_guid_table::FileGuidTable;

/// zombie.ZomboidFileSystem as far as the ported loaders use it.
pub struct ZomboidFileSystem {
    /// `base.canonicalFile.getPath()`: the canonical game folder, '/'-separated.
    base: String,
    /// `activeFileMap`: lower-case path relative to the game folder ("media/fonts/en/fonts.txt") → absolute path with
    /// the file's own letter case (every file under `media`, and the directories under `media/maps/` with a trailing
    /// separator).
    pub active_file_map: HashMap<String, String>,
    /// `fileGuidTable` (`getFileGuidTable` loads it on first use; here at `init`). `None` where it failed to load.
    file_guid_table: Option<FileGuidTable>,
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
        let mut fs = ZomboidFileSystem {
            base: slashes(&canonical),
            active_file_map: HashMap::new(),
            file_guid_table: FileGuidTable::load_game(&canonical).ok(),
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
            fs.active_file_map.insert(rel.to_lowercase(), abs);
        }
        Ok(fs)
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
        let absolute_path = self.absolute(&crate::java::file_normalize(string));
        let lower = absolute_path.to_lowercase();
        let mut child = normalize_uri_path(&lower);
        // File.toURI marks a directory with a trailing slash.
        if !child.ends_with('/') && Path::new(&lower).is_dir() {
            child.push('/');
        }
        let mut root = normalize_uri_path(&self.base.to_lowercase());
        if !root.ends_with('/') {
            root.push('/');
        }
        if child == root {
            return String::new();
        }
        let Some(rel) = child.strip_prefix(&root) else {
            return string.to_string();
        };
        let mut rel_path = rel.to_string();
        if string.ends_with('/') && !rel_path.ends_with('/') {
            rel_path.push('/');
        }
        rel_path
    }

    /// zombie.ZomboidFileSystem.getString: the absolute path of a known file, looked up case-insensitively by its path
    /// relative to the game folder. `None` where the original returns its argument itself (callers compare the
    /// result by reference: `getString(path) != path` means "found").
    pub fn get_string(&self, s: &str) -> Option<&str> {
        let lower = self.get_relative_file(s).to_lowercase();
        self.active_file_map.get(&lower).map(String::as_str)
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
