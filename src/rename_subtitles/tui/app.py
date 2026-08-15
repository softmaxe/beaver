"""The Textual application: a three-step workspace for renaming subtitles.

The steps mirror the shape of the task itself — point at a directory, read a
preview that touches nothing, then tick off the renames you actually want. The
apply step is the only one that writes, and it re-checks the filesystem first.
"""

from __future__ import annotations

import sys
from dataclasses import replace
from pathlib import Path
from typing import ClassVar

from textual import on, work
from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical, VerticalScroll
from textual.content import Content
from textual.widgets import (
    Button,
    DataTable,
    Footer,
    Header,
    Input,
    Label,
    RadioButton,
    RadioSet,
    SelectionList,
    Static,
    Switch,
    TabbedContent,
    TabPane,
)
from textual.widgets.selection_list import Selection

from ..applying import (
    ApplyResult,
    PlanChangedError,
    PreparedOperation,
    apply_operations,
    display_path,
    prepare_operations,
)
from ..i18n import DEFAULT_LANGUAGE, Translator
from ..planning import RenamePlan, plan_directory
from ..presentation import (
    DEFAULT_MATCH_LEVEL,
    demo_plan,
    match_kind,
    match_level_score,
    skip_reason_code,
)
from .screens import ConfirmApplyScreen, DirectoryPickerScreen, HelpScreen
from .themes import THEMES

__all__ = ["RenameSubtitlesApp", "main"]

MATCH_LEVEL_ORDER: tuple[str, ...] = ("relaxed", "balanced", "cautious")
CONFIRM_EXAMPLE_LIMIT = 5

_DEFAULT_TRANSLATOR = Translator(DEFAULT_LANGUAGE)


