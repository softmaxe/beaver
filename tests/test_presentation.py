from pathlib import Path

import pytest

from rename_subtitles.planning import RenameOp, SkippedRename, plan_directory
from rename_subtitles.presentation import (
    DEFAULT_MATCH_LEVEL,
    DEMO_FILES,
    MATCH_LEVELS,
    demo_plan,
    match_kind,
    match_level_score,
    skip_reason_code,
)


@pytest.mark.parametrize(
    ("reason", "expected"),
    [
        ("episode:S01E02", ("episode", "S01E02")),
        ("episode:2x01", ("episode", "2x01")),
        ("fuzzy:0.87", ("fuzzy", "0.87")),
    ],
)
def test_match_kind_splits_the_reason_string(reason: str, expected: tuple[str, str]):
    operation = RenameOp(src=Path("a.srt"), dst=Path("b.srt"), reason=reason)

    assert match_kind(operation) == expected


@pytest.mark.parametrize(
    ("reason", "expected"),
    [
        ("unmatched (best_score=0.41)", "unmatched"),
        ("unmatched (ambiguous episode:S01E01)", "unmatched"),
        ("already matches", "already_matching"),
        ("target collision in strict mode", "strict_collision"),
        ("no video files in this directory", "no_video"),
        ("target collision", "collision"),
    ],
)
def test_skip_reason_code_covers_every_reason_planning_produces(reason: str, expected: str):
    assert skip_reason_code(SkippedRename(path=Path("a.srt"), reason=reason)) == expected


def test_skip_reason_codes_match_what_a_real_plan_reports(tmp_path: Path):
    """Guards against planning changing a reason string out from under the UI."""
    (tmp_path / "Show.S01E01.mkv").touch()
    (tmp_path / "Show.S01E01.eng.srt").touch()
    (tmp_path / "Show.S01E01.chs.srt").touch()
    (tmp_path / "Unrelated.Feature.srt").touch()
    extras = tmp_path / "extras"
    extras.mkdir()
    (extras / "Bonus.srt").touch()

    plan = plan_directory(tmp_path, recursive=True, strict=True)
    codes = {skip_reason_code(item) for item in plan.skipped}

    assert codes <= {
        "unmatched",
        "already_matching",
        "strict_collision",
        "no_video",
        "collision",
    }
    assert {"strict_collision", "unmatched", "no_video"} <= codes


def test_match_levels_are_ordered_and_default_to_plannings_threshold():
    assert list(MATCH_LEVELS) == ["relaxed", "balanced", "cautious"]
    assert sorted(MATCH_LEVELS.values()) == list(MATCH_LEVELS.values())
    assert MATCH_LEVELS[DEFAULT_MATCH_LEVEL] == 0.72


def test_match_level_score_falls_back_to_the_default():
    assert match_level_score("cautious") == 0.84
    assert match_level_score("nonsense") == MATCH_LEVELS[DEFAULT_MATCH_LEVEL]


def test_demo_plan_never_touches_the_filesystem(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    monkeypatch.chdir(tmp_path)

    plan = demo_plan()

    assert plan.root == Path("/virtual-subtitle-library")
    assert plan.video_count == 3
    assert plan.subtitle_count == 4
    assert plan.matched_count == 3
    assert plan.skipped_count == 1
    assert list(tmp_path.iterdir()) == []
    assert len(DEMO_FILES) == 7
