//! zombie.MapGroups: the map directories (`media/maps/*` and the active mods') that reference each other through their
//! `map.info` `lots=` lines, grouped so that one world is a group. The original's MapDirectory objects are kept in one
//! arena (`real_directories`) and the groups hold indices into it: the original compares them by identity.
use std::path::Path;

use crate::java;
use crate::java_formatter::FormatArg;
use crate::zombie::core::translator::Translator;
use crate::zombie::zomboid_file_system::ZomboidFileSystem;

/// zombie.MapGroups.MapDirectory
#[derive(Clone, Debug, PartialEq)]
pub struct MapDirectory {
    pub name: String,
    pub path: String,
    pub lot_dirs: Vec<String>,
    pub conflicts: Vec<String>,
}

impl MapDirectory {
    /// zombie.MapGroups.MapDirectory.<init>(name, path, lotDirs)
    fn new(name: &str, path: &str, lot_dirs: &[String]) -> MapDirectory {
        MapDirectory {
            name: name.to_string(),
            path: path.to_string(),
            lot_dirs: lot_dirs.to_vec(),
            conflicts: Vec::new(),
        }
    }

    /// zombie.MapGroups.MapDirectory.getLotHeaders: the `.lotheader` entries of the directory in listing order.
    fn get_lot_headers(&self, result: &mut Vec<String>) {
        let fo = Path::new(&self.path);
        if fo.is_dir() {
            if let Some(names) = file_list(fo) {
                for n in names {
                    if n.ends_with(".lotheader") {
                        result.push(n);
                    }
                }
            }
        }
    }
}

/// An active mod's directories as ChooseGameInfo.Mod gives them (`getCommonDir()`, `getVersionDir()`).
#[derive(Clone, Debug, PartialEq)]
pub struct ModDirs {
    pub common_dir: String,
    pub version_dir: String,
}

/// java.io.File.list: the names in the directory in the order the file system lists them; `None` when it cannot be
/// read.
fn file_list(dir: &Path) -> Option<Vec<String>> {
    let rd = std::fs::read_dir(dir).ok()?;
    Some(
        rd.flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect(),
    )
}

/// java.io.BufferedReader.readLine over a whole text: `\n`, `\r` and `\r\n` end a line; no empty last line.
fn read_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        match rest.find(['\n', '\r']) {
            Some(i) => {
                lines.push(&rest[..i]);
                let skip = if rest[i..].starts_with("\r\n") { 2 } else { 1 };
                rest = &rest[i + skip..];
            }
            None => {
                lines.push(rest);
                rest = "";
            }
        }
    }
    lines
}

/// zombie.MapGroups
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapGroups {
    /// `groups`: each a LinkedList of indices into `real_directories`.
    groups: Vec<Vec<usize>>,
    real_directories: Vec<MapDirectory>,
}

/// zombie.MapGroups.getVanillaMapDirectories: the entries of `media/maps` in listing order, with the challenge maps
/// (`challengemaps/<dir>` holding a `map.info`) in place of the `challengemaps` folder when `include_challenges`.
fn get_vanilla_map_directories(fs: &ZomboidFileSystem, include_challenges: bool) -> Vec<String> {
    let mut result = Vec::new();
    let maps_file = fs.get_media_file("maps");
    if let Some(names) = file_list(&maps_file) {
        for directory_name in names {
            if directory_name.eq_ignore_ascii_case("challengemaps") {
                if include_challenges {
                    // a failure to open the stream is swallowed
                    if let Ok(rd) = std::fs::read_dir(maps_file.join(&directory_name)) {
                        for e in rd.flatten() {
                            let p = e.path();
                            if p.is_dir() && p.join("map.info").exists() {
                                result.push(format!(
                                    "{directory_name}/{}",
                                    e.file_name().to_string_lossy()
                                ));
                            }
                        }
                    }
                }
            } else {
                result.push(directory_name);
            }
        }
    }
    result
}

