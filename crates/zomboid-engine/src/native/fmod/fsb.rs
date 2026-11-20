//! The sample data of a data bank: the `SND ` chunk holds an FSB5 file (FMOD sample bank), preceded by zero padding. Only
//! the head is ever read (the file header, the sample headers and the name table; the sample data of the game's banks is
//! 1.3 GB): `pz-dump fmodsamples` against `FmodBanksProbe samples` checks the stream count and every stream's
//! frequency, channels and length against the game's own FMOD core, which opens the same head. The Vorbis decoder is
//! not ported: see `Fsb5::vorbis_crcs`.
use std::path::Path;

use super::bank::Bank;
use super::riff::u32le;
use crate::vfs;

/// `FMOD_SOUND_TYPE_FSB` and the decoded format FMOD reports for a stream of codec 15 (Vorbis): `FMOD_SOUND_FORMAT_PCMFLOAT`
/// (5), 32 bits. Measured for every stream of the four banks; the other codecs of FSB5 are not in the banks (UNKNOWN).
pub const SOUND_TYPE_FSB: i32 = 20;
pub const SOUND_FORMAT_PCMFLOAT: i32 = 5;
pub const CODEC_VORBIS: u32 = 15;

/// The frequencies of the 4-bit frequency field of a sample header (0 and 11..15 are not used by the game's banks).
const FREQUENCIES: [u32; 11] = [
    0, 8000, 11000, 11025, 16000, 22050, 24000, 32000, 44100, 48000, 96000,
];

/// One stream of the FSB5: what a sample header (and its chunks) says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fsb5Stream {
    pub name: String,
    pub frequency: u32,
    pub channels: u32,
    /// where the stream's data starts, from the start of the data section
    pub data_offset: u64,
    /// the length in PCM samples
    pub samples: u32,
    /// chunk types of the sample header in order (2 frequency, 11 vorbis, 13 and 14: see `channels`; the banks hold 4, 11, 13, 14)
    pub chunk_types: Vec<u32>,
    /// the CRC-32 of the Vorbis setup header the stream uses (first u32 of its chunk 11): FMOD keeps the setup headers
    /// itself and looks them up by this value, which is why a Vorbis stream cannot be decoded from the bank alone
    pub vorbis_crc: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct Fsb5 {
    /// where `FSB5` starts in the bank file
    pub offset: u64,
    pub version: u32,
    pub num_samples: u32,
    pub sample_headers_size: u32,
    pub name_table_size: u32,
    pub data_size: u32,
    /// the codec (15 is Vorbis)
    pub mode: u32,
    pub streams: Vec<Fsb5Stream>,
}

impl Fsb5 {
    /// Reads the FSB5 of `bank` (its `SND ` chunk), the head only.
    pub fn open(bank: &Bank) -> Result<Option<Fsb5>, String> {
        let Some(snd) = bank.snd else {
            return Ok(None);
        };
        Self::read(&bank.path, snd.body_start(), u64::from(snd.size))
    }

