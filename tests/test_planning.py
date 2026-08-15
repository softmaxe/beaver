from pathlib import Path

from rename_subtitles.planning import plan_directory, plan_virtual_files


def test_plan_directory_collects_operations_and_skips_orphans(tmp_path: Path):
    (tmp_path / "Show.S01E01.1080p.mkv").touch()
    (tmp_path / "Show.S01E01.eng.srt").touch()
    extras = tmp_path / "extras"
    extras.mkdir()
    (extras / "Behind.The.Scenes.srt").touch()

    plan = plan_directory(tmp_path, recursive=True)

    assert plan.video_count == 1
    assert plan.subtitle_count == 2
    assert plan.matched_count == 1
    assert plan.operations[0].dst.name == "Show.S01E01.1080p.srt"
    assert plan.skipped_count == 1
    assert plan.skipped[0].path == extras / "Behind.The.Scenes.srt"
    assert plan.skipped[0].reason == "no video files in this directory"


def test_plan_directory_reports_strict_collision(tmp_path: Path):
    (tmp_path / "Movie.S01E01.mkv").touch()
    (tmp_path / "Movie.S01E01.eng.srt").touch()
    (tmp_path / "Movie.S01E01.chs.srt").touch()

    plan = plan_directory(tmp_path, strict=True)

    assert plan.matched_count == 1
    assert plan.skipped_count == 1
    assert plan.skipped[0].reason == "target collision in strict mode"


def test_plan_virtual_files_uses_in_memory_paths_only():
    plan = plan_virtual_files(
        [
            "Season 1/Nebula.S01E01.2160p.mkv",
            "Season 1/Nebula.S01E01.zh-en.srt",
            "Season 1/Interview.Compilation.srt",
        ]
    )

    assert plan.root == Path("/virtual-subtitle-library")
    assert plan.video_count == 1
    assert plan.subtitle_count == 2
    assert plan.matched_count == 1
    assert plan.operations[0].src.name == "Nebula.S01E01.zh-en.srt"
    assert plan.operations[0].dst.name == "Nebula.S01E01.2160p.srt"
    assert plan.skipped_count == 1
    assert plan.skipped[0].reason.startswith("unmatched")


def test_plan_directory_matches_cjk_title_without_episode_number(tmp_path: Path):
    (tmp_path / "琅琊榜.1080p.WEB-DL.mkv").touch()
    (tmp_path / "琅琊榜.1080p.WEB-DL.chs.srt").touch()

    plan = plan_directory(tmp_path)

    assert plan.matched_count == 1
    assert plan.operations[0].dst.name == "琅琊榜.1080p.WEB-DL.srt"
    assert plan.operations[0].reason.startswith("fuzzy:")


def test_plan_directory_does_not_misidentify_distinct_cjk_titles(tmp_path: Path):
    (tmp_path / "琅琊榜.1080p.WEB-DL.mkv").touch()
    (tmp_path / "琅琊榜之风起长林.1080p.WEB-DL.srt").touch()

    plan = plan_directory(tmp_path)

    assert plan.operations == ()
    assert plan.skipped_count == 1
    assert plan.skipped[0].reason.startswith("unmatched (best_score=")


def test_plan_virtual_files_rejects_unsafe_paths():
    for file_name in ("/absolute/movie.mkv", "../outside/movie.srt"):
        try:
            plan_virtual_files([file_name])
        except ValueError as error:
            assert str(error) == "virtual file names must be relative paths"
        else:
            raise AssertionError("unsafe virtual path was accepted")


def test_plan_directory_ignores_nested_files_without_recursive(tmp_path: Path):
    nested = tmp_path / "Season 1"
    nested.mkdir()
    (nested / "Archive.Show.S01E01.mkv").touch()
    (nested / "Archive.Show.S01E01.eng.srt").touch()

    plan = plan_directory(tmp_path, recursive=False)

    assert plan.video_count == 0
    assert plan.subtitle_count == 0
    assert plan.directory_count == 0


def test_plan_directory_keeps_matches_isolated_by_directory(tmp_path: Path):
    nested = tmp_path / "Season 1"
    nested.mkdir()
    (tmp_path / "Root.Show.S01E01.mkv").touch()
    (nested / "Nested.Show.S01E01.mkv").touch()
    (nested / "Different.Release.1x01.eng.srt").touch()

    plan = plan_directory(tmp_path, recursive=True)

    assert plan.matched_count == 1
    assert plan.operations[0].dst == nested / "Nested.Show.S01E01.srt"
    assert plan.operations[0].reason == "episode:S01E01"


def test_plan_directory_accepts_case_insensitive_extensions(tmp_path: Path):
    (tmp_path / "Archive.Show.S01E02.MKV").touch()
    (tmp_path / "Archive.Show.1x02.eng.SRT").touch()
    (tmp_path / "Archive.Show.S01E03.txt").touch()

    plan = plan_directory(tmp_path)

    assert plan.video_count == 1
    assert plan.subtitle_count == 1
    assert plan.matched_count == 1
    assert plan.operations[0].dst.name == "Archive.Show.S01E02.SRT"


def test_plan_virtual_files_keeps_same_episode_isolated_by_directory():
    plan = plan_virtual_files(
        [
            "Season 1/Alpha.S01E01.mkv",
            "Season 1/Release.1x01.eng.srt",
            "Season 2/Beta.S01E01.mkv",
            "Season 2/Release.1x01.eng.srt",
        ]
    )

    destinations = {operation.src.parent.name: operation.dst.name for operation in plan.operations}
    assert destinations == {
        "Season 1": "Alpha.S01E01.srt",
        "Season 2": "Beta.S01E01.srt",
    }


def test_plan_virtual_files_treats_input_files_as_existing_collision_targets():
    plan = plan_virtual_files(
        [
            "Archive.Show.S01E01.mkv",
            "Archive.Show.S01E01.srt",
            "Alternate.Release.1x01.ZH-EN.srt",
        ]
    )

    operation_by_source = {operation.src.name: operation.dst.name for operation in plan.operations}
    assert "Archive.Show.S01E01.srt" not in operation_by_source.values()
    assert operation_by_source["Alternate.Release.1x01.ZH-EN.srt"] == (
        "Archive.Show.S01E01.zhen.srt"
    )
