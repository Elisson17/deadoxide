//! java.text.RuleBasedCollator as `Collator.getInstance(Locale.ENGLISH)` builds it (the JDK's default rules, strength
//! TERTIARY, NO_DECOMPOSITION), for strings of printable ASCII: the comparison loop is ported from the JDK's
//! `RuleBasedCollator.compare`; the collation element of each character is measured, not rebuilt from the rules.
use std::cmp::Ordering;

/// The collation element (`CollationElementIterator.next()`) of each character 0x20..=0x7E under the English collator,
/// measured on the system JDK 27 and on the game's JRE 25 (identical) by `tests/original_probe/CollatorProbe.java`,
/// which also checks that every such character has exactly one element and that no pair of them contracts. Primary
/// order in the high 16 bits, secondary in bits 8..16, tertiary in the low byte: '-' (0x6d01) is ignorable at the
/// primary level, upper case differs from lower case only at the tertiary level.
const ENGLISH_ASCII: [i32; 95] = [
    0x100, 0x60000, 0x140000, 0x370000, 0x270000, 0x380000, 0x360000, 0x130000, 0x170000, 0x180000,
    0x340000, 0x390000, 0x30000, 0x6d01, 0xb0000, 0xa0000, 0x450000, 0x460000, 0x470000, 0x480000,
    0x490000, 0x4a0000, 0x4b0000, 0x4c0000, 0x4d0000, 0x4e0000, 0x50000, 0x40000, 0x3d0000,
    0x3e0000, 0x3f0000, 0x80000, 0x210000, 0x520001, 0x530001, 0x540001, 0x550001, 0x570001,
    0x580001, 0x590001, 0x5a0001, 0x5b0001, 0x5c0001, 0x5d0001, 0x5e0001, 0x5f0001, 0x600001,
    0x610001, 0x620001, 0x630001, 0x640001, 0x650001, 0x660001, 0x670001, 0x680001, 0x690001,
    0x6a0001, 0x6b0001, 0x6c0001, 0x190000, 0x350000, 0x1a0000, 0xe0000, 0x10000, 0xd0000,
    0x520000, 0x530000, 0x540000, 0x550000, 0x570000, 0x580000, 0x590000, 0x5a0000, 0x5b0000,
    0x5c0000, 0x5d0000, 0x5e0000, 0x5f0000, 0x600000, 0x610000, 0x620000, 0x630000, 0x640000,
    0x650000, 0x660000, 0x670000, 0x680000, 0x690000, 0x6a0000, 0x6b0000, 0x6c0000, 0x1b0000,
    0x410000, 0x1c0000, 0x100000,
];

/// java.text.CollationElementIterator.NULLORDER
const NULLORDER: i32 = -1;

/// java.text.CollationElementIterator.primaryOrder
fn primary_order(order: i32) -> i32 {
    ((order as u32) >> 16) as i32
}

/// java.text.CollationElementIterator.secondaryOrder
fn secondary_order(order: i32) -> i16 {
    ((order & 0xff00) >> 8) as i16
}

/// java.text.CollationElementIterator.tertiaryOrder
fn tertiary_order(order: i32) -> i16 {
    (order & 0xff) as i16
}

/// The collation elements of `s`, then NULLORDER; `None` when a character is outside the measured range.
fn elements(s: &str) -> Option<Vec<i32>> {
    let mut v = Vec::with_capacity(s.len());
    for c in s.chars() {
        let i = (c as u32).checked_sub(0x20)?;
        v.push(*ENGLISH_ASCII.get(i as usize)?);
    }
    Some(v)
}

/// java.text.RuleBasedCollator.compare(String, String) of `Collator.getInstance(Locale.ENGLISH)`: equal strings are
/// EQUAL; then the first primary difference decides, else the first secondary or tertiary difference (a primary
/// ignorable, such as '-', counts as a secondary difference), else the length in non-ignorable elements. `None` when
/// either string has a character outside printable ASCII (not measured).
pub fn compare_english(source: &str, target: &str) -> Option<Ordering> {
    if source == target {
        return Some(Ordering::Equal);
    }
    let (s, t) = (elements(source)?, elements(target)?);
    let mut result = Ordering::Equal;
    let (mut si, mut ti) = (0usize, 0usize);
    let next = |v: &Vec<i32>, i: &mut usize| -> i32 {
        let o = v.get(*i).copied().unwrap_or(NULLORDER);
        *i += 1;
        o
    };
    // strength TERTIARY: both checks start on; the English rules are not French (no isFrenchSec branch).
    let mut check_sec_ter = true;
    let mut check_tertiary = true;
    let (mut gets, mut gett) = (true, true);
    let (mut s_order, mut t_order) = (0i32, 0i32);
    loop {
        if gets {
            s_order = next(&s, &mut si);
        } else {
            gets = true;
        }
        if gett {
            t_order = next(&t, &mut ti);
        } else {
            gett = true;
        }
        if s_order == NULLORDER || t_order == NULLORDER {
            break;
        }
        let (ps, pt) = (primary_order(s_order), primary_order(t_order));
        if s_order == t_order {
            continue;
        }
        if ps != pt {
            if s_order == 0 {
                gett = false;
                continue;
            }
            if t_order == 0 {
                gets = false;
                continue;
            }
            if ps == 0 {
                if check_sec_ter {
                    result = Ordering::Greater;
                    check_sec_ter = false;
                }
                gett = false;
            } else if pt == 0 {
                if check_sec_ter {
                    result = Ordering::Less;
                    check_sec_ter = false;
                }
                gets = false;
            } else {
                return Some(ps.cmp(&pt));
            }
        } else if check_sec_ter {
            let (ss, st) = (secondary_order(s_order), secondary_order(t_order));
            if ss != st {
                result = ss.cmp(&st);
                check_sec_ter = false;
            } else if check_tertiary {
                let (ts, tt) = (tertiary_order(s_order), tertiary_order(t_order));
                if ts != tt {
                    result = ts.cmp(&tt);
                    check_tertiary = false;
                }
            }
        }
    }
    if s_order != NULLORDER {
        loop {
            if primary_order(s_order) != 0 {
                return Some(Ordering::Greater);
            } else if secondary_order(s_order) != 0 && check_sec_ter {
                result = Ordering::Greater;
                check_sec_ter = false;
            }
            s_order = next(&s, &mut si);
            if s_order == NULLORDER {
                break;
            }
        }
    } else if t_order != NULLORDER {
        loop {
            if primary_order(t_order) != 0 {
                return Some(Ordering::Less);
            } else if secondary_order(t_order) != 0 && check_sec_ter {
                result = Ordering::Less;
                check_sec_ter = false;
            }
            t_order = next(&t, &mut ti);
            if t_order == NULLORDER {
                break;
            }
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::compare_english;
    use std::cmp::Ordering::{Greater, Less};

    /// Known JDK collator results: '-' is ignored at the primary level, case is a tertiary difference that a later
    /// primary difference overrides, and a longer string with the same base letters sorts after.
    #[test]
    fn english_collator_orders() {
        assert_eq!(
            compare_english("corpseDraggedLegs-onback.xml", "corpseDragged-onback.xml"),
            Some(Less)
        );
        assert_eq!(compare_english("AA", "aB"), Some(Less));
        assert_eq!(compare_english("a", "A"), Some(Less));
        assert_eq!(compare_english("ab", "a-b"), Some(Less));
        assert_eq!(compare_english("abc", "ab"), Some(Greater));
        assert_eq!(compare_english("\u{e9}", "e"), None);
    }
}
