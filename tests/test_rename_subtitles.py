from pathlib import Path

import pytest

from rename_subtitles import planning
from rename_subtitles.cli import (
    Candidate,
    _extract_episode_key,
    _extract_lang_tag,
    _normalize_stem,
    _plan_renames,
    _split_tokens,
    main,
)


def test_extract_episode_key_sxxeyy():
    assert _extract_episode_key("Show.Name.S01E02.1080p.mkv") == "S01E02"


def test_extract_episode_key_x_notation():
    assert _extract_episode_key("Show.Name.2x03.mkv") == "S02E03"


def test_extract_episode_key_case_insensitive():
    assert _extract_episode_key("show.s01e04.mkv") == "S01E04"


def test_extract_episode_key_with_brackets_and_underscores():
    assert _extract_episode_key("[kaze_hi][is_a_S01_E01][1080p_avc].ass") == "S01E01"


def test_extract_episode_key_with_separated_x_notation():
    assert _extract_episode_key("[Show_Name][2_x_03].srt") == "S02E03"


def test_extract_episode_key_absent():
    assert _extract_episode_key("Movie.Title.2020.mkv") is None


@pytest.mark.parametrize(
    ("file_name", "expected"),
    [
        pytest.param("Show.S1E2.mkv", "S01E02", id="single-digit-sxxeyy"),
        pytest.param("Show.S01-E02.mkv", "S01E02", id="hyphen-sxxeyy"),
        pytest.param("Show.S01 E02.mkv", "S01E02", id="space-sxxeyy"),
        pytest.param("Show.S01.E02.mkv", "S01E02", id="dot-sxxeyy"),
        pytest.param("Show.S01__E02.mkv", "S01E02", id="repeated-sxxeyy-separator"),
        pytest.param("Show.2-x-03.mkv", "S02E03", id="hyphen-x-notation"),
        pytest.param("Show.2 x 03.mkv", "S02E03", id="space-x-notation"),
        pytest.param("Show.2.x.03.mkv", "S02E03", id="dot-x-notation"),
    ],
)
def test_extract_episode_key_accepts_supported_variants(file_name, expected):
    assert _extract_episode_key(file_name) == expected


@pytest.mark.parametrize(
    "file_name",
    [
        pytest.param("SeasonS01E02.mkv", id="letter-before-sxxeyy"),
        pytest.param("Show.S01E020.mkv", id="three-digit-episode"),
        pytest.param("Show.S01E02x.mkv", id="letter-after-sxxeyy"),
        pytest.param("Show.A2x03B.mkv", id="letters-around-x-notation"),
        pytest.param("Show.2024x0102.mkv", id="year-like-number"),
        pytest.param("Show.S01_E_02.mkv", id="separator-after-e"),
    ],
)
def test_extract_episode_key_rejects_false_positives(file_name):
    assert _extract_episode_key(file_name) is None


def test_normalize_stem_strips_junk_and_lowercases():
    assert _normalize_stem("Show.Name.1080p.WEB.x265") == "showname"


def test_normalize_stem_strips_brackets():
    assert _normalize_stem("Show [ReleaseGroup].2020") == "show2020"


def test_split_tokens_preserves_unicode_letters_and_splits_underscores():
    assert _split_tokens("Show_Name.Amélie.Титаник.琅琊榜") == [
        "Show",
        "Name",
        "Amélie",
        "Титаник",
        "琅琊榜",
    ]


def test_normalize_stem_preserves_unicode_title_signal_and_strips_tail_language_tag():
    assert _normalize_stem("琅琊榜.1080p.WEB-DL.chs") == "琅琊榜"
    assert _normalize_stem("Amélie.2001.1080p.eng") == "amélie2001"


