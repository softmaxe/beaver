from __future__ import annotations

import argparse
import json
import secrets
import subprocess
import sys
import threading
import time
import webbrowser
from collections.abc import Sequence
from dataclasses import dataclass
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

from .planning import RenameOp, RenamePlan, SkippedRename, plan_directory, plan_virtual_files

ASSET_DIR = Path(__file__).resolve().parent / "web_assets"
PLAN_TTL_SECONDS = 10 * 60
MAX_REQUEST_BYTES = 64 * 1024

STATIC_FILES = {
    "/": ("index.html", "text/html; charset=utf-8"),
    "/styles.css": ("styles.css", "text/css; charset=utf-8"),
    "/app.js": ("app.js", "text/javascript; charset=utf-8"),
}

DEMO_FILES = (
    "Nebula.Archive.S01E01.2160p.WEB-DL.mkv",
    "Nebula.Archive.S01E02.2160p.WEB-DL.mkv",
    "Nebula.Archive.S01E03.2160p.WEB-DL.mkv",
    "Nebula.Archive.S01E01.zh-en.srt",
    "Nebula.Archive.S01E02.chs.ass",
    "Nebula.Archive.S01E03.eng.srt",
    "Unsorted.Bonus.Feature.srt",
)


@dataclass(frozen=True)
class FileState:
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
        except FileNotFoundError:
            return cls(False, False, None, None, None, None)
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
class StoredOperation:
    operation_id: str
    operation: RenameOp
    source_state: FileState
    destination_state: FileState


@dataclass
class StoredPlan:
    plan_id: str
    plan: RenamePlan
    operations: tuple[StoredOperation, ...]
    created_at: float
    used: bool = False


class ApiError(Exception):
    def __init__(self, code: str, status: HTTPStatus, detail: str | None = None) -> None:
        self.code = code
        self.status = status
        self.detail = detail
        super().__init__(detail or code)


class PlanStore:
    def __init__(self, *, ttl_seconds: float = PLAN_TTL_SECONDS) -> None:
        self._ttl_seconds = ttl_seconds
        self._plans: dict[str, StoredPlan] = {}
        self._lock = threading.Lock()

    def create(self, plan: RenamePlan) -> StoredPlan:
        stored_operations = tuple(
            StoredOperation(
                operation_id=f"op_{index}_{secrets.token_urlsafe(8)}",
                operation=operation,
                source_state=FileState.capture(operation.src),
                destination_state=FileState.capture(operation.dst),
            )
            for index, operation in enumerate(plan.operations, start=1)
        )
        stored_plan = StoredPlan(
            plan_id=secrets.token_urlsafe(24),
            plan=plan,
            operations=stored_operations,
            created_at=time.monotonic(),
        )
        with self._lock:
            self._discard_expired_locked()
            self._plans[stored_plan.plan_id] = stored_plan
        return stored_plan

    def apply(self, plan_id: str, operation_ids: Sequence[str]) -> dict[str, Any]:
        with self._lock:
            self._discard_expired_locked()
            stored_plan = self._plans.get(plan_id)
            if stored_plan is None:
                raise ApiError("invalid_or_expired_plan", HTTPStatus.NOT_FOUND)
            if stored_plan.used:
                raise ApiError("plan_already_used", HTTPStatus.CONFLICT)

            selected_operations = self._select_operations(stored_plan, operation_ids)
            changes = self._find_state_changes(selected_operations)
            if changes:
                raise ApiError("plan_changed", HTTPStatus.CONFLICT, "; ".join(changes))

            applied: list[dict[str, str]] = []
            failures: list[dict[str, str]] = []
            for stored_operation in selected_operations:
                operation = stored_operation.operation
                try:
                    operation.src.rename(operation.dst)
                except OSError as error:
                    failures.append(
                        {
                            "id": stored_operation.operation_id,
                            "source": _display_path(operation.src, stored_plan.plan.root),
                            "target": _display_path(operation.dst, stored_plan.plan.root),
                            "error": str(error),
                        }
                    )
                else:
                    applied.append(
                        {
                            "id": stored_operation.operation_id,
                            "source": _display_path(operation.src, stored_plan.plan.root),
                            "target": _display_path(operation.dst, stored_plan.plan.root),
                        }
                    )

            stored_plan.used = True
            return {"applied": applied, "failures": failures}

    def _discard_expired_locked(self) -> None:
        now = time.monotonic()
        expired_ids = [
            plan_id
            for plan_id, stored_plan in self._plans.items()
            if now - stored_plan.created_at > self._ttl_seconds
        ]
        for plan_id in expired_ids:
            self._plans.pop(plan_id, None)

    @staticmethod
    def _select_operations(
        stored_plan: StoredPlan,
        operation_ids: Sequence[str],
    ) -> list[StoredOperation]:
        if not operation_ids:
            raise ApiError("no_operations_selected", HTTPStatus.BAD_REQUEST)
        if len(set(operation_ids)) != len(operation_ids):
            raise ApiError("invalid_operation_selection", HTTPStatus.BAD_REQUEST)

        operations_by_id = {
            stored_operation.operation_id: stored_operation
            for stored_operation in stored_plan.operations
        }
        selected_operations: list[StoredOperation] = []
        for operation_id in operation_ids:
            stored_operation = operations_by_id.get(operation_id)
            if stored_operation is None:
                raise ApiError("invalid_operation_selection", HTTPStatus.BAD_REQUEST)
            selected_operations.append(stored_operation)
        return selected_operations

    @staticmethod
    def _find_state_changes(operations: Sequence[StoredOperation]) -> list[str]:
        changes: list[str] = []
        for stored_operation in operations:
            operation = stored_operation.operation
            if FileState.capture(operation.src) != stored_operation.source_state:
                changes.append(f"source changed: {operation.src.name}")
            if FileState.capture(operation.dst) != stored_operation.destination_state:
                changes.append(f"target changed: {operation.dst.name}")
        return changes


