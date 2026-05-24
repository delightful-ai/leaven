"""`lv.workspace.firkin(...)` — Firkin (snapshot-based) workspace backend.

Reserved scaffold per the Rust workspace topology. Firkin is the
snapshot-fast workspace backend used by the agentic git path.
"""

from __future__ import annotations

from typing import Literal

from .config import WorkspaceConfig


class FirkinWorkspace(WorkspaceConfig):
    """Firkin snapshot workspace backend (reserved name; pending stabilization)."""

    backend: Literal["firkin"] = "firkin"
    root: str = ".firkin"


def firkin(*, root: str = ".firkin") -> FirkinWorkspace:
    """Firkin workspace backend config."""
    return FirkinWorkspace(root=root)


__all__ = ["FirkinWorkspace", "firkin"]