class RenameSubtitlesApp(App[None]):
    """Preview and apply subtitle renames without leaving the terminal."""

    CSS_PATH = "app.tcss"
    ENABLE_COMMAND_PALETTE = False

    # Below 100 columns the two panes cannot sit side by side; app.tcss stacks them.
    HORIZONTAL_BREAKPOINTS: ClassVar[list[tuple[int, str]]] = [(0, "-narrow"), (100, "-wide")]

    #: key, action, translation key, show in footer.
    BINDING_SPECS: tuple[tuple[str, str, str, bool], ...] = (
        ("p", "preview", "action.preview", True),
        ("a", "apply", "action.apply", True),
        ("d", "demo", "action.demo", True),
        ("o", "browse", "action.browse", True),
        ("ctrl+a", "select_all", "action.select_all", False),
        ("ctrl+r", "clear_all", "action.clear_all", False),
        ("l", "toggle_language", "action.language", True),
        ("t", "toggle_theme", "action.theme", True),
        ("question_mark", "help", "action.help", True),
        ("q", "quit", "action.quit", True),
    )

    BINDINGS: ClassVar[list[Binding]] = [
        Binding(key, action, _DEFAULT_TRANSLATOR(description), show=show)
        for key, action, description, show in BINDING_SPECS
    ]

    def __init__(self, *, language: str = DEFAULT_LANGUAGE) -> None:
        super().__init__()
        self._ = Translator(language)
        self._plan: RenamePlan | None = None
        self._prepared: tuple[PreparedOperation, ...] = ()
        self._pending: tuple[PreparedOperation, ...] = ()
        self._is_demo = False
        self._is_stale = False
        self._scanning = False

    # ------------------------------------------------------------------ layout

    def compose(self) -> ComposeResult:
        yield Header()
        with Horizontal(id="workspace"):
            with VerticalScroll(id="setup"):
                yield Label("", id="setup-heading", classes="section-heading")
                yield Label("", id="label-directory")
                yield Input(id="directory")
                yield Button("", id="browse", classes="wide-button")
                with Horizontal(classes="option-row"):
                    yield Switch(id="recursive")
                    yield Label("", id="label-recursive", classes="option-label")
                with Horizontal(classes="option-row"):
                    yield Switch(id="strict")
                    yield Label("", id="label-strict", classes="option-label")
                yield Static("", id="hint-strict", classes="hint")
                yield Label("", id="label-match-level")
                with RadioSet(id="match-level"):
                    yield RadioButton("", id="level-relaxed")
                    yield RadioButton("", value=True, id="level-balanced")
                    yield RadioButton("", id="level-cautious")
                yield Static("", id="hint-level", classes="hint")
                yield Button("", variant="primary", id="preview", classes="wide-button")
                yield Button("", variant="success", id="apply", classes="wide-button", disabled=True)
                yield Button("", id="demo", classes="wide-button")
            with Vertical(id="results"):
                yield Static("", id="summary")
                with TabbedContent(id="tabs"):
                    with TabPane("", id="tab-matched"):
                        yield SelectionList[str](id="operations")
                    with TabPane("", id="tab-skipped"):
                        yield DataTable(id="skipped", cursor_type="row", zebra_stripes=True)
                yield Static("", id="detail")
                yield Static("", id="status")
        yield Footer()

    def on_mount(self) -> None:
        for theme in THEMES:
            self.register_theme(theme)
        self.theme = THEMES[0].name
        self._refresh_texts()
        self._set_status("status.ready")
        self.query_one("#directory", Input).focus()

    # ------------------------------------------------------- translated labels

    def _refresh_texts(self) -> None:
        """Re-render every piece of text after a language change."""
        translate = self._
        self.title = translate("app.title")
        self.sub_title = translate("app.subtitle")

        self.query_one("#setup-heading", Label).update(translate("setup.heading"))
        self.query_one("#label-directory", Label).update(translate("setup.directory_label"))
        self.query_one("#label-recursive", Label).update(translate("setup.recursive"))
        self.query_one("#label-strict", Label).update(translate("setup.strict"))
        self.query_one("#label-match-level", Label).update(translate("setup.match_level"))
        self.query_one("#hint-strict", Static).update(translate("setup.strict_hint"))
        self.query_one("#directory", Input).placeholder = translate("setup.directory_placeholder")

        self.query_one("#browse", Button).label = translate("setup.browse")
        self.query_one("#preview", Button).label = translate("action.preview")
        self.query_one("#apply", Button).label = translate("action.apply")
        self.query_one("#demo", Button).label = translate("action.demo")

        for level in MATCH_LEVEL_ORDER:
            self.query_one(f"#level-{level}", RadioButton).label = translate(f"setup.level_{level}")
        self._refresh_level_hint()

        self._refresh_tab_labels()
        self._render_plan()
        self._refresh_binding_descriptions()

    def _refresh_tab_labels(self) -> None:
        tabs = self.query_one("#tabs", TabbedContent)
        matched = len(self._prepared) if self._plan else 0
        skipped = len(self._plan.skipped) if self._plan else 0
        tabs.get_tab("tab-matched").label = self._("tab.matched", count=matched)
        tabs.get_tab("tab-skipped").label = self._("tab.skipped", count=skipped)

    def _refresh_level_hint(self) -> None:
        level = self._selected_level()
        self.query_one("#hint-level", Static).update(self._(f"setup.level_hint_{level}"))

    def _refresh_binding_descriptions(self) -> None:
        """Retranslate the footer in place, leaving inherited bindings untouched."""
        wanted = {(key, action): description for key, action, description, _ in self.BINDING_SPECS}
        key_to_bindings = self._bindings.key_to_bindings
        for key, bindings in key_to_bindings.items():
            key_to_bindings[key] = [
                replace(binding, description=self._(wanted[(key, binding.action)]))
                if (key, binding.action) in wanted
                else binding
                for binding in bindings
            ]
        self.refresh_bindings()

    # ------------------------------------------------------------ scan options

    def _selected_level(self) -> str:
        index = self.query_one("#match-level", RadioSet).pressed_index
        if 0 <= index < len(MATCH_LEVEL_ORDER):
            return MATCH_LEVEL_ORDER[index]
        return DEFAULT_MATCH_LEVEL

    # --------------------------------------------------------------- rendering

    def _render_plan(self) -> None:
        self._render_operations()
        self._render_skipped()
        self._render_summary()
        self._render_detail()
        self._refresh_tab_labels()
        self._update_apply_enabled()

    def _render_detail(self) -> None:
        """Spell out the highlighted rename in full, since prompts elide long paths."""
        detail = self.query_one("#detail", Static)
        index = self.query_one("#operations", SelectionList).highlighted
        if self._plan is None or index is None or not 0 <= index < len(self._prepared):
            detail.update("")
            return
        prepared = self._prepared[index]
        root = self._plan.root
        detail.update(
            f"{display_path(prepared.source, root)}  →  {display_path(prepared.destination, root)}"
        )

    def _render_operations(self) -> None:
        options = self.query_one("#operations", SelectionList)
        selected = set(options.selected)
        highlighted = options.highlighted
        options.clear_options()
        if self._plan is None:
            return

        root = self._plan.root
        # A fresh plan starts fully ticked; a re-render (language switch) keeps
        # whatever the user had already unticked.
        fresh = not selected
        options.add_options(
            Selection(
                self._operation_prompt(prepared, root),
                prepared.operation_id,
                fresh or prepared.operation_id in selected,
            )
            for prepared in self._prepared
        )
        # Highlight something straight away so the detail line is never blank
        # while the list has content.
        if self._prepared:
            options.highlighted = min(highlighted or 0, len(self._prepared) - 1)

    def _operation_prompt(self, prepared: PreparedOperation, root: Path) -> Content:
        kind, detail = match_kind(prepared.operation)
        badge = self._(f"match.{kind}", detail=detail)
        style = "$accent" if kind == "episode" else "$text-muted"
        return Content.from_markup(
            "$source [$text-muted]→[/] [b]$target[/]  [" + style + "]$badge[/]",
            source=display_path(prepared.source, root),
            target=prepared.destination.name,
            badge=badge,
        )

    def _render_skipped(self) -> None:
        table = self.query_one("#skipped", DataTable)
        table.clear(columns=True)
        table.add_columns(self._("table.source"), self._("table.reason"))
        if self._plan is None:
            return
        root = self._plan.root
        for item in self._plan.skipped:
            table.add_row(
                display_path(item.path, root),
                self._(f"skip.{skip_reason_code(item)}"),
            )

    def _render_summary(self) -> None:
        summary = self.query_one("#summary", Static)
        if self._plan is None:
            summary.update(self._("summary.empty"))
            return
        plan = self._plan
        parts = [
            self._(
                "summary.headline",
                videos=plan.video_count,
                subtitles=plan.subtitle_count,
                matched=plan.matched_count,
                skipped=plan.skipped_count,
            ),
            self._("summary.directories", directories=plan.directory_count),
        ]
        options = self.query_one("#operations", SelectionList)
        if options.option_count:
            parts.append(
                self._(
                    "summary.selected",
                    selected=len(options.selected),
                    total=options.option_count,
                )
            )
        summary.update(" · ".join(parts))

    def _set_status(self, key: str, *, error: bool = False, **kwargs: object) -> None:
        status = self.query_one("#status", Static)
        status.update(self._(key, **kwargs))
        status.set_class(error, "-error")

    def _update_apply_enabled(self) -> None:
        has_selection = bool(self.query_one("#operations", SelectionList).selected)
        ready = (
            self._plan is not None
            and not self._is_demo
            and not self._is_stale
            and not self._scanning
            and has_selection
        )
        self.query_one("#apply", Button).disabled = not ready

    # ------------------------------------------------------------ interactions

    @on(Input.Changed, "#directory")
    @on(Switch.Changed)
    def _options_changed(self) -> None:
        self._invalidate_preview()

    @on(Input.Submitted, "#directory")
    def _directory_submitted(self) -> None:
        # Single-letter shortcuts type into a focused Input, so Enter is the way
        # to start a preview without leaving the path field.
        self.action_preview()

    @on(RadioSet.Changed, "#match-level")
    def _level_changed(self) -> None:
        self._refresh_level_hint()
        self._invalidate_preview()

    def _invalidate_preview(self) -> None:
        """A preview only describes the options it was generated from."""
        if self._plan is None or self._is_stale:
            return
        self._is_stale = True
        self._update_apply_enabled()
        self._set_status("status.stale")

    @on(SelectionList.SelectedChanged, "#operations")
    def _selection_changed(self) -> None:
        # The selected count belongs in the summary bar; the status line is
        # reserved for what just happened, so ticking never eats an error.
        self._update_apply_enabled()
        self._render_summary()

    @on(SelectionList.SelectionHighlighted, "#operations")
    def _highlight_changed(self) -> None:
        self._render_detail()

    @on(Button.Pressed, "#preview")
    def _preview_pressed(self) -> None:
        self.action_preview()

    @on(Button.Pressed, "#apply")
    def _apply_pressed(self) -> None:
        self.action_apply()

    @on(Button.Pressed, "#demo")
    def _demo_pressed(self) -> None:
        self.action_demo()

    @on(Button.Pressed, "#browse")
    def _browse_pressed(self) -> None:
        self.action_browse()

    # ----------------------------------------------------------------- actions

    def action_preview(self) -> None:
        raw = self.query_one("#directory", Input).value.strip()
        if not raw:
            self._set_status("error.empty_directory", error=True)
            return
        try:
            directory = Path(raw).expanduser().resolve()
        except OSError:
            self._set_status("error.invalid_directory", error=True, path=raw)
            return
        if not directory.is_dir():
            self._set_status("error.invalid_directory", error=True, path=raw)
            return

        self._scanning = True
        self._set_status("status.scanning")
        self.query_one("#preview", Button).disabled = True
        self.query_one("#results", Vertical).loading = True
        self._scan(
            directory,
            recursive=self.query_one("#recursive", Switch).value,
            strict=self.query_one("#strict", Switch).value,
            min_score=match_level_score(self._selected_level()),
        )

    def action_demo(self) -> None:
        self._plan = demo_plan()
        self._prepared = prepare_operations(self._plan)
        self._is_demo = True
        self._is_stale = False
        self._render_plan()
        self._set_status("status.demo")

    def action_apply(self) -> None:
        if self._is_demo:
            self._set_status("error.demo_not_applicable", error=True)
            return
        if self._plan is None:
            self._set_status("status.ready", error=True)
            return
        if self._is_stale:
            self._set_status("status.stale", error=True)
            return

        selected = set(self.query_one("#operations", SelectionList).selected)
        chosen = tuple(p for p in self._prepared if p.operation_id in selected)
        if not chosen:
            self._set_status("error.nothing_selected", error=True)
            return

        root = self._plan.root
        examples = [
            f"{display_path(p.source, root)}  →  {p.destination.name}"
            for p in chosen[:CONFIRM_EXAMPLE_LIMIT]
        ]
        self._pending = chosen
        self.push_screen(
            ConfirmApplyScreen(self._, examples, len(chosen)),
            self._confirmed_apply,
        )

    def action_browse(self) -> None:
        raw = self.query_one("#directory", Input).value.strip()
        start = Path(raw).expanduser() if raw else Path.cwd()
        if not start.is_dir():
            start = Path.cwd()
        self.push_screen(DirectoryPickerScreen(self._, start), self._picked_directory)

    def action_help(self) -> None:
        self.push_screen(HelpScreen(self._))

    def action_select_all(self) -> None:
        self.query_one("#operations", SelectionList).select_all()

    def action_clear_all(self) -> None:
        self.query_one("#operations", SelectionList).deselect_all()

    def action_toggle_language(self) -> None:
        self._.toggle()
        self._refresh_texts()
        self._restore_status()

    def action_toggle_theme(self) -> None:
        names = [theme.name for theme in THEMES]
        index = names.index(self.theme) if self.theme in names else 0
        self.theme = names[(index + 1) % len(names)]

    def _restore_status(self) -> None:
        if self._is_demo:
            self._set_status("status.demo")
        elif self._is_stale:
            self._set_status("status.stale")
        elif self._plan is None:
            self._set_status("status.ready")
        else:
            self._set_status("status.previewed")

    # ----------------------------------------------------- modal screen results

    def _picked_directory(self, directory: Path | None) -> None:
        if directory is None:
            return
        self.query_one("#directory", Input).value = str(directory)

    def _confirmed_apply(self, confirmed: bool | None) -> None:
        if not confirmed or self._plan is None:
            self._pending = ()
            return
        self._set_status("status.applying")
        self.query_one("#apply", Button).disabled = True
        self._apply(self._pending, self._plan.root)

    # ----------------------------------------------------------------- workers

    @work(thread=True, exclusive=True, group="scan")
    def _scan(self, directory: Path, *, recursive: bool, strict: bool, min_score: float) -> None:
        try:
            plan = plan_directory(
                directory,
                recursive=recursive,
                strict=strict,
                min_score=min_score,
            )
        except (OSError, ValueError) as error:
            self.call_from_thread(self._scan_failed, str(error))
        else:
            self.call_from_thread(self._scan_succeeded, plan)

    def _finish_scan(self) -> None:
        self._scanning = False
        self.query_one("#preview", Button).disabled = False
        self.query_one("#results", Vertical).loading = False

    def _scan_succeeded(self, plan: RenamePlan) -> None:
        self._plan = plan
        self._prepared = prepare_operations(plan)
        self._is_demo = False
        self._is_stale = False
        self._finish_scan()
        # Re-rendering from scratch, so nothing carries over from the last plan.
        self.query_one("#operations", SelectionList).clear_options()
        self._render_plan()

        if plan.video_count == 0:
            self._set_status("error.no_video", error=True)
        elif plan.subtitle_count == 0:
            self._set_status("error.no_subtitle", error=True)
        else:
            self._restore_status()

        # Move focus off the path field so the single-letter shortcuts work.
        if self._prepared:
            self.query_one("#operations", SelectionList).focus()

    def _scan_failed(self, detail: str) -> None:
        self._finish_scan()
        self._set_status("error.scan_failed", error=True, detail=detail)

    @work(thread=True, exclusive=True, group="apply")
    def _apply(self, operations: tuple[PreparedOperation, ...], root: Path) -> None:
        try:
            result = apply_operations(operations, root, force=False, verify=True)
        except PlanChangedError as error:
            self.call_from_thread(self._apply_blocked, "; ".join(error.changes))
        else:
            self.call_from_thread(self._apply_finished, result)

    def _apply_finished(self, result: ApplyResult) -> None:
        self._pending = ()
        # The files just moved, so the preview no longer describes what is on disk.
        self._is_stale = True
        self._update_apply_enabled()
        self._set_status(
            f"result.{result.status}",
            error=result.status != "completed",
            applied=len(result.applied),
            failed=len(result.failed),
        )

    def _apply_blocked(self, detail: str) -> None:
        self._pending = ()
        self._is_stale = True
        self._update_apply_enabled()
        self._set_status("error.plan_changed", error=True, detail=detail)


def main() -> int:
    """Entry point for ``rename-subs-tui``."""
    RenameSubtitlesApp().run()
    return 0


if __name__ == "__main__":
    sys.exit(main())
