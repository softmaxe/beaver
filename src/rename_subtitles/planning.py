from __future__ import annotations

import re
import unicodedata
from collections.abc import Callable, Iterator, Sequence
from dataclasses import dataclass
from difflib import SequenceMatcher
from pathlib import Path

VIDEO_EXTS_DEFAULT = (".mkv", ".mp4", ".avi", ".mov", ".wmv", ".m4v", ".webm")
SUB_EXTS_DEFAULT = (".ass", ".srt", ".ssa", ".vtt", ".sub")


EPISODE_PATTERNS: tuple[re.Pattern[str], ...] = (
    re.compile(r"(?i)(?<![0-9A-Za-z])S(?P<s>\d{1,2})[-_.\s]*E(?P<e>\d{1,2})(?![0-9A-Za-z])"),
    re.compile(r"(?i)(?<![0-9A-Za-z])(?P<s>\d{1,2})[-_.\s]*x[-_.\s]*(?P<e>\d{1,2})(?![0-9A-Za-z])"),
)


LANG_TAGS = {
    "en",
    "eng",
    "english",
    "zh",
    "zho",
    "chi",
    "chs",
    "cht",
    "chinese",
    "cn",
    "ja",
    "jpn",
    "japanese",
    "jp",
    "ko",
    "kor",
    "korean",
    "chseng",
    "engchs",
    "chs-eng",
    "zh-en",
}


JUNK_TOKENS = {
    "1080p",
    "720p",
    "2160p",
    "4k",
    "webrip",
    "web",
    "webdl",
    "web-dl",
    "dl",
    "bluray",
    "bdrip",
    "dvdrip",
    "hdrip",
    "x264",
    "x265",
    "h264",
    "h265",
    "hevc",
    "aac",
    "dts",
    "rarbg",
    "yify",
    "proper",
    "repack",
    "extended",
    "remux",
    "hdr",
    "sdr",
}


PathExists = Callable[[Path], bool]


@dataclass(frozen=True)
class Candidate:
    path: Path
    stem_norm: str
    episode_key: str | None


@dataclass(frozen=True)
class RenameOp:
    src: Path
    dst: Path
    reason: str
    score: float | None = None


@dataclass(frozen=True)
class SkippedRename:
    path: Path
    reason: str
    score: float | None = None


@dataclass(frozen=True)
class RenamePlan:
    root: Path
    operations: tuple[RenameOp, ...]
    skipped: tuple[SkippedRename, ...]
    video_count: int
    subtitle_count: int
    directory_count: int

    @property
    def matched_count(self) -> int:
        return len(self.operations)

    @property
    def skipped_count(self) -> int:
        return len(self.skipped)


def _split_tokens(text: str) -> list[str]:
    normalized = unicodedata.normalize("NFC", text)
    return re.findall(r"[^\W_]+", normalized)


def _normalize_stem(stem: str) -> str:
    stem = re.sub(r"[\[\(\{].*?[\]\)\}]", " ", stem)
    tokens = [token.casefold() for token in _split_tokens(stem)]
    tokens = [token for token in tokens if token not in JUNK_TOKENS]
    while tokens and tokens[-1] in LANG_TAGS:
        tokens.pop()
    return "".join(tokens)


def _extract_episode_key(name: str) -> str | None:
    for pattern in EPISODE_PATTERNS:
        match = pattern.search(name)
        if not match:
            continue
        season = int(match.group("s"))
        episode = int(match.group("e"))
        return f"S{season:02d}E{episode:02d}"
    return None


LANG_COMBO_RE = re.compile(
    r"(?i)(?:^|[^0-9A-Za-z])"
    r"((?:chs|cht|eng|zho|chi|zh|en|ja|jpn|ko|kor)[-_.](?:chs|cht|eng|zho|chi|zh|en|ja|jpn|ko|kor))"
    r"(?:$|[^0-9A-Za-z])"
)


def _extract_lang_tag(stem: str) -> str | None:
    match = LANG_COMBO_RE.search(stem)
    if match:
        return re.sub(r"[-_.]", "", match.group(1)).casefold()
    tokens = [token.lower() for token in _split_tokens(stem)]
    for token in reversed(tokens[-8:]):
        if token in LANG_TAGS:
            return token
    return None


def _iter_files(root: Path, recursive: bool) -> Iterator[Path]:
    if recursive:
        yield from (path for path in root.rglob("*") if path.is_file())
    else:
        yield from (path for path in root.iterdir() if path.is_file())


def _candidate_from_path(path: Path) -> Candidate:
    return Candidate(
        path=path,
        stem_norm=_normalize_stem(path.stem),
        episode_key=_extract_episode_key(path.name),
    )


