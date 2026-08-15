"""Modal screens: directory picker, apply confirmation, and the shortcut list."""

from __future__ import annotations

from pathlib import Path
from typing import ClassVar

from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import Button, DirectoryTree, Label, Static

from ..i18n import Translator

__all__ = ["ConfirmApplyScreen", "DirectoryPickerScreen", "HelpScreen"]


class DirectoryPickerScreen(ModalScreen[Path | None]):
    """Browse the filesystem and return the highlighted directory.

    Replaces the web version's platform-specific native dialog, which shelled out
    to osascript on macOS and imported tkinter everywhere else.
    """

    BINDINGS: ClassVar[list[Binding]] = [Binding("escape", "cancel", "Cancel", show=False)]

    def __init__(self, translator: Translator, start: Path) -> None:
        super().__init__()
        self._ = translator
        self._start = start
        self._selected = start

    def compose(self) -> ComposeResult:
        with Vertical(id="picker-dialog"):
            yield Label(self._("picker.title"), id="picker-title")
            yield DirectoryTree(self._start, id="picker-tree")
            yield Static(str(self._start), id="picker-current")
            with Horizontal(id="picker-actions"):
                yield Button(self._("picker.cancel"), id="picker-cancel")
                yield Button(self._("picker.select"), variant="primary", id="picker-select")

    def on_mount(self) -> None:
        self.query_one(DirectoryTree).focus()

    def on_directory_tree_directory_selected(
        self, event: DirectoryTree.DirectorySelected
    ) -> None:
        # Selecting expands the node rather than closing the modal, so browsing
        # deeper and confirming stay separate actions.
        self._selected = event.path
        self.query_one("#picker-current", Static).update(str(event.path))

    def on_button_pressed(self, event: Button.Pressed) -> None:
        self.dismiss(self._selected if event.button.id == "picker-select" else None)

    def action_cancel(self) -> None:
        self.dismiss(None)


class ConfirmApplyScreen(ModalScreen[bool]):
    """Last stop before anything on disk is touched."""

    BINDINGS: ClassVar[list[Binding]] = [Binding("escape", "cancel", "Cancel", show=False)]

    def __init__(self, translator: Translator, examples: list[str], count: int) -> None:
        super().__init__()
        self._ = translator
        self._examples = examples
        self._count = count

    def compose(self) -> ComposeResult:
        with Vertical(id="confirm-dialog"):
            yield Label(self._("confirm.title"), id="confirm-title")
            yield Static(self._("confirm.body", count=self._count), id="confirm-body")
            yield Static("\n".join(self._examples), id="confirm-examples")
            with Horizontal(id="confirm-actions"):
                yield Button(self._("confirm.cancel"), id="confirm-cancel")
                yield Button(self._("confirm.ok"), variant="primary", id="confirm-ok")

    def on_mount(self) -> None:
        self.query_one("#confirm-ok", Button).focus()

    def on_button_pressed(self, event: Button.Pressed) -> None:
        self.dismiss(event.button.id == "confirm-ok")

    def action_cancel(self) -> None:
        self.dismiss(False)


class HelpScreen(ModalScreen[None]):
    """The shortcut list, in the currently selected language."""

    BINDINGS: ClassVar[list[Binding]] = [
        Binding("escape", "close", "Close", show=False),
        Binding("question_mark", "close", "Close", show=False),
    ]

    #: Key, translation key for the description.
    SHORTCUTS: tuple[tuple[str, str], ...] = (
        ("p", "action.preview"),
        ("a", "action.apply"),
        ("d", "action.demo"),
        ("o", "action.browse"),
        ("space", "action.toggle"),
        ("ctrl+a", "action.select_all"),
        ("ctrl+r", "action.clear_all"),
        ("l", "action.language"),
        ("t", "action.theme"),
        ("?", "action.help"),
        ("q", "action.quit"),
    )

    def __init__(self, translator: Translator) -> None:
        super().__init__()
        self._ = translator

    def compose(self) -> ComposeResult:
        with Vertical(id="help-dialog"):
            yield Label(self._("help.title"), id="help-title")
            yield Static(self._("help.workflow"), id="help-workflow")
            rows = "\n".join(
                f"{key:<8}{self._(description)}" for key, description in self.SHORTCUTS
            )
            yield Static(rows, id="help-shortcuts")
            yield Static(self._("help.note"), id="help-note")
            yield Button(self._("help.close"), variant="primary", id="help-close")

    def on_mount(self) -> None:
        self.query_one("#help-close", Button).focus()

    def on_button_pressed(self, event: Button.Pressed) -> None:
        self.dismiss(None)

    def action_close(self) -> None:
        self.dismiss(None)
