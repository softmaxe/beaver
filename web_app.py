from __future__ import annotations

import sys
from pathlib import Path


def _main() -> int:
    repo_root = Path(__file__).resolve().parent
    sys.path.insert(0, str(repo_root / "src"))

    from rename_subtitles.web import main

    return main()


if __name__ == "__main__":
    raise SystemExit(_main())
