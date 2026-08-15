"""End-to-end tests driven through Textual's Pilot.

These exercise the real widgets against real files in ``tmp_path``: when a test
says a rename happened, it happened on disk.
"""

from pathlib import Path

from textual.pilot import Pilot
from textual.widgets import Button, DataTable, Input, SelectionList, Static, Switch

from rename_subtitles.i18n import CATALOG
from rename_subtitles.tui.app import RenameSubtitlesApp
from rename_subtitles.tui.screens import ConfirmApplyScreen, DirectoryPickerScreen
from rename_subtitles.tui.themes import SUBTITLE_DARK, SUBTITLE_LIGHT

TERMINAL_SIZE = (120, 40)


def build_library(root: Path) -> None:
    """Two clean episode matches plus one subtitle with nothing to match."""
    for episode in ("01", "02"):
        (root / f"Show.S01E{episode}.1080p.mkv").write_text("video")
    (root / "Show.S01E01.chs.srt").write_text("subtitle one")
    (root / "Show.S01E02.eng.ass").write_text("subtitle two")
    (root / "Orphan.Documentary.srt").write_text("subtitle three")


def widget_text(app: RenameSubtitlesApp, selector: str) -> str:
    return str(app.query_one(selector, Static).content)


async def preview(app: RenameSubtitlesApp, pilot: Pilot, directory: Path) -> None:
    app.query_one("#directory", Input).value = str(directory)
    await pilot.pause()
    app.action_preview()
    await app.workers.wait_for_complete()
    await pilot.pause()


async def test_preview_then_apply_renames_files_on_disk(tmp_path: Path):
    build_library(tmp_path)
    app = RenameSubtitlesApp()

    async with app.run_test(size=TERMINAL_SIZE) as pilot:
        await preview(app, pilot, tmp_path)

        options = app.query_one("#operations", SelectionList)
        assert options.option_count == 2
        assert len(options.selected) == 2
        assert app.query_one("#skipped", DataTable).row_count == 1
        assert not app.query_one("#apply", Button).disabled
        assert "2" in widget_text(app, "#summary")

        app.action_apply()
        await pilot.pause()
        await pilot.press("enter")
        await app.workers.wait_for_complete()
        await pilot.pause()

        assert widget_text(app, "#status") == CATALOG["zh"]["result.completed"].format(applied=2)

    assert (tmp_path / "Show.S01E01.1080p.srt").read_text() == "subtitle one"
    assert (tmp_path / "Show.S01E02.1080p.ass").read_text() == "subtitle two"
    assert not (tmp_path / "Show.S01E01.chs.srt").exists()
    assert (tmp_path / "Orphan.Documentary.srt").exists()


async def test_cancelling_the_confirmation_leaves_the_files_alone(tmp_path: Path):
    build_library(tmp_path)
    app = RenameSubtitlesApp()

    async with app.run_test(size=TERMINAL_SIZE) as pilot:
        await preview(app, pilot, tmp_path)
        app.action_apply()
        await pilot.pause()
        await pilot.press("escape")
        await pilot.pause()

    assert (tmp_path / "Show.S01E01.chs.srt").exists()
    assert not (tmp_path / "Show.S01E01.1080p.srt").exists()


async def test_apply_is_refused_once_the_files_have_moved(tmp_path: Path):
    build_library(tmp_path)
    app = RenameSubtitlesApp()

    async with app.run_test(size=TERMINAL_SIZE) as pilot:
        await preview(app, pilot, tmp_path)

        # Something else renames a source file behind the interface's back.
        (tmp_path / "Show.S01E01.chs.srt").rename(tmp_path / "moved.srt")

        app.action_apply()
        await pilot.pause()
        await pilot.press("enter")
        await app.workers.wait_for_complete()
        await pilot.pause()

        assert "Show.S01E01.chs.srt" in widget_text(app, "#status")
        assert app.query_one("#status", Static).has_class("-error")

    # Neither operation ran: a stale plan renames nothing at all.
    assert not (tmp_path / "Show.S01E01.1080p.srt").exists()
    assert not (tmp_path / "Show.S01E02.1080p.ass").exists()
    assert (tmp_path / "Show.S01E02.eng.ass").exists()


async def test_a_missing_directory_reports_an_error(tmp_path: Path):
    app = RenameSubtitlesApp()

    async with app.run_test(size=TERMINAL_SIZE) as pilot:
        missing = tmp_path / "nowhere"
        app.query_one("#directory", Input).value = str(missing)
        await pilot.pause()
        app.action_preview()
        await pilot.pause()

        assert str(missing) in widget_text(app, "#status")
        assert app.query_one("#status", Static).has_class("-error")
        assert app.query_one("#operations", SelectionList).option_count == 0


async def test_an_empty_directory_field_reports_an_error():
    app = RenameSubtitlesApp()

    async with app.run_test(size=TERMINAL_SIZE) as pilot:
        app.action_preview()
        await pilot.pause()

        assert widget_text(app, "#status") == CATALOG["zh"]["error.empty_directory"]


async def test_changing_an_option_invalidates_the_preview(tmp_path: Path):
    build_library(tmp_path)
    app = RenameSubtitlesApp()

    async with app.run_test(size=TERMINAL_SIZE) as pilot:
        await preview(app, pilot, tmp_path)
        assert not app.query_one("#apply", Button).disabled

        app.query_one("#recursive", Switch).value = True
        await pilot.pause()

        assert app.query_one("#apply", Button).disabled
        assert widget_text(app, "#status") == CATALOG["zh"]["status.stale"]

        app.action_apply()
        await pilot.pause()
        assert not isinstance(app.screen, ConfirmApplyScreen)

    assert (tmp_path / "Show.S01E01.chs.srt").exists()


