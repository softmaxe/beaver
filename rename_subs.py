from __future__ import annotations

import sys
from pathlib import Path


def _main() -> int:
    # Allow running from a fresh venv without installing the package.
    repo_root = Path(__file__).resolve().parent
    sys.path.insert(0, str(repo_root / "src"))

    from rename_subtitles.cli import main

    return main()


if __name__ == "__main__":
    raise SystemExit(_main())
