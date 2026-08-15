from pathlib import Path

import pytest

from rename_subtitles.applying import (
    FileState,
    PlanChangedError,
    apply_operations,
    detect_state_changes,
    display_path,
    prepare_operations,
)
from rename_subtitles.planning import plan_directory


def _library(root: Path, episodes: int = 2) -> None:
    for episode in range(1, episodes + 1):
        (root / f"Show.S01E0{episode}.1080p.mkv").write_text("video")
        (root / f"Show.S01E0{episode}.chs.srt").write_text("subtitle")


def test_prepare_operations_gives_every_operation_a_unique_id(tmp_path: Path):
    _library(tmp_path, episodes=3)

    prepared = prepare_operations(plan_directory(tmp_path))

    assert len(prepared) == 3
    assert len({item.operation_id for item in prepared}) == 3
    assert all(item.source_state.exists for item in prepared)
    assert not any(item.destination_state.exists for item in prepared)


def test_detect_state_changes_notices_a_replaced_source(tmp_path: Path):
    _library(tmp_path, episodes=1)
    prepared = prepare_operations(plan_directory(tmp_path))

    (tmp_path / "Show.S01E01.chs.srt").write_text("a different subtitle entirely")

    assert detect_state_changes(prepared) == ("source changed: Show.S01E01.chs.srt",)


def test_detect_state_changes_notices_a_claimed_target(tmp_path: Path):
    _library(tmp_path, episodes=1)
    prepared = prepare_operations(plan_directory(tmp_path))

    prepared[0].destination.write_text("someone got there first")

    assert detect_state_changes(prepared) == ("target changed: Show.S01E01.1080p.srt",)


def test_detect_state_changes_is_quiet_when_nothing_moved(tmp_path: Path):
    _library(tmp_path, episodes=2)

    assert detect_state_changes(prepare_operations(plan_directory(tmp_path))) == ()


def test_apply_operations_renames_and_reports_relative_paths(tmp_path: Path):
    _library(tmp_path, episodes=2)
    prepared = prepare_operations(plan_directory(tmp_path))

    result = apply_operations(prepared, tmp_path)

    assert result.status == "completed"
    assert not result.failed
    assert [outcome.target for outcome in result.applied] == [
        "Show.S01E01.1080p.srt",
        "Show.S01E02.1080p.srt",
    ]
    assert (tmp_path / "Show.S01E01.1080p.srt").read_text() == "subtitle"
    assert not (tmp_path / "Show.S01E01.chs.srt").exists()


def test_verified_apply_aborts_without_touching_anything(tmp_path: Path):
    _library(tmp_path, episodes=2)
    prepared = prepare_operations(plan_directory(tmp_path))
    (tmp_path / "Show.S01E01.chs.srt").unlink()

    with pytest.raises(PlanChangedError) as error:
        apply_operations(prepared, tmp_path, verify=True)

    assert error.value.changes == ("source changed: Show.S01E01.chs.srt",)
    # The second operation was perfectly applicable and must still be untouched.
    assert (tmp_path / "Show.S01E02.chs.srt").exists()
    assert not (tmp_path / "Show.S01E02.1080p.srt").exists()


def test_unverified_apply_proceeds_with_a_stale_plan(tmp_path: Path):
    _library(tmp_path, episodes=2)
    prepared = prepare_operations(plan_directory(tmp_path))
    (tmp_path / "Show.S01E01.chs.srt").unlink()

    result = apply_operations(prepared, tmp_path, verify=False)

    assert result.status == "partial"
    assert [outcome.source for outcome in result.applied] == ["Show.S01E02.chs.srt"]
    assert [outcome.source for outcome in result.failed] == ["Show.S01E01.chs.srt"]
    assert result.failed[0].error


def test_apply_refuses_to_overwrite_an_existing_target(tmp_path: Path):
    _library(tmp_path, episodes=1)
    prepared = prepare_operations(plan_directory(tmp_path))
    prepared[0].destination.write_text("precious")

    result = apply_operations(prepared, tmp_path, verify=False)

    assert result.status == "failed"
    assert (tmp_path / "Show.S01E01.1080p.srt").read_text() == "precious"
    assert (tmp_path / "Show.S01E01.chs.srt").exists()


def test_force_overwrites_the_existing_target(tmp_path: Path):
    _library(tmp_path, episodes=1)
    prepared = prepare_operations(plan_directory(tmp_path))
    prepared[0].destination.write_text("precious")

    result = apply_operations(prepared, tmp_path, force=True, verify=False)

    assert result.status == "completed"
    assert (tmp_path / "Show.S01E01.1080p.srt").read_text() == "subtitle"


def test_file_state_of_a_missing_path_is_all_empty(tmp_path: Path):
    state = FileState.capture(tmp_path / "nowhere.srt")

    assert state == FileState(False, False, None, None, None, None)


def test_display_path_falls_back_to_the_absolute_path(tmp_path: Path):
    inside = tmp_path / "season" / "one.srt"

    assert display_path(inside, tmp_path) == "season/one.srt"
    assert display_path(Path("/elsewhere/one.srt"), tmp_path) == "/elsewhere/one.srt"
