//! What the Studio API reads from the event and parameter records of a bank: the length of an event, whether it has a
//! sustain point, and its parameters (name, id, flags). Every layout below was found by comparing the records with the
//! values the original FMOD returns (`FMOD_Studio_EventDescription_GetLength`, `HasSustainPoint`,
//! `GetParameterDescriptionByIndex`, `FMOD_Studio_System_GetParameterDescriptionList`); `pz-dump fmodevents` against
//! `FmodBanksProbe events` is the check. The fields that are not read are not decoded (UNKNOWN).
use std::collections::HashMap;

use super::bank::Bank;
use super::riff::{u32le, Chunk};

/// The sample rate the timeline positions of an event are counted in (a length of `n` samples is `n * 1000 / 48000`
/// milliseconds; measured: 3215 of 3215 event lengths).
const TIMELINE_RATE: u64 = 48000;

/// Where the vector of `PMLO` GUIDs starts in an `EVTB` record: the record starts with the event GUID, 16 bytes
/// that are zeros or a parent (UNKNOWN), the GUIDs of its timeline (`TLNS`), input bus (`IBUS`) and master bus
/// (`MBUS`), and 13 bytes of numeric fields (UNKNOWN).
const EVTB_PMLO_VECTOR: usize = 93;
/// What lies between the `PMLO` vector and the vector of GUIDs that precedes the parameter ids: 20 bytes (UNKNOWN).
const EVTB_GAP: usize = 20;

fn u16le(d: &[u8], at: usize) -> Result<u16, String> {
    d.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| format!("fmod bank: read past the end at {at}"))
}

fn guid_at(d: &[u8], at: usize) -> Result<[u8; 16], String> {
    let mut g = [0u8; 16];
    g.copy_from_slice(
        d.get(at..at + 16)
            .ok_or_else(|| format!("fmod bank: GUID past the end at {at}"))?,
    );
    Ok(g)
}

/// One array of a record: a `u16` control word (the element count is `ctrl >> 1`; an empty array is that word alone),
/// then, when bit 0 is set, the size of every element as a `u16`, else each element is preceded by its own `u16` size.
/// Returns the elements and where the array ends.
fn read_array(d: &[u8], at: usize) -> Result<(Vec<&[u8]>, usize), String> {
    let ctrl = u16le(d, at)?;
    let count = usize::from(ctrl >> 1);
    let mut at = at + 2;
    let mut out = Vec::with_capacity(count);
    if count == 0 {
        return Ok((out, at));
    }
    let fixed = if ctrl & 1 == 1 {
        let n = usize::from(u16le(d, at)?);
        at += 2;
        Some(n)
    } else {
        None
    };
    for _ in 0..count {
        let n = match fixed {
            Some(n) => n,
            None => {
                let n = usize::from(u16le(d, at)?);
                at += 2;
                n
            }
        };
        out.push(
            d.get(at..at + n)
                .ok_or_else(|| format!("fmod bank: array element past the end at {at}"))?,
        );
        at += n;
    }
    Ok((out, at))
}

/// `PRMS` entry (`LIST PARM`, first chunk `PRMB`): the parameter GUID, two u32 of kind flags and the name. The
/// parameter's id the API returns is not stored here (it is a hash of something the banks do not hold: UNKNOWN); the
/// events that use the parameter carry it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParamDef {
    pub guid: [u8; 16],
    pub name: String,
    pub kind: u32,
    pub kind2: u32,
}

impl ParamDef {
    fn parse(b: &[u8]) -> Result<ParamDef, String> {
        let n = usize::from(u16le(b, 24)?);
        let name = b
            .get(26..26 + n)
            .ok_or("fmod bank: parameter name past the end")?;
        Ok(ParamDef {
            guid: guid_at(b, 0)?,
            name: String::from_utf8_lossy(name).into_owned(),
            kind: u32le(b, 16)?,
            kind2: u32le(b, 20)?,
        })
    }

