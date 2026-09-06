//! Ratcliff/Obershelp string similarity.
//!
//! This is a faithful port of CPython's `difflib.SequenceMatcher.ratio`, which the
//! previous implementation of this tool relied on. Keeping the exact algorithm —
//! including the "autojunk" heuristic for long sequences — means a directory that
//! matched a certain way before still matches the same way now.
//!
//! The measure is asymmetric: junk detection only ever looks at `b`, so the
//! argument order matters. Everywhere in this crate `a` is the subtitle stem and
//! `b` is the video stem.
//!
//! Matching one subtitle against every video in a folder compares the same `b`
//! over and over, so the parts that depend only on `b` live in [`Target`], which
//! is built once per video. [`Scratch`] holds the working buffers, so a whole
//! folder is scored without allocating per comparison.

use std::collections::HashMap;

/// Similarity of two strings in `0.0..=1.0`, compared character by character.
///
/// Returns `0.0` when either side is empty: an empty stem carries no evidence, so
/// it should never be treated as a perfect match for another empty stem.
///
/// Convenience wrapper over [`Target`]; prefer that when `b` is reused.
pub fn ratio(a: &str, b: &str) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let target = Target::new(&b);
    let mut scratch = Scratch::default();
    target.ratio(&a, &mut scratch)
}

/// The `b` side of a comparison, indexed once so it can be scored against many
/// `a`s.
pub struct Target {
    b: Vec<char>,
    /// Positions of each element of `b`, concatenated; see [`Self::ranges`].
    positions: Vec<u32>,
    /// Where a character's ascending run of positions sits in [`Self::positions`].
    ///
    /// Characters dropped by the autojunk heuristic are absent.
    ranges: HashMap<char, (u32, u32)>,
    /// How often each character occurs, junk included. Bounds the match count.
    counts: HashMap<char, u32>,
}

