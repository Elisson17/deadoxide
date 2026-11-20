//! zlib `inflate` (RFC 1950/1951) as assimp's FBX parser uses it (`inflateInit`, `inflate(Z_FINISH)`, `inflateEnd`):
//! the output buffer has a fixed size; a stream that does not end inside it, a bad header, a bad block or a wrong
//! Adler-32 is an error.

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    bit: u32,
    nbits: u32,
}

impl Bits<'_> {
    fn take(&mut self, n: u32) -> Result<u32, String> {
        if n == 0 {
            return Ok(0);
        }
        while self.nbits < n {
            let b = *self.data.get(self.pos).ok_or("inflate: truncated input")?;
            self.pos += 1;
            self.bit |= u32::from(b) << self.nbits;
            self.nbits += 8;
        }
        let v = self.bit & ((1u32 << n) - 1);
        self.bit >>= n;
        self.nbits -= n;
        Ok(v)
    }
}

struct Huffman {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl Huffman {
    fn new(lengths: &[u8]) -> Result<Huffman, String> {
        let mut counts = [0u16; 16];
        for &l in lengths {
            counts[l as usize] += 1;
        }
        counts[0] = 0;
        let mut left: i32 = 1;
        for c in counts.iter().skip(1) {
            left <<= 1;
            left -= i32::from(*c);
            if left < 0 {
                return Err("inflate: over-subscribed code".to_string());
            }
        }
        let mut offs = [0u16; 16];
        for len in 1..15 {
            offs[len + 1] = offs[len] + counts[len];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (sym, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbols[offs[l as usize] as usize] = sym as u16;
                offs[l as usize] += 1;
            }
        }
        Ok(Huffman { counts, symbols })
    }

