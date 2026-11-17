//! An FMOD Studio bank file: the RIFF `FEV ` container with the `FMT ` header, `LIST PROJ` (the metadata: one list per
//! kind of object, then the string/hash tables) and, in the data banks, the `SND ` chunk with the samples.
use std::path::Path;

use super::riff::{parse_chunks, u32le, Chunk};
use super::strings::StringTable;
use crate::vfs;

/// What `BNKI` holds: the bank's GUID, then 8 bytes of zeros, then a u32 that equals the number of events the Studio
/// API reports for the bank (`FMOD_Studio_Bank_GetEventCount`: 3215, 272, 7, 0 for the game's banks and 10 for
/// `radio.bank`), then a u32 7 in every bank (UNKNOWN meaning).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bnki {
    pub guid: [u8; 16],
    pub zeros: u64,
    pub events: u32,
    pub tail: u32,
}

/// One list of `LIST PROJ` (`IBSS`, `EVTS`, ...): its chunk and what it holds.
#[derive(Clone, Debug)]
pub struct ProjList {
    pub chunk: Chunk,
    /// the value of the leading `LCNT` chunk, the entry count the list declares
    pub lcnt: Option<u32>,
    /// the children by name (`LIST/EVNT`, `LCNT`, `WAV `, ...) with their counts, in order of first appearance
    pub children: Vec<(String, usize)>,
}

#[derive(Clone, Debug)]
pub struct Bank {
    pub name: String,
    pub len: u64,
    pub riff_size: u32,
    pub form: [u8; 4],
    /// the two u32 of `FMT ` (0x92 and 0x92 in the 2.03.09 banks; the meaning of each is UNKNOWN)
    pub fmt: (u32, u32),
    pub bnki: Bnki,
    /// `LIST PROJ` as a chunk, its body (everything after the list type) and the file offset it starts at
    pub proj: Chunk,
    pub proj_data: Vec<u8>,
    /// the lists directly inside `PROJ`, in file order
    pub lists: Vec<ProjList>,
    /// the other chunks directly inside `PROJ` (`SNDH`, `STDT`, `STBL`, `HASH`, `DEL `, `MUTE`, `REFI`, `PLAT`)
    pub tail: Vec<Chunk>,
    /// the top-level `SND ` chunk, the sample data (never read whole)
    pub snd: Option<Chunk>,
    /// the `STDT` string table, when the bank carries one (only `ZomboidSound.strings.bank`)
    pub strings: Option<StringTable>,
}