@pytest.mark.parametrize(
    ("stem", "expected"),
    [
        pytest.param("Café.2001", "café2001", id="nfd-latin"),
        pytest.param("進撃の巨人.シーズン1", "進撃の巨人シーズン1", id="japanese"),
        pytest.param("Тёмные начала.2022", "тёмныеначала2022", id="cyrillic"),
        pytest.param("琅琊榜—Nirvana_in_Fire", "琅琊榜nirvanainfire", id="mixed-scripts"),
    ],
)
def test_normalize_stem_preserves_multilingual_titles(stem, expected):
    assert _normalize_stem(stem) == expected


@pytest.mark.parametrize(
    "release_metadata",
    [
        pytest.param("720p.1080p.2160p.4K", id="resolutions"),
        pytest.param("WEB.WEB-DL.WEBRip.BluRay.BDRip", id="sources"),
        pytest.param("x264.x265.H264.H265.HEVC", id="video-codecs"),
        pytest.param("AAC.DTS", id="audio-codecs"),
        pytest.param("RARBG.YIFY.PROPER.REPACK", id="release-groups"),
        pytest.param("EXTENDED.REMUX.HDR.SDR", id="release-variants"),
    ],
)
def test_normalize_stem_strips_release_metadata_groups(release_metadata):
    assert _normalize_stem(f"Archive.Show.{release_metadata}.eng.chs") == "archiveshow"


def test_normalize_stem_only_removes_complete_junk_tokens():
    assert _normalize_stem("Webster.DLsite.HDRipley") == "websterdlsitehdripley"


def test_normalize_stem_treats_nfc_and_nfd_as_equivalent():
    assert _normalize_stem("Café.2001") == _normalize_stem("Café.2001")


def test_plan_renames_does_not_match_empty_normalized_stems(tmp_path):
    video = tmp_path / "[ReleaseGroup].1080p.mkv"
    sub = tmp_path / "[AnotherGroup].1080p.srt"
    video.touch()
    sub.touch()

    ops = _plan_renames(
        [_candidate(sub)],
        [_candidate(video)],
        strict=False,
        min_score=0.6,
    )

    assert ops == []


def test_extract_lang_tag_tail():
    assert _extract_lang_tag("Show.S01E01.chs") == "chs"
    assert _extract_lang_tag("Show.S01E01.zh-en") == "zhen"


def test_extract_lang_tag_absent():
    assert _extract_lang_tag("Show.S01E01") is None


@pytest.mark.parametrize(
    ("stem", "expected"),
    [
        pytest.param("Show.S01E01.en", "en", id="short-english"),
        pytest.param("Show.S01E01.eng", "eng", id="english"),
        pytest.param("Show.S01E01.jpn", "jpn", id="japanese"),
        pytest.param("Show.S01E01.chs_eng", "chseng", id="underscore-combo"),
        pytest.param("Show.S01E01.eng.chs", "engchs", id="dot-combo"),
        pytest.param("Show.S01E01.engchs", "engchs", id="compact-combo"),
        pytest.param("Show.S01E01.ZH-EN", "zhen", id="uppercase-combo"),
    ],
)
def test_extract_lang_tag_supports_language_variants(stem, expected):
    assert _extract_lang_tag(stem) == expected


def test_normalize_stem_strips_consecutive_tail_language_tags():
    assert _normalize_stem("Show.S01E01.eng.chs") == "shows01e01"


def _candidate(path: Path, *, ep: str | None = None) -> Candidate:
    return Candidate(path=path, stem_norm=_normalize_stem(path.stem), episode_key=ep)


def _parsed_candidate(path: Path) -> Candidate:
    return _candidate(path, ep=_extract_episode_key(path.name))


def test_plan_renames_episode_id_beats_fuzzy_and_ignores_threshold(tmp_path):
    correct_video = tmp_path / "Completely.Different.S01E02.mkv"
    fuzzy_video = tmp_path / "Show.Name.S03E04.mkv"
    sub = tmp_path / "Show.Name.1x02.eng.srt"

    ops = _plan_renames(
        [_parsed_candidate(sub)],
        [_parsed_candidate(correct_video), _parsed_candidate(fuzzy_video)],
        strict=False,
        min_score=1.0,
    )

    assert len(ops) == 1
    assert ops[0].dst == tmp_path / "Completely.Different.S01E02.srt"
    assert ops[0].reason == "episode:S01E02"
    assert ops[0].score is None


