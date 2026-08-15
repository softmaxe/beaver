# rename-subtitles

A dependency-free local tool for matching subtitle filenames (`.ass`, `.srt`, and more) to the
corresponding video files (`.mkv`, `.mp4`, and more). It provides both a safe command-line
workflow and a bilingual local Web workspace.

The Web workspace runs only on your computer. It does not upload files or send filename data to a
remote service.

## Requirements

- Python 3.10+
- [uv](https://docs.astral.sh/uv/)

## Setup

From the repository root:

```bash
uv venv
```

There are no third-party runtime dependencies.

## Web workspace

Start the local workspace:

```bash
uv run python web_app.py
```

It opens `http://127.0.0.1:8765/` in your browser. Use a different port or keep the browser closed
at startup when needed:

```bash
uv run python web_app.py --port 9000 --no-browser
```

Closing the workspace page automatically stops the local server. If the browser or computer is
terminated abruptly, use `Ctrl+C` in the terminal instead.

### Three-step workflow

1. Enter or choose a local folder containing videos and subtitle files.
2. Build a read-only preview and review the suggested source-to-target filenames.
3. Select the intended operations and confirm the rename in the final dialog.

The workspace defaults to Simplified Chinese and can be switched to English from the top-right
language control. The **Try virtual demo** action provides safe sample data without reading or
changing local files.

### Safety model

- The server binds to `127.0.0.1` only.
- The browser never uploads local files.
- The apply endpoint accepts only operation IDs from a server-side preview; it does not accept
  arbitrary source or target paths.
- Every selected file is checked again before renaming. If a source or target changed after the
  preview, the workspace requires a new preview.
- Web operations never overwrite existing files and do not expose the CLI `--force` behavior.

If the native folder picker is unavailable on your platform, paste the absolute local folder path
into the input field.

## CLI usage

### Dry run

```bash
uv run python rename_subs.py /path/to/folder --dry-run
```

### Apply renames

```bash
uv run python rename_subs.py /path/to/folder --apply
```

### Common options

- `--recursive`: process subfolders as well
- `--min-score 0.72`: adjust the fuzzy matching threshold when no episode ID exists
- `--strict`: skip a subtitle when the exact target filename would collide
- `--force`: allow overwriting existing files in the CLI only; use with care

## How matching works

1. **Episode ID match** is preferred when filenames contain `SxxEyy` (for example, `S02E01`) or
   `2x01`.
2. **Fuzzy stem match** is used when an episode ID is unavailable. Common release metadata is
   removed before comparing filename stems.
3. **Collision handling** keeps the original extension, uses a detected language suffix when
   possible, then falls back to a numeric suffix. Strict mode skips collisions instead.

## Tests and checks

```bash
uv run python -m pytest -q
uv run python -m ruff check .
```
