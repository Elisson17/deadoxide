//! The clips of the human skeleton as far as the animator's timing goes (each clip's name and duration, built with
//! `AnimationClip`): `media/anims_X/<animationDirectory>/**.X`,
//! each file read the way `ImportedSkeleton.processAnimation` reads an Assimp `aiAnimation`.
//!
//! The keyframes (bone poses) are not read: they only feed the skeleton's pose and the deferred (root) movement, which
//! this port does not compute (UNKNOWN: bone transforms and deferred movement). Nothing the animator's state, its
//! variables or the random draws depend on is in them.
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use super::animation_clip::AnimationClip;
use crate::zombie::zomboid_file_system::ZomboidFileSystem;

/// One `AnimationSet` of an `.X` file as Assimp's X importer hands it to `ImportedSkeleton.processAnimation`:
/// `aiAnimation.getName()`, `getDuration()` (ticks) and `getTicksPerSecond()`.
#[derive(Clone, Debug, PartialEq)]
pub struct XAnimation {
    pub name: String,
    pub duration_ticks: f64,
    pub ticks_per_second: f64,
}

#[derive(Debug, PartialEq)]
enum Tok {
    Open,
    Close,
    Word(String),
}

/// A DirectX text `.X` file's tokens: braces and words; `;` and `,` separate; `//` and `#` start a comment; a quoted
/// string is one word (as Assimp's XFileParser reads them).
fn tokenize(text: &str) -> Vec<Tok> {
    let b = text.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        match c {
            b' ' | b'\t' | b'\r' | b'\n' | b';' | b',' => i += 1,
            b'#' => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'{' => {
                out.push(Tok::Open);
                i += 1;
            }
            b'}' => {
                out.push(Tok::Close);
                i += 1;
            }
            b'"' => {
                let start = i + 1;
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += 1;
                }
                out.push(Tok::Word(
                    String::from_utf8_lossy(&b[start..i.min(b.len())]).into_owned(),
                ));
                i += 1;
            }
            _ => {
                let start = i;
                while i < b.len()
                    && !matches!(
                        b[i],
                        b' ' | b'\t' | b'\r' | b'\n' | b';' | b',' | b'{' | b'}'
                    )
                {
                    i += 1;
                }
                out.push(Tok::Word(
                    String::from_utf8_lossy(&b[start..i]).into_owned(),
                ));
            }
        }
    }
    out
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn skip_block(&mut self) -> Result<(), String> {
        // positioned after `{`
        let mut depth = 1usize;
        while depth > 0 {
            match self.toks.get(self.pos) {
                Some(Tok::Open) => depth += 1,
                Some(Tok::Close) => depth -= 1,
                Some(Tok::Word(_)) => {}
                None => return Err("X file: unexpected end of file in a block".into()),
            }
            self.pos += 1;
        }
        Ok(())
    }

    /// `kind [name] {` at the current position: the kind, the name (empty when none) and the position after `{`.
    fn head(&mut self) -> Result<Option<(String, String)>, String> {
        let Some(Tok::Word(kind)) = self.toks.get(self.pos) else {
            return match self.toks.get(self.pos) {
                None => Ok(None),
                Some(_) => Err("X file: a data object does not start with its type".into()),
            };
        };
        let kind = kind.clone();
        self.pos += 1;
        let mut name = String::new();
        if let Some(Tok::Word(n)) = self.toks.get(self.pos) {
            name = n.clone();
            self.pos += 1;
        }
        match self.toks.get(self.pos) {
            Some(Tok::Open) => {
                self.pos += 1;
                Ok(Some((kind, name)))
            }
            _ => Err(format!("X file: no '{{' after {kind} {name}")),
        }
    }

    fn number(&mut self) -> Result<f64, String> {
        match self.toks.get(self.pos) {
            Some(Tok::Word(w)) => {
                self.pos += 1;
                w.parse::<f64>()
                    .map_err(|_| format!("X file: {w:?} is not a number"))
            }
            _ => Err("X file: a number was expected".into()),
        }
    }

    /// An `AnimationKey` body (after `{`): `type; count; (time; n; values...)*`. Returns the key type and the last key's
    /// time (`None` for no key).
    fn animation_key(&mut self) -> Result<(i64, Option<f64>), String> {
        let key_type = self.number()? as i64;
        let count = self.number()? as usize;
        let mut last = None;
        for _ in 0..count {
            let time = self.number()?;
            let n = self.number()? as usize;
            for _ in 0..n {
                self.number()?;
            }
            last = Some(time);
        }
        match self.toks.get(self.pos) {
            Some(Tok::Close) => self.pos += 1,
            _ => return Err("X file: AnimationKey is not closed".into()),
        }
        Ok((key_type, last))
    }

    /// An `Animation` body (after `{`): the longest-lasting key sequence of the bone (XFileImporter::CreateAnimations:
    /// the matrix keys when there are any, else the rotation, scale and position keys).
    fn animation(&mut self) -> Result<f64, String> {
        let mut trafo: Option<f64> = None;
        let mut separate: Option<f64> = None;
        loop {
            match self.toks.get(self.pos) {
                Some(Tok::Close) => {
                    self.pos += 1;
                    break;
                }
                Some(Tok::Open) => {
                    // a reference to the animated frame: `{ Name }`
                    self.pos += 1;
                    self.skip_block()?;
                }
                Some(Tok::Word(_)) => {
                    let (kind, _) = self.head()?.ok_or("X file: unexpected end of file")?;
                    if kind == "AnimationKey" {
                        let (t, last) = self.animation_key()?;
                        if let Some(l) = last {
                            match t {
                                3 | 4 => trafo = Some(trafo.map_or(l, |x: f64| x.max(l))),
                                _ => separate = Some(separate.map_or(l, |x: f64| x.max(l))),
                            }
                        }
                    } else {
                        self.skip_block()?;
                    }
                }
                None => return Err("X file: unexpected end of file in Animation".into()),
            }
        }
        Ok(trafo.or(separate).unwrap_or(0.0))
    }
}

