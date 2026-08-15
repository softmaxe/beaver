//! The Catppuccin Mocha palette, and what each colour is allowed to mean here.
//!
//! A terminal has no shadows, no gradients and no hover states, so hierarchy has
//! to come from position, weight and a small number of colours used consistently.
//! Every entry below is named for its *role*, and each role means one thing:
//! mauve is "this is the control you are on", teal is "this match is certain",
//! red is "this did not happen".

use ratatui::style::{Color, Modifier, Style};

// Catppuccin Mocha, as published: https://catppuccin.com/palette
const BASE: Color = Color::Rgb(0x1e, 0x1e, 0x2e);
const MANTLE: Color = Color::Rgb(0x18, 0x18, 0x25);
const CRUST: Color = Color::Rgb(0x11, 0x11, 0x1b);
const SURFACE0: Color = Color::Rgb(0x31, 0x32, 0x44);
const SURFACE1: Color = Color::Rgb(0x45, 0x47, 0x5a);
const OVERLAY1: Color = Color::Rgb(0x7f, 0x84, 0x9c);
const TEXT: Color = Color::Rgb(0xcd, 0xd6, 0xf4);
const SUBTEXT0: Color = Color::Rgb(0xa6, 0xad, 0xc8);
const MAUVE: Color = Color::Rgb(0xcb, 0xa6, 0xf7);
const LAVENDER: Color = Color::Rgb(0xb4, 0xbe, 0xfe);
const BLUE: Color = Color::Rgb(0x89, 0xb4, 0xfa);
const TEAL: Color = Color::Rgb(0x94, 0xe2, 0xd5);
const GREEN: Color = Color::Rgb(0xa6, 0xe3, 0xa1);
const YELLOW: Color = Color::Rgb(0xf9, 0xe2, 0xaf);
const PEACH: Color = Color::Rgb(0xfa, 0xb3, 0x87);
const RED: Color = Color::Rgb(0xf3, 0x8b, 0xa8);

/// The page floor.
pub const BACKGROUND: Color = BASE;
/// Panels that sit on the floor: the setup column, the result panes.
pub const SURFACE: Color = MANTLE;
/// Modal dialogs, which have to read as being in front of everything else.
pub const PANEL: Color = CRUST;
/// Body text.
pub const FOREGROUND: Color = TEXT;
/// Text that is present but secondary: hints, the detail line.
pub const MUTED: Color = SUBTEXT0;
/// Text that should recede further still: unfocused labels, fuzzy scores.
pub const FAINT: Color = OVERLAY1;
/// Resting borders.
pub const BORDER: Color = SURFACE1;
/// The control the keyboard is on.
pub const FOCUS: Color = MAUVE;
/// Section headings.
pub const HEADING: Color = LAVENDER;
/// Keyboard shortcuts, wherever they are printed.
pub const KEY: Color = BLUE;
/// A match derived from an episode id, which is as good as certain.
pub const CERTAIN: Color = TEAL;
/// Something completed.
pub const SUCCESS: Color = GREEN;
/// Something is fine but not resting: a scan in flight, a stale preview.
pub const WORKING: Color = YELLOW;
/// Demo mode, which looks real but writes nothing.
pub const DEMO: Color = PEACH;
/// Something failed or was refused.
pub const ERROR: Color = RED;
/// The row the cursor is on.
pub const SELECTION_BACKGROUND: Color = SURFACE0;
/// A ticked checkbox.
pub const TICK: Color = GREEN;

pub fn base() -> Style {
    Style::default().fg(FOREGROUND).bg(BACKGROUND)
}

pub fn muted() -> Style {
    Style::default().fg(MUTED)
}

pub fn faint() -> Style {
    Style::default().fg(FAINT)
}

pub fn heading() -> Style {
    Style::default().fg(HEADING).add_modifier(Modifier::BOLD)
}

pub fn key() -> Style {
    Style::default().fg(KEY).add_modifier(Modifier::BOLD)
}

/// The border of a panel, brightened while it holds the keyboard.
pub fn border(focused: bool) -> Style {
    if focused {
        Style::default().fg(FOCUS)
    } else {
        Style::default().fg(BORDER)
    }
}