@pytest.mark.parametrize(
    ("scores", "min_score", "should_match"),
    [
        pytest.param((0.72,), 0.72, True, id="score-equals-threshold"),
        pytest.param((0.719,), 0.72, False, id="score-below-threshold"),
        pytest.param((0.80, 0.80), 0.72, False, id="tied-best"),
        pytest.param((0.80, 0.75), 0.72, False, id="margin-below-minimum"),
        pytest.param((0.80, 0.73), 0.72, True, id="margin-above-minimum"),
    ],
)
def test_plan_renames_fuzzy_score_boundaries(
    tmp_path, monkeypatch, scores, min_score, should_match
):
    sub = Candidate(path=tmp_path / "Subtitle.srt", stem_norm="subtitle", episode_key=None)
    videos = [
        Candidate(
            path=tmp_path / f"Video{index}.mkv",
            stem_norm=f"video{index}",
            episode_key=None,
        )
        for index in range(len(scores))
    ]
    scores_by_stem = {video.stem_norm: score for video, score in zip(videos, scores)}
    monkeypatch.setattr(planning, "_score", lambda _left, right: scores_by_stem[right])

    ops = _plan_renames([sub], videos, strict=False, min_score=min_score)

    assert bool(ops) is should_match
    if should_match:
        assert ops[0].dst == tmp_path / "Video0.srt"


def test_plan_renames_duplicate_episode_does_not_block_unique_episode(tmp_path):
    videos = [
        _parsed_candidate(tmp_path / "Nebula.S01E01.mkv"),
        _parsed_candidate(tmp_path / "Aurora.1x01.mkv"),
        _parsed_candidate(tmp_path / "Nebula.S01E02.mkv"),
    ]
    subtitles = [
        _parsed_candidate(tmp_path / "Nebula.S01E01.eng.srt"),
        _parsed_candidate(tmp_path / "Nebula.1x02.eng.srt"),
    ]

    ops = _plan_renames(subtitles, videos, strict=False, min_score=0.72)

    assert len(ops) == 1
    assert ops[0].src.name == "Nebula.1x02.eng.srt"
    assert ops[0].dst.name == "Nebula.S01E02.srt"
    assert ops[0].reason == "episode:S01E02"


def test_plan_renames_episode_id(tmp_path):
    video = tmp_path / "Show.S01E01.1080p.mkv"
    sub = tmp_path / "Show.S01E01.1080p.en.srt"
    video.touch()
    sub.touch()
    ops = _plan_renames(
        [_candidate(sub, ep="S01E01")],
        [_candidate(video, ep="S01E01")],
        strict=False,
        min_score=0.72,
    )
    assert len(ops) == 1
    assert ops[0].dst == tmp_path / "Show.S01E01.1080p.srt"
    assert ops[0].reason == "episode:S01E01"


def test_plan_renames_fuzzy(tmp_path):
    video = tmp_path / "Inception.2010.1080p.BluRay.x265.mkv"
    sub = tmp_path / "Inception.2010.1080p.BluRay.x265.chs.srt"
    video.touch()
    sub.touch()
    ops = _plan_renames(
        [_candidate(sub)],
        [_candidate(video)],
        strict=False,
        min_score=0.72,
    )
    assert len(ops) == 1
    assert ops[0].dst == tmp_path / "Inception.2010.1080p.BluRay.x265.srt"
    assert ops[0].reason.startswith("fuzzy:")


