//! Turning a filename into something comparable.
//!
//! Release names carry a lot of text that says nothing about *which* episode a
//! file holds — resolution, codec, release group, language. This module strips
//! that away, and pulls out the two identifiers matching actually relies on: an
//! `SxxEyy` episode id, and a trailing language tag.

use unicode_normalization::UnicodeNormalization;

/// Language markers that may trail a subtitle stem, sorted for binary search.
///
/// Only alphanumeric runs are ever looked up here, so hyphenated forms such as
/// `zh-en` are absent by design: [`language_tag`] recognises those separately.
const LANGUAGE_TAGS: &[&str] = &[
    "chi", "chinese", "chs", "cht", "cn", "en", "eng", "engchs", "english", "ja", "japanese", "jp",
    "jpn", "ko", "kor", "korean", "zh", "zho",
];

/// Release metadata that never helps tell two files apart, sorted for binary search.
const JUNK_TOKENS: &[&str] = &[
    "1080p", "2160p", "4k", "720p", "aac", "bdrip", "bluray", "dl", "dts", "dvdrip", "extended",
    "h264", "h265", "hdr", "hdrip", "hevc", "proper", "rarbg", "remux", "repack", "sdr", "web",
    "webdl", "webrip", "x264", "x265", "yify",
];

/// The two halves of a combined language tag such as `chs-eng`, in match order.
const LANGUAGE_PAIR_PARTS: &[&str] = &[
    "chs", "cht", "eng", "zho", "chi", "zh", "en", "ja", "jpn", "ko", "kor",
];

fn is_language_tag(token: &str) -> bool {
    LANGUAGE_TAGS.binary_search(&token).is_ok()
}

fn is_junk_token(token: &str) -> bool {
    JUNK_TOKENS.binary_search(&token).is_ok()
}