/// The animations of an `.X` file as Assimp's XFileImporter creates them (`aiAnimation`: name, duration = the last
/// key time over the file's animated bones, ticks per second from `AnimTicksPerSecond`).
pub fn read_x_animations(text: &str) -> Result<Vec<XAnimation>, String> {
    let body = text
        .get(16..)
        .ok_or("X file: shorter than its 16 byte header")?;
    if !text.starts_with("xof ") {
        return Err("X file: no xof header".into());
    }
    let mut p = Parser {
        toks: tokenize(body),
        pos: 0,
    };
    let mut ticks_per_second = 0.0f64;
    let mut sets: Vec<(String, f64)> = Vec::new();
    while let Some((kind, name)) = p.head()? {
        match kind.as_str() {
            "AnimTicksPerSecond" => {
                ticks_per_second = p.number()?;
                match p.toks.get(p.pos) {
                    Some(Tok::Close) => p.pos += 1,
                    _ => return Err("X file: AnimTicksPerSecond is not closed".into()),
                }
            }
            "AnimationSet" => {
                let mut duration = 0.0f64;
                loop {
                    match p.toks.get(p.pos) {
                        Some(Tok::Close) => {
                            p.pos += 1;
                            break;
                        }
                        Some(Tok::Word(_)) => {
                            let (k, _) = p.head()?.ok_or("X file: unexpected end of file")?;
                            if k == "Animation" {
                                duration = duration.max(p.animation()?);
                            } else {
                                p.skip_block()?;
                            }
                        }
                        _ => return Err("X file: unexpected token in AnimationSet".into()),
                    }
                }
                sets.push((name, duration));
            }
            _ => p.skip_block()?,
        }
    }
    Ok(sets
        .into_iter()
        .map(|(name, duration_ticks)| XAnimation {
            name,
            duration_ticks,
            ticks_per_second,
        })
        .collect())
}

