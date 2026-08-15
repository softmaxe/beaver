# Design specification — terminal interface

This document describes how `rename-subtitles-tui` looks and behaves in a terminal. It replaces the
browser design system that shipped with the previous web workspace; the palette carried over, the
px-and-shadow vocabulary did not.

The guiding constraint: a terminal has no shadows, no gradients, no hover states, and a grid of
character cells instead of pixels. Hierarchy has to come from **position, weight, and a small
number of colours used consistently** — so every colour in this system means one thing and only
one thing.

## Palette

Both themes share the same accent hues and swap only the neutral ground. Toggling the theme never
changes what a colour means.

| Role         | `subtitle-dark` (default) | `subtitle-light` | Meaning                          |
| ------------ | ------------------------- | ---------------- | -------------------------------- |
| `background` | `#181715`                 | `#faf9f5`        | The page floor                   |
| `surface`    | `#1f1e1b`                 | `#efe9de`        | Setup column, result panes       |
| `panel`      | `#252320`                 | `#f5f0e8`        | Modal dialogs, dividers          |
| `foreground` | `#faf9f5`                 | `#141413`        | Body text                        |
| `primary`    | `#cc785c`                 | `#cc785c`        | The primary action, focus, keys  |
| `secondary`  | `#a9583e`                 | `#a9583e`        | Pressed and deep primary states  |
| `accent`     | `#5db8a6`                 | `#5db8a6`        | Episode-ID matches, headings     |
| `success`    | `#5db872`                 | `#5db872`        | Apply button, completed runs     |
| `warning`    | `#d4a017`                 | `#d4a017`        | Reserved for degraded states     |
| `error`      | `#c64545`                 | `#c64545`        | Errors, refused applies          |

The warm clay `#cc785c` and the teal `#5db8a6` are the two colours a user actually learns:
**clay means "this is the thing to press", teal means "this match is certain."**

### Terminal capability degradation

| Terminal     | Result                                                                   |
| ------------ | ------------------------------------------------------------------------ |
| Truecolor    | The palette as specified.                                                |
| 256 colour   | Textual quantises to the nearest xterm-256 entry. The clay/teal split survives; the three near-black greys of the dark theme flatten toward each other, so the panel-vs-surface separation reads mainly through borders. |
| 16 colour    | Only the semantic roles survive. This is why nothing in the interface depends on colour alone — every state also carries text (`已跳过`, `预览完成`, the error line), and the match badge is a word, not a dot. |

Nothing is ever encoded in colour alone. `app.tcss` references only theme variables
(`$primary`, `$surface`, `$text-muted`, …), never literal hex, so a theme switch is a variable
swap with no stylesheet reload.

## Layout

Measured in character cells, not pixels.

### Wide (≥ 100 columns)

```
┌ Header ─────────────────────────────────────────────────────────────┐
│ 40 cells             │ remaining width                              │
│  Setup               │  Summary line                                │
│  ├ directory input   │  ┌ Tabs ────────────────────────────────┐    │
│  ├ browse            │  │ To rename (N) │ Skipped (N)          │    │
│  ├ recursive switch  │  │  SelectionList — checkbox per rename │    │
│  ├ strict switch     │  │  DataTable    — source | reason      │    │
│  ├ match level       │  └──────────────────────────────────────┘    │
│  ├ preview           │  Detail line (full path of the highlight)    │
│  ├ apply             │  Status line                                 │
│  └ demo              │                                              │
└ Footer: shortcuts ──────────────────────────────────────────────────┘
```

The 40-cell setup column is fixed. Everything the user configures lives on the left, everything
the tool reports lives on the right; that split is what makes the three-step workflow legible
without any step numbering.

### Narrow (< 100 columns)

Below 100 columns the two panes cannot sit side by side, so `HORIZONTAL_BREAKPOINTS` puts a
`-narrow` class on the screen and the panes stack vertically. Setup takes at most 45% of the
height and scrolls internally; results take the rest with a floor of 8 rows. The smallest
supported size is **80 × 24**, where setup gets 9 rows and results get 13.

## Component mapping

Each web component was replaced by the terminal control with the closest semantics, not the
closest appearance.