/// Split `text` into runs of alphanumeric characters, after NFC normalisation.
///
/// Normalising first means a macOS-decomposed filename and a composed one produce
/// the same tokens.
pub fn split_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in text.nfc() {
        if character.is_alphanumeric() {
            current.push(character);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Reduce a filename stem to the part worth comparing.
///
/// Bracketed groups go first, then junk tokens, then any trailing language tags;
/// what is left is joined without separators, so `.` `_` and ` ` stop mattering.
pub fn normalize_stem(stem: &str) -> String {
    let without_brackets = strip_bracketed_groups(stem);
    let mut tokens: Vec<String> = split_tokens(&without_brackets)
        .iter()
        .map(|token| token.to_lowercase())
        .filter(|token| !is_junk_token(token))
        .collect();
    while tokens.last().is_some_and(|token| is_language_tag(token)) {
        tokens.pop();
    }
    tokens.concat()
}

/// Replace every `[...]`, `(...)` or `{...}` group with a space.
///
/// An unclosed opener is left alone rather than swallowing the rest of the name.
fn strip_bracketed_groups(stem: &str) -> String {
    let characters: Vec<char> = stem.chars().collect();
    let mut result = String::new();
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        if matches!(character, '[' | '(' | '{') {
            if let Some(offset) = characters[index + 1..]
                .iter()
                .position(|candidate| matches!(candidate, ']' | ')' | '}'))
            {
                result.push(' ');
                index += offset + 2;
                continue;
            }
        }
        result.push(character);
        index += 1;
    }
    result
}

/// Extract a season/episode identifier from a filename, as `S01E02`.
///
/// Recognises `S01E02` (with any of `-`, `_`, `.` or spaces between the parts)
/// and `1x02`. Both forms must stand alone: neither may be glued to surrounding
/// letters or digits, so `MYS01E02X` is not an episode id.
pub fn episode_key(name: &str) -> Option<String> {
    let characters: Vec<char> = name.chars().collect();
    season_episode_form(&characters).or_else(|| cross_form(&characters))
}

/// `S01E02` and friends.
fn season_episode_form(characters: &[char]) -> Option<String> {
    for start in 0..characters.len() {
        if !starts_on_boundary(characters, start) {
            continue;
        }
        if !matches!(characters[start], 's' | 'S') {
            continue;
        }
        if let Some(key) = match_episode_tail(characters, start + 1) {
            return Some(key);
        }
    }
    None
}

fn match_episode_tail(characters: &[char], after_marker: usize) -> Option<String> {
    // Digit runs are greedy but may give a digit back, exactly as a regex engine
    // would: `S123E45` matches nothing rather than reading `S12` and stopping.
    for season_length in [2, 1] {
        let Some(season) = read_digits(characters, after_marker, season_length) else {
            continue;
        };
        let mut position = skip_separators(characters, after_marker + season_length);
        if !matches!(characters.get(position), Some('e' | 'E')) {
            continue;
        }
        position += 1;
        for episode_length in [2, 1] {
            let Some(episode) = read_digits(characters, position, episode_length) else {
                continue;
            };
            if ends_on_boundary(characters, position + episode_length) {
                return Some(format_key(season, episode));
            }
        }
    }
    None
}

/// `1x02` and friends.
fn cross_form(characters: &[char]) -> Option<String> {
    for start in 0..characters.len() {
        if !starts_on_boundary(characters, start) {
            continue;
        }
        for season_length in [2, 1] {
            let Some(season) = read_digits(characters, start, season_length) else {
                continue;
            };
            let mut position = skip_separators(characters, start + season_length);
            if !matches!(characters.get(position), Some('x' | 'X')) {
                continue;
            }
            position = skip_separators(characters, position + 1);
            for episode_length in [2, 1] {
                let Some(episode) = read_digits(characters, position, episode_length) else {
                    continue;
                };
                if ends_on_boundary(characters, position + episode_length) {
                    return Some(format_key(season, episode));
                }
            }
        }
    }
    None
}

fn format_key(season: u32, episode: u32) -> String {
    format!("S{season:02}E{episode:02}")
}

fn starts_on_boundary(characters: &[char], index: usize) -> bool {
    index == 0 || !characters[index - 1].is_ascii_alphanumeric()
}

fn ends_on_boundary(characters: &[char], index: usize) -> bool {
    characters
        .get(index)
        .is_none_or(|character| !character.is_ascii_alphanumeric())
}

fn read_digits(characters: &[char], start: usize, length: usize) -> Option<u32> {
    let slice = characters.get(start..start + length)?;
    if !slice.iter().all(char::is_ascii_digit) {
        return None;
    }
    slice.iter().collect::<String>().parse().ok()
}

fn skip_separators(characters: &[char], mut index: usize) -> usize {
    while characters
        .get(index)
        .is_some_and(|character| matches!(character, '-' | '_' | '.') || character.is_whitespace())
    {
        index += 1;
    }
    index
}

/// Find the language a subtitle stem announces, as a lowercase tag.
///
/// A combined tag such as `zh-en` wins wherever it appears and is returned with
/// its separator removed (`zhen`); otherwise the last recognisable tag near the
/// end of the stem is used. Used only to keep two subtitles for the same video
/// apart, so a miss simply falls back to a numeric suffix.
pub fn language_tag(stem: &str) -> Option<String> {
    let characters: Vec<char> = stem.chars().collect();
    if let Some(pair) = language_pair(&characters) {
        return Some(pair);
    }
    let tokens: Vec<String> = split_tokens(stem)
        .iter()
        .map(|token| token.to_lowercase())
        .collect();
    tokens
        .iter()
        .rev()
        .take(8)
        .find(|token| is_language_tag(token))
        .cloned()
}

fn language_pair(characters: &[char]) -> Option<String> {
    for start in 0..characters.len() {
        if !starts_on_boundary(characters, start) {
            continue;
        }
        for first in LANGUAGE_PAIR_PARTS {
            let Some(after_first) = match_word(characters, start, first) else {
                continue;
            };
            if !matches!(characters.get(after_first), Some('-' | '_' | '.')) {
                continue;
            }
            for second in LANGUAGE_PAIR_PARTS {
                let Some(after_second) = match_word(characters, after_first + 1, second) else {
                    continue;
                };
                if ends_on_boundary(characters, after_second) {
                    return Some(format!("{first}{second}"));
                }
            }
        }
    }
    None
}

/// Match `word` case-insensitively at `start`, returning the position after it.
fn match_word(characters: &[char], start: usize, word: &str) -> Option<usize> {
    let mut index = start;
    for expected in word.chars() {
        let actual = characters.get(index)?;
        if !actual.eq_ignore_ascii_case(&expected) {
            return None;
        }
        index += 1;
    }
    Some(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_tables_are_sorted_for_binary_search() {
        assert!(LANGUAGE_TAGS.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(JUNK_TOKENS.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn splits_on_anything_that_is_not_alphanumeric() {
        assert_eq!(split_tokens("Show.Name_01-x"), ["Show", "Name", "01", "x"]);
        assert_eq!(split_tokens("星际档案 S01"), ["星际档案", "S01"]);
        assert!(split_tokens("...").is_empty());
    }

    #[test]
    fn normalize_drops_brackets_junk_and_trailing_language() {
        assert_eq!(
            normalize_stem("[Group] Nebula.Archive.S01E02.1080p.WEB-DL.x265.chs"),
            "nebulaarchives01e02"
        );
        assert_eq!(normalize_stem("Nebula Archive - 03"), "nebulaarchive03");
    }

    #[test]
    fn normalize_keeps_a_language_word_that_is_not_at_the_end() {
        assert_eq!(
            normalize_stem("English.Patient.S01E01"),
            "englishpatients01e01"
        );
    }

    #[test]
    fn normalize_leaves_an_unclosed_bracket_alone() {
        assert_eq!(
            normalize_stem("Nebula [Archive S01E01"),
            "nebulaarchives01e01"
        );
    }

    #[test]
    fn finds_episode_ids_in_both_forms() {
        assert_eq!(episode_key("Show.S01E02.mkv").as_deref(), Some("S01E02"));
        assert_eq!(episode_key("Show S1 E2.mkv").as_deref(), Some("S01E02"));
        assert_eq!(episode_key("Show.s01.e02.mkv").as_deref(), Some("S01E02"));
        assert_eq!(episode_key("Show.2x01.mkv").as_deref(), Some("S02E01"));
        assert_eq!(episode_key("Show.10x11.mkv").as_deref(), Some("S10E11"));
    }

    #[test]
    fn rejects_episode_ids_glued_to_other_text() {
        assert_eq!(episode_key("MYS01E02X.mkv"), None);
        assert_eq!(episode_key("Show.S123E45.mkv"), None);
        assert_eq!(episode_key("Show.S01E123.mkv"), None);
        assert_eq!(episode_key("Show.1080p.mkv"), None);
    }

    #[test]
    fn takes_the_leftmost_episode_id() {
        assert_eq!(
            episode_key("S01E01.rip.S02E02.mkv").as_deref(),
            Some("S01E01")
        );
    }

    #[test]
    fn reads_combined_and_single_language_tags() {
        assert_eq!(language_tag("Show.S01E01.zh-en").as_deref(), Some("zhen"));
        assert_eq!(
            language_tag("Show.S01E01.CHS_ENG").as_deref(),
            Some("chseng")
        );
        assert_eq!(language_tag("Show.S01E01.jpn").as_deref(), Some("jpn"));
        assert!(language_tag("Show.S01E01").is_none());
    }

    #[test]
    fn ignores_a_language_tag_buried_far_from_the_end() {
        let stem = "en.a.b.c.d.e.f.g.h.i";
        assert_eq!(language_tag(stem), None);
    }
}
