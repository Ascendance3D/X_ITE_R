//! V8's `Array.prototype.sort` (Torque `third_party/v8/builtins/array-sort.tq`, a TimSort), ported
//! so that libtess' priority queue (`PriorityQ.init` sorts with an inconsistent comparator that
//! returns 1 for equal vertices in both directions) orders equal vertices exactly like Chrome.
//!
//! `compare (a, b)` returns the JS comparator's number; only its sign is used. Elements are
//! compared as `compare (x, y)` with the same argument order V8 uses at each call site.

const MIN_GALLOP_WINS: usize = 7;

struct State<'a, T, F: FnMut(&T, &T) -> i32> {
    work: &'a mut Vec<T>,
    compare: F,
    min_gallop: usize,
    runs: Vec<(usize, usize)>, // (base, length)
}

pub fn sort<T: Clone, F: FnMut(&T, &T) -> i32>(array: &mut Vec<T>, compare: F) {
    let length = array.len();

    if length < 2 {
        return;
    }

    let mut st = State {
        work: array,
        compare,
        min_gallop: MIN_GALLOP_WINS,
        runs: vec![],
    };
    let min_run = compute_min_run_length(length);
    let mut low = 0;
    let mut remaining = length;

    while remaining != 0 {
        let mut current = st.count_and_make_run(low, low + remaining);

        if current < min_run {
            let forced = min_run.min(remaining);
            st.binary_insertion_sort(low, low + current, low + forced);
            current = forced;
        }

        st.runs.push((low, current));
        st.merge_collapse();
        low += current;
        remaining -= current;
    }

    st.merge_force_collapse();
}

fn compute_min_run_length(mut n: usize) -> usize {
    let mut r = 0;

    while n >= 64 {
        r |= n & 1;
        n >>= 1;
    }

    n + r
}

