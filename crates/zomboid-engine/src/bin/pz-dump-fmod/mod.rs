//! The FMOD sound bank dumps of `pz-dump` (cases `fmodbanks`, `fmodstrings`, `fmodevents`, `fmodsamples`): the Rust side
//! of `FmodBanksProbe` in `tests/original_probe`, which asks the game's own FMOD (natives of this OS) what it loaded
//! from `media/sound/banks/Desktop`.
use std::fmt::Write as _;
use std::path::Path;

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

/// Dispatch of the `fmod*` cases: `mode` is the probe's second argument.
pub fn dump(root: &Path, mode: &str) -> Result<String, String> {
    match mode {
        "banks" => banks(root),
        _ => Err(format!("fmod: unknown mode {mode}")),
    }
}
