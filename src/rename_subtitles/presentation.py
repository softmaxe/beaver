"""UI-neutral vocabulary shared by every front-end.

``planning`` describes its decisions with free-form ``reason`` strings. Rather than
letting each front-end re-parse those strings, this module turns them into stable
codes, and holds the named match levels and demo dataset that the UI presents.
"""

from __future__ import annotations

from .planning import RenameOp, RenamePlan, SkippedRename, plan_virtual_files

__all__ = [
    "DEFAULT_MATCH_LEVEL",
    "DEMO_FILES",
    "MATCH_LEVELS",
    "demo_plan",
    "match_kind",
    "match_level_score",
    "skip_reason_code",
]

# Named levels instead of a raw threshold: the number means nothing to a user,
# but "cautious" does. The middle value is planning's own default.
MATCH_LEVELS: dict[str, float] = {
    "relaxed": 0.6,
    "balanced": 0.72,
    "cautious": 0.84,
}
DEFAULT_MATCH_LEVEL = "balanced"

# A self-contained sample library for the demo mode. These names exercise every
# outcome: episode matches, a language-tagged collision, and one unmatched file.
DEMO_FILES: tuple[str, ...] = (
    "Nebula.Archive.S01E01.2160p.WEB-DL.mkv",
    "Nebula.Archive.S01E02.2160p.WEB-DL.mkv",
    "Nebula.Archive.S01E03.2160p.WEB-DL.mkv",
    "Nebula.Archive.S01E01.zh-en.srt",
    "Nebula.Archive.S01E02.chs.ass",
    "Nebula.Archive.S01E03.eng.srt",
    "Unsorted.Bonus.Feature.srt",
)


def match_level_score(level: str) -> float:
    """Translate a named match level into planning's ``min_score`` threshold."""
    return MATCH_LEVELS.get(level, MATCH_LEVELS[DEFAULT_MATCH_LEVEL])


def match_kind(operation: RenameOp) -> tuple[str, str]:
    """Split a matched operation's reason into ``(kind, detail)``.

    ``("episode", "S01E02")`` for an episode-id match, ``("fuzzy", "0.87")`` otherwise.
    """
    if operation.reason.startswith("episode:"):
        return "episode", operation.reason.removeprefix("episode:")
    return "fuzzy", operation.reason.removeprefix("fuzzy:")


def skip_reason_code(skipped: SkippedRename) -> str:
    """Map a skip reason onto one of the five codes the UI knows how to label."""
    if skipped.reason.startswith("unmatched"):
        return "unmatched"
    if skipped.reason == "already matches":
        return "already_matching"
    if skipped.reason == "target collision in strict mode":
        return "strict_collision"
    if skipped.reason == "no video files in this directory":
        return "no_video"
    return "collision"


def demo_plan() -> RenamePlan:
    """Build the demo plan from :data:`DEMO_FILES` without touching the filesystem."""
    return plan_virtual_files(DEMO_FILES)
