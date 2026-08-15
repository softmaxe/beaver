# rename-subtitles-tui

A local tool for matching subtitle filenames (`.ass`, `.srt`, and more) to the video files beside
them (`.mkv`, `.mp4`, and more). It runs as a bilingual terminal interface, with the original
command-line workflow kept for scripting.

Everything happens on your computer. No filename or file content ever leaves it.

## Requirements

- Python 3.10+
- [uv](https://docs.astral.sh/uv/)

## Setup

From the repository root:

```bash
uv sync
```

The terminal interface depends on [Textual](https://textual.textualize.io/); the matching core and
the CLI have no third-party dependencies.

## Terminal interface

```bash
uv run rename-subs-tui
```

Without installing the package:

```bash
uv run python tui_app.py
```

### Three-step workflow

1. **Point at a folder.** Type or paste a path, or press `o` to browse. `Enter` starts a preview
   without leaving the field.
2. **Read the preview.** It is entirely read-only: a summary line, a checkbox list of proposed
   renames, and a table of what was skipped and why.
3. **Tick and apply.** Untick anything you do not want, press `a`, and confirm.

Changing the directory, the recursive switch, the strict switch, or the match level invalidates the
preview immediately — apply stays disabled until you preview again.

Press `d` for demo mode, which loads a sample library so you can see the whole workflow without a
real folder. Demo plans can never be applied.

### Shortcuts

| Key      | Action                                          |
| -------- | ----------------------------------------------- |
| `Enter`  | Preview (while the path field has focus)        |
| `p`      | Preview                                         |
| `a`      | Apply the ticked renames                        |
| `d`      | Demo mode                                       |
| `o`      | Browse for a directory                          |
| `Space`  | Tick or untick the highlighted rename           |
| `Ctrl+A` | Tick everything                                 |
| `Ctrl+R` | Untick everything                               |
| `l`      | Switch language (简体中文 / English)             |
| `t`      | Switch theme (dark / light)                     |
| `?`      | Shortcut list                                   |
| `q`      | Quit                                            |

Single-letter shortcuts type into the path field while it has focus. Press `Tab` to leave it, or
use `Enter` to preview from there.

The interface starts in Simplified Chinese with the dark theme. Both choices apply instantly and
keep whatever preview is already loaded.

### Match level

Instead of a raw threshold, the fuzzy matcher is exposed as three named levels:

| Level        | Threshold | Use when                                       |
| ------------ | --------- | ---------------------------------------------- |
| Relaxed      | 0.60      | Naming is messy and you will review each match |
| Balanced     | 0.72      | Default                                        |
| Cautious     | 0.84      | You only want near-certain matches             |

Episode-ID matches ignore the threshold entirely.

### Safety model

- The preview never writes. Only the apply step touches the filesystem.
- Every source and destination is fingerprinted when the preview is built and checked again just
  before renaming. If anything moved in between, the **entire batch is refused** — no partial
  application — and you are asked to preview again.
- Existing files are never overwritten. The terminal interface does not expose the CLI's `--force`.
- Applying runs on a worker thread, as does scanning, so a large recursive directory never freezes
  the interface.

## CLI usage

### Dry run

```bash
uv run rename-subs /path/to/folder --dry-run
```

Or without installing: `uv run python rename_subs.py /path/to/folder --dry-run`.

### Apply renames

```bash
uv run rename-subs /path/to/folder --apply
```

### Common options

- `--recursive`: process subfolders as well
- `--min-score 0.72`: adjust the fuzzy matching threshold when no episode ID exists
- `--strict`: skip a subtitle when the exact target filename would collide
- `--force`: allow overwriting existing files in the CLI only; use with care

The CLI plans and applies in a single pass, so it skips the re-verification step the terminal
interface performs. Like the TUI, it refuses to overwrite an existing target unless `--force` is
given.

## How matching works

1. **Episode ID match** is preferred when filenames contain `SxxEyy` (for example, `S02E01`) or
   `2x01`.
2. **Fuzzy stem match** is used when an episode ID is unavailable. Common release metadata is
   removed before comparing filename stems.
3. **Collision handling** keeps the original extension, uses a detected language suffix when
   possible, then falls back to a numeric suffix. Strict mode skips collisions instead.

Matching is scoped per directory: a subtitle is only ever matched against videos in its own folder.

## Layout

```
src/rename_subtitles/
├── planning.py       # pure matching core, no I/O beyond stat and iteration
├── applying.py       # two-phase safe execution, shared by both front-ends
├── presentation.py   # UI-neutral vocabulary: match levels, reason codes, demo data
├── i18n.py           # bilingual string catalog
├── cli.py            # argparse front-end
└── tui/              # Textual front-end
```

## Tests and checks

```bash
uv run python -m pytest -q
uv run python -m ruff check .
```

The terminal interface is tested end to end through Textual's `Pilot`, against real files in a
temporary directory.