    /// `FSB5` is found in the first 256 bytes of the chunk body (after the zero padding).
    fn read(path: &Path, body: u64, size: u64) -> Result<Option<Fsb5>, String> {
        let rd = |off: u64, n: usize| -> Result<Vec<u8>, String> {
            vfs::read_range(path, off, n).map_err(|e| format!("{}: {e}", path.display()))
        };
        let probe = rd(body, 256.min(size as usize))?;
        let Some(at) = probe.windows(4).position(|w| w == b"FSB5") else {
            return Ok(None);
        };
        let offset = body + at as u64;
        let h = rd(offset, 60)?;
        let version = u32le(&h, 4)?;
        if version != 1 {
            return Err(format!("fsb5: version {version} (only 1 is in the banks)"));
        }
        let num_samples = u32le(&h, 8)?;
        let sample_headers_size = u32le(&h, 12)?;
        let name_table_size = u32le(&h, 16)?;
        let data_size = u32le(&h, 20)?;
        let mode = u32le(&h, 24)?;
        let tables = rd(
            offset + 60,
            sample_headers_size as usize + name_table_size as usize,
        )?;
        if tables.len() != sample_headers_size as usize + name_table_size as usize {
            return Err("fsb5: head shorter than its tables".to_string());
        }
        let (headers, names) = tables.split_at(sample_headers_size as usize);
        let mut streams = Vec::with_capacity(num_samples as usize);
        let mut p = 0usize;
        for i in 0..num_samples as usize {
            let lo = u64::from(u32le(headers, p)?);
            let hi = u64::from(u32le(headers, p + 4)?);
            let raw = lo | hi << 32;
            p += 8;
            let mut next = raw & 1 != 0;
            let freq_ix = ((raw >> 1) & 15) as usize;
            // 2 bits: 0 is mono, 1 stereo, 2 six channels (measured against FMOD); 3 is not in the banks (UNKNOWN)
            let mut channels = match (raw >> 5) & 3 {
                0 => 1,
                1 => 2,
                2 => 6,
                c => return Err(format!("fsb5: stream {i} channel code {c} (UNKNOWN)")),
            };
            let data_offset = ((raw >> 7) & 0x07ff_ffff) << 5;
            let samples = ((raw >> 34) & 0x3fff_ffff) as u32;
            let frequency = *FREQUENCIES
                .get(freq_ix)
                .filter(|f| **f != 0)
                .ok_or_else(|| format!("fsb5: stream {i} frequency index {freq_ix}"))?;
            let mut chunk_types = Vec::new();
            let mut vorbis_crc = None;
            while next {
                let c = u32le(headers, p)?;
                p += 4;
                next = c & 1 != 0;
                let len = ((c >> 1) & 0x00ff_ffff) as usize;
                let kind = c >> 25;
                let data = headers
                    .get(p..p + len)
                    .ok_or("fsb5: chunk past the sample headers")?;
                p += len;
                match kind {
                    // chunk 14 carries the channel count of a stream with more than the 2 bits can say (measured: 89 streams
                    // of ZomboidSound, 4 channels, FMOD reports 4)
                    14 => channels = u32le(data, 0)?,
                    // a frequency chunk (type 2) is not in the banks: its effect is UNKNOWN here
                    2 => return Err(format!("fsb5: stream {i} has a frequency chunk (UNKNOWN)")),
                    11 => vorbis_crc = Some(u32le(data, 0)?),
                    _ => {}
                }
                chunk_types.push(kind);
            }
            // the name table: u32 offsets (from the start of the table), then zero-terminated names
            let name_at = u32le(names, 4 * i)? as usize;
            let tail = names
                .get(name_at..)
                .ok_or("fsb5: name offset past the table")?;
            let end = tail
                .iter()
                .position(|b| *b == 0)
                .ok_or("fsb5: unterminated name")?;
            streams.push(Fsb5Stream {
                name: String::from_utf8_lossy(&tail[..end]).into_owned(),
                frequency,
                channels,
                data_offset,
                samples,
                chunk_types,
                vorbis_crc,
            });
        }
        if p != headers.len() {
            return Err(format!(
                "fsb5: sample headers use {p} of {} bytes",
                headers.len()
            ));
        }
        Ok(Some(Fsb5 {
            offset,
            version,
            num_samples,
            sample_headers_size,
            name_table_size,
            data_size,
            mode,
            streams,
        }))
    }

    /// `FMOD_Sound_GetLength(sub, FMOD_TIMEUNIT_MS)` of stream `i`: the PCM length over the frequency, rounded down
    /// (measured: every stream of the four banks).
    pub fn length_ms(&self, i: usize) -> u64 {
        let s = &self.streams[i];
        u64::from(s.samples) * 1000 / u64::from(s.frequency)
    }

    /// The Vorbis setup CRCs the streams use, each once, in order of first use (UNKNOWN: the setup headers FMOD looks
    /// them up in are inside its library, not in the banks).
    pub fn vorbis_crcs(&self) -> Vec<u32> {
        let mut v: Vec<u32> = Vec::new();
        for s in &self.streams {
            if let Some(c) = s.vorbis_crc {
                if !v.contains(&c) {
                    v.push(c);
                }
            }
        }
        v
    }
}
