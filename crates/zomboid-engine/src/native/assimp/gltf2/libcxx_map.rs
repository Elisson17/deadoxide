//! The iteration order of libc++'s `std::unordered_map<unsigned int, V>` (`std::hash<unsigned>` is the identity,
//! `max_load_factor` 1.0), which `glTF2Importer::ImportAnimations` exposes: the channels of an animation come out in the
//! order of `GatherSamplers`' map, not in the order of the document. The table follows libc++ 18
//! (`include/__hash_table`): the bucket count grows `0 -> 2 -> 5 -> 11 -> 23 -> ...` (`__next_prime` of
//! `2 * bc + 1`), a new node goes to the front of its bucket (or the front of the list when the bucket is empty), and a
//! rehash moves nodes one at a time behind the first node of their new bucket.

const NONE: usize = usize::MAX;
/// the "before begin" anchor: the pointer to the first node of the list
const BEFORE_BEGIN: usize = usize::MAX - 1;

struct Slot<V> {
    key: u32,
    value: V,
    next: usize,
}

/// `std::unordered_map<unsigned int, V>` with `operator[]`-style insertion and iteration in libc++ order.
pub struct UintMap<V> {
    slots: Vec<Slot<V>>,
    head: usize,
    buckets: Vec<usize>,
    size: usize,
}

/// `std::__constrain_hash`
fn constrain(h: u32, bc: usize) -> usize {
    let h = h as usize;
    if bc & (bc - 1) == 0 {
        h & (bc - 1)
    } else if h < bc {
        h
    } else {
        h % bc
    }
}

/// `std::__next_prime(n)`: the smallest prime not below `n`.
fn next_prime(n: usize) -> usize {
    let mut p = n.max(2);
    loop {
        let mut prime = true;
        let mut d = 2;
        while d * d <= p {
            if p % d == 0 {
                prime = false;
                break;
            }
            d += 1;
        }
        if prime {
            return p;
        }
        p += 1;
    }
}

impl<V: Default> Default for UintMap<V> {
    fn default() -> Self {
        UintMap {
            slots: Vec::new(),
            head: NONE,
            buckets: Vec::new(),
            size: 0,
        }
    }
}

impl<V: Default> UintMap<V> {
    fn next_of(&self, before: usize) -> usize {
        if before == BEFORE_BEGIN {
            self.head
        } else {
            self.slots[before].next
        }
    }

    fn set_next(&mut self, before: usize, to: usize) {
        if before == BEFORE_BEGIN {
            self.head = to;
        } else {
            self.slots[before].next = to;
        }
    }

    /// `__do_rehash<true>(n)`
    fn do_rehash(&mut self, n: usize) {
        self.buckets = vec![NONE; n];
        let mut pp = BEFORE_BEGIN;
        let mut cp = self.next_of(pp);
        if cp == NONE {
            return;
        }
        let mut phash = constrain(self.slots[cp].key, n);
        self.buckets[phash] = pp;
        pp = cp;
        cp = self.slots[cp].next;
        while cp != NONE {
            let chash = constrain(self.slots[cp].key, n);
            if chash == phash {
                pp = cp;
            } else if self.buckets[chash] == NONE {
                self.buckets[chash] = pp;
                pp = cp;
                phash = chash;
            } else {
                let after = self.slots[cp].next;
                self.set_next(pp, after);
                let b = self.buckets[chash];
                self.slots[cp].next = self.next_of(b);
                self.set_next(b, cp);
            }
            cp = self.next_of(pp);
        }
    }

    /// `__rehash_unique(n)` for a growing table
    fn rehash(&mut self, n: usize) {
        let n = if n == 1 {
            2
        } else if n & (n - 1) != 0 {
            next_prime(n)
        } else {
            n
        };
        if n > self.buckets.len() {
            self.do_rehash(n);
        }
    }

    /// `__emplace_unique_key_args`: the value of `key`, inserted (default constructed) when it is not there yet.
    pub fn entry(&mut self, key: u32) -> &mut V {
        let bc = self.buckets.len();
        if bc != 0 {
            let chash = constrain(key, bc);
            let mut nd = self.buckets[chash];
            if nd != NONE {
                nd = self.next_of(nd);
                while nd != NONE && constrain(self.slots[nd].key, bc) == chash {
                    if self.slots[nd].key == key {
                        return &mut self.slots[nd].value;
                    }
                    nd = self.slots[nd].next;
                }
            }
        }
        let idx = self.slots.len();
        self.slots.push(Slot {
            key,
            value: V::default(),
            next: NONE,
        });
        if bc == 0 || self.size + 1 > bc {
            let pow2 = bc > 2 && bc & (bc - 1) == 0;
            let grown = 2 * bc + usize::from(!pow2);
            self.rehash(grown.max(self.size + 1));
        }
        let bc = self.buckets.len();
        let chash = constrain(key, bc);
        let pn = self.buckets[chash];
        if pn == NONE {
            self.slots[idx].next = self.head;
            self.head = idx;
            self.buckets[chash] = BEFORE_BEGIN;
            let nxt = self.slots[idx].next;
            if nxt != NONE {
                let h = constrain(self.slots[nxt].key, bc);
                self.buckets[h] = idx;
            }
        } else {
            self.slots[idx].next = self.next_of(pn);
            self.set_next(pn, idx);
        }
        self.size += 1;
        &mut self.slots[idx].value
    }

    /// The elements in iteration order.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &V)> {
        let mut cur = self.head;
        std::iter::from_fn(move || {
            if cur == NONE {
                return None;
            }
            let s = &self.slots[cur];
            cur = s.next;
            Some((s.key, &s.value))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(keys: &[u32]) -> Vec<u32> {
        let mut m: UintMap<()> = UintMap::default();
        for &k in keys {
            m.entry(k);
        }
        m.iter().map(|(k, _)| k).collect()
    }

    #[test]
    fn small_tables_iterate_newest_first_within_the_list_head() {
        // 2 buckets, then 5: 1 goes to bucket 1; 2 to bucket 0 and in front of the list
        assert_eq!(order(&[1]), vec![1]);
        assert_eq!(order(&[1, 2]), vec![2, 1]);
    }

    #[test]
    fn repeated_keys_are_one_element() {
        assert_eq!(order(&[7, 7, 7]), vec![7]);
    }

    #[test]
    fn every_key_is_listed_once() {
        let keys: Vec<u32> = (0..100).map(|i| (i * 37) % 211).collect();
        let mut got = order(&keys);
        got.sort_unstable();
        let mut want = keys.clone();
        want.sort_unstable();
        want.dedup();
        assert_eq!(got, want);
    }
}
