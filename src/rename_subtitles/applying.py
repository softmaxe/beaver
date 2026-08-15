"""Front-end agnostic execution layer for a :class:`~rename_subtitles.planning.RenamePlan`.

``planning`` decides *what* should be renamed without touching the filesystem.
This module decides *how* those renames are carried out safely, and is shared by
every front-end so the safety rules only exist in one place.

The core of that safety is a two-phase model: the state of every source and
destination path is recorded when a plan is prepared, and re-checked immediately
before the renames run. If anything moved in between, nothing is applied.
"""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path

from .planning import RenameOp, RenamePlan

__all__ = [
    "ApplyOutcome",
    "ApplyResult",
    "FileState",
    "PlanChangedError",
    "PreparedOperation",
    "apply_operations",
    "detect_state_changes",
    "display_path",
    "prepare_operations",
]


@dataclass(frozen=True)
class FileState:
    """A cheap fingerprint of a path, used to detect changes between two points in time."""

    exists: bool
    is_file: bool
    device: int | None
    inode: int | None
    size: int | None
    modified_ns: int | None

    @classmethod
    def capture(cls, path: Path) -> FileState:
        try:
            stat = path.stat()
        except OSError:
            return cls(False, False, None, None, None, None)
        return cls(
            exists=True,
            is_file=path.is_file(),
            device=stat.st_dev,
            inode=stat.st_ino,
            size=stat.st_size,
            modified_ns=stat.st_mtime_ns,
        )


@dataclass(frozen=True)
class PreparedOperation:
    """A planned rename plus the filesystem state observed when the plan was made."""

    operation_id: str
    operation: RenameOp
    source_state: FileState
    destination_state: FileState

    @property
    def source(self) -> Path:
        return self.operation.src

    @property
    def destination(self) -> Path:
        return self.operation.dst


@dataclass(frozen=True)
class ApplyOutcome:
    """The result of a single rename, with paths already formatted for display."""

    operation_id: str
    source: str
    target: str
    error: str | None = None


@dataclass(frozen=True)
class ApplyResult:
    applied: tuple[ApplyOutcome, ...]
    failed: tuple[ApplyOutcome, ...]

    @property
    def status(self) -> str:
        """One of ``completed``, ``partial`` or ``failed``."""
        if not self.failed:
            return "completed"
        if self.applied:
            return "partial"
        return "failed"


class PlanChangedError(Exception):
    """Raised when the filesystem no longer matches the state the plan was built from."""

    def __init__(self, changes: Sequence[str]) -> None:
        self.changes = tuple(changes)
        super().__init__("; ".join(self.changes))


def display_path(path: Path, root: Path) -> str:
    """Render ``path`` relative to ``root``, falling back to the absolute path."""
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return str(path)


def prepare_operations(plan: RenamePlan) -> tuple[PreparedOperation, ...]:
    """Pair every operation in ``plan`` with a stable id and a snapshot of its paths."""
    return tuple(
        PreparedOperation(
            operation_id=f"op-{index}",
            operation=operation,
            source_state=FileState.capture(operation.src),
            destination_state=FileState.capture(operation.dst),
        )
        for index, operation in enumerate(plan.operations, start=1)
    )


def detect_state_changes(operations: Sequence[PreparedOperation]) -> tuple[str, ...]:
    """Report which prepared operations no longer match what is on disk."""
    changes: list[str] = []
    for prepared in operations:
        if FileState.capture(prepared.source) != prepared.source_state:
            changes.append(f"source changed: {prepared.source.name}")
        if FileState.capture(prepared.destination) != prepared.destination_state:
            changes.append(f"target changed: {prepared.destination.name}")
    return tuple(changes)


def _target_is_occupied(source: Path, destination: Path) -> bool:
    """Whether ``destination`` already holds a *different* file than ``source``.

    A rename that only changes letter case resolves to the same file on
    case-insensitive filesystems, and must not be treated as an overwrite.
    """
    if not destination.exists():
        return False
    try:
        return not destination.samefile(source)
    except OSError:
        return True


def apply_operations(
    operations: Sequence[PreparedOperation],
    root: Path,
    *,
    force: bool = False,
    verify: bool = True,
) -> ApplyResult:
    """Execute ``operations``, reporting each rename as an :class:`ApplyOutcome`.

    With ``verify`` the whole batch is aborted via :class:`PlanChangedError` if any
    path drifted since the plan was prepared, so a stale preview can never rename
    the wrong file. Without ``force`` an operation fails rather than overwriting an
    existing destination.
    """
    if verify:
        changes = detect_state_changes(operations)
        if changes:
            raise PlanChangedError(changes)

    applied: list[ApplyOutcome] = []
    failed: list[ApplyOutcome] = []
    for prepared in operations:
        source = prepared.source
        destination = prepared.destination
        outcome = ApplyOutcome(
            operation_id=prepared.operation_id,
            source=display_path(source, root),
            target=display_path(destination, root),
        )
        try:
            if _target_is_occupied(source, destination):
                if not force:
                    raise FileExistsError(f"target already exists: {destination.name}")
                destination.unlink()
            source.rename(destination)
        except OSError as error:
            failed.append(
                ApplyOutcome(
                    operation_id=outcome.operation_id,
                    source=outcome.source,
                    target=outcome.target,
                    error=str(error),
                )
            )
        else:
            applied.append(outcome)

    return ApplyResult(applied=tuple(applied), failed=tuple(failed))
