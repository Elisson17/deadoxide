//! `std::sort(first, last)` of libc++ 18 (`include/__algorithm/sort.h`, the library the game's macOS `libjassimp` is
//! built with: its symbols carry the ABI tag `ne180100`) for a trivially copyable element type and a strict weak
//! order `less`, for ranges of fewer than 24 elements. The order of equal elements depends on the algorithm, and
//! assimp's results depend on that order (the weights of one vertex that tie in `LimitBoneWeightsProcess`), so it is
//! reproduced as written: `__sort3`/`__sort4`/`__sort5` for two to five elements, the guarded `__insertion_sort` up to
//! 23 (the limit 24 is read from the disassembled `__introsort<..., Weight*, false>`). Longer ranges take the
//! partitioning path of `__introsort`, which no game file reaches, and give `Err`.

fn sort3<T: Copy>(v: &mut [T], x: usize, y: usize, z: usize, less: &dyn Fn(&T, &T) -> bool) {
    if !less(&v[y], &v[x]) {
        if !less(&v[z], &v[y]) {
            return;
        }
        v.swap(y, z);
        if less(&v[y], &v[x]) {
            v.swap(x, y);
        }
        return;
    }
    if less(&v[z], &v[y]) {
        v.swap(x, z);
        return;
    }
    v.swap(x, y);
    if less(&v[z], &v[y]) {
        v.swap(y, z);
    }
}

fn sort4<T: Copy>(v: &mut [T], x: [usize; 4], less: &dyn Fn(&T, &T) -> bool) {
    sort3(v, x[0], x[1], x[2], less);
    if less(&v[x[3]], &v[x[2]]) {
        v.swap(x[2], x[3]);
        if less(&v[x[2]], &v[x[1]]) {
            v.swap(x[1], x[2]);
            if less(&v[x[1]], &v[x[0]]) {
                v.swap(x[0], x[1]);
            }
        }
    }
}

fn sort5<T: Copy>(v: &mut [T], x: [usize; 5], less: &dyn Fn(&T, &T) -> bool) {
    sort4(v, [x[0], x[1], x[2], x[3]], less);
    if less(&v[x[4]], &v[x[3]]) {
        v.swap(x[3], x[4]);
        if less(&v[x[3]], &v[x[2]]) {
            v.swap(x[2], x[3]);
            if less(&v[x[2]], &v[x[1]]) {
                v.swap(x[1], x[2]);
                if less(&v[x[1]], &v[x[0]]) {
                    v.swap(x[0], x[1]);
                }
            }
        }
    }
}

/// `std::__insertion_sort`
fn insertion_sort<T: Copy>(v: &mut [T], less: &dyn Fn(&T, &T) -> bool) {
    for i in 1..v.len() {
        let mut j = i - 1;
        if less(&v[i], &v[j]) {
            let t = v[i];
            let mut k = j;
            j = i;
            loop {
                v[j] = v[k];
                j = k;
                if j == 0 {
                    break;
                }
                k -= 1;
                if !less(&t, &v[k]) {
                    break;
                }
            }
            v[j] = t;
        }
    }
}

/// `std::sort(v.begin(), v.end())` with the strict weak order `less`.
pub fn sort<T: Copy>(v: &mut [T], less: &dyn Fn(&T, &T) -> bool) -> Result<(), String> {
    match v.len() {
        0 | 1 => {}
        2 => {
            if less(&v[1], &v[0]) {
                v.swap(0, 1);
            }
        }
        3 => sort3(v, 0, 1, 2, less),
        4 => sort4(v, [0, 1, 2, 3], less),
        5 => sort5(v, [0, 1, 2, 3, 4], less),
        n if n < 24 => insertion_sort(v, less),
        _ => return Err("UNPORTED: std::sort of 24 or more elements".to_string()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::sort;

    /// Every range up to 23 elements comes out ordered, and equal keys keep a permutation of their elements.
    #[test]
    fn sorts_small_ranges_by_the_key() {
        let mut state = 0x2468_ace1u32;
        for n in 0..24usize {
            for _ in 0..50 {
                let mut v: Vec<(u32, u32)> = (0..n)
                    .map(|i| {
                        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                        ((state >> 24) % 5, i as u32)
                    })
                    .collect();
                let mut want = v.clone();
                want.sort();
                sort(&mut v, &|a: &(u32, u32), b: &(u32, u32)| a.0 < b.0).expect("small range");
                assert!(v.windows(2).all(|w| w[0].0 <= w[1].0));
                v.sort();
                assert_eq!(v, want);
            }
        }
    }

    #[test]
    fn long_ranges_are_refused() {
        let mut v = vec![0u32; 24];
        assert!(sort(&mut v, &|a: &u32, b: &u32| a < b).is_err());
    }
}