    fn decode(&self, b: &mut Bits) -> Result<u16, String> {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: i32 = 0;
        for len in 1..16 {
            code |= b.take(1)? as i32;
            let count = i32::from(self.counts[len]);
            if code - count < first {
                return Ok(self.symbols[(index + (code - first)) as usize]);
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err("inflate: invalid code".to_string())
    }
}

const LBASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEXT: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DBASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DEXT: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

fn codes(
    b: &mut Bits,
    out: &mut Vec<u8>,
    limit: usize,
    lit: &Huffman,
    dist: &Huffman,
) -> Result<(), String> {
    loop {
        let sym = lit.decode(b)?;
        if sym < 256 {
            if out.len() >= limit {
                return Err("inflate: output buffer too small".to_string());
            }
            out.push(sym as u8);
        } else if sym == 256 {
            return Ok(());
        } else {
            let s = (sym - 257) as usize;
            if s >= 29 {
                return Err("inflate: invalid length code".to_string());
            }
            let len = LBASE[s] as usize + b.take(u32::from(LEXT[s]))? as usize;
            let ds = dist.decode(b)? as usize;
            if ds >= 30 {
                return Err("inflate: invalid distance code".to_string());
            }
            let d = DBASE[ds] as usize + b.take(u32::from(DEXT[ds]))? as usize;
            if d > out.len() {
                return Err("inflate: distance too far back".to_string());
            }
            if out.len() + len > limit {
                return Err("inflate: output buffer too small".to_string());
            }
            for _ in 0..len {
                let c = out[out.len() - d];
                out.push(c);
            }
        }
    }
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// `inflate(strm, Z_FINISH)` of the zlib stream `data` into a buffer of `size` bytes: the decompressed bytes padded
/// with zeros to `size` (the stream may end early; zlib leaves the rest of the buffer as it was, zero here).
pub fn inflate_zlib(data: &[u8], size: usize) -> Result<Vec<u8>, String> {
    if data.len() < 2 {
        return Err("inflate: truncated input".to_string());
    }
    let (cmf, flg) = (data[0], data[1]);
    if ((u32::from(cmf) << 8) | u32::from(flg)) % 31 != 0 || cmf & 0x0f != 8 || flg & 0x20 != 0 {
        return Err("inflate: incorrect header check".to_string());
    }
    let mut b = Bits {
        data,
        pos: 2,
        bit: 0,
        nbits: 0,
    };
    let mut out: Vec<u8> = Vec::with_capacity(size);
    loop {
        let last = b.take(1)?;
        match b.take(2)? {
            0 => {
                b.bit = 0;
                b.nbits = 0;
                if b.pos + 4 > data.len() {
                    return Err("inflate: truncated input".to_string());
                }
                let len = u16::from_le_bytes([data[b.pos], data[b.pos + 1]]) as usize;
                let nlen = u16::from_le_bytes([data[b.pos + 2], data[b.pos + 3]]) as usize;
                if len != (!nlen & 0xffff) {
                    return Err("inflate: invalid stored block lengths".to_string());
                }
                b.pos += 4;
                if b.pos + len > data.len() {
                    return Err("inflate: truncated input".to_string());
                }
                if out.len() + len > size {
                    return Err("inflate: output buffer too small".to_string());
                }
                out.extend_from_slice(&data[b.pos..b.pos + len]);
                b.pos += len;
            }
            1 => {
                let mut l = [0u8; 288];
                for (i, v) in l.iter_mut().enumerate() {
                    *v = if i < 144 {
                        8
                    } else if i < 256 {
                        9
                    } else if i < 280 {
                        7
                    } else {
                        8
                    };
                }
                let lit = Huffman::new(&l)?;
                let dist = Huffman::new(&[5u8; 30])?;
                codes(&mut b, &mut out, size, &lit, &dist)?;
            }
            2 => {
                let nlen = b.take(5)? as usize + 257;
                let ndist = b.take(5)? as usize + 1;
                let ncode = b.take(4)? as usize + 4;
                if nlen > 286 || ndist > 30 {
                    return Err("inflate: too many length or distance symbols".to_string());
                }
                const ORDER: [usize; 19] = [
                    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
                ];
                let mut cl_lengths = [0u8; 19];
                for &o in ORDER.iter().take(ncode) {
                    cl_lengths[o] = b.take(3)? as u8;
                }
                let cl = Huffman::new(&cl_lengths)?;
                let mut lengths = [0u8; 320];
                let mut i = 0;
                while i < nlen + ndist {
                    let sym = cl.decode(&mut b)?;
                    if sym < 16 {
                        lengths[i] = sym as u8;
                        i += 1;
                    } else {
                        let (prev, rep) = match sym {
                            16 => {
                                if i == 0 {
                                    return Err("inflate: invalid bit length repeat".to_string());
                                }
                                (lengths[i - 1], 3 + b.take(2)? as usize)
                            }
                            17 => (0, 3 + b.take(3)? as usize),
                            _ => (0, 11 + b.take(7)? as usize),
                        };
                        if i + rep > nlen + ndist {
                            return Err("inflate: invalid bit length repeat".to_string());
                        }
                        for _ in 0..rep {
                            lengths[i] = prev;
                            i += 1;
                        }
                    }
                }
                if lengths[256] == 0 {
                    return Err("inflate: invalid code -- missing end-of-block".to_string());
                }
                let lit = Huffman::new(&lengths[..nlen])?;
                let dist = Huffman::new(&lengths[nlen..nlen + ndist])?;
                codes(&mut b, &mut out, size, &lit, &dist)?;
            }
            _ => return Err("inflate: invalid block type".to_string()),
        }
        if last == 1 {
            break;
        }
    }
    // the Adler-32 trailer starts at the next byte boundary
    let tail = b.pos - (b.nbits / 8) as usize;
    if tail + 4 > data.len() {
        return Err("inflate: truncated input".to_string());
    }
    let want = u32::from_be_bytes([data[tail], data[tail + 1], data[tail + 2], data[tail + 3]]);
    if adler32(&out) != want {
        return Err("inflate: incorrect data check".to_string());
    }
    out.resize(size, 0);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_and_stored_blocks() {
        // zlib.compress(b"hello hello hello hello")
        let z = [
            0x78, 0x9c, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0xc8, 0x40, 0x27, 0x01, 0x68, 0x03,
            0x08, 0xb1,
        ];
        assert_eq!(&inflate_zlib(&z, 23).unwrap(), b"hello hello hello hello");
        assert!(inflate_zlib(&z, 22).is_err());
        let mut s = vec![0x78, 0x01, 0x01, 0x03, 0x00, 0xfc, 0xff, b'a', b'b', b'c'];
        s.extend_from_slice(&adler32(b"abc").to_be_bytes());
        assert_eq!(inflate_zlib(&s, 3).unwrap(), b"abc");
    }
}
