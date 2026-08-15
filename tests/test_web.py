from __future__ import annotations

import json
import threading
from collections.abc import Iterator
from pathlib import Path
from subprocess import CompletedProcess
from urllib.error import HTTPError
from urllib.request import Request, urlopen

import pytest

from rename_subtitles import web
from rename_subtitles.web import PlanStore, SubtitleWebServer, create_server


@pytest.fixture
def web_server() -> Iterator[SubtitleWebServer]:
    server = create_server(port=0)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield server
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


@pytest.fixture
def server_url(web_server: SubtitleWebServer) -> str:
    host, port = web_server.server_address[:2]
    return f"http://{host}:{port}"


def _request_json(
    server_url: str,
    path: str,
    payload: dict[str, object] | None = None,
) -> tuple[int, dict[str, object]]:
    data = None if payload is None else json.dumps(payload).encode("utf-8")
    request = Request(
        f"{server_url}{path}",
        data=data,
        headers={"Content-Type": "application/json"} if data is not None else {},
        method="POST" if data is not None else "GET",
    )
    try:
        with urlopen(request, timeout=2) as response:
            return response.status, json.loads(response.read().decode("utf-8"))
    except HTTPError as error:
        return error.code, json.loads(error.read().decode("utf-8"))


def _preview(server_url: str, directory: Path) -> dict[str, object]:
    status, payload = _request_json(
        server_url,
        "/api/preview",
        {"directory": str(directory), "recursive": False, "strict": False, "min_score": 0.72},
    )
    assert status == 200
    return payload


def test_workspace_assets_and_demo_are_available(server_url: str):
    with urlopen(f"{server_url}/", timeout=2) as response:
        page = response.read().decode("utf-8")
        assert response.status == 200
    assert "Subtitle Renamer" in page
    assert "preview-form" in page

    status, payload = _request_json(server_url, "/api/demo")

    assert status == 200
    assert payload["mode"] == "demo"
    assert payload["plan_id"] is None
    assert payload["summary"]["matched"] == 3
    assert len(payload["operations"]) == 3
    assert payload["skipped"][0]["reason_code"] == "unmatched"


def test_shutdown_endpoint_stops_the_server() -> None:
    server = create_server(port=0)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    host, port = server.server_address[:2]
    server_url = f"http://{host}:{port}"

    try:
        status, payload = _request_json(server_url, "/api/shutdown", {})

        assert status == 200
        assert payload == {"status": "shutting_down"}
        thread.join(timeout=2)
        assert not thread.is_alive()
    finally:
        if thread.is_alive():
            server.shutdown()
            thread.join(timeout=2)
        server.server_close()


def test_workspace_requests_server_shutdown_when_the_page_closes():
    app_script = (web.ASSET_DIR / "app.js").read_text(encoding="utf-8")

    assert 'navigator.sendBeacon("/api/shutdown")' in app_script
    assert 'window.addEventListener("pagehide", (event) => {' in app_script


def test_preview_and_apply_selected_operations(server_url: str, tmp_path: Path):
    video = tmp_path / "Show.S01E01.1080p.mkv"
    subtitle = tmp_path / "Show.S01E01.eng.srt"
    target = tmp_path / "Show.S01E01.1080p.srt"
    video.touch()
    subtitle.touch()

    preview = _preview(server_url, tmp_path)

    assert preview["mode"] == "live"
    assert preview["summary"]["matched"] == 1
    operation = preview["operations"][0]
    assert operation["source"] == subtitle.name
    assert operation["target"] == target.name

    status, result = _request_json(
        server_url,
        "/api/apply",
        {"plan_id": preview["plan_id"], "operation_ids": [operation["id"]]},
    )

    assert status == 200
    assert result["status"] == "completed"
    assert len(result["applied"]) == 1
    assert target.exists()
    assert not subtitle.exists()

    status, repeated_result = _request_json(
        server_url,
        "/api/apply",
        {"plan_id": preview["plan_id"], "operation_ids": [operation["id"]]},
    )
    assert status == 409
    assert repeated_result["error"]["code"] == "plan_already_used"


