"""`lv.runs.open(path)` — inspect a completed run from outside.

Same `Optimized[A]` type as `lv.optimize(...).run()` returns; the engine is
spawned read-only against the run directory. Useful for retrospective
analysis, ablation reports, sharing run state with teammates.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .result import Optimized


def open(path: str | Path) -> Optimized[Any]:
    """Open a completed run from its run directory.

    The artifact type is `Any` here because the run's artifact type is
    determined at write time; callers can cast if they know the type.
    A future API revision may make this generic over a passed artifact type.
    """
    raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


def list_local(root: str | Path = ".leaven/runs") -> list[str]:
    """List run directory names under the local leaven root."""
    raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["list_local", "open"]
