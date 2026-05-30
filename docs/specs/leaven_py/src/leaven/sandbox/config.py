"""Sandbox config — internal frozen dataclass.

Governing spec: `docs/specs/leaven_python.md` — Runtime / sandbox.
"""

from __future__ import annotations

from dataclasses import dataclass

__all__ = ["SandboxConfig"]


@dataclass(frozen=True, slots=True)
class SandboxConfig:
    """Sandbox config produced by `lv.sandbox.*` builders."""

    kind: str
    image: str | None = None