def _display_path(path: Path, root: Path) -> str:
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return str(path)


def _operation_payload(
    operation: RenameOp,
    root: Path,
    operation_id: str,
) -> dict[str, Any]:
    if operation.reason.startswith("episode:"):
        match_type = "episode"
        detail = operation.reason.removeprefix("episode:")
    else:
        match_type = "fuzzy"
        detail = operation.reason.removeprefix("fuzzy:")
    return {
        "id": operation_id,
        "source": _display_path(operation.src, root),
        "target": _display_path(operation.dst, root),
        "match_type": match_type,
        "detail": detail,
        "score": operation.score,
    }


def _skip_payload(skipped: SkippedRename, root: Path) -> dict[str, Any]:
    reason_code = "collision"
    if skipped.reason.startswith("unmatched"):
        reason_code = "unmatched"
    elif skipped.reason == "already matches":
        reason_code = "already_matching"
    elif skipped.reason == "target collision in strict mode":
        reason_code = "strict_collision"
    elif skipped.reason == "no video files in this directory":
        reason_code = "no_video"
    return {
        "source": _display_path(skipped.path, root),
        "reason_code": reason_code,
        "score": skipped.score,
    }


def _plan_payload(stored_plan: StoredPlan) -> dict[str, Any]:
    plan = stored_plan.plan
    return {
        "mode": "live",
        "plan_id": stored_plan.plan_id,
        "summary": {
            "videos": plan.video_count,
            "subtitles": plan.subtitle_count,
            "matched": plan.matched_count,
            "skipped": plan.skipped_count,
            "directories": plan.directory_count,
        },
        "operations": [
            _operation_payload(stored.operation, plan.root, stored.operation_id)
            for stored in stored_plan.operations
        ],
        "skipped": [_skip_payload(item, plan.root) for item in plan.skipped],
    }


def demo_payload() -> dict[str, Any]:
    plan = plan_virtual_files(DEMO_FILES)
    return {
        "mode": "demo",
        "plan_id": None,
        "summary": {
            "videos": plan.video_count,
            "subtitles": plan.subtitle_count,
            "matched": plan.matched_count,
            "skipped": plan.skipped_count,
            "directories": plan.directory_count,
        },
        "operations": [
            _operation_payload(operation, plan.root, f"demo_{index}")
            for index, operation in enumerate(plan.operations, start=1)
        ],
        "skipped": [_skip_payload(item, plan.root) for item in plan.skipped],
        "files": list(DEMO_FILES),
    }


