# beaver

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

The binary lands in `target/release/beaver`. To put it on your `PATH`:

```bash
cargo install --path .
```

## Terminal interface

```bash
beaver
```

Or with a folder already filled in:

```bash
beaver --tui ~/Videos/Some.Show
```

During development, `cargo run` does the same thing.

### The four steps

The interface is a wizard: one step is on screen at a time, and a bar of dots across the top says
where you are.

1. **Folder.** Type or paste a path, press `o` to browse, or press `d` for a demo library that looks
   real and writes nothing. `Enter` moves on.
2. **Rules.** Two of them: a match level, and whether subfolders are included. `Enter` starts the
   scan.
3. **Preview.** Entirely read-only: a ticked count, a checkbox list of proposed renames, and the
   skipped subtitles behind `s`. Untick anything you do not want and press `a`.
4. **Apply.** A progress bar while the renames land, then what happened. `Enter` starts over.

Going back with `←` and changing a rule drops the preview, so the list you apply is always the list
the current rules produced.

### Shortcuts

`Enter` always means forward, from wherever the keyboard is. `Space` presses the focused control in
place. Left and right walk the wizard; up and down stay inside the card. The vim keys are aliases,
not a separate mode.

| Key                  | Vim     | Action                                       |
| -------------------- | ------- | -------------------------------------------- |
| `Enter`              |         | Forward one step                             |
| `←` `→`              | `h` `l` | Back / forward one step                      |
| `Esc`                |         | Back one step, or leave the path field       |
| `↑` `↓`              | `k` `j` | Move inside the current step                 |
| `Tab` / `Shift+Tab`  |         | Next / previous control                      |
| `Space`              |         | Press the focused control                    |
| `Home` / `End`       | `g` `G` | First / last rename                          |
| `PgUp` / `PgDn`      | `Ctrl+U` / `Ctrl+D` | A page, or half of one           |
| `Ctrl+A` / `Ctrl+R`  |         | Tick everything / nothing                    |
| `s`                  |         | The skipped subtitles, and why               |
| `i`                  |         | Back into the path field                     |
| `p`                  |         | Rescan                                       |
| `a`                  |         | Apply the ticked renames                     |
| `d`                  |         | Demo library                                 |
| `o`                  |         | Browse for a folder                          |
| `?`                  |         | Shortcut list                                |
| `q`                  |         | Quit                                         |

The footer always spells out the keys the focused control answers to right now, so nothing above
has to be memorised.

Single-letter shortcuts type into the path field while it has focus. Press `Tab`, `Esc` or `↓` to
leave it, `Enter` to move on from there, and `i` to get back in. Inside the field the control keys
are the shell's: `Ctrl+A` / `Ctrl+E` jump to either end, and `Ctrl+U` / `Ctrl+K` / `Ctrl+W` delete
the line, the tail, or one path segment.

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
- Existing files are never overwritten. The terminal interface does not expose the CLI's `--force`,
  and it always matches strictly: a subtitle whose target name is already taken is skipped rather
  than given an invented suffix. `--strict` is the CLI's way to ask for the same thing.
- Applying runs on a worker thread, as does scanning, so a large recursive directory never freezes
  the interface.

## CLI usage

### Dry run

```bash
beaver /path/to/folder --dry-run
```

A path with no `--dry-run` or `--apply` is a dry run as well.

### Apply renames

```bash
beaver /path/to/folder --apply --yes
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
cargo run -- --tui demo-library       # then turn on "include subfolders" on step 2
```

It contains fake videos and subtitles named the way real releases are, arranged so every outcome
shows up at once: episode-ID matches, fuzzy matches at three different strengths, a missing episode,
files already named correctly, a taken target name, an episode two videos claim, CJK names, and
subtitles with no video beside them. Stepping the match level or turning on subfolders visibly
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
