//! assimp `include/assimp/fast_atof.h`: `strtoul10_64` and `fast_atoreal_move<float>` (the parser the text importers
//! use; it is not `strtof`: the fraction is read as an integer of at most 15 digits scaled by a double table entry).

/// `fast_atof_table[16]`: 0, 1e-1 ... 1e-15 as the C++ source writes them (double literals).
const FAST_ATOF_TABLE: [f64; 16] = [
    0.0,
    0.1,
    0.01,
    0.001,
    0.0001,
    0.00001,
    0.000001,
    0.0000001,
    0.00000001,
    0.000000001,
    0.0000000001,
    0.00000000001,
    0.000000000001,
    0.0000000000001,
    0.00000000000001,
    0.000000000000001,
];

/// `AI_FAST_ATOF_RELAVANT_DECIMALS`
const RELEVANT_DECIMALS: u32 = 15;

fn at(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

/// `strtoul10_64(in, &out, max_inout)`: the value, the index after the digits and the digit count; `Err` is the
/// `std::invalid_argument` thrown when `in` does not start with a digit. A digit run that overflows `uint64_t` returns 0
/// (the library logs a warning) with the index left where the overflow was found.
pub fn strtoul10_64(s: &[u8], mut i: usize, max: Option<u32>) -> Result<(u64, usize, u32), String> {
    if !at(s, i).is_ascii_digit() {
        return Err("The string \"\" cannot be converted into a value.".to_string());
    }
    let mut value: u64 = 0;
    let mut cur: u32 = 0;
    loop {
        if !at(s, i).is_ascii_digit() {
            break;
        }
        let digit = u64::from(at(s, i) - b'0');
        let new_value = value.wrapping_mul(10).wrapping_add(digit);
        if new_value < value {
            return Ok((0, i, cur));
        }
        value = new_value;
        i += 1;
        cur += 1;
        if max == Some(cur) {
            while at(s, i).is_ascii_digit() {
                i += 1;
            }
            return Ok((value, i, cur));
        }
    }
    Ok((value, i, cur))
}

/// `fast_atoreal_move<float>(c, out, check_comma = true)` starting at byte index `i`: the value and the index after it.
/// `Err` is the `std::invalid_argument` ("does not start with digit or decimal point followed by digit").
pub fn fast_atoreal_move(s: &[u8], mut i: usize) -> Result<(f32, usize), String> {
    let mut f: f32 = 0.0;
    let inv = at(s, i) == b'-';
    if inv || at(s, i) == b'+' {
        i += 1;
    }
    let lower3 = |i: usize| -> [u8; 3] {
        [
            at(s, i).to_ascii_lowercase(),
            at(s, i + 1).to_ascii_lowercase(),
            at(s, i + 2).to_ascii_lowercase(),
        ]
    };
    if (at(s, i) == b'N' || at(s, i) == b'n') && lower3(i) == *b"nan" {
        return Ok((f32::NAN, i + 3));
    }
    if (at(s, i) == b'I' || at(s, i) == b'i') && lower3(i) == *b"inf" {
        let mut out = f32::INFINITY;
        if inv {
            out = -out;
        }
        i += 3;
        if (at(s, i) == b'I' || at(s, i) == b'i')
            && s.get(i..i + 5)
                .is_some_and(|w| w.eq_ignore_ascii_case(b"inity"))
        {
            i += 5;
        }
        return Ok((out, i));
    }
    let digit = |b: u8| b.is_ascii_digit();
    if !digit(at(s, i)) && !((at(s, i) == b'.' || at(s, i) == b',') && digit(at(s, i + 1))) {
        return Err(
            "Cannot parse string as a real number: does not start with digit or decimal point followed by digit."
                .to_string(),
        );
    }
    if at(s, i) != b'.' && at(s, i) != b',' {
        let (v, next, _) = strtoul10_64(s, i, None)?;
        f = v as f32;
        i = next;
    }
    if (at(s, i) == b'.' || at(s, i) == b',') && digit(at(s, i + 1)) {
        i += 1;
        let (v, next, diff) = strtoul10_64(s, i, Some(RELEVANT_DECIMALS))?;
        i = next;
        let mut pl = v as f64;
        pl *= FAST_ATOF_TABLE[diff as usize];
        f += pl as f32;
    } else if at(s, i) == b'.' {
        i += 1;
    }
    if at(s, i) == b'e' || at(s, i) == b'E' {
        i += 1;
        let einv = at(s, i) == b'-';
        if einv || at(s, i) == b'+' {
            i += 1;
        }
        let (v, next, _) = strtoul10_64(s, i, None)?;
        i = next;
        let mut exp = v as f32;
        if einv {
            exp = -exp;
        }
        f *= 10.0f32.powf(exp);
    }
    if inv {
        f = -f;
    }
    Ok((f, i))
}
