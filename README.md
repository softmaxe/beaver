<p align="center">
  <img src="./docs/assets/beaver-logo.png" alt="beaver logo" width="180">
</p>

<h1 align="center">beaver</h1>

<p align="center">
  <a href="README.md"><kbd>English</kbd></a>
  <a href="README.zh-CN.md"><kbd>简体中文</kbd></a>
</p>

Rename subtitle files to match the videos beside them.

<p align="center">
  <img src="./docs/assets/beaver-demo.gif" alt="beaver terminal interface demo" width="720">
</p>

beaver scans a video library and proposes new names for subtitle files. Use the terminal interface
to review and select each rename, or use the CLI for scripts and batch work.

- Matches episode IDs such as `S02E01` and `2x01`, then falls back to filename similarity.
- Scans subfolders when requested, but never matches files from different folders.
- Reads filenames and file metadata only. It does not read media contents or upload anything.
- Previews changes before writing. The TUI never overwrites an existing file.

## Install

### Homebrew on macOS or Linux

```bash
brew install softmaxe/tap/beaver
```

Upgrade later with `brew upgrade beaver`.

### Prebuilt binaries

Download the archive for Windows, macOS, or Linux from
[GitHub Releases](https://github.com/softmaxe/beaver/releases). Extract it and place `beaver` or
`beaver.exe` on your `PATH`.

Release binaries are unsigned, so macOS Gatekeeper or Windows SmartScreen may show a warning.

### From source

Rust 1.88 or newer is required.

```bash
git clone https://github.com/softmaxe/beaver.git
cd beaver
cargo install --path .
```

Verify the installation with `beaver --help`.

## Quick start

Open the terminal interface:

```bash
beaver
beaver --tui ~/Videos/Some.Show
```

Preview renames in the CLI without changing files:

```bash
beaver /path/to/library
```

Apply the displayed plan after confirmation:

```bash
beaver /path/to/library --apply
```

Add `--recursive` to include subfolders. Matching still stays within each folder.

## Terminal interface

The TUI guides you through four steps:

1. **Folder:** choose the directory to scan.
2. **Rules:** choose `Relaxed`, `Balanced`, or `Cautious`, and optionally include subfolders.
3. **Preview:** review the proposed names and uncheck anything you do not want to apply.
4. **Apply:** confirm the batch. beaver checks the selected files again before renaming them.

Keyboard and mouse input are both supported. Press `?` or `F1` for the full shortcut list. The main
keys are `Enter` to continue, arrow keys or `hjkl` to move, `Space` to toggle a proposal, `s` to
view skipped subtitles, and `q` to quit.

The TUI never invents a suffix for a taken target name and never overwrites an existing file. After
an apply finishes, it discards the old preview and scans again for the next run.

## CLI

Supplying a path runs the CLI. A plain path and `--dry-run` both print the plan without changing
files.

```bash
# Preview only
beaver /path/to/library
beaver /path/to/library --dry-run

# Apply with a prompt
beaver /path/to/library --apply

# Apply without a prompt
beaver /path/to/library --apply --yes
```

Common options:

| Option | What it does |
| --- | --- |
| `--tui` | Open the TUI, with the supplied path filled in. |
| `-r`, `--recursive` | Scan subfolders. Matching remains within each folder. |
| `--level relaxed\|balanced\|cautious` | Set the fuzzy matching level. The default is `balanced`. |
| `--min-score SCORE` | Set a fuzzy threshold from `0` to `1`, overriding `--level`. |
| `--video-ext EXT` | Replace the default video extensions. Repeat for multiple values. |
| `--sub-ext EXT` | Replace the default subtitle extensions. Repeat for multiple values. |
| `--strict` | Skip a rename if its plain target name already exists. |
| `--apply` | Apply the plan after confirmation. |
| `-y`, `--yes` | Skip the CLI confirmation prompt. |
| `--force` | Allow the CLI to overwrite an existing target. Conflicts with `--strict`. |

Without `--strict`, a target-name collision gets a detected language tag when possible, then a
numeric suffix. `--force` is the only mode that overwrites a file.

## Matching rules

beaver groups files by directory and matches each subtitle in this order:

1. If the subtitle name contains an episode ID such as `S02E01` or `2x01`, find the video with the
   same ID. Missing or ambiguous IDs are skipped.
2. Otherwise, remove common release metadata, language tags, bracketed text, and separators, then
   compare the remaining names with Ratcliff/Obershelp similarity.
3. Accept the best fuzzy match only when it reaches the selected threshold and leads the runner-up
   by at least `0.06`.

| Level | Threshold | Best for |
| --- | ---: | --- |
| Relaxed | `0.60` | Messy names that you will review closely. |
| Balanced | `0.72` | General use. This is the default. |
| Cautious | `0.84` | Only very similar names. |

Episode-ID matches do not use the fuzzy threshold.

Default file types:

- Video: `.mkv`, `.mp4`, `.avi`, `.mov`, `.wmv`, `.m4v`, `.webm`
- Subtitle: `.ass`, `.srt`, `.ssa`, `.vtt`, `.sub`

Extensions are case-insensitive. The renamed subtitle keeps its original extension.

## Safety and privacy

- Preview and dry-run never rename files.
- The TUI asks for confirmation, rechecks every selected source and target, and rejects the whole
  batch if anything changed after the preview.
- The CLI plans and applies in one pass. It asks for confirmation unless `--yes` is supplied and
  refuses to overwrite an existing target unless `--force` is supplied.
- All work stays local. beaver has no upload, cache, or history and does not open video streams or
  read subtitle contents.

## Try the demo library

The included script creates placeholder video and subtitle files for a safe test run:

```bash
scripts/make-demo-library.sh
cargo run -- --tui demo-library
```

Enable `Include subfolders` in the Rules step to see nested examples. Run the script again after an
apply to restore the demo names.

## Development

```bash
cargo test
cargo clippy --all-targets
cargo fmt --check
```

See [DESIGN.md](./DESIGN.md) for the TUI design and interaction notes.

## License

beaver is licensed under [GNU AGPL v3.0 only](./LICENSE).
