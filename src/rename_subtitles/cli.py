from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence
from pathlib import Path

from .planning import (
    SUB_EXTS_DEFAULT,
    VIDEO_EXTS_DEFAULT,
    Candidate,
    RenameOp,
    _choose_unique_best,
    _collect_by_directory,
    _collect_candidates,
    _extract_episode_key,
    _extract_lang_tag,
    _iter_files,
    _normalize_stem,
    _plan_renames,
    _score,
    _split_tokens,
    plan_directory,
)

__all__ = [
    "SUB_EXTS_DEFAULT",
    "VIDEO_EXTS_DEFAULT",
    "Candidate",
    "RenameOp",
    "_choose_unique_best",
    "_collect_by_directory",
    "_collect_candidates",
    "_extract_episode_key",
    "_extract_lang_tag",
    "_iter_files",
    "_normalize_stem",
    "_plan_renames",
    "_score",
    "_split_tokens",
    "main",
    "plan_directory",
]


def _format_ops(operations: Sequence[RenameOp]) -> str:
    if not operations:
        return "No renames planned."
    return "\n".join(
        f"- {operation.src.name}  ->  {operation.dst.name}  ({operation.reason})"
        for operation in operations
    )


def _format_ops_grouped(root: Path, grouped: dict[Path, list[RenameOp]]) -> str:
    if not grouped:
        return "No renames planned."

    lines: list[str] = []
    for directory in sorted(grouped, key=lambda item: str(item).casefold()):
        relative_directory = (
            directory.relative_to(root)
            if directory == root or root in directory.parents
            else directory
        )
        lines.append(f"Directory: {relative_directory}")
        lines.extend(
            f"- {operation.src.name}  ->  {operation.dst.name}  ({operation.reason})"
            for operation in grouped[directory]
        )
    return "\n".join(lines)


def _confirm(prompt: str) -> bool:
    try:
        answer = input(prompt).strip().lower()
    except EOFError:
        return False
    return answer in {"y", "yes"}


def _rename_one(src: Path, dst: Path, force: bool) -> None:
    if force and dst.exists():
        dst.unlink()
    src.rename(dst)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="rename-subs",
        description="Rename subtitle files to match video filenames in the same folder.",
    )
    parser.add_argument("path", type=Path, help="Target folder path.")
    parser.add_argument("--recursive", action="store_true", help="Process subfolders recursively.")
    parser.add_argument(
        "--video-ext",
        action="append",
        default=[],
        help="Video extension to include (repeatable). Default includes common video formats.",
    )
    parser.add_argument(
        "--sub-ext",
        action="append",
        default=[],
        help="Subtitle extension to include (repeatable). Default includes .ass/.srt and a few others.",
    )
    parser.add_argument(
        "--min-score", type=float, default=0.72, help="Fuzzy match threshold (0-1)."
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="Skip any subtitle that cannot be renamed to VideoStem+Ext without a suffix.",
    )
    parser.add_argument(
        "--dry-run", action="store_true", help="Show planned renames without changing files."
    )
    parser.add_argument("--apply", action="store_true", help="Apply renames.")
    parser.add_argument("--yes", action="store_true", help="Do not prompt for confirmation.")
    parser.add_argument(
        "--force", action="store_true", help="Allow overwriting existing files (dangerous)."
    )

    args = parser.parse_args(list(argv) if argv is not None else None)
    root = args.path.expanduser().resolve()
    if not root.is_dir():
        print(f"Error: not a directory: {root}", file=sys.stderr)
        return 2

    if args.apply and args.dry_run:
        print("Error: use either --dry-run or --apply, not both.", file=sys.stderr)
        return 2
    if not args.apply and not args.dry_run:
        args.dry_run = True

    video_exts = tuple(args.video_ext) if args.video_ext else VIDEO_EXTS_DEFAULT
    sub_exts = tuple(args.sub_ext) if args.sub_ext else SUB_EXTS_DEFAULT
    plan = plan_directory(
        root,
        recursive=args.recursive,
        video_exts=video_exts,
        sub_exts=sub_exts,
        strict=args.strict,
        min_score=args.min_score,
    )

    if plan.video_count == 0:
        print("No video files found.", file=sys.stderr)
        return 1
    if plan.subtitle_count == 0:
        print("No subtitle files found.", file=sys.stderr)
        return 1

    grouped_operations: dict[Path, list[RenameOp]] = {}
    for operation in plan.operations:
        grouped_operations.setdefault(operation.src.parent, []).append(operation)

    print(_format_ops_grouped(root, grouped_operations))
    if args.dry_run:
        return 0
    if not plan.operations:
        return 0

    if not args.yes and not _confirm("Apply these renames? [y/N] "):
        print("Aborted.")
        return 1

    failures: list[str] = []
    for operation in plan.operations:
        try:
            _rename_one(operation.src, operation.dst, force=args.force)
        except Exception as error:  # noqa: BLE001
            failures.append(f"{operation.src} -> {operation.dst}: {error}")

    if failures:
        print("Some renames failed:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1

    return 0
