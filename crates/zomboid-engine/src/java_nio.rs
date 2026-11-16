//! java.nio.ByteBuffer reading (big-endian, the default byte order the game never changes) and the game's small
//! framing helpers built on it: `GameWindow.ReadString`, `zombie.util.io.BitHeader`, `zombie.core.utils.ByteBlock`.
//! Every read that would pass the limit is `Err`, where Java throws BufferUnderflowException.

const UNDERFLOW: &str = "java.nio.BufferUnderflowException";

/// java.nio.ByteBuffer over a byte slice, read-only.
#[derive(Clone, Debug)]
pub struct ByteBuffer<'a> {
    data: &'a [u8],
    position: usize,
    capacity: usize,
}

/// zombie.iso.SliceY.SliceBuffer's capacity: `ByteBuffer.allocate(10485760)`.
const SLICE_BUFFER_CAPACITY: usize = 10_485_760;

impl<'a> ByteBuffer<'a> {
    /// `ByteBuffer.wrap(data)`: the capacity is the limit.
    pub fn wrap(data: &'a [u8]) -> Self {
        ByteBuffer {
            data,
            position: 0,
            capacity: data.len(),
        }
    }
    /// The shared `SliceY.SliceBuffer` holding a file that was read into it and limited to its length: the
    /// capacity stays 10 MiB, so a `GameWindow.ReadString` whose count runs past the limit (a truncated file) reads
    /// the buffer's bytes beyond the data. Those are whatever earlier loads left there; this port takes them as
    /// zeros, the state of a buffer nothing has used (the oracle probes zero the buffer before such a load).
    pub fn wrap_slice_buffer(data: &'a [u8]) -> Self {
        ByteBuffer {
            data,
            position: 0,
            capacity: SLICE_BUFFER_CAPACITY.max(data.len()),
        }
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    pub fn position(&self) -> usize {
        self.position
    }
    pub fn limit(&self) -> usize {
        self.data.len()
    }
    pub fn remaining(&self) -> usize {
        self.data.len() - self.position
    }
    pub fn array(&self) -> &'a [u8] {
        self.data
    }
    /// java.nio.Buffer.position(int): IllegalArgumentException past the limit.
    pub fn set_position(&mut self, p: usize) -> Result<(), String> {
        if p > self.data.len() {
            return Err(format!(
                "java.lang.IllegalArgumentException: newPosition > limit: ({p} > {})",
                self.data.len()
            ));
        }
        self.position = p;
        Ok(())
    }
    fn take<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let b = self
            .data
            .get(self.position..self.position + N)
            .ok_or(UNDERFLOW)?;
        self.position += N;
        let mut out = [0u8; N];
        out.copy_from_slice(b);
        Ok(out)
    }
    pub fn get_bytes(&mut self, n: usize) -> Result<&'a [u8], String> {
        let b = self
            .data
            .get(self.position..self.position + n)
            .ok_or(UNDERFLOW)?;
        self.position += n;
        Ok(b)
    }
    pub fn get(&mut self) -> Result<i8, String> {
        Ok(i8::from_be_bytes(self.take::<1>()?))
    }
    pub fn get_short(&mut self) -> Result<i16, String> {
        Ok(i16::from_be_bytes(self.take::<2>()?))
    }
    pub fn get_int(&mut self) -> Result<i32, String> {
        Ok(i32::from_be_bytes(self.take::<4>()?))
    }
    pub fn get_long(&mut self) -> Result<i64, String> {
        Ok(i64::from_be_bytes(self.take::<8>()?))
    }
    pub fn get_float(&mut self) -> Result<f32, String> {
        Ok(f32::from_bits(u32::from_be_bytes(self.take::<4>()?)))
    }
    pub fn get_double(&mut self) -> Result<f64, String> {
        Ok(f64::from_bits(u64::from_be_bytes(self.take::<8>()?)))
    }
    /// java.nio.ByteBuffer.getChar: one big-endian UTF-16 unit.
    pub fn get_char(&mut self) -> Result<u16, String> {
        Ok(u16::from_be_bytes(self.take::<2>()?))
    }
}

/// zombie.GameWindow.ReadString (ByteBuffer) -> GameWindow.StringUTF.load: a short byte count, then that many bytes
/// decoded as UTF-8 with malformed input replaced (U+FFFD). A count of 0 or less is the empty string. A count past
/// the limit is `in.limit(in.position() + numBytes)`: past the capacity too it is an `IllegalArgumentException`
/// (a buffer that wraps exactly its bytes); inside the capacity (the shared slice buffer) the string takes the
/// bytes beyond the data (zeros, see `wrap_slice_buffer`) and the position ends at the limit.
pub fn read_string(b: &mut ByteBuffer) -> Result<String, String> {
    let n = b.get_short()?;
    if n <= 0 {
        return Ok(String::new());
    }
    let n = n as usize;
    if b.remaining() < n {
        if b.position + n > b.capacity {
            return Err(format!(
                "java.lang.IllegalArgumentException: newLimit > capacity: ({} > {})",
                b.position + n,
                b.capacity
            ));
        }
        let mut bytes = b.data[b.position..].to_vec();
        bytes.resize(n, 0);
        b.position = b.data.len();
        return Ok(String::from_utf8_lossy(&bytes).into_owned());
    }
    Ok(String::from_utf8_lossy(b.get_bytes(n)?).into_owned())
}

