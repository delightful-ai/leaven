"""Workspace config — internal frozen dataclass.

Governing spec: `docs/specs/leaven_python.md` — Runtime / workspace.
"""

from __future__ import annotations

from dataclasses import dataclass

__all__ = ["WorkspaceConfig"]


@dataclass(frozen=True, slots=True)
class WorkspaceConfig:
    """Workspace config produced by `lv.workspace.*` builders."""

    kind: str
    root: str | None = None
