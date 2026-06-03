"""Module entrypoint for `python -m leaven._seam_worker`."""

from __future__ import annotations

from .main import main

if __name__ == "__main__":
    raise SystemExit(main())
