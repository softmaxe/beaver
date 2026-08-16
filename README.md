# rename-subtitles-tui

A local tool for matching subtitle filenames (`.ass`, `.srt`, and more) to the video files beside
them (`.mkv`, `.mp4`, and more). It runs as a terminal interface, with a command-line mode kept for
scripting.

Everything happens on your computer. No filename or file content ever leaves it.

## Requirements

- Rust 1.88 or newer

## Build

From the repository root:

```bash
cargo build --release
```

The binary lands in `target/release/submv`. To put it on your `PATH`:

```bash
cargo install --path .
```

## Terminal interface

```bash
submv
```

Or with a folder already filled in:

```bash
submv --tui ~/Videos/Some.Show
```

During development, `cargo run` does the same thing.

### Three-step workflow

1. **Point at a folder.** Type or paste a path, or press `o` to browse. `Enter` starts a preview
   without leaving the field.
2. **Read the preview.** It is entirely read-only: a summary line, a checkbox list of proposed
   renames, and a table of what was skipped and why.
3. **Tick and apply.** Untick anything you do not want, press `a`, and confirm.

Changing the directory, the subfolder switch, the strict switch, or the match level invalidates the
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
| `Tab`    | Move between controls                           |
| `← →`    | Switch tab, or set the focused option           |
| `Esc`    | Back to the path field                          |
| `?`      | Shortcut list                                   |
| `q`      | Quit                                            |

Single-letter shortcuts type into the path field while it has focus. Press `Tab` or `Esc` to leave
it, or use `Enter` to preview from there. A successful preview moves focus to the results list, so
the shortcuts work straight away.

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
submv /path/to/folder --dry-run
```

A path with no `--dry-run` or `--apply` is a dry run as well.

### Apply renames

```bash
submv /path/to/folder --apply --yes
```

### Common options

- `-r`, `--recursive`: process subfolders as well
- `--level relaxed|balanced|cautious`: how eager fuzzy matching should be
- `--min-score 0.72`: an explicit threshold, overriding `--level`
- `--video-ext`, `--sub-ext`: extensions to include, repeatable, with or without a leading dot
- `--strict`: skip a subtitle when the exact target filename would collide
- `--force`: allow overwriting existing files in the CLI only; use with care

The CLI plans and applies in a single pass, so it skips the re-verification step the terminal
interface performs. Like the TUI, it refuses to overwrite an existing target unless `--force` is
given.

## How matching works

1. **Episode ID match** is preferred when filenames contain `SxxEyy` (for example, `S02E01`) or
   `2x01`.
2. **Fuzzy stem match** is used when an episode ID is unavailable. Common release metadata is
   removed before comparing filename stems, which are then scored with the Ratcliff/Obershelp
   measure. A match is only taken when it is clearly ahead of the runner-up.
3. **Collision handling** keeps the original extension, uses a detected language suffix when
   possible, then falls back to a numeric suffix. Strict mode skips collisions instead.

Matching is scoped per directory: a subtitle is only ever matched against videos in its own folder.

## A folder to practise on

Demo mode shows the workflow but never writes anything. To try a real apply on files that do not
matter, generate a throwaway library:

```bash
scripts/make-demo-library.sh          # creates demo-library/ at the repository root
cargo run -- --tui demo-library       # then turn on "include subfolders" and press p
```

It contains fake videos and subtitles named the way real releases are, arranged so every outcome
shows up at once: episode-ID matches, fuzzy matches at three different strengths, a missing episode,
files already named correctly, a taken target name, an episode two videos claim, CJK names, and
subtitles with no video beside them. Stepping the match level or flipping the strict switch visibly
changes the plan.

Renaming mutates the folder, so re-run the script for a clean slate. It only ever writes to a
directory it created itself, `.gitignore` keeps it out of the repository, and `rm -rf demo-library`
removes it. Pass a path to put it somewhere else.

## Layout

```
src/
├── planning.rs       # pure matching core, no I/O beyond reading directory entries
├── applying.rs       # two-phase safe execution, shared by both front-ends
├── names.rs          # filename normalisation, episode IDs, language tags
├── similarity.rs     # Ratcliff/Obershelp string similarity
├── presentation.rs   # match levels, human wording, demo data
├── paths.rs          # path helpers
├── cli.rs            # command-line front-end
└── tui/              # terminal front-end (ratatui)

scripts/
└── make-demo-library.sh   # throwaway files to try the interface on
```

## Tests and checks

```bash
cargo test
cargo clippy --all-targets
cargo fmt --check
```

The terminal interface is tested end to end: keystrokes go in, a frame is rendered to a test
backend, and the text on screen is what the assertions read — against real files in a temporary
directory.
