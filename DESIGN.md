# Design specification — terminal interface

This document describes how `beaver` looks and behaves in a terminal.

The guiding constraint: a terminal has no shadows, no gradients, no hover states, and a grid of
character cells instead of pixels. Hierarchy has to come from **position, weight, and a small
number of colours used consistently** — so every colour in this system means one thing and only
one thing.

## Palette

The interface is Catppuccin Mocha, one flavour only. There is no theme switch: a second palette
would double the surface area of "what does this colour mean" for no gain in a tool this small.

| Role                   | Mocha        | Hex       | Meaning                                    |
| ---------------------- | ------------ | --------- | ------------------------------------------ |
| `BACKGROUND`           | Base         | `#1e1e2e` | The page floor                             |
| `SURFACE`              | Mantle       | `#181825` | The card the current step lives on         |
| `PANEL`                | Crust        | `#11111b` | Modal dialogs, header and footer bars      |
| `SELECTION_BACKGROUND` | Surface 0    | `#313244` | The row the cursor or pointer is on        |
| `HOVER`                | Surface 1    | `#45475a` | A filled control under the pointer         |
| `BORDER`               | Surface 1    | `#45475a` | Resting borders                            |
| `FAINT`                | Overlay 1    | `#7f849c` | Fuzzy scores, hints, disabled controls     |
| `MUTED`                | Subtext 0    | `#a6adc8` | Secondary text, finished step labels       |
| `FOREGROUND`           | Text         | `#cdd6f4` | Body text                                  |
| `FOCUS`                | Mauve        | `#cba6f7` | The control the keyboard is on             |
| `HEADING`              | Lavender     | `#b4befe` | Section headings                           |
| `KEY`                  | Blue         | `#89b4fa` | Keyboard shortcuts, wherever printed       |
| `CERTAIN`              | Teal         | `#94e2d5` | Episode-ID matches                         |
| `SUCCESS` / `TICK`     | Green        | `#a6e3a1` | Ticked boxes, finished steps, the progress bar, completion |
| `WORKING`              | Yellow       | `#f9e2af` | A scan or a rename in flight               |
| `ERROR`                | Red          | `#f38ba8` | Errors, refused applies                    |

Mauve and teal are the two colours a user actually learns: **mauve means "this is what the keyboard
is on", teal means "this match is certain."** Green is reserved for the affirmative — a ticked box,
the apply button, a finished run — so the one destructive-looking step in the tool reads as safe
before you press it.

`tui/theme.rs` names every colour for its role rather than its hue, and nothing outside that file
refers to a hex value. Swapping to another Catppuccin flavour would be an edit to one table.

### Terminal capability degradation

| Terminal     | Result                                                                    |
| ------------ | ------------------------------------------------------------------------- |
| Truecolor    | The palette as specified.                                                 |
| 256 colour   | Crossterm quantises to the nearest xterm-256 entry. The mauve/teal split survives; Base, Mantle and Crust flatten toward each other, so the panel-vs-surface separation reads mainly through borders. |
| 16 colour    | Only the semantic roles survive. This is why nothing depends on colour alone — every state also carries text (the footer, the step labels, the `done / total` beside the progress bar), and a match badge is a word, not a dot. |

## Layout

Measured in character cells, not pixels. The smallest supported size is **80 × 24**.

The screen is a wizard that runs left to right, and only one step of it is ever drawn:

```
┌ beaver · rename subtitles to match their videos ─────── ? help  q quit ┐
│                                                                        │
│              ●───────────●───────────○───────────○                     │
│            Folder      Rules      Preview      Apply                   │
│                                                                        │
│           ╭ 2 · Rules ──────────────────────────────╮                  │
│           │   Match level                           │                  │
│           │   ( ) Relaxed    Matches more…          │                  │
│           │ ▸ (●) Balanced   Recommended…           │                  │
│           │   ( ) Cautious   Only near-certain…     │                  │
│           │                                         │                  │
│           │   Scope                                 │                  │
│           │   [ ] Include subfolders                │                  │
│           │                                         │                  │
│           │   ← Back (esc)          Preview → (↵)   │                  │
│           ╰─────────────────────────────────────────╯                  │
│                                                                        │
└ Two rules, then a preview ───────── ↑↓ level · ← back · ↵ preview ─────┘
```

Three things are on screen and nothing else: a bar that says where you are, one
card that holds the whole of the current step, and a footer that says what just
happened and which keys the focused control answers to. The previous version put
every control and every result on screen at once, in two columns; that is a
denser screen but it never tells you where to look, and the three-step workflow
it was built around was only legible if you already knew it.

The step bar is the memory the single card gives up. A finished step keeps a
green dot, the current one is mauve, and the ones ahead are hollow and faint — so
the shape of the whole job stays visible while only one part of it is live.

The card is centred and capped at 62 columns, except the preview, which is capped
at 100 and takes the full height: it is the one step whose content is a list of
filenames rather than a handful of controls. A card is never full-bleed, so the
floor around it is what separates it from the two bars.

### The four steps

| Step        | Holds                                                                 |
| ----------- | --------------------------------------------------------------------- |
| **Folder**  | The path field and a browse button.                                   |
| **Rules**   | Three match levels and one subfolder switch. Nothing else.            |
| **Preview** | The proposed renames, a ticked count, and the skipped ones behind `s`. |
| **Apply**   | A progress bar while it runs, then a tick or a cross and what happened. |