    /// `FMOD_STUDIO_PARAMETER_FLAGS` the API reports: bit 0 of the kind word is global (4), bit 3 is discrete (8), bit 5
    /// is labeled (16), bit 4 is a built-in parameter (readonly and automatic, 1 and 2). The other bits (1, 2) and the
    /// second word do not show in the flags (measured on the 10 kind pairs the three banks hold).
    pub fn flags(&self) -> i32 {
        let mut f = 0;
        if self.kind & 1 != 0 {
            f |= 4;
        }
        if self.kind & 8 != 0 {
            f |= 8;
        }
        if self.kind & 32 != 0 {
            f |= 16;
        }
        if self.kind & 16 != 0 {
            f |= 3;
        }
        f
    }

    pub fn is_global(&self) -> bool {
        self.flags() & 4 != 0
    }
}

/// One parameter of an event, as `GetParameterDescriptionByIndex` returns it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventParam {
    pub name: String,
    pub flags: i32,
    /// `FMOD_STUDIO_PARAMETER_ID` (the 8 bytes `data1`, `data2`), read from the event record
    pub id: [u8; 8],
}

/// What `FMODManager.initEvent` asks an event description for besides its path and GUID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventDetails {
    /// `FMOD_Studio_EventDescription_HasSustainPoint`
    pub has_sustain_points: bool,
    /// `FMOD_Studio_EventDescription_GetLength`, milliseconds
    pub length: u64,
    /// in `GetParameterDescriptionByIndex` order
    pub parameters: Vec<EventParam>,
}

/// One timeline entry (`LIST TMLN` of `TLNS`): its `TLNB` and the transitions of its `TRNS` list that have a transition
/// timeline (`TRAN`: a `TRNB` whose second GUID is the destination marker, and a `TRTL` whose first u32 is the
/// length of that timeline in samples).
struct Timeline {
    tlnb: Chunk,
    /// (destination marker GUID, length of the transition timeline in samples)
    transitions: Vec<([u8; 16], u32)>,
}

/// The record tables of one bank, by GUID.
pub struct EventTables {
    evtb: HashMap<[u8; 16], Chunk>,
    timelines: HashMap<[u8; 16], Timeline>,
    pmlb: HashMap<[u8; 16], Chunk>,
    pub params: Vec<ParamDef>,
}

impl EventTables {
    fn index(
        bank: &Bank,
        list: &[u8; 4],
        first: &[u8; 4],
    ) -> Result<HashMap<[u8; 16], Chunk>, String> {
        let mut m = HashMap::new();
        for e in bank.entries(list)? {
            let c = bank.entry_child(&e, first)?;
            m.insert(guid_at(bank.body(&c), 0)?, c);
        }
        Ok(m)
    }

    fn timelines(bank: &Bank) -> Result<HashMap<[u8; 16], Timeline>, String> {
        let mut m = HashMap::new();
        for e in bank.entries(b"TLNS")? {
            let kids = bank.children(&e)?;
            let tlnb = *kids
                .iter()
                .find(|k| &k.tag == b"TLNB")
                .ok_or("fmod bank: timeline without TLNB")?;
            let mut transitions = Vec::new();
            for trns in kids.iter().filter(|k| k.list.as_ref() == Some(b"TRNS")) {
                for tran in bank
                    .children(trns)?
                    .iter()
                    .filter(|k| k.list.as_ref() == Some(b"TRAN"))
                {
                    let parts = bank.children(tran)?;
                    let (Some(trnb), Some(trtl)) = (
                        parts.iter().find(|k| &k.tag == b"TRNB"),
                        parts.iter().find(|k| &k.tag == b"TRTL"),
                    ) else {
                        continue;
                    };
                    if trtl.body_len() > 0 {
                        transitions
                            .push((guid_at(bank.body(trnb), 16)?, u32le(bank.body(trtl), 0)?));
                    }
                }
            }
            m.insert(
                guid_at(bank.body(&tlnb), 0)?,
                Timeline { tlnb, transitions },
            );
        }
        Ok(m)
    }

    /// Reads the `EVTS`, `TLNS`, `PMLS` and `PRMS` lists of `bank`.
    pub fn build(bank: &Bank) -> Result<EventTables, String> {
        let mut params = Vec::new();
        for e in bank.entries(b"PRMS")? {
            let c = bank.entry_child(&e, b"PRMB")?;
            params.push(ParamDef::parse(bank.body(&c))?);
        }
        Ok(EventTables {
            evtb: Self::index(bank, b"EVTS", b"EVTB")?,
            timelines: Self::timelines(bank)?,
            pmlb: Self::index(bank, b"PMLS", b"PMLB")?,
            params,
        })
    }