| Web workspace                      | Terminal                                    | Why                                                                 |
| ---------------------------------- | ------------------------------------------- | ------------------------------------------------------------------- |
| Directory text field               | `Input`                                     | Direct equivalent.                                                    |
| Native folder picker (osascript / tkinter) | `DirectoryTree` in a `ModalScreen`  | No platform branches, no subprocess, no optional GUI dependency.      |
| Two toggle switches                | `Switch` × 2                                | Direct equivalent.                                                    |
| Three named match-level radios     | `RadioSet` + 3 `RadioButton`                | Keeps the decision to name the levels rather than expose a threshold. |
| Operations table with checkboxes   | `SelectionList`                             | Ticking is the semantic core of step 3; `SelectionList` provides it natively rather than simulating a checkbox column in a `DataTable`. |
| Skipped table                      | `DataTable`                                 | Read-only tabular data with no selection semantics.                   |
| Confirm `<dialog>`                 | `ConfirmApplyScreen(ModalScreen[bool])`     | Direct equivalent.                                                    |
| Summary cards                      | One summary line                            | Four numbers do not need four boxes in a terminal.                    |
| `aria-live` status region          | Status line + `Footer`                      | The status line is the single place "what just happened" is reported. |

### Match badges

A matched row reads `source → target  badge`:

- **Episode match** — badge in `$accent` (teal). The match is derived from an `SxxEyy` identifier
  and is effectively certain.
- **Fuzzy match** — badge in `$text-muted`, showing the score. Deliberately quieter: it is the
  match a user should actually look at.

## Interaction rules

- **The preview never writes.** Only apply touches the filesystem.
- **A changed option invalidates the preview** the moment it changes — directory, recursive,
  strict, or match level. The apply button greys out and the status line says why. This mirrors
  the web version's `invalidatePreview()`.
- **The selected count lives in the summary bar, not the status line.** The status line is
  reserved for what just happened, so ticking a box can never overwrite an error message.
- **The detail line always describes the highlighted row** in full, because list prompts elide
  long paths.
- **Long work runs on a worker thread.** Scanning and applying both do; the results pane shows
  Textual's loading overlay while a scan is in flight, and `exclusive=True` means a new preview
  cancels the previous one.

## Keyboard

The full table lives in `README.md` and in the `?` modal. Two rules govern it:

- Single-letter keys for the workflow verbs (`p`, `a`, `d`, `o`), `Ctrl`-modified for the bulk
  selection operations, and `l` / `t` / `?` / `q` for the interface itself.
- Single-letter keys necessarily type into a focused `Input`. Rather than fight that with
  `priority` bindings — which would make paths untypeable — `Enter` submits the path field, and
  focus moves to the results list after a successful preview so the verbs work immediately.

## Internationalisation

`i18n.py` holds one flat catalog per language. Keys are dot-namespaced by where they appear:

| Namespace  | Covers                                                    |
| ---------- | --------------------------------------------------------- |
| `app.*`    | Title and subtitle                                        |
| `setup.*`  | Everything in the left column, including the level hints   |
| `action.*` | Button labels and footer binding descriptions              |
| `summary.*`, `tab.*`, `table.*` | Result headings and counts               |
| `match.*`  | The two match badges                                       |
| `skip.*`   | The five skip reason codes from `presentation.py`          |
| `status.*` | Resting workflow states                                    |
| `error.*`  | Everything that goes red                                   |
| `confirm.*`, `picker.*`, `help.*` | Modal contents                          |
| `result.*` | Apply outcomes                                             |

Rules enforced by `tests/test_i18n.py`: both languages define exactly the same keys, no value is
blank, every key is namespaced, and `{placeholders}` agree across languages. A missing key renders
as the key itself rather than raising, so a gap is visible instead of fatal.

Switching language retranslates every label, the tab titles, the list prompts, and the footer
binding descriptions in place — the loaded preview and the user's ticks both survive.

## What was dropped from the browser design

Shadows, gradients, `backdrop-filter`, hover states, transitions and reduced-motion handling,
px spacing scales, responsive breakpoints in pixels, web fonts and the whole typographic voice
(a terminal has one font, chosen by the user), the radial-spike brand mark, and the marketing
copy that made up most of the web version's translation keys. None of it has a terminal analogue,
and simulating any of it would fight the medium.

What survived is the part that was never about the browser: the palette, the warm-neutral ground,
the discipline of one accent colour with one meaning, and the generous whitespace — expressed here
as blank rows and a 40-cell gutter rather than padding values.
