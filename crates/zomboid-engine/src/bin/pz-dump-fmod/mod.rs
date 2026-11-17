//! The FMOD sound bank dumps of `pz-dump` (cases `fmodbanks`, `fmodstrings`, `fmodevents`, `fmodsamples`): the Rust side
//! of `FmodBanksProbe` in `tests/original_probe`, which asks the game's own FMOD (natives of this OS) what it loaded
//! from `media/sound/banks/Desktop`.
use std::fmt::Write as _;
use std::path::Path;

use zomboid_engine::fmod::fmod::fmod_manager::FmodManager;
use zomboid_engine::native::fmod::bank::Bank;
use zomboid_engine::native::fmod::studio::{StudioSystem, BANK_DIR};

fn hex(b: &[u8]) -> String {
    b.iter().fold(String::new(), |mut s, x| {
        let _ = write!(s, "{x:02x}");
        s
    })
}

/// `FmodBanksProbe banks`: the Studio bank list as `FMODManager.init` builds it, `radio.bank` loaded after it, and the
/// RIFF container of every bank file.
fn banks(root: &Path) -> Result<String, String> {
    let mut sys = StudioSystem::init_banks(root)?;
    let mut o = String::new();
    let events: u32 = (0..sys.bank_count()).map(|i| sys.bank_event_count(i)).sum();
    let _ = writeln!(o, "FMOD init banks={} events={events}", sys.bank_count());
    let line = |sys: &StudioSystem, i: usize| {
        format!(
            "BANK {i} {} events={} vcas={}",
            sys.banks[i].name,
            sys.bank_event_count(i),
            sys.bank_vca_count(i)
        )
    };
    for i in 0..sys.bank_count() {
        let _ = writeln!(o, "{}", line(&sys, i));
    }
    sys.load_bank_file(&root.join(BANK_DIR).join("radio.bank"))?;
    let _ = writeln!(o, "FMOD radio banks={}", sys.bank_count());
    let _ = writeln!(o, "{}", line(&sys, sys.bank_count() - 1));
    for b in &sys.banks {
        container(&mut o, b);
    }
    Ok(o)
}

/// The container lines of one bank: `FILE`, `BNKI`, one `LIST` per list of `PROJ`, the `TAIL` chunks and `SND`.
fn container(o: &mut String, b: &Bank) {
    let _ = writeln!(
        o,
        "FILE {} bytes={} riff={} form={} fmt={},{}",
        b.name,
        b.len,
        b.riff_size,
        String::from_utf8_lossy(&b.form),
        b.fmt.0,
        b.fmt.1
    );
    let _ = writeln!(
        o,
        "BNKI {} guid={} zeros={:x} events={} tail={}",
        b.name,
        hex(&b.bnki.guid),
        b.bnki.zeros,
        b.bnki.events,
        b.bnki.tail
    );
    for l in &b.lists {
        let kids: Vec<String> = l.children.iter().map(|(n, c)| format!("{n}:{c}")).collect();
        let _ = writeln!(
            o,
            "LIST {} {} size={} lcnt={} children={}",
            b.name,
            String::from_utf8_lossy(&l.chunk.list.unwrap_or(*b"????")),
            l.chunk.size,
            l.lcnt.map_or("-".to_string(), |n| n.to_string()),
            if kids.is_empty() {
                "-".to_string()
            } else {
                kids.join(",")
            }
        );
    }
    for c in &b.tail {
        let _ = writeln!(
            o,
            "TAIL {} {} size={}",
            b.name,
            String::from_utf8_lossy(&c.tag),
            c.size
        );
    }
    if let Some(s) = &b.snd {
        let _ = writeln!(o, "SND {} offset={} size={}", b.name, s.offset, s.size);
    }
}

/// `FmodBanksProbe strings <candidates>`: the events and VCAs of every bank (paths from the string table), the
/// event path list of `FMODManager`, the parameter names, and the bus/VCA queries of the candidate file.
fn strings(root: &Path, candidates: &Path) -> Result<String, String> {
    let m = FmodManager::init(root)?;
    let sys = &m.system;
    let mut o = String::new();
    for i in 0..sys.bank_count() {
        for (j, (g, path)) in sys.bank_event_list(i)?.iter().enumerate() {
            let _ = writeln!(o, "BEV {i} {j} {path} {}", hex(g));
        }
        for (j, path) in sys.bank_vca_list(i)?.iter().enumerate() {
            let _ = writeln!(o, "VCA {i} {j} {path}");
        }
    }
    let _ = writeln!(o, "EVENTS {}", m.event_path_list.len());
    for (i, p) in m.event_path_list.iter().enumerate() {
        let _ = writeln!(o, "EVT {i} {p}");
    }
    let names = parameter_names(sys.strings().ok_or("no string table")?)?;
    let _ = writeln!(o, "PARAMS {}", names.len());
    for n in &names {
        let _ = writeln!(o, "PARAM {n}");
    }
    let text = std::fs::read_to_string(candidates)
        .map_err(|e| format!("{}: {e}", candidates.display()))?;
    for c in text.lines() {
        let _ = writeln!(
            o,
            "Q {} {} {c}",
            u8::from(sys.get_bus(c)),
            u8::from(sys.get_vca(c))
        );
    }
    Ok(o)
}

