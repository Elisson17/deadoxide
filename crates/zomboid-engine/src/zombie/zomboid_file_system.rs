//! zombie.ZomboidFileSystem: the game folder's file map (`init` over `media`) and the lookups through it (`getString`,
//! `getRelativeFile`). No mods (the oracles run the game without any). The original keeps it in a static; here one
//! value per game folder. Paths are Unix paths with '/' separators; the process working directory is the game folder,
//! as when the game runs.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct ZomboidFileSystem {
    /// ZomboidFileSystem.base.canonicalFile.getPath(): the canonical game folder.
    base: String,
    /// ZomboidFileSystem.base.lowercaseUri's path: the lower-cased game folder with a trailing '/'.
    base_lower: String,
    /// ZomboidFileSystem.activeFileMap: lower-case path relative to the game folder ("media/fonts/en/fonts.txt") →
    /// absolute path with the file's own letter case.
    pub active_file_map: HashMap<String, String>,
}

impl ZomboidFileSystem {
    /// zombie.ZomboidFileSystem.init: `base` is the canonical game folder, `searchFolders(<base>/media)` lists every
    /// file (and every folder under `media/maps/`), then each goes into `activeFileMap` under its lower-cased
    /// relative path (`getRelativeFile`), a folder's absolute path ending in '/'. The original lists folders in the file
    /// system's order; that order only matters for two names differing in case only (one overwrites the other).
    pub fn init(game_root: &Path) -> Result<ZomboidFileSystem, String> {
        let canonical = game_root
            .canonicalize()
            .map_err(|e| format!("{}: {e}", game_root.display()))?;
        let base = canonical.to_string_lossy().replace('\\', "/");
        let mut fs = ZomboidFileSystem {
            base_lower: format!("{}/", base.to_lowercase()),
            base,
            active_file_map: HashMap::new(),
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

    /// The canonical game folder (`base.canonicalFile.getPath()`).
    pub fn base(&self) -> &str {
        &self.base
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

    /// zombie.ZomboidFileSystem.getRelativeFile(URI, String) against `base.lowercaseUri`: the lower-cased absolute
    /// path made relative to the lower-cased game folder (a folder that exists gets a trailing '/', as
    /// `File.toURI` gives it), or `string` itself when it lies outside. `URI.relativize`'s "."/".." folding and
    /// percent-encoding round trip are not reproduced (the font lookups use neither).
    pub fn get_relative_file(&self, string: &str) -> String {
        let absolute_path = self.absolute(&crate::java::file_normalize(string));
        let mut lower = absolute_path.to_lowercase();
        if !lower.ends_with('/') && Path::new(&lower).is_dir() {
            lower.push('/');
        }
        match lower.strip_prefix(&self.base_lower) {
            Some(rel) => {
                let mut rel_path = rel.to_string();
                if string.ends_with('/') && !rel_path.ends_with('/') {
                    rel_path.push('/');
                }
                rel_path
            }
            None => string.to_string(),
        }
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
}

/// zombie.ZomboidFileSystem.searchFolders: every file under `fo`, and the folders whose path contains `media/maps/`,
/// as `getAbsolutePath()` with "./" removed.
fn search_folders(fo: &Path, load_list: &mut Vec<String>) {
    let path = fo.to_string_lossy().replace('\\', "/").replace("./", "");
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
