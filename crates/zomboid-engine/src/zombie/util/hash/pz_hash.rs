//! zombie.util.hash.PZHash: the 64-bit MurmurHash2 the script versions use.

/// zombie.util.hash.PZHash.murmur_64(String): over `text.getBytes()`, the JVM's default charset (UTF-8 since JDK 18),
/// with the seed -512093083.
pub fn murmur_64_str(text: &str) -> i64 {
    murmur_64(text.as_bytes(), -512_093_083)
}

/// zombie.util.hash.PZHash.murmur_64(byte[], int, int): MurmurHash64A with Java's wrapping long arithmetic.
pub fn murmur_64(data: &[u8], seed: i32) -> i64 {
    const M: i64 = -4_132_994_306_676_758_123;
    let length = data.len() as i32;
    let mut h: i64 = (i64::from(seed) & 0xFFFF_FFFF) ^ i64::from(length).wrapping_mul(M);
    let length8 = length / 8;
    for i in 0..length8 {
        let i8 = (i * 8) as usize;
        let mut k: i64 = 0;
        for b in 0..8 {
            k = k.wrapping_add(i64::from(data[i8 + b]) << (8 * b));
        }
        k = k.wrapping_mul(M);
        k ^= ((k as u64) >> 47) as i64;
        k = k.wrapping_mul(M);
        h ^= k;
        h = h.wrapping_mul(M);
    }
    let base = (length & -8) as usize;
    let rest = length % 8;
    // the switch falls through from `rest` down to 1
    if rest >= 1 {
        for b in (1..rest as usize).rev() {
            h ^= i64::from(data[base + b]) << (8 * b);
        }
        h ^= i64::from(data[base]);
        h = h.wrapping_mul(M);
    }
    h ^= ((h as u64) >> 47) as i64;
    h = h.wrapping_mul(M);
    h ^ ((h as u64) >> 47) as i64
}