def test_plan_renames_collision_uses_numeric_suffix(tmp_path):
    video = tmp_path / "Movie.Title.2020.mkv"
    sub_en = tmp_path / "Movie.Title.2020.en.srt"
    sub_chs = tmp_path / "Movie.Title.2020.chs.srt"
    video.touch()
    sub_en.touch()
    sub_chs.touch()
    ops = _plan_renames(
        [_candidate(sub_en), _candidate(sub_chs)],
        [_candidate(video)],
        strict=False,
        min_score=0.72,
    )
    assert [o.dst.name for o in ops] == ["Movie.Title.2020.srt", "Movie.Title.2020.2.srt"]


@pytest.mark.parametrize("subtitle_suffix", [".srt", ".ass", ".vtt"])
def test_plan_renames_preserves_subtitle_extension(tmp_path, subtitle_suffix):
    video = tmp_path / "Archive.Show.S01E02.mkv"
    sub = tmp_path / f"Archive.Show.1x02.eng{subtitle_suffix}"
    video.touch()
    sub.touch()

    ops = _plan_renames(
        [_parsed_candidate(sub)],
        [_parsed_candidate(video)],
        strict=False,
        min_score=0.72,
    )

    assert len(ops) == 1
    assert ops[0].dst.name == f"Archive.Show.S01E02{subtitle_suffix}"


def test_plan_renames_collision_uses_normalized_language_suffix(tmp_path):
    video = tmp_path / "Archive.Show.S01E02.mkv"
    existing_base = tmp_path / "Archive.Show.S01E02.srt"
    sub = tmp_path / "Alternate.Release.1x02.ZH-EN.srt"
    for path in (video, existing_base, sub):
        path.touch()

    ops = _plan_renames(
        [_parsed_candidate(sub)],
        [_parsed_candidate(video)],
        strict=False,
        min_score=0.72,
    )

    assert len(ops) == 1
    assert ops[0].dst.name == "Archive.Show.S01E02.zhen.srt"


def test_plan_renames_collision_skips_existing_numeric_suffixes(tmp_path):
    video = tmp_path / "Archive.Show.S01E02.mkv"
    sub = tmp_path / "Alternate.Release.1x02.ZH-EN.srt"
    occupied = [
        tmp_path / "Archive.Show.S01E02.srt",
        tmp_path / "Archive.Show.S01E02.zhen.srt",
        tmp_path / "Archive.Show.S01E02.2.srt",
    ]
    for path in (video, sub, *occupied):
        path.touch()

    ops = _plan_renames(
        [_parsed_candidate(sub)],
        [_parsed_candidate(video)],
        strict=False,
        min_score=0.72,
    )

    assert len(ops) == 1
    assert ops[0].dst.name == "Archive.Show.S01E02.3.srt"


def test_plan_renames_ambiguous_pair_skipped(tmp_path):
    video1 = tmp_path / "Show.Name.01.mkv"
    video2 = tmp_path / "Show.Name.02.mkv"
    sub = tmp_path / "Show.Name.chs.srt"
    video1.touch()
    video2.touch()
    sub.touch()
    ops = _plan_renames(
        [_candidate(sub)],
        [_candidate(video1), _candidate(video2)],
        strict=False,
        min_score=0.72,
    )
    assert ops == []


def test_plan_renames_skips_duplicate_episode_key_even_when_fuzzy_match_is_unique(tmp_path):
    video_one = tmp_path / "Nebula.S01E01.1080p.mkv"
    video_two = tmp_path / "Aurora.S01E01.1080p.mkv"
    sub = tmp_path / "Nebula.S01E01.chs.srt"
    for path in (video_one, video_two, sub):
        path.touch()

    ops = _plan_renames(
        [_candidate(sub, ep="S01E01")],
        [_candidate(video_one, ep="S01E01"), _candidate(video_two, ep="S01E01")],
        strict=False,
        min_score=0.72,
    )

    assert ops == []