/// zombie.GameWindow.ReadString(DataInputStream): an int length (0 is ""; over 65536 the original throws, and a
/// negative one fails in `new StringBuilder(len)`), then that many UTF-16 chars (`readChar`). Unpaired surrogates,
/// which a Java String can hold, become U+FFFD.
pub fn read_string_stream(b: &mut ByteBuffer) -> Result<String, String> {
    let len = b.get_int()?;
    if len == 0 {
        return Ok(String::new());
    }
    if len > 65536 {
        return Err(
            "java.lang.RuntimeException: GameWindow.ReadString: string is too long, corrupted save?".to_string(),
        );
    }
    if len < 0 {
        return Err(format!("java.lang.NegativeArraySizeException: {len}"));
    }
    let mut units = Vec::with_capacity(len as usize);
    for _ in 0..len {
        units.push(b.get_char()?);
    }
    Ok(String::from_utf16_lossy(&units))
}

/// java.io.DataInputStream.readUTF: an unsigned short byte count, then "modified UTF-8" (1-, 2- and 3-byte forms; NUL as
/// `C0 80`; characters outside the BMP as two 3-byte surrogates). Malformed input is the original's
/// `UTFDataFormatException`. Unpaired surrogates, which a Java String can hold, become U+FFFD.
pub fn read_utf(b: &mut ByteBuffer) -> Result<String, String> {
    let n = b.get_char()? as usize;
    let bytes = b.get_bytes(n)?;
    let bad = |i: usize| format!("java.io.UTFDataFormatException: malformed input around byte {i}");
    let mut units: Vec<u16> = Vec::with_capacity(n);
    let mut i = 0;
    while i < n {
        let c = u16::from(bytes[i]);
        match c >> 4 {
            0..=7 => {
                units.push(c);
                i += 1;
            }
            12 | 13 => {
                let c2 = u16::from(*bytes.get(i + 1).ok_or_else(|| bad(n))?);
                if c2 & 0xC0 != 0x80 {
                    return Err(bad(i + 1));
                }
                units.push(((c & 0x1F) << 6) | (c2 & 0x3F));
                i += 2;
            }
            14 => {
                let (Some(&c2), Some(&c3)) = (bytes.get(i + 1), bytes.get(i + 2)) else {
                    return Err(bad(n));
                };
                let (c2, c3) = (u16::from(c2), u16::from(c3));
                if c2 & 0xC0 != 0x80 || c3 & 0xC0 != 0x80 {
                    return Err(bad(i + 1));
                }
                units.push(((c & 0x0F) << 12) | ((c2 & 0x3F) << 6) | (c3 & 0x3F));
                i += 3;
            }
            _ => return Err(bad(i)),
        }
    }
    Ok(String::from_utf16_lossy(&units))
}

/// zombie.util.io.BitHeader.HeaderSize
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderSize {
    Byte,
    Short,
    Integer,
    Long,
}

/// zombie.util.io.BitHeaderRead: a flag word read from the buffer.
#[derive(Clone, Copy, Debug)]
pub struct BitHeaderRead {
    header: i64,
    limit: i64,
}

impl BitHeaderRead {
    /// zombie.util.io.BitHeader.allocRead
    pub fn alloc_read(size: HeaderSize, b: &mut ByteBuffer) -> Result<Self, String> {
        Ok(match size {
            HeaderSize::Byte => BitHeaderRead {
                header: i64::from(b.get()?),
                limit: 64,
            },
            HeaderSize::Short => BitHeaderRead {
                header: i64::from(b.get_short()?),
                limit: 16384,
            },
            HeaderSize::Integer => BitHeaderRead {
                header: i64::from(b.get_int()?),
                limit: 1 << 30,
            },
            HeaderSize::Long => BitHeaderRead {
                header: b.get_long()?,
                limit: 1 << 62,
            },
        })
    }

    /// zombie.core.utils.Bits.hasFlags -> checkFlags(..., ContainsAll); flags above the size's limit throw.
    pub fn has_flags(&self, flags: i64) -> Result<bool, String> {
        if flags < 0 || flags > self.limit {
            return Err(format!(
                "java.lang.RuntimeException: Cannot check for flags, exceeding byte bounds or negative number flags. ({flags})"
            ));
        }
        Ok(self.header & flags == flags)
    }