/// ImportedSkeleton.processAnimation: the clip's name (the Assimp name after a '|', trimmed) and its duration
/// `(float) duration / (float) ticksPerSecond`.
pub fn clip_of(a: &XAnimation) -> AnimationClip {
    let mut name = a.name.as_str();
    if let Some(p) = name.find('|') {
        if p > 0 {
            name = &name[p + 1..];
        }
    }
    let duration = a.duration_ticks as f32;
    AnimationClip::new(
        duration / (a.ticks_per_second as f32),
        &[],
        crate::java::trim(name),
        false,
    )
}

/// The `animationClips` of the human skeleton's `SkinningData` (ModelManager: `animationsMesh Human` of
/// `media/scripts/generated/animations_meshes.txt` names the directories `Bob`, `Kate` and `Zombie`; each file's
/// animations are put in by name, a later one replacing an earlier of the same name).
#[derive(Clone, Debug, Default)]
pub struct AnimationClips {
    pub clips: HashMap<String, Rc<AnimationClip>>,
}

impl AnimationClips {
    /// `skinningData.animationClips.get(name)`
    pub fn get(&self, name: &str) -> Option<&Rc<AnimationClip>> {
        self.clips.get(name)
    }

    /// The clips of `animationsMesh <mesh>`'s `animationDirectory` lines. Files are read in path order: the original
    /// loads them asynchronously, so which of two files with the same animation name wins is UNKNOWN (the game's
    /// files have none: `duplicates` lists them).
    pub fn load(
        fs: &ZomboidFileSystem,
        mesh: &str,
    ) -> Result<(AnimationClips, Vec<String>), String> {
        let script =
            std::fs::read_to_string(fs.get_media_file("scripts/generated/animations_meshes.txt"))
                .map_err(|e| format!("animations_meshes.txt: {e}"))?;
        let dirs = animation_directories(&script, mesh)?;
        let mut clips = AnimationClips::default();
        let mut duplicates = Vec::new();
        for dir in dirs {
            let root = fs.get_media_file(&format!("anims_X/{dir}"));
            let mut files = Vec::new();
            walk(&root, &mut files)?;
            files.sort();
            for f in files {
                let text = std::fs::read(&f).map_err(|e| format!("{}: {e}", f.display()))?;
                let text = String::from_utf8_lossy(&text).into_owned();
                for a in read_x_animations(&text).map_err(|e| format!("{}: {e}", f.display()))? {
                    let clip = clip_of(&a);
                    if clips.clips.contains_key(&clip.name) {
                        duplicates.push(clip.name.clone());
                    }
                    clips.clips.insert(clip.name.clone(), Rc::new(clip));
                }
            }
        }
        Ok((clips, duplicates))
    }
}

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    let rd = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for e in rd {
        let p = e.map_err(|e| e.to_string())?.path();
        if p.is_dir() {
            walk(&p, out)?;
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("x")) {
            out.push(p);
        }
    }
    Ok(())
}

/// The `animationDirectory = <dir>,` lines of `animationsMesh <mesh> { ... }` in a script (`AnimationsMesh.Load`).
fn animation_directories(script: &str, mesh: &str) -> Result<Vec<String>, String> {
    let mut dirs = Vec::new();
    let mut inside = false;
    let mut found = false;
    for line in script.lines() {
        let l = line.trim();
        if !inside {
            if l == format!("animationsMesh {mesh}") {
                inside = true;
                found = true;
            }
            continue;
        }
        if l == "}" {
            break;
        }
        if let Some(rest) = l.strip_prefix("animationDirectory") {
            let v = rest
                .trim_start()
                .trim_start_matches('=')
                .trim()
                .trim_end_matches(',');
            dirs.push(v.trim().to_string());
        }
    }
    if !found {
        return Err(format!("animations_meshes.txt: no animationsMesh {mesh}"));
    }
    Ok(dirs)
}