Two options were removed rather than moved. **Strict mode is now always on**: the
alternative silently appends a suffix when the target name is taken, and a rename
tool that quietly invents a second name is exactly the surprise this design is
trying to remove. The escape hatch stays on the command line. **The skipped table
is folded away** behind one key: it explains why something did *not* happen, which
is worth a keystroke but not a permanent half of the screen.

## Components

Every control is drawn from scratch rather than pulled from a widget set, because
each one only has to do one job:

| Element             | Drawn as                                        | Why                                                                 |
| ------------------- | ----------------------------------------------- | ------------------------------------------------------------------- |
| Step bar            | Four dots, connected, labelled underneath       | The one place the whole workflow is visible at once.                  |
| Path field          | One card row with a real terminal cursor        | A path scrolls horizontally instead of wrapping; the cursor is the system's, so it blinks like every other prompt. |
| Folder browser      | A modal list of subdirectories                  | No platform branches, no subprocess, no optional GUI dependency.      |
| Three match levels  | `(●) label` rows with the trade-off beside them | Named levels rather than an exposed threshold.                        |
| Subfolder switch    | A `[✓] label` row                               | A checkbox is the shape of a boolean.                                 |
| Proposed renames    | `ratatui::List` with a `[✓]` per row            | Ticking is the semantic core of the preview.                          |
| Skipped subtitles   | A modal list, opened with `s`                   | Read-only, and only wanted when a file is missing from the preview.   |
| Confirmation        | A modal that spells out three examples          | The last stop before anything on disk is touched.                     |
| Progress            | A filled bar and `done / total`                 | The apply step reports each rename as it lands, so the bar is real rather than a spinner pretending. |
| Card buttons        | Back on the left edge, forward on the right     | The direction of the wizard is the direction of the buttons.          |

Focus is shown one way and one way only: the focused row takes
`SELECTION_BACKGROUND`, carries a mauve `▸`, and the forward button turns mauve
when the keyboard is on it. The card border is mauve because the card *is* the
current step.

### Match badges

A matched row reads `[✓] source → target  badge`, with the badge pushed to the
right edge:

- **Episode match** — badge in teal. The match is derived from an `SxxEyy`
  identifier and is effectively certain.
- **Fuzzy match** — badge in `FAINT`, showing the score. Deliberately quieter: it
  is the match a user should actually look at.

Long paths are trimmed from the left, keeping the filename, because that is the
end that identifies the file. The line under the list spells the highlighted row
out in full.

## Interaction rules

- **The preview never writes.** Only apply touches the filesystem.
- **A changed rule drops the preview** the moment it changes. There is no stale
  preview to reason about, because stepping forward from the rules always rescans.
- **The whole batch or nothing.** Every path is fingerprinted when the preview is
  built and checked again immediately before renaming; any drift refuses the
  entire apply, and the apply card says so.
- **Long work runs on a worker thread.** Scanning and applying both do, reporting
  back through a channel. Each scan carries a generation number, so a superseded
  result is discarded rather than overwriting a newer preview.
- **The mouse points, the keyboard drives.** Mouse capture is always on: moving
  the pointer tints whatever is clickable with `SELECTION_BACKGROUND` — never
  bold, never mauve — and a left click focuses, selects, toggles or activates
  exactly what it landed on, with a second click on an already-selected row
  acting on it. A filled button carries its tint in the fill instead, stepping one
  place up the same ramp under the pointer. The wheel scrolls whichever list is on
  top. Copying text stays with the terminal's shift-drag.
- **A dot walks back, never forward.** Clicking a finished step returns to it.
  Moving on has preconditions, and a dot that silently refuses reads as broken, so
  the card's own button owns that direction.
- **Nothing is reachable by keyboard alone.** Every action has a button or a
  clickable strip: the card's own controls, `help` and `quit` in the header, the
  ticking strip and the skipped count on the preview's summary row, and a button
  row in each dialog. A key is always a shortcut for something visible.

## Keyboard

Four rules govern it:

- **`Enter` always means forward**, from wherever the keyboard is; `Space` presses
  the focused control in place. One key that means one thing everywhere is worth
  more than one that means five things depending on the row.
- **Left and right are the workflow.** They walk the wizard in the direction the
  step bar is drawn. Up and down stay inside the card. Nothing means two things.
- Single-letter keys necessarily type into a focused path field. Rather than fight
  that with priority bindings — which would make paths untypeable — `Enter`
  submits the field, `Esc` and `↓` leave it, and `i` is the way back in.
- Nothing is ever trapped. Up and down leave the match-level group and the rename
  list at their edges rather than stopping dead, so `Tab` is a convenience and
  never the only way out.

Arrows are the default and `hjkl` / `g` / `G` / `Ctrl+D` / `Ctrl+U` are aliases
for them. A vim user should not have to look anything up, and everyone else should
never meet a mode.

Discoverability is a layout problem, not a documentation one. The footer lists the
keys the focused control answers to and rewrites itself as focus moves; every
button carries its own key in brackets; a `▸` marks the row holding the keyboard.
The `?` modal is then a reference, not a prerequisite.

## Language

English only. The previous version carried a bilingual catalog and a language
toggle; a single voice is one less thing to keep in sync between the two
front-ends, and every string now lives beside the code that shows it.