    /// zombie.util.io.BitHeader.BitHeaderBase.equals
    pub fn equals(&self, flags: i64) -> bool {
        self.header == flags
    }
}

/// zombie.core.utils.ByteBlock in load mode: an int length, then that many bytes.
#[derive(Clone, Copy, Debug)]
pub struct ByteBlock {
    start: usize,
    length: usize,
}

impl ByteBlock {
    /// zombie.core.utils.ByteBlock.Start (Load)
    pub fn start(b: &mut ByteBuffer) -> Result<Self, String> {
        let length = b.get_int()?;
        if length < 0 {
            return Err("java.io.IOException: DataBlock possible corruption.".into());
        }
        Ok(ByteBlock {
            start: b.position(),
            length: length as usize,
        })
    }

    /// zombie.core.utils.ByteBlock.End (Load): the block must end exactly here.
    pub fn end(self, b: &ByteBuffer) -> Result<(), String> {
        if b.position() != self.start + self.length {
            return Err("java.io.IOException: DataBlock size mismatch during load.".into());
        }
        Ok(())
    }

    /// zombie.core.utils.ByteBlock.SkipAndEnd
    pub fn skip_and_end(self, b: &mut ByteBuffer) -> Result<(), String> {
        b.set_position(self.start + self.length)
    }
}

/// zombie.core.utils.Bits.packFloatUnitToByte: the value is clamped to 0..1 (the original throws instead when
/// `Core.debug` is set, which a normal run does not), then scaled to a signed byte.
pub fn pack_float_unit_to_byte(f: f32) -> i8 {
    let f = f.clamp(0.0, 1.0);
    (f * 255.0 + -128.0) as i32 as i8
}

/// zombie.core.utils.Bits.unpackByteToFloatUnit
pub fn unpack_byte_to_float_unit(b: i8) -> f32 {
    (i32::from(b) - -128) as f32 / 255.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_big_endian_and_utf8_strings() {
        let data = [0, 3, b'a', 0xC3, 0xA9, 0xFF, 0xFE, 0x40, 0x49, 0x0F, 0xDB];
        let mut b = ByteBuffer::wrap(&data);
        assert_eq!(read_string(&mut b).as_deref(), Ok("a\u{e9}"));
        assert_eq!(b.get_short(), Ok(-2));
        assert_eq!(b.get_float(), Ok(std::f32::consts::PI));
        assert!(b.get().is_err());
    }

    #[test]
    fn byte_header_rejects_flags_above_64() {
        let data = [0x41u8];
        let h = BitHeaderRead::alloc_read(HeaderSize::Byte, &mut ByteBuffer::wrap(&data));
        let h = h.expect("one byte");
        assert_eq!(h.has_flags(1), Ok(true));
        assert_eq!(h.has_flags(64), Ok(true));
        assert_eq!(h.has_flags(2), Ok(false));
        assert!(h.has_flags(128).is_err());
    }

    /// Bytes as java.io.DataOutputStream.writeUTF("a\u0000\u00e9\u20ac") writes them (JDK spec of modified UTF-8):
    /// length 8, then 61, C0 80, C3 A9, E2 82 AC.
    #[test]
    fn read_utf_decodes_modified_utf8() {
        let data = [0, 8, 0x61, 0xC0, 0x80, 0xC3, 0xA9, 0xE2, 0x82, 0xAC, 0x7F];
        let mut b = ByteBuffer::wrap(&data);
        assert_eq!(read_utf(&mut b).as_deref(), Ok("a\u{0}\u{e9}\u{20ac}"));
        assert_eq!(b.remaining(), 1);
        // A continuation byte in first position is malformed (UTFDataFormatException).
        let mut bad = ByteBuffer::wrap(&[0, 1, 0x80]);
        assert!(read_utf(&mut bad).is_err());
    }

    /// zombie.core.utils.Bits.packFloatUnitToByte: (byte) (int) (f * 255 - 128) after clamping to 0..1; the Java
    /// cast truncates toward zero, so 0.5 gives (int) -0.5 = 0.
    #[test]
    fn pack_float_unit_to_byte_scales_and_clamps() {
        assert_eq!(pack_float_unit_to_byte(0.0), -128);
        assert_eq!(pack_float_unit_to_byte(1.0), 127);
        assert_eq!(pack_float_unit_to_byte(0.5), 0);
        assert_eq!(pack_float_unit_to_byte(-3.0), -128);
        assert_eq!(pack_float_unit_to_byte(2.0), 127);
        assert_eq!(pack_float_unit_to_byte(f32::NAN), 0);
    }
}