def _collect_candidates(
    root: Path,
    recursive: bool,
    exts: Sequence[str],
) -> list[Candidate]:
    ext_set = {ext.lower() for ext in exts}
    candidates = [
        _candidate_from_path(path)
        for path in _iter_files(root, recursive=recursive)
        if path.suffix.lower() in ext_set
    ]
    return sorted(candidates, key=lambda candidate: str(candidate.path).casefold())


def _collect_by_directory(
    root: Path,
    *,
    recursive: bool,
    video_exts: Sequence[str],
    sub_exts: Sequence[str],
) -> tuple[dict[Path, list[Candidate]], dict[Path, list[Candidate]]]:
    video_set = {ext.lower() for ext in video_exts}
    sub_set = {ext.lower() for ext in sub_exts}
    videos_by_dir: dict[Path, list[Candidate]] = {}
    subs_by_dir: dict[Path, list[Candidate]] = {}

    for path in _iter_files(root, recursive=recursive):
        suffix = path.suffix.lower()
        if suffix not in video_set and suffix not in sub_set:
            continue

        candidate = _candidate_from_path(path)
        if suffix in video_set:
            videos_by_dir.setdefault(path.parent, []).append(candidate)
        else:
            subs_by_dir.setdefault(path.parent, []).append(candidate)

    for candidates in (*videos_by_dir.values(), *subs_by_dir.values()):
        candidates.sort(key=lambda candidate: str(candidate.path).casefold())

    return videos_by_dir, subs_by_dir


def _score(left: str, right: str) -> float:
    if not left or not right:
        return 0.0
    return SequenceMatcher(a=left, b=right).ratio()


def _choose_unique_best(
    sub: Candidate,
    videos: Sequence[Candidate],
    min_score: float,
) -> tuple[Candidate | None, float]:
    if not sub.stem_norm:
        return None, 0.0

    scored = sorted(
        (
            (_score(sub.stem_norm, video.stem_norm), video)
            for video in videos
            if video.stem_norm
        ),
        key=lambda item: item[0],
        reverse=True,
    )
    if not scored:
        return None, 0.0

    best_score, best = scored[0]
    if best_score < min_score:
        return None, best_score

    second_score = scored[1][0] if len(scored) > 1 else 0.0
    if best_score - second_score < 0.06:
        return None, best_score
    return best, best_score


def _build_rename_plan(
    subs: Sequence[Candidate],
    videos: Sequence[Candidate],
    *,
    strict: bool,
    min_score: float,
    path_exists: PathExists,
) -> tuple[list[RenameOp], list[SkippedRename]]:
    videos_by_ep: dict[str, Candidate] = {}
    duplicate_episode_keys: set[str] = set()
    for video in videos:
        if not video.episode_key:
            continue
        if video.episode_key in videos_by_ep:
            duplicate_episode_keys.add(video.episode_key)
        else:
            videos_by_ep[video.episode_key] = video
    for episode_key in duplicate_episode_keys:
        videos_by_ep.pop(episode_key, None)

    planned_destinations: set[Path] = set()
    operations: list[RenameOp] = []
    skipped: list[SkippedRename] = []

    for subtitle in subs:
        video: Candidate | None = None
        reason = ""
        score: float | None = None

        if subtitle.episode_key and subtitle.episode_key in duplicate_episode_keys:
            skipped.append(
                SkippedRename(
                    path=subtitle.path,
                    reason=f"unmatched (ambiguous episode:{subtitle.episode_key})",
                )
            )
            continue
        if subtitle.episode_key and subtitle.episode_key in videos_by_ep:
            video = videos_by_ep[subtitle.episode_key]
            reason = f"episode:{subtitle.episode_key}"
        else:
            video, fuzzy_score = _choose_unique_best(subtitle, videos, min_score=min_score)
            score = fuzzy_score
            if video is None:
                skipped.append(
                    SkippedRename(
                        path=subtitle.path,
                        reason=f"unmatched (best_score={fuzzy_score:.2f})",
                        score=fuzzy_score,
                    )
                )
                continue
            reason = f"fuzzy:{fuzzy_score:.2f}"

        base_destination = video.path.with_suffix(subtitle.path.suffix)
        if base_destination == subtitle.path:
            skipped.append(SkippedRename(path=subtitle.path, reason="already matches"))
            continue

        destination = base_destination
        if path_exists(destination) or destination in planned_destinations:
            if strict:
                skipped.append(
                    SkippedRename(
                        path=subtitle.path,
                        reason="target collision in strict mode",
                        score=score,
                    )
                )
                continue
            language_tag = _extract_lang_tag(subtitle.path.stem)
            if language_tag:
                destination = video.path.with_name(
                    f"{video.path.stem}.{language_tag}{subtitle.path.suffix}"
                )

        if (path_exists(destination) or destination in planned_destinations) and not strict:
            suffix_number = 2
            while True:
                destination = video.path.with_name(
                    f"{video.path.stem}.{suffix_number}{subtitle.path.suffix}"
                )
                if not path_exists(destination) and destination not in planned_destinations:
                    break
                suffix_number += 1

        if path_exists(destination) or destination in planned_destinations:
            skipped.append(
                SkippedRename(
                    path=subtitle.path,
                    reason="target collision",
                    score=score,
                )
            )
            continue

        planned_destinations.add(destination)
        operations.append(RenameOp(src=subtitle.path, dst=destination, reason=reason, score=score))

    return operations, skipped