def test_apply_rejects_an_unsigned_operation_id(server_url: str, tmp_path: Path):
    subtitle = tmp_path / "Show.S01E01.eng.srt"
    (tmp_path / "Show.S01E01.mkv").touch()
    subtitle.touch()
    preview = _preview(server_url, tmp_path)

    status, result = _request_json(
        server_url,
        "/api/apply",
        {
            "plan_id": preview["plan_id"],
            "operation_ids": ["not-signed-by-the-server"],
            "source": str(subtitle),
            "target": str(tmp_path / "unexpected.srt"),
        },
    )

    assert status == 400
    assert result["error"]["code"] == "invalid_operation_selection"
    assert subtitle.exists()


def test_apply_rejects_a_plan_when_the_target_changed(server_url: str, tmp_path: Path):
    (tmp_path / "Show.S01E01.1080p.mkv").touch()
    subtitle = tmp_path / "Show.S01E01.eng.srt"
    subtitle.touch()
    preview = _preview(server_url, tmp_path)
    operation = preview["operations"][0]
    (tmp_path / operation["target"]).touch()

    status, result = _request_json(
        server_url,
        "/api/apply",
        {"plan_id": preview["plan_id"], "operation_ids": [operation["id"]]},
    )

    assert status == 409
    assert result["error"]["code"] == "plan_changed"
    assert subtitle.exists()


def test_preview_rejects_an_invalid_directory(server_url: str, tmp_path: Path):
    status, result = _request_json(
        server_url,
        "/api/preview",
        {
            "directory": str(tmp_path / "missing"),
            "recursive": False,
            "strict": False,
            "min_score": 0.72,
        },
    )

    assert status == 400
    assert result["error"]["code"] == "invalid_directory"


def test_macos_folder_picker_returns_the_native_selected_path(monkeypatch: pytest.MonkeyPatch):
    selected_path = "/Users/example/Media"

    def fake_run(command: list[str], **_kwargs: object) -> CompletedProcess[str]:
        assert command[0] == "osascript"
        assert "choose folder" in command[2]
        return CompletedProcess(command, 0, stdout=f"{selected_path}\n", stderr="")

    monkeypatch.setattr(web.sys, "platform", "darwin")
    monkeypatch.setattr(web.subprocess, "run", fake_run)

    assert web._choose_directory() == {"available": True, "path": selected_path}


def test_macos_folder_picker_handles_cancel_without_an_error(monkeypatch: pytest.MonkeyPatch):
    def fake_run(command: list[str], **_kwargs: object) -> CompletedProcess[str]:
        return CompletedProcess(command, 1, stdout="", stderr="User canceled. (-128)")

    monkeypatch.setattr(web.sys, "platform", "darwin")
    monkeypatch.setattr(web.subprocess, "run", fake_run)

    assert web._choose_directory() == {"available": True, "path": None}


def test_macos_folder_picker_falls_back_when_osascript_is_unavailable(
    monkeypatch: pytest.MonkeyPatch,
):
    def fail_run(*_args: object, **_kwargs: object) -> CompletedProcess[str]:
        raise OSError("osascript unavailable")

    fallback_result = {"available": False, "code": "folder_picker_unavailable"}
    monkeypatch.setattr(web.sys, "platform", "darwin")
    monkeypatch.setattr(web.subprocess, "run", fail_run)
    monkeypatch.setattr(web, "_choose_directory_tk", lambda: fallback_result)

    assert web._choose_directory() == fallback_result


def test_choose_directory_endpoint_returns_the_selected_path(
    server_url: str,
    monkeypatch: pytest.MonkeyPatch,
):
    selected_path = "/Users/example/Media"
    monkeypatch.setattr(
        web,
        "_choose_directory",
        lambda: {"available": True, "path": selected_path},
    )

    status, payload = _request_json(server_url, "/api/choose-directory", {})

    assert status == 200
    assert payload == {"available": True, "path": selected_path}


def test_expired_plan_is_rejected_by_the_http_api(
    server_url: str,
    web_server: SubtitleWebServer,
    tmp_path: Path,
):
    (tmp_path / "Show.S01E01.mkv").touch()
    (tmp_path / "Show.S01E01.eng.srt").touch()
    web_server.plan_store = PlanStore(ttl_seconds=-1)

    preview = _preview(server_url, tmp_path)
    operation = preview["operations"][0]

    status, result = _request_json(
        server_url,
        "/api/apply",
        {"plan_id": preview["plan_id"], "operation_ids": [operation["id"]]},
    )

    assert status == 404
    assert result["error"]["code"] == "invalid_or_expired_plan"