impl Target {
    pub fn new(b: &[char]) -> Self {
        let b = b.to_vec();
        let mut counts: HashMap<char, u32> = HashMap::new();
        let mut occurrences: HashMap<char, Vec<u32>> = HashMap::new();
        for (index, element) in b.iter().enumerate() {
            *counts.entry(*element).or_default() += 1;
            occurrences.entry(*element).or_default().push(index as u32);
        }
        // Autojunk: in a long sequence, an element that shows up in more than 1%
        // of positions carries no signal and would dominate the search, so it is
        // dropped from the index.
        if b.len() >= 200 {
            let limit = b.len() / 100 + 1;
            occurrences.retain(|_, positions| positions.len() <= limit);
        }

        let mut positions = Vec::with_capacity(b.len());
        let mut ranges = HashMap::with_capacity(occurrences.len());
        for (element, element_positions) in occurrences {
            let start = positions.len() as u32;
            positions.extend_from_slice(&element_positions);
            ranges.insert(element, (start, positions.len() as u32));
        }
        Self {
            b,
            positions,
            ranges,
            counts,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.b.is_empty()
    }

    /// Similarity of `a` against this target, in `0.0..=1.0`.
    pub fn ratio(&self, a: &[char], scratch: &mut Scratch) -> f64 {
        if a.is_empty() || self.b.is_empty() {
            return 0.0;
        }
        let matched = self.total_matched(a, scratch);
        2.0 * matched as f64 / (a.len() + self.b.len()) as f64
    }

    /// The highest ratio `a` could possibly reach here, from character counts alone.
    ///
    /// Every matched character is one character of `a` paired with a distinct
    /// equal character of `b`, so the size of the multiset intersection caps the
    /// match count. Cheap to compute, and it lets a hopeless video be dropped
    /// without running the quadratic search.
    pub fn ratio_bound(&self, counts: &CharCounts) -> f64 {
        if counts.total == 0 || self.b.is_empty() {
            return 0.0;
        }
        let shared: u32 = if counts.counts.len() <= self.counts.len() {
            counts
                .counts
                .iter()
                .map(|(element, count)| (*count).min(self.count_of(*element)))
                .sum()
        } else {
            self.counts
                .iter()
                .map(|(element, count)| (*count).min(counts.count_of(*element)))
                .sum()
        };
        2.0 * shared as f64 / (counts.total + self.b.len() as u32) as f64
    }

    fn count_of(&self, element: char) -> u32 {
        self.counts.get(&element).copied().unwrap_or(0)
    }

    /// Sum of the lengths of every matching block, which is all `ratio` needs.
    fn total_matched(&self, a: &[char], scratch: &mut Scratch) -> usize {
        scratch.prepare(a, self);

        let mut total = 0;
        scratch.queue.clear();
        scratch.queue.push((0, a.len(), 0, self.b.len()));
        while let Some((a_low, a_high, b_low, b_high)) = scratch.queue.pop() {
            let (i, j, size) = self.longest_match(a, scratch, a_low, a_high, b_low, b_high);
            if size == 0 {
                continue;
            }
            total += size;
            if a_low < i && b_low < j {
                scratch.queue.push((a_low, i, b_low, j));
            }
            if i + size < a_high && j + size < b_high {
                scratch.queue.push((i + size, a_high, j + size, b_high));
            }
        }
        total
    }

    /// The longest common block within the given windows, as `(a_start, b_start, len)`.
    ///
    /// Ties go to the earliest block in `a`, and then to the earliest in `b`.
    fn longest_match(
        &self,
        a: &[char],
        scratch: &mut Scratch,
        a_low: usize,
        a_high: usize,
        b_low: usize,
        b_high: usize,
    ) -> (usize, usize, usize) {
        let (mut best_i, mut best_j, mut best_size) = (a_low, b_low, 0usize);
        let Scratch {
            a_ranges,
            previous,
            current,
            previous_written,
            current_written,
            ..
        } = scratch;

        for (i, &(start, end)) in a_ranges.iter().enumerate().take(a_high).skip(a_low) {
            // `current` still holds the run lengths from two rows back; only the
            // positions it wrote need clearing, which keeps this off the
            // quadratic path.
            for &j in current_written.iter() {
                current[j as usize] = 0;
            }
            current_written.clear();

            for &j in &self.positions[start as usize..end as usize] {
                let j = j as usize;
                if j < b_low {
                    continue;
                }
                if j >= b_high {
                    break;
                }
                let length = if j > 0 { previous[j - 1] } else { 0 } + 1;
                current[j] = length;
                current_written.push(j as u32);
                if length as usize > best_size {
                    best_size = length as usize;
                    best_i = i + 1 - best_size;
                    best_j = j + 1 - best_size;
                }
            }
            std::mem::swap(previous, current);
            std::mem::swap(previous_written, current_written);
        }

        for &j in previous_written.iter() {
            previous[j as usize] = 0;
        }
        previous_written.clear();
        for &j in current_written.iter() {
            current[j as usize] = 0;
        }
        current_written.clear();

        // Grow the block over elements that were dropped from the index above.
        while best_i > a_low && best_j > b_low && a[best_i - 1] == self.b[best_j - 1] {
            best_i -= 1;
            best_j -= 1;
            best_size += 1;
        }
        while best_i + best_size < a_high
            && best_j + best_size < b_high
            && a[best_i + best_size] == self.b[best_j + best_size]
        {
            best_size += 1;
        }

        (best_i, best_j, best_size)
    }
}

/// How often each character occurs in an `a`, for [`Target::ratio_bound`].
#[derive(Default)]
pub struct CharCounts {
    counts: HashMap<char, u32>,
    total: u32,
}

impl CharCounts {
    pub fn new(a: &[char]) -> Self {
        let mut counts: HashMap<char, u32> = HashMap::with_capacity(a.len());
        for element in a {
            *counts.entry(*element).or_default() += 1;
        }
        Self {
            counts,
            total: a.len() as u32,
        }
    }

