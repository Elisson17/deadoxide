//! The RIFF-style container of an FMOD bank file (form type `FEV `). Chunks are `tag[4] size:u32le body`, padded to an
//! even length; a `LIST` chunk's body starts with a 4-byte list type followed by chunks.

/// One chunk header found at `offset` (absolute, in the file).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub tag: [u8; 4],
    /// the list type of a `LIST` chunk
    pub list: Option<[u8; 4]>,
    /// where the 8-byte chunk header starts
    pub offset: u64,
    /// the size field: for a `LIST` it counts the 4-byte list type
    pub size: u32,
}

impl Chunk {
    /// Where the first byte after the header (and after a list's type) is.
    pub fn body_start(&self) -> u64 {
        self.offset + 8 + if self.list.is_some() { 4 } else { 0 }
    }

    /// The number of body bytes (without a list's type).
    pub fn body_len(&self) -> u32 {
        self.size - if self.list.is_some() { 4 } else { 0 }
    }

    /// Where the next chunk starts (chunks are padded to an even size).
    pub fn next(&self) -> u64 {
        self.offset + 8 + u64::from(self.size) + u64::from(self.size & 1)
    }

    /// The tag as text; a `LIST` shows as `LIST/<type>`.
    pub fn name(&self) -> String {
        match self.list {
            Some(t) => format!("LIST/{}", String::from_utf8_lossy(&t)),
            None => String::from_utf8_lossy(&self.tag).into_owned(),
        }
    }
}

pub fn u32le(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| format!("fmod bank: read past the end at {at}"))
}

/// The chunks of `data[start..end]`, where `data[0]` is file offset `base`.
pub fn parse_chunks(
    data: &[u8],
    base: u64,
    start: usize,
    end: usize,
) -> Result<Vec<Chunk>, String> {
    let mut out = Vec::new();
    let mut at = start;
    while at + 8 <= end {
        let tag = [data[at], data[at + 1], data[at + 2], data[at + 3]];
        let size = u32le(data, at + 4)?;
        let list = if &tag == b"LIST" {
            let t = data
                .get(at + 8..at + 12)
                .ok_or("fmod bank: list type past the end")?;
            Some([t[0], t[1], t[2], t[3]])
        } else {
            None
        };
        let c = Chunk {
            tag,
            list,
            offset: base + at as u64,
            size,
        };
        if at + 8 + size as usize > end {
            return Err(format!(
                "fmod bank: chunk {} at {} (size {}) runs past its parent ({})",
                c.name(),
                c.offset,
                size,
                base + end as u64
            ));
        }
        at = (c.next() - base) as usize;
        out.push(c);
    }
    Ok(out)
}