def _parse_bool(value: Any, field_name: str) -> bool:
    if isinstance(value, bool):
        return value
    raise ApiError("invalid_option", HTTPStatus.BAD_REQUEST, f"{field_name} must be a boolean")


def _parse_min_score(value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ApiError("invalid_option", HTTPStatus.BAD_REQUEST, "min_score must be a number")
    min_score = float(value)
    if not 0.0 <= min_score <= 1.0:
        raise ApiError(
            "invalid_option", HTTPStatus.BAD_REQUEST, "min_score must be between 0 and 1"
        )
    return min_score


def _choose_directory() -> dict[str, Any]:
    if sys.platform == "darwin":
        macos_result = _choose_directory_macos()
        if macos_result is not None:
            return macos_result
    return _choose_directory_tk()


def _choose_directory_macos() -> dict[str, Any] | None:
    script = 'POSIX path of (choose folder with prompt "Choose a subtitle folder")'
    try:
        result = subprocess.run(
            ["osascript", "-e", script],
            capture_output=True,
            check=False,
            text=True,
            timeout=300,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None

    if result.returncode == 0:
        selected_directory = result.stdout.strip()
        return {"available": True, "path": selected_directory or None}
    if "-128" in result.stderr:
        return {"available": True, "path": None}
    return None


def _choose_directory_tk() -> dict[str, Any]:
    if threading.current_thread() is not threading.main_thread():
        return {"available": False, "code": "folder_picker_unavailable"}

    try:
        import tkinter
        from tkinter import filedialog
    except ImportError:
        return {"available": False, "code": "folder_picker_unavailable"}

    root: Any | None = None
    try:
        root = tkinter.Tk()
        root.withdraw()
        root.attributes("-topmost", True)
        selected_directory = filedialog.askdirectory(parent=root, mustexist=True)
    except tkinter.TclError:
        return {"available": False, "code": "folder_picker_unavailable"}
    finally:
        if root is not None:
            root.destroy()

    if not selected_directory:
        return {"available": True, "path": None}
    return {"available": True, "path": selected_directory}


class SubtitleWebServer(HTTPServer):
    allow_reuse_address = True

    def __init__(self, address: tuple[str, int]) -> None:
        super().__init__(address, SubtitleRequestHandler)
        self.plan_store = PlanStore()
        self._shutdown_requested = False
        self._shutdown_lock = threading.Lock()

    def request_shutdown(self) -> None:
        with self._shutdown_lock:
            if self._shutdown_requested:
                return
            self._shutdown_requested = True
        threading.Thread(target=self.shutdown, daemon=True).start()


class SubtitleRequestHandler(BaseHTTPRequestHandler):
    server: SubtitleWebServer

    def do_GET(self) -> None:
        path = urlparse(self.path).path
        if path == "/api/demo":
            self._send_json(HTTPStatus.OK, demo_payload())
            return
        static_file = STATIC_FILES.get(path)
        if static_file is None:
            self._send_error(ApiError("not_found", HTTPStatus.NOT_FOUND))
            return
        file_name, content_type = static_file
        self._send_static(file_name, content_type)

    def do_POST(self) -> None:
        path = urlparse(self.path).path
        try:
            if path == "/api/shutdown":
                self._handle_shutdown()
                return
            payload = self._read_json()
            if path == "/api/preview":
                self._handle_preview(payload)
            elif path == "/api/apply":
                self._handle_apply(payload)
            elif path == "/api/choose-directory":
                self._send_json(HTTPStatus.OK, _choose_directory())
            else:
                raise ApiError("not_found", HTTPStatus.NOT_FOUND)
        except ApiError as error:
            self._send_error(error)

    def _handle_preview(self, payload: dict[str, Any]) -> None:
        directory = payload.get("directory")
        if not isinstance(directory, str) or not directory.strip():
            raise ApiError("invalid_directory", HTTPStatus.BAD_REQUEST)

        recursive = _parse_bool(payload.get("recursive", False), "recursive")
        strict = _parse_bool(payload.get("strict", False), "strict")
        min_score = _parse_min_score(payload.get("min_score", 0.72))
        try:
            plan = plan_directory(
                Path(directory),
                recursive=recursive,
                strict=strict,
                min_score=min_score,
            )
        except (OSError, ValueError) as error:
            raise ApiError("invalid_directory", HTTPStatus.BAD_REQUEST, str(error)) from error

        stored_plan = self.server.plan_store.create(plan)
        self._send_json(HTTPStatus.OK, _plan_payload(stored_plan))

    def _handle_apply(self, payload: dict[str, Any]) -> None:
        plan_id = payload.get("plan_id")
        operation_ids = payload.get("operation_ids")
        if not isinstance(plan_id, str) or not plan_id:
            raise ApiError("missing_plan_id", HTTPStatus.BAD_REQUEST)
        if not isinstance(operation_ids, list) or not all(
            isinstance(operation_id, str) for operation_id in operation_ids
        ):
            raise ApiError("invalid_operation_selection", HTTPStatus.BAD_REQUEST)

        result = self.server.plan_store.apply(plan_id, operation_ids)
        status = "completed" if not result["failures"] else "partial"
        self._send_json(HTTPStatus.OK, {"status": status, **result})

    def _handle_shutdown(self) -> None:
        self._send_json(HTTPStatus.OK, {"status": "shutting_down"})
        self.server.request_shutdown()

    def _read_json(self) -> dict[str, Any]:
        content_length = self.headers.get("Content-Length")
        if content_length is None:
            raise ApiError("invalid_json", HTTPStatus.BAD_REQUEST)
        try:
            body_size = int(content_length)
        except ValueError as error:
            raise ApiError("invalid_json", HTTPStatus.BAD_REQUEST) from error
        if body_size < 0 or body_size > MAX_REQUEST_BYTES:
            raise ApiError("request_too_large", HTTPStatus.REQUEST_ENTITY_TOO_LARGE)

        try:
            payload = json.loads(self.rfile.read(body_size).decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ApiError("invalid_json", HTTPStatus.BAD_REQUEST) from error
        if not isinstance(payload, dict):
            raise ApiError("invalid_json", HTTPStatus.BAD_REQUEST)
        return payload

    def _send_static(self, file_name: str, content_type: str) -> None:
        try:
            body = (ASSET_DIR / file_name).read_bytes()
        except OSError:
            self._send_error(ApiError("asset_unavailable", HTTPStatus.INTERNAL_SERVER_ERROR))
            return

        self.send_response(HTTPStatus.OK)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def _send_json(self, status: HTTPStatus, payload: dict[str, Any]) -> None:
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def _send_error(self, error: ApiError) -> None:
        payload: dict[str, Any] = {"error": {"code": error.code}}
        if error.detail:
            payload["error"]["detail"] = error.detail
        self._send_json(error.status, payload)

    def log_message(self, _format: str, *_args: object) -> None:
        return


def create_server(*, host: str = "127.0.0.1", port: int = 8765) -> SubtitleWebServer:
    return SubtitleWebServer((host, port))


def _parse_port(value: str) -> int:
    try:
        port = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("port must be an integer") from error
    if not 0 <= port <= 65535:
        raise argparse.ArgumentTypeError("port must be between 0 and 65535")
    return port


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="rename-subs-web",
        description="Launch the local Subtitle Renamer web workspace.",
    )
    parser.add_argument("--port", type=_parse_port, default=8765, help="Local port to listen on.")
    parser.add_argument(
        "--no-browser",
        action="store_true",
        help="Start the local server without opening a browser window.",
    )
    args = parser.parse_args(list(argv) if argv is not None else None)

    try:
        server = create_server(port=args.port)
    except OSError as error:
        print(f"Error: unable to start local server: {error}")
        return 1

    host, port = server.server_address[:2]
    url = f"http://{host}:{port}/"
    print(f"Subtitle Renamer is available at {url}")
    if not args.no_browser:
        webbrowser.open(url)

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nStopping Subtitle Renamer.")
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