def _plan_renames(
    subs: Sequence[Candidate],
    videos: Sequence[Candidate],
    *,
    strict: bool,
    min_score: float,
) -> list[RenameOp]:
    operations, _ = _build_rename_plan(
        subs,
        videos,
        strict=strict,
        min_score=min_score,
        path_exists=Path.exists,
    )
    return operations


def _create_plan(
    root: Path,
    videos_by_dir: dict[Path, list[Candidate]],
    subs_by_dir: dict[Path, list[Candidate]],
    *,
    strict: bool,
    min_score: float,
    path_exists: PathExists,
) -> RenamePlan:
    operations: list[RenameOp] = []
    skipped: list[SkippedRename] = []
    all_directories = set(videos_by_dir) | set(subs_by_dir)

    for directory in sorted(all_directories, key=lambda item: str(item).casefold()):
        videos = videos_by_dir.get(directory, [])
        subtitles = subs_by_dir.get(directory, [])
        if not subtitles:
            continue
        if not videos:
            skipped.extend(
                SkippedRename(path=subtitle.path, reason="no video files in this directory")
                for subtitle in subtitles
            )
            continue

        directory_operations, directory_skipped = _build_rename_plan(
            subtitles,
            videos,
            strict=strict,
            min_score=min_score,
            path_exists=path_exists,
        )
        operations.extend(directory_operations)
        skipped.extend(directory_skipped)

    return RenamePlan(
        root=root,
        operations=tuple(operations),
        skipped=tuple(skipped),
        video_count=sum(len(videos) for videos in videos_by_dir.values()),
        subtitle_count=sum(len(subtitles) for subtitles in subs_by_dir.values()),
        directory_count=len(all_directories),
    )


def plan_directory(
    root: Path,
    *,
    recursive: bool = False,
    video_exts: Sequence[str] = VIDEO_EXTS_DEFAULT,
    sub_exts: Sequence[str] = SUB_EXTS_DEFAULT,
    strict: bool = False,
    min_score: float = 0.72,
) -> RenamePlan:
    root = root.expanduser().resolve()
    if not root.is_dir():
        raise ValueError(f"not a directory: {root}")

    videos_by_dir, subs_by_dir = _collect_by_directory(
        root,
        recursive=recursive,
        video_exts=video_exts,
        sub_exts=sub_exts,
    )
    return _create_plan(
        root,
        videos_by_dir,
        subs_by_dir,
        strict=strict,
        min_score=min_score,
        path_exists=Path.exists,
    )


def plan_virtual_files(
    file_names: Sequence[str],
    *,
    video_exts: Sequence[str] = VIDEO_EXTS_DEFAULT,
    sub_exts: Sequence[str] = SUB_EXTS_DEFAULT,
    strict: bool = False,
    min_score: float = 0.72,
) -> RenamePlan:
    root = Path("/virtual-subtitle-library")
    video_set = {ext.lower() for ext in video_exts}
    sub_set = {ext.lower() for ext in sub_exts}
    videos_by_dir: dict[Path, list[Candidate]] = {}
    subs_by_dir: dict[Path, list[Candidate]] = {}
    existing_paths: set[Path] = set()

    for file_name in file_names:
        relative_path = Path(file_name)
        if relative_path.is_absolute() or ".." in relative_path.parts:
            raise ValueError("virtual file names must be relative paths")
        path = root / relative_path
        existing_paths.add(path)
        suffix = path.suffix.lower()
        if suffix in video_set:
            videos_by_dir.setdefault(path.parent, []).append(_candidate_from_path(path))
        elif suffix in sub_set:
            subs_by_dir.setdefault(path.parent, []).append(_candidate_from_path(path))

    for candidates in (*videos_by_dir.values(), *subs_by_dir.values()):
        candidates.sort(key=lambda candidate: str(candidate.path).casefold())

    return _create_plan(
        root,
        videos_by_dir,
        subs_by_dir,
        strict=strict,
        min_score=min_score,
        path_exists=existing_paths.__contains__,
    )


__all__ = [
    "SUB_EXTS_DEFAULT",
    "VIDEO_EXTS_DEFAULT",
    "Candidate",
    "RenameOp",
    "RenamePlan",
    "SkippedRename",
    "plan_directory",
    "plan_virtual_files",
]