async def test_clearing_the_selection_disables_apply(tmp_path: Path):
    build_library(tmp_path)
    app = RenameSubtitlesApp()

    async with app.run_test(size=TERMINAL_SIZE) as pilot:
        await preview(app, pilot, tmp_path)

        app.action_clear_all()
        await pilot.pause()
        assert app.query_one("#apply", Button).disabled

        app.action_select_all()
        await pilot.pause()
        assert not app.query_one("#apply", Button).disabled


async def test_demo_mode_shows_a_plan_but_writes_nothing(tmp_path: Path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    app = RenameSubtitlesApp()

    async with app.run_test(size=TERMINAL_SIZE) as pilot:
        app.action_demo()
        await pilot.pause()

        assert app.query_one("#operations", SelectionList).option_count == 3
        assert app.query_one("#skipped", DataTable).row_count == 1
        assert widget_text(app, "#status") == CATALOG["zh"]["status.demo"]
        # Demo plans point at paths that do not exist, so applying is refused.
        assert app.query_one("#apply", Button).disabled

        app.action_apply()
        await pilot.pause()
        assert widget_text(app, "#status") == CATALOG["zh"]["error.demo_not_applicable"]

    assert list(tmp_path.iterdir()) == []


async def test_language_toggle_retranslates_labels_and_footer(tmp_path: Path):
    build_library(tmp_path)
    app = RenameSubtitlesApp()

    async with app.run_test(size=TERMINAL_SIZE) as pilot:
        await preview(app, pilot, tmp_path)
        assert app.title == CATALOG["zh"]["app.title"]

        app.action_toggle_language()
        await pilot.pause()

        assert app.title == CATALOG["en"]["app.title"]
        assert str(app.query_one("#preview", Button).label) == CATALOG["en"]["action.preview"]
        assert str(app.query_one("#level-cautious").label) == CATALOG["en"]["setup.level_cautious"]

        descriptions = {
            binding.description
            for bindings in app._bindings.key_to_bindings.values()
            for binding in bindings
        }
        assert CATALOG["en"]["action.preview"] in descriptions
        assert CATALOG["zh"]["action.preview"] not in descriptions

        # The plan itself survives a language change.
        assert app.query_one("#operations", SelectionList).option_count == 2
        assert not app.query_one("#apply", Button).disabled


async def test_language_toggle_keeps_the_user_selection(tmp_path: Path):
    build_library(tmp_path)
    app = RenameSubtitlesApp()

    async with app.run_test(size=TERMINAL_SIZE) as pilot:
        await preview(app, pilot, tmp_path)
        options = app.query_one("#operations", SelectionList)
        options.deselect(options.get_option_at_index(0).value)
        await pilot.pause()
        assert len(options.selected) == 1

        app.action_toggle_language()
        await pilot.pause()

        assert len(app.query_one("#operations", SelectionList).selected) == 1


async def test_theme_toggle_cycles_between_the_project_themes():
    app = RenameSubtitlesApp()

    async with app.run_test(size=TERMINAL_SIZE) as pilot:
        assert app.theme == SUBTITLE_DARK.name

        app.action_toggle_theme()
        await pilot.pause()
        assert app.theme == SUBTITLE_LIGHT.name

        app.action_toggle_theme()
        await pilot.pause()
        assert app.theme == SUBTITLE_DARK.name


async def test_narrow_terminals_stack_the_panes():
    app = RenameSubtitlesApp()

    async with app.run_test(size=(80, 24)) as pilot:
        app.action_demo()
        await pilot.pause()

        assert "-narrow" in app.screen.classes
        setup = app.query_one("#setup")
        results = app.query_one("#results")
        assert setup.region.width == results.region.width == 80
        # Both panes must remain on screen at the smallest supported size.
        assert results.region.height > 0
        assert setup.region.bottom <= results.region.y


async def test_wide_terminals_place_the_panes_side_by_side():
    app = RenameSubtitlesApp()

    async with app.run_test(size=TERMINAL_SIZE) as pilot:
        await pilot.pause()

        assert "-wide" in app.screen.classes
        setup = app.query_one("#setup")
        results = app.query_one("#results")
        assert setup.region.y == results.region.y
        assert setup.region.right <= results.region.x


async def test_the_directory_picker_fills_in_the_path(tmp_path: Path):
    build_library(tmp_path)
    season = tmp_path / "Season 2"
    season.mkdir()
    app = RenameSubtitlesApp()

    async with app.run_test(size=TERMINAL_SIZE) as pilot:
        app.query_one("#directory", Input).value = str(tmp_path)
        await pilot.pause()
        app.action_browse()
        await pilot.pause()

        assert isinstance(app.screen, DirectoryPickerScreen)
        await pilot.click("#picker-select")
        await pilot.pause()

        assert app.query_one("#directory", Input).value == str(tmp_path)
        assert not isinstance(app.screen, DirectoryPickerScreen)


async def test_cancelling_the_directory_picker_keeps_the_current_path(tmp_path: Path):
    app = RenameSubtitlesApp()

    async with app.run_test(size=TERMINAL_SIZE) as pilot:
        app.query_one("#directory", Input).value = str(tmp_path)
        await pilot.pause()
        app.action_browse()
        await pilot.pause()
        await pilot.press("escape")
        await pilot.pause()

        assert app.query_one("#directory", Input).value == str(tmp_path)
        assert not isinstance(app.screen, DirectoryPickerScreen)