    fn count_of(&self, element: char) -> u32 {
        self.counts.get(&element).copied().unwrap_or(0)
    }
}

/// Working buffers reused across comparisons.
#[derive(Default)]
pub struct Scratch {
    /// For each element of `a`, its slice of `Target::positions`.
    a_ranges: Vec<(u32, u32)>,
    /// Length of the run ending at each position of `b`, for the previous row.
    previous: Vec<u32>,
    /// The same for the row being filled.
    current: Vec<u32>,
    /// Which positions each row touched, so clearing stays proportional to them.
    previous_written: Vec<u32>,
    current_written: Vec<u32>,
    queue: Vec<(usize, usize, usize, usize)>,
}

impl Scratch {
    fn prepare(&mut self, a: &[char], target: &Target) {
        self.a_ranges.clear();
        self.a_ranges.extend(
            a.iter()
                // An empty range stands for "not in the index": either absent
                // from `b`, or dropped as junk.
                .map(|element| target.ranges.get(element).copied().unwrap_or((0, 0))),
        );
        // Rows are cleared as they are reused, so these only ever grow.
        if self.previous.len() < target.b.len() {
            self.previous.resize(target.b.len(), 0);
            self.current.resize(target.b.len(), 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ratio;

    fn close(left: f64, right: f64) -> bool {
        (left - right).abs() < 1e-9
    }

    #[test]
    fn identical_strings_score_one() {
        assert!(close(ratio("abcdef", "abcdef"), 1.0));
    }

    #[test]
    fn empty_input_scores_zero() {
        assert!(close(ratio("", "abc"), 0.0));
        assert!(close(ratio("abc", ""), 0.0));
        assert!(close(ratio("", ""), 0.0));
    }

    #[test]
    fn disjoint_strings_score_zero() {
        assert!(close(ratio("abc", "xyz"), 0.0));
    }

    /// Values taken from CPython's `difflib.SequenceMatcher(a=..., b=...).ratio()`.
    #[test]
    fn matches_cpython_difflib() {
        assert!(close(ratio("abcd", "bcde"), 0.75));
        assert!(close(ratio("private", "prwatte"), 0.7142857142857143));
        assert!(close(ratio("tide", "diet"), 0.25));
        assert!(close(
            ratio("nebulaarchives01e01", "nebulaarchives01e02"),
            0.9473684210526315
        ));
        assert!(close(ratio("kitten", "sitting"), 0.6153846153846154));
    }

    #[test]
    fn compares_by_character_not_byte() {
        assert!(close(
            ratio("星际档案第一集", "星际档案第二集"),
            0.8571428571428571
        ));
    }

    #[test]
    fn long_sequences_use_the_autojunk_heuristic() {
        // Past 200 elements, difflib drops elements that fill more than 1% of the
        // positions. Here every character of `b` qualifies, so the index is empty
        // and the score collapses to zero instead of finding the shared "ab".
        let b = "ab".repeat(110);
        assert!(close(ratio("bab", &b), 0.0));
        // The same comparison below the 200-element cutoff keeps its match.
        let short = "ab".repeat(90);
        assert!(ratio("bab", &short) > 0.0);
    }

    /// The cheap upper bound must never sit below the real score, or a video
    /// that should have won would be dropped without being compared.
    #[test]
    fn the_bound_is_never_below_the_real_ratio() {
        use super::{CharCounts, Scratch, Target};
        let pairs = [
            ("nebulaarchives01e01", "nebulaarchives01e02"),
            ("deepfieldreport", "deepfieldreport"),
            ("abc", "xyz"),
            ("星际档案第一集", "星际档案第二集"),
            ("bab", &"ab".repeat(110)),
        ];
        let mut scratch = Scratch::default();
        for (a, b) in pairs {
            let a: Vec<char> = a.chars().collect();
            let target = Target::new(&b.chars().collect::<Vec<_>>());
            let real = target.ratio(&a, &mut scratch);
            let bound = target.ratio_bound(&CharCounts::new(&a));
            assert!(bound >= real - 1e-9, "{bound} < {real}");
        }
    }
}