/// `FmodBanksProbe events`: the global parameter list, every event of the path list with its sustain flag, length and
/// parameters (as the record of the event gives them: id and flags; `gi` is the manager's `globalIndex` of the name),
/// then the manager's parameter table. The id of a parameter no event uses is UNKNOWN (a hash the banks do not hold).
fn events(root: &Path) -> Result<String, String> {
    let m = FmodManager::init(root)?;
    let globals = m.system.parameter_description_list()?;
    let mut o = String::new();
    let _ = writeln!(o, "GLOBALS {}", globals.len());
    for (i, p) in globals.iter().enumerate() {
        let _ = writeln!(o, "G {i} {} {}", p.name, p.flags());
    }
    let _ = writeln!(o, "EVENTS {}", m.event_path_list.len());
    for (i, path) in m.event_path_list.iter().enumerate() {
        let d = m
            .get_event_description(path)
            .ok_or_else(|| format!("{path}: no description"))?;
        let _ = writeln!(
            o,
            "EVENT {i} {path} {} {} {} {}",
            hex(&d.id),
            u8::from(d.has_sustain_points),
            d.length,
            d.parameter_records.len()
        );
        for (ix, r) in d.parameters.iter().zip(&d.parameter_records) {
            let _ = writeln!(
                o,
                "EP {} {} {} {}",
                m.parameters[*ix].global_index,
                r.name,
                hex(&r.id),
                r.flags
            );
        }
    }
    let _ = writeln!(o, "PARAMS {}", m.parameters.len());
    for p in &m.parameters {
        let _ = writeln!(
            o,
            "PD {} {} {} {}",
            p.global_index,
            p.name,
            p.flags,
            p.id.map_or("UNKNOWN".to_string(), |i| hex(&i))
        );
    }
    Ok(o)
}

/// The names of the `parameter:/` strings (a parameter path is `parameter:/<folders>/<name>`; the game and the API use
/// the last component), in Java `String` order.
fn parameter_names(
    st: &zomboid_engine::native::fmod::strings::StringTable,
) -> Result<Vec<String>, String> {
    let mut names: Vec<String> = st
        .entries()?
        .into_iter()
        .filter_map(|(p, _)| {
            p.strip_prefix("parameter:/")
                .map(|r| r.rsplit('/').next().unwrap_or(r).to_string())
        })
        .collect();
    names.sort_by(|a, b| zomboid_engine::java::string_compare_to(a, b));
    Ok(names)
}

/// `pz-dump fmodcandidates`: the paths `FmodBanksProbe strings` asks `GetBus`/`GetVCA` about, from the string table:
/// every string once, and for each `bus:/` and `vca:/` string also its lower case, its upper case, one character more
/// and one character less (case-insensitivity and no partial matches), then a few paths that name nothing.
pub fn candidates(root: &Path) -> Result<String, String> {
    let sys = StudioSystem::init_banks(root)?;
    let st = sys.strings().ok_or("no string table")?;
    let mut all: Vec<String> = st.entries()?.into_iter().map(|(p, _)| p).collect();
    all.sort_by(|a, b| zomboid_engine::java::string_compare_to(a, b));
    let mut o = String::new();
    for p in &all {
        let _ = writeln!(o, "{p}");
        if p.starts_with("bus:/") || p.starts_with("vca:/") {
            let _ = writeln!(o, "{}", p.to_ascii_lowercase());
            let _ = writeln!(o, "{}", p.to_ascii_uppercase());
            let _ = writeln!(o, "{p}_");
            let _ = writeln!(o, "{}", &p[..p.len() - 1]);
        }
    }
    for p in [
        "bus:",
        "bus:/Nope",
        "vca:/Nope",
        "bus:/Mix_Deaf",
        "vca:/Game",
    ] {
        let _ = writeln!(o, "{p}");
    }
    Ok(o)
}

/// Dispatch of the `fmod*` cases: `mode` is the probe's second argument, `rest` the arguments after it.
pub fn dump(root: &Path, mode: &str, rest: &[String]) -> Result<String, String> {
    match (mode, rest) {
        ("banks", []) => banks(root),
        ("strings", [cand]) => strings(root, Path::new(cand)),
        ("events", []) => events(root),
        _ => Err(format!("fmod: unknown mode {mode}")),
    }
}
