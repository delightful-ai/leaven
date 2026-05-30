"""`lv.artifacts.unsafe(path)` — opt out of the known mutable surface.

`unsafe(...)` warns at construction and allows an out-of-surface mutable path.
NOTE: the spec uses `lv.unsafe(...)` in prose, but it is NOT in the top-level
allow-list, so it is reached as `lv.artifacts.unsafe`.

Governing spec: `docs/specs/leaven_python.md` — codex_kit.
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict

__all__ = ["UnsafePath", "unsafe"]


class UnsafePath(BaseModel):
    """An explicitly-unsafe mutable path outside the known surface."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    path: str


def unsafe(path: str) -> UnsafePath:
    """Wrap an out-of-surface mutable path; warns at construction time."""
    raise NotImplementedError("see leaven_python.md — codex_kit / unsafe")