    /// The length (milliseconds) and the sustain flag of the timeline `tl` (`TLNB`: its GUID, five arrays: module
    /// instances and a second module array (24 bytes each: GUID, start, length in samples), sustain points (the
    /// position in samples first), markers (GUID, position, a counted name, then a length in samples that is 0 for a
    /// plain marker; a marker that is the destination of a transition with a transition timeline ends where that
    /// timeline ends: its position plus the timeline's length) and the tempo markers (not read: UNKNOWN whether they
    /// matter; patching one in a copy of the bank changed no length)). The length is the furthest end of any of them.
    /// Measured: 3494 of 3494 event lengths of the four banks, and in copies of `ZomboidMusic.bank` with a patched
    /// marker position (the length followed it, +1000 ms only for the destination marker).
    fn timeline(&self, bank: &Bank, tl: &[u8; 16]) -> Result<(bool, u64), String> {
        let t = self
            .timelines
            .get(tl)
            .ok_or("fmod bank: event without its timeline")?;
        let b = bank.body(&t.tlnb);
        let mut at = 16;
        let mut arrays = Vec::new();
        for _ in 0..5 {
            let (v, next) = read_array(b, at)?;
            arrays.push(v);
            at = next;
        }
        let mut end = 0u64;
        for modules in &arrays[..2] {
            for e in modules {
                end = end.max(u64::from(u32le(e, 16)?) + u64::from(u32le(e, 20)?));
            }
        }
        for e in &arrays[2] {
            end = end.max(u64::from(u32le(e, 0)?));
        }
        for e in &arrays[3] {
            let name = usize::from(u16le(e, 20)?);
            let pos = u64::from(u32le(e, 16)?);
            let mut marker_end = pos + u64::from(u32le(e, 22 + name)?);
            let guid = guid_at(e, 0)?;
            for (dest, len) in &t.transitions {
                if *dest == guid {
                    marker_end = marker_end.max(pos + u64::from(*len));
                }
            }
            end = end.max(marker_end);
        }
        Ok((!arrays[2].is_empty(), end * 1000 / TIMELINE_RATE))
    }

    /// The details of the event with GUID `guid`.
    pub fn details(&self, bank: &Bank, guid: &[u8; 16]) -> Result<EventDetails, String> {
        let c = self
            .evtb
            .get(guid)
            .ok_or("fmod bank: event without a record")?;
        let b = bank.body(c);
        let tl = guid_at(b, 32)?;
        let (modules, at) = read_array(b, EVTB_PMLO_VECTOR)?;
        let (_buses, at) = read_array(b, at + EVTB_GAP)?;
        let (ids, _) = read_array(b, at)?;
        if modules.len() != ids.len() {
            return Err(format!(
                "fmod bank: {} parameter links and {} parameter ids",
                modules.len(),
                ids.len()
            ));
        }
        let mut parameters = Vec::with_capacity(ids.len());
        for (link, id) in modules.iter().zip(&ids) {
            let pmlb = self
                .pmlb
                .get(&guid_at(link, 0)?)
                .ok_or("fmod bank: parameter link without a record")?;
            let pg = guid_at(bank.body(pmlb), 16)?;
            let def = self
                .params
                .iter()
                .find(|p| p.guid == pg)
                .ok_or("fmod bank: parameter link to a parameter the bank lacks")?;
            let mut raw = [0u8; 8];
            raw.copy_from_slice(id.get(..8).ok_or("fmod bank: short parameter id")?);
            parameters.push(EventParam {
                name: def.name.clone(),
                // measured: the API reports a global parameter of an event as readonly too (4 -> 5, 12 -> 13, 28 -> 29)
                flags: def.flags() | i32::from(def.is_global()),
                id: raw,
            });
        }
        let (has_sustain_points, length) = self.timeline(bank, &tl)?;
        Ok(EventDetails {
            has_sustain_points,
            length,
            parameters,
        })
    }
}