impl Bank {
    /// Reads the container of `path`: the head up to the end of `PROJ` is read, the sample data is only located.
    pub fn open(path: &Path) -> Result<Bank, String> {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let len = vfs::len(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let rd = |off: u64, n: usize| -> Result<Vec<u8>, String> {
            vfs::read_range(path, off, n).map_err(|e| format!("{}: {e}", path.display()))
        };
        let head = rd(0, 12)?;
        if head.len() < 12 || &head[0..4] != b"RIFF" {
            return Err(format!("{}: not a RIFF file", path.display()));
        }
        let riff_size = u32le(&head, 4)?;
        let form = [head[8], head[9], head[10], head[11]];
        if &form != b"FEV " {
            return Err(format!("{}: RIFF form is not FEV", path.display()));
        }
        let end = (8 + u64::from(riff_size)).min(len);
        let mut fmt = None;
        let mut proj = None;
        let mut proj_data = Vec::new();
        let mut snd = None;
        let mut at = 12u64;
        while at + 8 <= end {
            let h = rd(at, 12)?;
            let tag = [h[0], h[1], h[2], h[3]];
            let size = u32le(&h, 4)?;
            let list = (&tag == b"LIST").then(|| [h[8], h[9], h[10], h[11]]);
            let c = Chunk {
                tag,
                list,
                offset: at,
                size,
            };
            match (&tag, list) {
                (b"FMT ", None) => {
                    let b = rd(c.body_start(), c.body_len() as usize)?;
                    fmt = Some((u32le(&b, 0)?, u32le(&b, 4)?));
                }
                (b"LIST", Some(t)) if &t == b"PROJ" => {
                    proj_data = rd(c.body_start(), c.body_len() as usize)?;
                    proj = Some(c);
                }
                (b"SND ", None) => snd = Some(c),
                _ => {}
            }
            at = c.next();
        }
        let proj = proj.ok_or_else(|| format!("{}: no LIST PROJ", path.display()))?;
        let (fmt_a, fmt_b) = fmt.ok_or_else(|| format!("{}: no FMT chunk", path.display()))?;
        let base = proj.body_start();
        let inner = parse_chunks(&proj_data, base, 0, proj_data.len())?;
        let mut bnki = None;
        let mut lists = Vec::new();
        let mut tail = Vec::new();
        for c in inner {
            if &c.tag == b"BNKI" {
                let b = &proj_data[(c.body_start() - base) as usize..][..c.body_len() as usize];
                let mut guid = [0u8; 16];
                guid.copy_from_slice(b.get(..16).ok_or("BNKI shorter than a GUID")?);
                bnki = Some(Bnki {
                    guid,
                    zeros: u64::from(u32le(b, 16)?) | u64::from(u32le(b, 20)?) << 32,
                    events: u32le(b, 24)?,
                    tail: u32le(b, 28)?,
                });
            } else if c.list.is_some() {
                let s = (c.body_start() - base) as usize;
                let kids = parse_chunks(&proj_data, base, s, s + c.body_len() as usize)?;
                let mut children: Vec<(String, usize)> = Vec::new();
                for k in &kids {
                    let n = k.name();
                    match children.iter_mut().find(|(x, _)| *x == n) {
                        Some(e) => e.1 += 1,
                        None => children.push((n, 1)),
                    }
                }
                let lcnt = match kids.first() {
                    Some(k) if &k.tag == b"LCNT" => {
                        Some(u32le(&proj_data, (k.body_start() - base) as usize)?)
                    }
                    _ => None,
                };
                lists.push(ProjList {
                    chunk: c,
                    lcnt,
                    children,
                });
            } else {
                tail.push(c);
            }
        }
        let mut strings = None;
        for c in &tail {
            if &c.tag == b"STDT" && c.size > 0 {
                let b = &proj_data[(c.body_start() - base) as usize..][..c.body_len() as usize];
                strings = Some(StringTable::parse(b)?);
            }
        }
        Ok(Bank {
            name,
            len,
            riff_size,
            form,
            fmt: (fmt_a, fmt_b),
            bnki: bnki.ok_or_else(|| format!("{}: no BNKI chunk", path.display()))?,
            proj,
            proj_data,
            lists,
            tail,
            snd,
            strings,
        })
    }

    /// The list of `PROJ` with the type `tag` (`EVTS`, `VCAS`, ...).
    pub fn list(&self, tag: &[u8; 4]) -> Option<&ProjList> {
        self.lists
            .iter()
            .find(|l| l.chunk.list.as_ref() == Some(tag))
    }

    /// The body of a chunk directly inside `PROJ` or of a list, read from the head.
    pub fn body(&self, c: &Chunk) -> &[u8] {
        let base = self.proj.body_start();
        &self.proj_data[(c.body_start() - base) as usize..][..c.body_len() as usize]
    }

    /// The entries of a `PROJ` list (its children after the leading `LCNT`), as chunks.
    pub fn entries(&self, tag: &[u8; 4]) -> Result<Vec<Chunk>, String> {
        let Some(l) = self.list(tag) else {
            return Ok(Vec::new());
        };
        let base = self.proj.body_start();
        let s = (l.chunk.body_start() - base) as usize;
        let kids = parse_chunks(&self.proj_data, base, s, s + l.chunk.body_len() as usize)?;
        Ok(kids.into_iter().filter(|k| &k.tag != b"LCNT").collect())
    }

    /// The first 16 bytes of the first chunk named `first` inside the list entry `e`: the GUID the entry starts with
    /// (`EVNT` starts with an `EVTB`, `VCA ` with a `VCAB`).
    pub fn entry_guid(&self, e: &Chunk, first: &[u8; 4]) -> Result<[u8; 16], String> {
        let base = self.proj.body_start();
        let s = (e.body_start() - base) as usize;
        let kids = parse_chunks(&self.proj_data, base, s, s + e.body_len() as usize)?;
        let k = kids.iter().find(|k| &k.tag == first).ok_or_else(|| {
            format!(
                "{}: entry without {}",
                self.name,
                String::from_utf8_lossy(first)
            )
        })?;
        let b = &self.proj_data[(k.body_start() - base) as usize..][..k.body_len() as usize];
        let mut g = [0u8; 16];
        g.copy_from_slice(b.get(..16).ok_or("entry shorter than a GUID")?);
        Ok(g)
    }

    /// The GUIDs of the `EVNT` entries of `EVTS`, in file order (sorted by GUID).
    pub fn event_guids(&self) -> Result<Vec<[u8; 16]>, String> {
        self.entries(b"EVTS")?
            .iter()
            .map(|e| self.entry_guid(e, b"EVTB"))
            .collect()
    }

    /// The GUIDs of the `VCA ` entries of `VCAS`, in file order.
    pub fn vca_guids(&self) -> Result<Vec<[u8; 16]>, String> {
        self.entries(b"VCAS")?
            .iter()
            .map(|e| self.entry_guid(e, b"VCAB"))
            .collect()
    }
}
