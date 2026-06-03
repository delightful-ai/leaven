"""`lv.runs.open(path)` — inspect a completed run from outside.

Same `Optimized[A]` type as `lv.optimize(...).run()` returns; the engine is
spawned read-only against the run directory. Useful for retrospective
analysis, ablation reports, sharing run state with teammates.
"""

from pathlib import Path
from typing import Any

from ._runs import list_run_dirs, open_optimized
from .result import Optimized
from .run_inspection import RunInspection, inspect_optimized


def open(path: str | Path) -> Optimized[Any]:
    """Open a completed run from its run directory.

    The artifact type is `Any` here because the run's artifact type is
    determined at write time; callers can cast if they know the type.
    A future API revision may make this generic over a passed artifact type.
    """
    return open_optimized(path)


def list_local(root: str | Path = ".leaven/runs") -> list[str]:
    """List run directory names under the local leaven root."""
    return list_run_dirs(root)


def inspect(path: str | Path) -> RunInspection:
    """Open a completed run and return a flattened inspection summary."""
    return inspect_optimized(open_optimized(path))


__all__ = ["RunInspection", "inspect", "list_local", "open"]
