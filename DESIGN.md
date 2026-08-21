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
| `SURFACE`              | Mantle       | `#181825` | Setup column, result panes                 |
| `PANEL`                | Crust        | `#11111b` | Modal dialogs, header and footer bars      |
| `SELECTION_BACKGROUND` | Surface 0    | `#313244` | The row the cursor is on                   |
| `BORDER`               | Surface 1    | `#45475a` | Resting borders                            |
| `FAINT`                | Overlay 1    | `#7f849c` | Fuzzy scores, hints, disabled controls     |
| `MUTED`                | Subtext 0    | `#a6adc8` | Secondary text, the detail line            |
| `FOREGROUND`           | Text         | `#cdd6f4` | Body text                                  |
| `FOCUS`                | Mauve        | `#cba6f7` | The control the keyboard is on             |
| `HEADING`              | Lavender     | `#b4befe` | Section headings                           |
| `KEY`                  | Blue         | `#89b4fa` | Keyboard shortcuts, wherever printed       |
| `CERTAIN`              | Teal         | `#94e2d5` | Episode-ID matches                         |
| `SUCCESS` / `TICK`     | Green        | `#a6e3a1` | Ticked boxes, the apply button, completion |
| `WORKING`              | Yellow       | `#f9e2af` | Scanning, and a stale preview              |
| `DEMO`                 | Peach        | `#fab387` | Demo mode, which looks real but writes nothing |
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
| 16 colour    | Only the semantic roles survive. This is why nothing depends on colour alone — every state also carries text (the status line, the `Skipped` tab), and a match badge is a word, not a dot. |

## Layout

Measured in character cells, not pixels.

### Wide (≥ 100 columns)

```
┌ Header: title · subtitle ───────────────────────────────────────────┐
│ 38 cells             │ remaining width                              │
│ ╭ Setup ───────────╮ │  Summary line                                │
│ │ directory input  │ │ ╭ To rename (N) │ Skipped (N) ───────────╮   │
│ │ browse hint      │ │ │  [✓] source → target          badge    │   │
│ │ subfolder switch │ │ │  Source | Reason                       │   │
│ │ strict switch    │ │ ╰────────────────────────────────────────╯   │
│ │ match level ×3   │ │  Detail line (the highlighted row, in full)  │
│ │ preview / apply  │ │  Status line                                 │
│ │ demo             │ │                                              │
│ ╰──────────────────╯ │                                              │
└ Footer: shortcuts ──────────────────────────────────────────────────┘
```

The 38-cell setup column is fixed. Everything the user configures lives on the left, everything
the tool reports lives on the right; that split is what makes the three-step workflow legible
without any step numbering.

### Narrow (< 100 columns)

Below 100 columns the two panes cannot sit side by side, so they stack. Setup takes at most 45% of
the height and scrolls internally — the scroll offset is derived from which control holds the
keyboard, so the focused row is always on screen — and the results take the rest, with a floor of
five rows. The smallest supported size is **80 × 24**.

## Components

Every control is drawn from scratch rather than pulled from a widget set, because each one only has
to do one job:

| Element             | Drawn as                                        | Why                                                                 |
| ------------------- | ----------------------------------------------- | ------------------------------------------------------------------- |
| Directory field     | One line with its own background and a real terminal cursor | A path scrolls horizontally instead of wrapping; the cursor is the system's, so it blinks like every other prompt. |
| Folder browser      | A modal list of subdirectories                  | No platform branches, no subprocess, no optional GUI dependency.      |
| Two switches        | `[✓] label` rows                                | A checkbox is the shape of a boolean.                                 |
| Three match levels  | `(●) label` rows                                | Named levels rather than an exposed threshold.                        |
| Proposed renames    | `ratatui::List` with a `[✓]` per row            | Ticking is the semantic core of step 3.                               |
| Skipped subtitles   | `ratatui::Table`, source and reason             | Read-only tabular data with no selection semantics.                   |
| Confirmation        | A modal that spells out five examples           | The last stop before anything on disk is touched.                     |
| Summary             | One line of counts                              | Four numbers do not need four boxes in a terminal.                    |
| Buttons             | Full-width filled bars, keyed `p` / `a` / `d`   | The key that triggers them is printed on them, so the footer is a reminder rather than the only route. |

Focus is shown one way and one way only: the focused row takes `SELECTION_BACKGROUND` and its label
goes bold, and the results pane's border turns mauve when the list holds the keyboard.

### Match badges

A matched row reads `[✓] source → target  badge`, with the badge pushed to the right edge:

- **Episode match** — badge in teal. The match is derived from an `SxxEyy` identifier and is
  effectively certain.
- **Fuzzy match** — badge in `FAINT`, showing the score. Deliberately quieter: it is the match a
  user should actually look at.

Long paths are trimmed from the left, keeping the filename, because that is the end that identifies
the file. The detail line under the list always spells the highlighted row out in full.

## Interaction rules

- **The preview never writes.** Only apply touches the filesystem.
- **A changed option invalidates the preview** the moment it changes — directory, subfolders,
  strict, or match level. The apply button greys out and the status line says why.
- **The ticked count lives in the summary bar, not the status line.** The status line is reserved
  for what just happened, so ticking a box can never overwrite an error message.
- **Long work runs on a worker thread.** Scanning and applying both do, reporting back through a
  channel. Each scan carries a generation number, so a superseded result is discarded rather than
  overwriting a newer preview.
- **The whole batch or nothing.** Every path is fingerprinted when the preview is built and checked
  again immediately before renaming; any drift refuses the entire apply.

## Keyboard

The full table lives in `README.md` and in the `?` modal. Four rules govern it:

- Single-letter keys for the workflow verbs (`p`, `a`, `d`, `o`), `Ctrl`-modified for the bulk
  selection operations, and `?` / `q` for the interface itself.
- Single-letter keys necessarily type into a focused text field. Rather than fight that with
  priority bindings — which would make paths untypeable — `Enter` submits the path field, `Esc`
  leaves it, and focus moves to the results list after a successful preview so the verbs work
  immediately.
- Arrows are the default and `hjkl` / `g` / `G` / `Ctrl+D` / `Ctrl+U` are aliases for them. A vim
  user should not have to look anything up, and everyone else should never meet a mode. The same
  goes for `Ctrl`: over a list it selects and pages, inside the path field it is readline, because
  a key that means two things in two places is easier than a key that works in only one.
- Nothing is ever trapped. Up and down leave the match-level group and the path field at their
  edges rather than stopping dead, so `Tab` is a convenience and never the only way out.

Discoverability is a layout problem, not a documentation one. The footer lists the keys the focused
control answers to and rewrites itself as focus moves; the ticking keys are also printed on the
bottom edge of the list they act on; a `▸` marks the row holding the keyboard, and the focused
control repeats its key at the end of its own row. The `?` modal is then a reference, not a
prerequisite.

## Language

English only. The previous version carried a bilingual catalog and a language toggle; a single
voice is one less thing to keep in sync between the two front-ends, and every string now lives
beside the code that shows it.
