//! The vocabulary the interfaces speak: match levels, human wording, demo data.
//!
//! [`crate::planning`] deals in scores and enums. Everything here turns those into
//! the words a person reads, so the CLI and the TUI never disagree about what a
//! result is called.

use clap::ValueEnum;

use crate::planning::{plan_virtual_files, MatchReason, PlanOptions, RenamePlan, SkipReason};

/// Pick the singular or plural wording for `count`.
pub fn plural<'a>(count: usize, one: &'a str, many: &'a str) -> &'a str {
    if count == 1 {
        one
    } else {
        many
    }
}

/// How eager the fuzzy matcher should be.
///
/// A named level rather than a raw threshold: `0.72` means nothing to a person,
/// "balanced" does. Episode-id matches ignore the level entirely.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum MatchLevel {
    Relaxed,
    #[default]
    Balanced,
    Cautious,
}

impl MatchLevel {
    pub const ALL: [Self; 3] = [Self::Relaxed, Self::Balanced, Self::Cautious];

    pub fn score(self) -> f64 {
        match self {
            Self::Relaxed => crate::planning::RELAXED_MIN_SCORE,
            Self::Balanced => crate::planning::BALANCED_MIN_SCORE,
            Self::Cautious => crate::planning::CAUTIOUS_MIN_SCORE,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Relaxed => "Relaxed",
            Self::Balanced => "Balanced",
            Self::Cautious => "Cautious",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Relaxed => "Matches more, for messy naming",
            Self::Balanced => "Recommended, balanced coverage",
            Self::Cautious => "Only near-certain matches",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|level| *level == self)
            .unwrap_or(1)
    }

    pub fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or_default()
    }
}

/// A self-contained sample library for the demo mode.
///
/// These names cover what the interface has to report: episode matches, language
/// tags that get dropped on the way to the target name, and one subtitle that
/// matches nothing at all.
pub const DEMO_FILES: &[&str] = &[
    "Nebula.Archive.S01E01.2160p.WEB-DL.mkv",
    "Nebula.Archive.S01E02.2160p.WEB-DL.mkv",
    "Nebula.Archive.S01E03.2160p.WEB-DL.mkv",
    "Nebula.Archive.S01E01.zh-en.srt",
    "Nebula.Archive.S01E02.chs.ass",
    "Nebula.Archive.S01E03.eng.srt",
    "Unsorted.Bonus.Feature.srt",
];

/// Build the demo plan without touching the filesystem.
pub fn demo_plan() -> RenamePlan {
    plan_virtual_files(DEMO_FILES, &PlanOptions::default())
}

/// The short badge that explains a match, shown beside every proposed rename.
pub fn match_badge(reason: &MatchReason) -> String {
    match reason {
        MatchReason::Episode(key) => format!("episode {key}"),
        MatchReason::Fuzzy(score) => format!("fuzzy {score:.2}"),
    }
}

/// Why a subtitle was left alone, in one short phrase.
pub fn skip_label(reason: &SkipReason) -> String {
    match reason {
        SkipReason::Unmatched { best_score } => {
            format!("No matching video (best {best_score:.2})")
        }
        SkipReason::NoMatchingEpisode(key) => format!("No video with episode {key}"),
        SkipReason::AmbiguousEpisode(key) => format!("Two videos claim {key}"),
        SkipReason::AlreadyMatches => "Filename already matches".into(),
        SkipReason::StrictCollision => "Target name taken (strict mode)".into(),
        SkipReason::NoVideo => "No video files in this folder".into(),
        SkipReason::Collision => "Target name taken".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_levels_get_stricter_in_order() {
        assert!(MatchLevel::Relaxed.score() < MatchLevel::Balanced.score());
        assert!(MatchLevel::Balanced.score() < MatchLevel::Cautious.score());
        assert_eq!(MatchLevel::default(), MatchLevel::Balanced);
    }

    #[test]
    fn levels_round_trip_through_their_index() {
        for level in MatchLevel::ALL {
            assert_eq!(MatchLevel::from_index(level.index()), level);
        }
        assert_eq!(MatchLevel::from_index(99), MatchLevel::default());
    }

    #[test]
    fn the_demo_plan_shows_every_kind_of_outcome() {
        let plan = demo_plan();
        assert_eq!(plan.video_count, 3);
        assert_eq!(plan.subtitle_count, 4);
        assert_eq!(plan.operations.len(), 3);
        assert!(plan
            .operations
            .iter()
            .all(|operation| matches!(operation.reason, MatchReason::Episode(_))));
        assert_eq!(plan.skipped.len(), 1);
        assert!(matches!(
            plan.skipped[0].reason,
            SkipReason::Unmatched { .. }
        ));
    }

    #[test]
    fn badges_read_as_words() {
        assert_eq!(
            match_badge(&MatchReason::Episode("S01E02".into())),
            "episode S01E02"
        );
        assert_eq!(match_badge(&MatchReason::Fuzzy(0.8765)), "fuzzy 0.88");
        assert_eq!(
            skip_label(&SkipReason::NoMatchingEpisode("S01E02".into())),
            "No video with episode S01E02"
        );
        assert_eq!(
            skip_label(&SkipReason::AlreadyMatches),
            "Filename already matches"
        );
    }
}