def test_plan_renames_strict_skips_collision(tmp_path):
    video = tmp_path / "Movie.Title.2020.mkv"
    sub_en = tmp_path / "Movie.Title.2020.en.srt"
    sub_chs = tmp_path / "Movie.Title.2020.chs.srt"
    video.touch()
    sub_en.touch()
    sub_chs.touch()
    ops = _plan_renames(
        [_candidate(sub_en), _candidate(sub_chs)],
        [_candidate(video)],
        strict=True,
        min_score=0.72,
    )
    assert len(ops) == 1
    assert ops[0].dst.name == "Movie.Title.2020.srt"


def test_main_apply_renames(tmp_path):
    (tmp_path / "Show.S01E01.1080p.mkv").touch()
    (tmp_path / "Show.S01E01.1080p.en.srt").touch()
    rc = main([str(tmp_path), "--apply", "--yes"])
    assert rc == 0
    assert (tmp_path / "Show.S01E01.1080p.srt").exists()
    assert not (tmp_path / "Show.S01E01.1080p.en.srt").exists()


def test_main_apply_handles_combined_rules_recursively_without_overwriting(tmp_path):
    season_one = tmp_path / "Season 1"
    season_two = tmp_path / "Season 2"
    season_one.mkdir()
    season_two.mkdir()

    episode_video = season_one / "琅琊榜.S01-E02.2160p.WEB-DL.x265.mkv"
    episode_sub = season_one / "琅琊榜.1x02.zh-en.ass"
    fuzzy_video = season_one / "Amélie.2001.2160p.WEB-DL.x265.mkv"
    fuzzy_sub = season_one / "Amélie.2001.720p.BluRay.AAC.chs.vtt"
    collision_video = season_two / "Тёмные.Начала.S02E03.1080p.BluRay.H264.mkv"
    collision_sub = season_two / "Alternate.Release.2-x-03.ZH-EN.srt"
    existing_base = season_two / "Тёмные.Начала.S02E03.1080p.BluRay.H264.srt"

    for video in (episode_video, fuzzy_video, collision_video):
        video.touch()
    episode_sub.write_text("episode subtitle", encoding="utf-8")
    fuzzy_sub.write_text("fuzzy subtitle", encoding="utf-8")
    collision_sub.write_text("collision subtitle", encoding="utf-8")
    existing_base.write_text("existing subtitle", encoding="utf-8")

    rc = main([str(tmp_path), "--recursive", "--apply", "--yes"])

    assert rc == 0
    expected_contents = {
        season_one / "琅琊榜.S01-E02.2160p.WEB-DL.x265.ass": "episode subtitle",
        season_one / "Amélie.2001.2160p.WEB-DL.x265.vtt": "fuzzy subtitle",
        season_two / "Тёмные.Начала.S02E03.1080p.BluRay.H264.zhen.srt": (
            "collision subtitle"
        ),
        existing_base: "existing subtitle",
    }
    for path, content in expected_contents.items():
        assert path.read_text(encoding="utf-8") == content
    for source in (episode_sub, fuzzy_sub, collision_sub):
        assert not source.exists()


def test_main_dry_run_leaves_files_untouched(tmp_path):
    video = tmp_path / "Show.S01E01.1080p.mkv"
    sub = tmp_path / "Show.S01E01.1080p.en.srt"
    video.touch()
    sub.touch()
    rc = main([str(tmp_path), "--dry-run"])
    assert rc == 0
    assert (tmp_path / "Show.S01E01.1080p.en.srt").exists()


def test_main_conflicting_flags_rejected(tmp_path):
    rc = main([str(tmp_path), "--dry-run", "--apply"])
    assert rc == 2


def test_main_missing_directory_rejected():
    rc = main([str(Path("/nonexistent/rename/dir")), "--dry-run"])
    assert rc == 2


def test_main_no_video_files(tmp_path):
    (tmp_path / "orphan.srt").touch()
    rc = main([str(tmp_path), "--dry-run"])
    assert rc == 1