impl<T: Clone, F: FnMut(&T, &T) -> i32> State<'_, T, F> {
    fn cmp(&mut self, a: &T, b: &T) -> i32 {
        (self.compare)(a, b)
    }

    /// memmove within the work array (V8 `Copy` picks the direction for overlapping ranges).
    fn copy_work(&mut self, src: usize, dst: usize, n: usize) {
        if src < dst {
            for k in (0..n).rev() {
                self.work[dst + k] = self.work[src + k].clone();
            }
        } else {
            for k in 0..n {
                self.work[dst + k] = self.work[src + k].clone();
            }
        }
    }

    fn count_and_make_run(&mut self, low_arg: usize, high: usize) -> usize {
        let low = low_arg + 1;

        if low == high {
            return 1;
        }

        let mut run_length = 2;
        let element_low = self.work[low].clone();
        let element_high = self.work[low - 1].clone();
        let order = self.cmp(&element_low, &element_high);
        let descending = order < 0;
        let mut previous = element_low;

        for idx in low + 1..high {
            let current = self.work[idx].clone();
            let order = self.cmp(&current, &previous);

            if descending {
                if order >= 0 {
                    break;
                }
            } else if order < 0 {
                break;
            }

            previous = current;
            run_length += 1;
        }

        if descending {
            self.work[low_arg..low_arg + run_length].reverse();
        }

        run_length
    }

    fn binary_insertion_sort(&mut self, low: usize, start_arg: usize, high: usize) {
        let mut start = if low == start_arg {
            start_arg + 1
        } else {
            start_arg
        };

        while start < high {
            let mut left = low;
            let mut right = start;
            let pivot = self.work[right].clone();

            while left < right {
                let mid = left + ((right - left) >> 1);
                let m = self.work[mid].clone();
                let order = self.cmp(&pivot, &m);

                if order < 0 {
                    right = mid;
                } else {
                    left = mid + 1;
                }
            }

            let mut p = start;

            while p > left {
                self.work[p] = self.work[p - 1].clone();
                p -= 1;
            }

            self.work[left] = pivot;
            start += 1;
        }
    }

    fn run_invariant_established(&self, n: isize) -> bool {
        if n < 2 {
            return true;
        }

        let n = n as usize;
        self.runs[n - 2].1 > self.runs[n - 1].1 + self.runs[n].1
    }

    fn merge_collapse(&mut self) {
        while self.runs.len() > 1 {
            let mut n = self.runs.len() as isize - 2;

            if !self.run_invariant_established(n + 1) || !self.run_invariant_established(n) {
                if n >= 1 && self.runs[(n - 1) as usize].1 < self.runs[(n + 1) as usize].1 {
                    n -= 1;
                }

                self.merge_at(n as usize);
            } else if self.runs[n as usize].1 <= self.runs[(n + 1) as usize].1 {
                self.merge_at(n as usize);
            } else {
                break;
            }
        }
    }

    fn merge_force_collapse(&mut self) {
        while self.runs.len() > 1 {
            let mut n = self.runs.len() - 2;

            if n > 0 && self.runs[n - 1].1 < self.runs[n + 1].1 {
                n -= 1;
            }

            self.merge_at(n);
        }
    }

    fn merge_at(&mut self, i: usize) {
        let size = self.runs.len();
        let (mut base_a, mut length_a) = self.runs[i];
        let (base_b, mut length_b) = self.runs[i + 1];

        self.runs[i].1 = length_a + length_b;

        if i + 3 == size {
            self.runs[i + 1] = self.runs[i + 2];
        }

        self.runs.pop();

        let key_right = self.work[base_b].clone();
        let k = self.gallop_right(false, &key_right, base_a, length_a, 0, None);
        base_a += k;
        length_a -= k;

        if length_a == 0 {
            return;
        }

        let key_left = self.work[base_a + length_a - 1].clone();
        length_b = self.gallop_left(false, &key_left, base_b, length_b, length_b - 1, None);

        if length_b == 0 {
            return;
        }

        if length_a <= length_b {
            self.merge_low(base_a, length_a, base_b, length_b);
        } else {
            self.merge_high(base_a, length_a, base_b, length_b);
        }
    }

    /// Element access in either the work array or a temp array.
    fn at(&self, temp: Option<&Vec<T>>, i: usize) -> T {
        match temp {
            Some(t) => t[i].clone(),
            None => self.work[i].clone(),
        }
    }

    fn gallop_left(
        &mut self,
        _unused: bool,
        key: &T,
        base: usize,
        length: usize,
        hint: usize,
        temp: Option<&Vec<T>>,
    ) -> usize {
        let mut last_ofs: isize = 0;
        let mut offset: isize = 1;
        let hint_i = hint as isize;
        let length_i = length as isize;
        let base_hint = self.at(temp, base + hint);
        let order = self.cmp(&base_hint, key);

        if order < 0 {
            let max_ofs = length_i - hint_i;

            while offset < max_ofs {
                let e = self.at(temp, (base as isize + hint_i + offset) as usize);
                let order = self.cmp(&e, key);

                if order >= 0 {
                    break;
                }

                last_ofs = offset;
                offset = (offset << 1) + 1;
            }

            if offset > max_ofs {
                offset = max_ofs;
            }

            last_ofs += hint_i;
            offset += hint_i;
        } else {
            let max_ofs = hint_i + 1;

            while offset < max_ofs {
                let e = self.at(temp, (base as isize + hint_i - offset) as usize);
                let order = self.cmp(&e, key);

                if order < 0 {
                    break;
                }

                last_ofs = offset;
                offset = (offset << 1) + 1;
            }

            if offset > max_ofs {
                offset = max_ofs;
            }

            let tmp = last_ofs;
            last_ofs = hint_i - offset;
            offset = hint_i - tmp;
        }

        last_ofs += 1;

        while last_ofs < offset {
            let m = last_ofs + ((offset - last_ofs) >> 1);
            let e = self.at(temp, (base as isize + m) as usize);
            let order = self.cmp(&e, key);

            if order < 0 {
                last_ofs = m + 1;
            } else {
                offset = m;
            }
        }

        offset as usize
    }

    fn gallop_right(
        &mut self,
        _unused: bool,
        key: &T,
        base: usize,
        length: usize,
        hint: usize,
        temp: Option<&Vec<T>>,
    ) -> usize {
        let mut last_ofs: isize = 0;
        let mut offset: isize = 1;
        let hint_i = hint as isize;
        let length_i = length as isize;
        let base_hint = self.at(temp, base + hint);
        let order = self.cmp(key, &base_hint);

        if order < 0 {
            let max_ofs = hint_i + 1;

            while offset < max_ofs {
                let e = self.at(temp, (base as isize + hint_i - offset) as usize);
                let order = self.cmp(key, &e);

                if order >= 0 {
                    break;
                }

                last_ofs = offset;
                offset = (offset << 1) + 1;
            }

            if offset > max_ofs {
                offset = max_ofs;
            }

            let tmp = last_ofs;
            last_ofs = hint_i - offset;
            offset = hint_i - tmp;
        } else {
            let max_ofs = length_i - hint_i;

            while offset < max_ofs {
                let e = self.at(temp, (base as isize + hint_i + offset) as usize);
                let order = self.cmp(key, &e);

                if order < 0 {
                    break;
                }

                last_ofs = offset;
                offset = (offset << 1) + 1;
            }

            if offset > max_ofs {
                offset = max_ofs;
            }

            last_ofs += hint_i;
            offset += hint_i;
        }

        last_ofs += 1;

        while last_ofs < offset {
            let m = last_ofs + ((offset - last_ofs) >> 1);
            let e = self.at(temp, (base as isize + m) as usize);
            let order = self.cmp(key, &e);

            if order < 0 {
                offset = m;
            } else {
                last_ofs = m + 1;
            }
        }

        offset as usize
    }

    fn merge_low(
        &mut self,
        base_a: usize,
        length_a_arg: usize,
        base_b: usize,
        length_b_arg: usize,
    ) {
        let mut length_a = length_a_arg;
        let mut length_b = length_b_arg;
        let temp: Vec<T> = self.work[base_a..base_a + length_a].to_vec();
        let mut dest = base_a;
        let mut cursor_temp = 0usize;
        let mut cursor_b = base_b;

        self.work[dest] = self.work[cursor_b].clone();
        dest += 1;
        cursor_b += 1;

        // 0 = Succeed, 1 = CopyB
        let exit = 'outer: {
            length_b -= 1;

            if length_b == 0 {
                break 'outer 0;
            }

            if length_a == 1 {
                break 'outer 1;
            }

            let mut min_gallop = self.min_gallop;

            loop {
                let mut wins_a = 0usize;
                let mut wins_b = 0usize;

                loop {
                    let b = self.work[cursor_b].clone();
                    let order = self.cmp(&b, &temp[cursor_temp]);

                    if order < 0 {
                        self.work[dest] = self.work[cursor_b].clone();
                        dest += 1;
                        cursor_b += 1;
                        wins_b += 1;
                        length_b -= 1;
                        wins_a = 0;

                        if length_b == 0 {
                            break 'outer 0;
                        }

                        if wins_b >= min_gallop {
                            break;
                        }
                    } else {
                        self.work[dest] = temp[cursor_temp].clone();
                        dest += 1;
                        cursor_temp += 1;
                        wins_a += 1;
                        length_a -= 1;
                        wins_b = 0;

                        if length_a == 1 {
                            break 'outer 1;
                        }

                        if wins_a >= min_gallop {
                            break;
                        }
                    }
                }

                min_gallop += 1;
                let mut first = true;

                while wins_a >= MIN_GALLOP_WINS || wins_b >= MIN_GALLOP_WINS || first {
                    first = false;
                    min_gallop = 1.max(min_gallop.saturating_sub(1));
                    self.min_gallop = min_gallop;

                    let key = self.work[cursor_b].clone();
                    wins_a = self.gallop_right(false, &key, cursor_temp, length_a, 0, Some(&temp));

                    if wins_a > 0 {
                        for k in 0..wins_a {
                            self.work[dest + k] = temp[cursor_temp + k].clone();
                        }

                        dest += wins_a;
                        cursor_temp += wins_a;
                        length_a -= wins_a;

                        if length_a == 1 {
                            break 'outer 1;
                        }

                        if length_a == 0 {
                            break 'outer 0;
                        }
                    }

                    self.work[dest] = self.work[cursor_b].clone();
                    dest += 1;
                    cursor_b += 1;
                    length_b -= 1;

                    if length_b == 0 {
                        break 'outer 0;
                    }

                    let key = temp[cursor_temp].clone();
                    wins_b = self.gallop_left(false, &key, cursor_b, length_b, 0, None);

                    if wins_b > 0 {
                        self.copy_work(cursor_b, dest, wins_b);

                        dest += wins_b;
                        cursor_b += wins_b;
                        length_b -= wins_b;

                        if length_b == 0 {
                            break 'outer 0;
                        }
                    }

                    self.work[dest] = temp[cursor_temp].clone();
                    dest += 1;
                    cursor_temp += 1;
                    length_a -= 1;

                    if length_a == 1 {
                        break 'outer 1;
                    }
                }

                min_gallop += 1;
                self.min_gallop = min_gallop;
            }
        };

        if exit == 0 {
            for k in 0..length_a {
                self.work[dest + k] = temp[cursor_temp + k].clone();
            }
        } else {
            self.copy_work(cursor_b, dest, length_b);
            self.work[dest + length_b] = temp[cursor_temp].clone();
        }
    }

    fn merge_high(
        &mut self,
        base_a: usize,
        length_a_arg: usize,
        base_b: usize,
        length_b_arg: usize,
    ) {
        let mut length_a = length_a_arg as isize;
        let mut length_b = length_b_arg as isize;
        let temp: Vec<T> = self.work[base_b..base_b + length_b as usize].to_vec();
        let mut dest = (base_b + length_b_arg - 1) as isize;
        let mut cursor_temp = length_b - 1;
        let mut cursor_a = (base_a + length_a_arg - 1) as isize;

        self.work[dest as usize] = self.work[cursor_a as usize].clone();
        dest -= 1;
        cursor_a -= 1;

        // 0 = Succeed, 1 = CopyA
        let exit = 'outer: {
            length_a -= 1;

            if length_a == 0 {
                break 'outer 0;
            }

            if length_b == 1 {
                break 'outer 1;
            }

            let mut min_gallop = self.min_gallop;

            loop {
                let mut wins_a = 0usize;
                let mut wins_b = 0usize;

                loop {
                    let a = self.work[cursor_a as usize].clone();
                    let order = self.cmp(&temp[cursor_temp as usize], &a);

                    if order < 0 {
                        self.work[dest as usize] = self.work[cursor_a as usize].clone();
                        dest -= 1;
                        cursor_a -= 1;
                        wins_a += 1;
                        length_a -= 1;
                        wins_b = 0;

                        if length_a == 0 {
                            break 'outer 0;
                        }

                        if wins_a >= min_gallop {
                            break;
                        }
                    } else {
                        self.work[dest as usize] = temp[cursor_temp as usize].clone();
                        dest -= 1;
                        cursor_temp -= 1;
                        wins_b += 1;
                        length_b -= 1;
                        wins_a = 0;

                        if length_b == 1 {
                            break 'outer 1;
                        }

                        if wins_b >= min_gallop {
                            break;
                        }
                    }
                }

                min_gallop += 1;
                let mut first = true;

                while wins_a >= MIN_GALLOP_WINS || wins_b >= MIN_GALLOP_WINS || first {
                    first = false;
                    min_gallop = 1.max(min_gallop.saturating_sub(1));
                    self.min_gallop = min_gallop;

                    let key = temp[cursor_temp as usize].clone();
                    let k = self.gallop_right(
                        false,
                        &key,
                        base_a,
                        length_a as usize,
                        (length_a - 1) as usize,
                        None,
                    ) as isize;
                    wins_a = (length_a - k) as usize;

                    if wins_a > 0 {
                        dest -= wins_a as isize;
                        cursor_a -= wins_a as isize;

                        self.copy_work((cursor_a + 1) as usize, (dest + 1) as usize, wins_a);

                        length_a -= wins_a as isize;

                        if length_a == 0 {
                            break 'outer 0;
                        }
                    }

                    self.work[dest as usize] = temp[cursor_temp as usize].clone();
                    dest -= 1;
                    cursor_temp -= 1;
                    length_b -= 1;

                    if length_b == 1 {
                        break 'outer 1;
                    }

                    let key = self.work[cursor_a as usize].clone();
                    let k = self.gallop_left(
                        false,
                        &key,
                        0,
                        length_b as usize,
                        (length_b - 1) as usize,
                        Some(&temp),
                    ) as isize;
                    wins_b = (length_b - k) as usize;

                    if wins_b > 0 {
                        dest -= wins_b as isize;
                        cursor_temp -= wins_b as isize;

                        for j in 0..wins_b {
                            self.work[(dest + 1) as usize + j] =
                                temp[(cursor_temp + 1) as usize + j].clone();
                        }

                        length_b -= wins_b as isize;

                        if length_b == 1 {
                            break 'outer 1;
                        }

                        if length_b == 0 {
                            break 'outer 0;
                        }
                    }

                    self.work[dest as usize] = self.work[cursor_a as usize].clone();
                    dest -= 1;
                    cursor_a -= 1;
                    length_a -= 1;

                    if length_a == 0 {
                        break 'outer 0;
                    }
                }

                min_gallop += 1;
                self.min_gallop = min_gallop;
            }
        };

        if exit == 0 {
            if length_b > 0 {
                let start = (dest - (length_b - 1)) as usize;

                for j in 0..length_b as usize {
                    self.work[start + j] = temp[j].clone();
                }
            }
        } else {
            dest -= length_a;
            cursor_a -= length_a;

            self.copy_work(
                (cursor_a + 1) as usize,
                (dest + 1) as usize,
                length_a as usize,
            );

            self.work[dest as usize] = temp[cursor_temp as usize].clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_with_consistent_comparator() {
        let mut state = 12345u64;
        let mut next = || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 33) as i64 % 1000
        };

        for n in [0usize, 1, 2, 5, 31, 32, 33, 64, 65, 100, 257, 1000, 5000] {
            let mut v: Vec<i64> = (0..n).map(|_| next()).collect();
            let mut expected = v.clone();
            expected.sort();
            sort(&mut v, |a, b| (*a - *b).signum() as i32);
            assert_eq!(v, expected, "n = {n}");
        }
    }
}