/// zombie.MapGroups.addMissingVanillaDirectories: `map_name` plus the vanilla directories it does not name, when it
/// names a vanilla one.
pub fn add_missing_vanilla_directories(fs: &ZomboidFileSystem, map_name: &str) -> String {
    let vanilla_dirs = get_vanilla_map_directories(fs, false);
    let mut uses_vanilla = false;
    let ss = java::split_char(map_name, ';');
    for name in &ss {
        let name = java::trim(name);
        if !name.is_empty() && vanilla_dirs.iter().any(|v| v == name) {
            uses_vanilla = true;
            break;
        }
    }
    if !uses_vanilla {
        return map_name.to_string();
    }
    let mut names: Vec<String> = Vec::new();
    for name in &ss {
        let name = java::trim(name);
        if !name.is_empty() {
            names.push(name.to_string());
        }
    }
    for v in &vanilla_dirs {
        if !names.contains(v) {
            names.push(v.clone());
        }
    }
    let mut result = String::new();
    for n in &names {
        if !result.is_empty() {
            result.push(';');
        }
        result.push_str(n);
    }
    result
}

impl MapGroups {
    /// zombie.MapGroups.<init>
    pub fn new() -> MapGroups {
        MapGroups::default()
    }

    /// zombie.MapGroups.createGroups(activeMods, includeVanilla, includeChallenges): `mods` are the details of the
    /// active mods that `ChooseGameInfo.getAvailableModDetails` found (in `getMods()` order), `map_order`
    /// `activeMods.getMapOrder()`.
    pub fn create_groups(
        &mut self,
        fs: &ZomboidFileSystem,
        mods: &[ModDirs],
        map_order: &[String],
        include_vanilla: bool,
        include_challenges: bool,
    ) {
        self.groups.clear();
        self.real_directories.clear();
        for mod_details in mods {
            let common = format!("{}/media/maps/", mod_details.common_dir);
            let fo = Path::new(&common);
            if fo.exists() {
                if let Some(names) = file_list(fo) {
                    for directory_name in &names {
                        if directory_name.eq_ignore_ascii_case("challengemaps") {
                            // includeChallenges: an empty branch in the original
                            let _ = include_challenges;
                        } else {
                            self.handle_map_directory(
                                directory_name,
                                &format!("{}/media/maps/{directory_name}", mod_details.common_dir),
                            );
                        }
                    }
                    let version = format!("{}/media/maps/", mod_details.version_dir);
                    let fo = Path::new(&version);
                    if fo.exists() {
                        if let Some(names) = file_list(fo) {
                            for directory_name in &names {
                                if !directory_name.eq_ignore_ascii_case("challengemaps") {
                                    self.handle_map_directory(
                                        directory_name,
                                        &format!(
                                            "{}/media/maps/{directory_name}",
                                            mod_details.version_dir
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        if include_vanilla {
            let vanilla_maps = get_vanilla_map_directories(fs, include_challenges);
            let maps_path = fs.get_media_file("maps").to_string_lossy().into_owned();
            for directory_name in &vanilla_maps {
                self.handle_map_directory(
                    directory_name,
                    &format!("{maps_path}{}{directory_name}", std::path::MAIN_SEPARATOR),
                );
            }
        }
        for map_dir in 0..self.real_directories.len() {
            let mut directories = Vec::new();
            self.get_dirs_recursively(map_dir, &mut directories);
            let group = match self.find_group_with_any_of_these_directories(&directories) {
                Some(g) => g,
                None => {
                    self.groups.push(Vec::new());
                    self.groups.len() - 1
                }
            };
            for map_dir2 in directories {
                if !self.group_has_directory(group, &self.real_directories[map_dir2].name) {
                    self.groups[group].push(map_dir2);
                }
            }
        }
        for g in 0..self.groups.len() {
            self.group_set_priority(g);
        }
        for g in 0..self.groups.len() {
            // MapGroup.setOrder
            if !map_order.is_empty() {
                self.group_set_priority_list(g, map_order);
            }
        }
    }

    /// zombie.MapGroups.getDirsRecursively
    fn get_dirs_recursively(&self, map_dir: usize, result: &mut Vec<usize>) {
        if !result.contains(&map_dir) {
            result.push(map_dir);
            for lot_dir in &self.real_directories[map_dir].lot_dirs {
                for (i, map_dir2) in self.real_directories.iter().enumerate() {
                    if map_dir2.name == *lot_dir {
                        self.get_dirs_recursively(i, result);
                        break;
                    }
                }
            }
        }
    }

    /// zombie.MapGroups.getNumberOfGroups
    pub fn get_number_of_groups(&self) -> i32 {
        self.groups.len() as i32
    }

    /// zombie.MapGroups.getMapDirectoriesInGroup: the names; an index out of range is a RuntimeException.
    pub fn get_map_directories_in_group(&self, group_index: i32) -> Result<Vec<String>, String> {
        if group_index >= 0 && (group_index as usize) < self.groups.len() {
            Ok(self.groups[group_index as usize]
                .iter()
                .map(|d| self.real_directories[*d].name.clone())
                .collect())
        } else {
            Err(format!(
                "java.lang.RuntimeException: invalid MapGroups index {group_index}"
            ))
        }
    }

    /// zombie.MapGroups.handleMapDirectory
    fn handle_map_directory(&mut self, directory_name: &str, path: &str) {
        if let Some(lot_dirs) = Self::get_lot_directories(path) {
            self.real_directories
                .push(MapDirectory::new(directory_name, path, &lot_dirs));
        }
    }

    /// zombie.MapGroups.getLotDirectories: the `lots=` values of `<path>/map.info`; `None` when it does not exist
    /// or cannot be read.
    fn get_lot_directories(path: &str) -> Option<Vec<String>> {
        let file = Path::new(path).join("map.info");
        if !file.exists() {
            return None;
        }
        let bytes = std::fs::read(file).ok()?;
        let text = String::from_utf8_lossy(&bytes);
        let mut lot_dirs = Vec::new();
        for line in read_lines(&text) {
            let line = java::trim(line);
            if line.starts_with("lots=") {
                lot_dirs.push(java::trim(&line.replace("lots=", "")).to_string());
            }
        }
        Some(lot_dirs)
    }

    /// zombie.MapGroups.findGroupWithAnyOfTheseDirectories
    fn find_group_with_any_of_these_directories(&self, directories: &[usize]) -> Option<usize> {
        self.groups
            .iter()
            .position(|g| directories.iter().any(|d| g.contains(d)))
    }

    /// zombie.MapGroups.getAllMapsInOrder
    pub fn get_all_maps_in_order(&self) -> Vec<String> {
        self.groups
            .iter()
            .flatten()
            .map(|d| self.real_directories[*d].name.clone())
            .collect()
    }

    /// zombie.MapGroups.checkMapConflicts
    pub fn check_map_conflicts(&mut self, translator: &mut Translator) -> Result<bool, String> {
        let mut has_conflicts = false;
        for g in 0..self.groups.len() {
            has_conflicts |= self.group_check_map_conflicts(g, translator)?;
        }
        Ok(has_conflicts)
    }

    /// zombie.MapGroups.getMapConflicts: the conflicts of the first directory with this name, `None` (null) when no
    /// group has one.
    pub fn get_map_conflicts(&self, map_name: &str) -> Option<Vec<String>> {
        for g in &self.groups {
            if let Some(d) = g
                .iter()
                .find(|d| self.real_directories[**d].name == map_name)
            {
                return Some(self.real_directories[*d].conflicts.clone());
            }
        }
        None
    }

    // ---- zombie.MapGroups.MapGroup ----

    /// MapGroup.getDirectoryByName
    fn group_directory_by_name(&self, g: usize, name: &str) -> Option<usize> {
        self.groups[g]
            .iter()
            .copied()
            .find(|d| self.real_directories[*d].name == name)
    }

    /// MapGroup.hasDirectory
    fn group_has_directory(&self, g: usize, name: &str) -> bool {
        self.group_directory_by_name(g, name).is_some()
    }

    /// MapGroup.isReferencedByOtherMaps
    fn group_is_referenced_by_other_maps(&self, g: usize, map_dir: usize) -> bool {
        let name = &self.real_directories[map_dir].name;
        self.groups[g]
            .iter()
            .any(|d2| *d2 != map_dir && self.real_directories[*d2].lot_dirs.contains(name))
    }

    /// MapGroup.getDirsRecursively
    fn group_get_dirs_recursively(&self, g: usize, map_dir: usize, result: &mut Vec<String>) {
        let d = &self.real_directories[map_dir];
        if !result.contains(&d.name) {
            result.push(d.name.clone());
            for lot_dir in &d.lot_dirs {
                if let Some(d2) = self.group_directory_by_name(g, lot_dir) {
                    self.group_get_dirs_recursively(g, d2, result);
                }
            }
        }
    }

    /// MapGroup.setPriority()
    fn group_set_priority(&mut self, g: usize) {
        for map_dir in self.groups[g].clone() {
            if !self.group_is_referenced_by_other_maps(g, map_dir) {
                let mut priority_list = Vec::new();
                self.group_get_dirs_recursively(g, map_dir, &mut priority_list);
                self.group_set_priority_list(g, &priority_list);
            }
        }
    }

    /// MapGroup.setPriority(List<String>)
    fn group_set_priority_list(&mut self, g: usize, priority_list: &[String]) {
        let mut sorted: Vec<usize> = Vec::with_capacity(priority_list.len());
        for name in priority_list {
            if let Some(d) = self.group_directory_by_name(g, name) {
                sorted.push(d);
            }
        }
        for i in 0..self.groups[g].len() {
            let map_dir1 = self.groups[g][i];
            if priority_list.contains(&self.real_directories[map_dir1].name) {
                self.groups[g][i] = sorted.remove(0);
            }
        }
    }

    /// MapGroup.checkMapConflicts
    fn group_check_map_conflicts(
        &mut self,
        g: usize,
        translator: &mut Translator,
    ) -> Result<bool, String> {
        // HashMap<String, ArrayList<String>>: keys in put order, iterated in the HashMap's order
        let mut lot_header_to_dir: Vec<(String, Vec<String>)> = Vec::new();
        let mut lot_headers = Vec::new();
        for map_dir in self.groups[g].clone() {
            self.real_directories[map_dir].conflicts.clear();
            lot_headers.clear();
            self.real_directories[map_dir].get_lot_headers(&mut lot_headers);
            for lot_header in &lot_headers {
                let name = self.real_directories[map_dir].name.clone();
                match lot_header_to_dir.iter_mut().find(|(k, _)| k == lot_header) {
                    Some((_, v)) => v.push(name),
                    None => lot_header_to_dir.push((lot_header.clone(), vec![name])),
                }
            }
        }
        let mut has_conflicts = false;
        let keys: Vec<&str> = lot_header_to_dir.iter().map(|(k, _)| k.as_str()).collect();
        for k in java::hash_map_order(&keys) {
            let (lot_header, map_names) = &lot_header_to_dir[k];
            if map_names.len() > 1 {
                for (i, name_i) in map_names.iter().enumerate() {
                    let Some(map_dir) = self.group_directory_by_name(g, name_i) else {
                        return Err("java.lang.NullPointerException".into());
                    };
                    for (j, name_j) in map_names.iter().enumerate() {
                        if i != j {
                            let conflict = translator.get_text(
                                "UI_MapConflict",
                                &[
                                    FormatArg::Str(self.real_directories[map_dir].name.clone()),
                                    FormatArg::Str(name_j.clone()),
                                    FormatArg::Str(lot_header.clone()),
                                ],
                            )?;
                            self.real_directories[map_dir].conflicts.push(conflict);
                            has_conflicts = true;
                        }
                    }
                }
            }
        }
        Ok(has_conflicts)
    }
}
