"""Terminal themes derived from the project palette in ``DESIGN.md``.

Both themes share the same accent colours and only swap the neutral ground, so
toggling between them never changes what a colour *means* — warm clay is always
the primary action, teal is always an episode match.
"""

from __future__ import annotations

from textual.theme import Theme

__all__ = ["SUBTITLE_DARK", "SUBTITLE_LIGHT", "THEMES"]

_CLAY = "#cc785c"
_CLAY_DEEP = "#a9583e"
_TEAL = "#5db8a6"
_SUCCESS = "#5db872"
_WARNING = "#d4a017"
_ERROR = "#c64545"

SUBTITLE_DARK = Theme(
    name="subtitle-dark",
    primary=_CLAY,
    secondary=_CLAY_DEEP,
    accent=_TEAL,
    success=_SUCCESS,
    warning=_WARNING,
    error=_ERROR,
    foreground="#faf9f5",
    background="#181715",
    surface="#1f1e1b",
    panel="#252320",
    dark=True,
    variables={
        "block-cursor-background": _CLAY,
        "block-cursor-foreground": "#181715",
        "block-cursor-text-style": "none",
        "footer-key-foreground": _CLAY,
        "input-selection-background": f"{_CLAY} 35%",
    },
)

SUBTITLE_LIGHT = Theme(
    name="subtitle-light",
    primary=_CLAY,
    secondary=_CLAY_DEEP,
    accent=_TEAL,
    success=_SUCCESS,
    warning=_WARNING,
    error=_ERROR,
    foreground="#141413",
    background="#faf9f5",
    surface="#efe9de",
    panel="#f5f0e8",
    dark=False,
    variables={
        "block-cursor-background": _CLAY,
        "block-cursor-foreground": "#faf9f5",
        "block-cursor-text-style": "none",
        "footer-key-foreground": _CLAY_DEEP,
        "input-selection-background": f"{_CLAY} 30%",
    },
)

THEMES: tuple[Theme, ...] = (SUBTITLE_DARK, SUBTITLE_LIGHT)
