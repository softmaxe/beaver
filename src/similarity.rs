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

use std::collections::HashMap;

/// Similarity of two strings in `0.0..=1.0`, compared character by character.
///
/// Returns `0.0` when either side is empty: an empty stem carries no evidence, so
/// it should never be treated as a perfect match for another empty stem.
pub fn ratio(a: &str, b: &str) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let matches = SequenceMatcher::new(&a, &b).total_matched();
    2.0 * matches as f64 / (a.len() + b.len()) as f64
}

struct SequenceMatcher<'a> {
    a: &'a [char],
    b: &'a [char],
    /// Where each element of `b` occurs, with over-frequent elements removed.
    b2j: HashMap<char, Vec<usize>>,
}

impl<'a> SequenceMatcher<'a> {
    fn new(a: &'a [char], b: &'a [char]) -> Self {
        let mut b2j: HashMap<char, Vec<usize>> = HashMap::new();
        for (index, element) in b.iter().enumerate() {
            b2j.entry(*element).or_default().push(index);
        }
        // Autojunk: in a long sequence, an element that shows up in more than 1%
        // of positions carries no signal and would dominate the search, so it is
        // dropped from the index.
        if b.len() >= 200 {
            let limit = b.len() / 100 + 1;
            b2j.retain(|_, positions| positions.len() <= limit);
        }
        Self { a, b, b2j }
    }

    /// Sum of the lengths of every matching block, which is all `ratio` needs.
    fn total_matched(&self) -> usize {
        let mut total = 0;
        let mut queue = vec![(0, self.a.len(), 0, self.b.len())];
        while let Some((a_low, a_high, b_low, b_high)) = queue.pop() {
            let (i, j, size) = self.longest_match(a_low, a_high, b_low, b_high);
            if size == 0 {
                continue;
            }
            total += size;
            if a_low < i && b_low < j {
                queue.push((a_low, i, b_low, j));
            }
            if i + size < a_high && j + size < b_high {
                queue.push((i + size, a_high, j + size, b_high));
            }
        }
        total
    }

    /// The longest common block within the given windows, as `(a_start, b_start, len)`.
    ///
    /// Ties go to the earliest block in `a`, and then to the earliest in `b`.
    fn longest_match(
        &self,
        a_low: usize,
        a_high: usize,
        b_low: usize,
        b_high: usize,
    ) -> (usize, usize, usize) {
        let (mut best_i, mut best_j, mut best_size) = (a_low, b_low, 0usize);
        // Length of the run ending at each position of `b`, for the previous `i`.
        let mut run_lengths: HashMap<usize, usize> = HashMap::new();

        for i in a_low..a_high {
            let mut next_run_lengths: HashMap<usize, usize> = HashMap::new();
            if let Some(positions) = self.b2j.get(&self.a[i]) {
                for &j in positions {
                    if j < b_low {
                        continue;
                    }
                    if j >= b_high {
                        break;
                    }
                    let length = j
                        .checked_sub(1)
                        .and_then(|previous| run_lengths.get(&previous).copied())
                        .unwrap_or(0)
                        + 1;
                    next_run_lengths.insert(j, length);
                    if length > best_size {
                        best_i = i + 1 - length;
                        best_j = j + 1 - length;
                        best_size = length;
                    }
                }
            }
            run_lengths = next_run_lengths;
        }

        // Grow the block over elements that were dropped from the index above.
        while best_i > a_low && best_j > b_low && self.a[best_i - 1] == self.b[best_j - 1] {
            best_i -= 1;
            best_j -= 1;
            best_size += 1;
        }
        while best_i + best_size < a_high
            && best_j + best_size < b_high
            && self.a[best_i + best_size] == self.b[best_j + best_size]
        {
            best_size += 1;
        }

        (best_i, best_j, best_size)
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
}
