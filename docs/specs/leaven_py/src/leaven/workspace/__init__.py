"""`lv.workspace.*` — workspace backend builders.

Governing spec: `docs/specs/leaven_python.md` — Runtime / workspace.
"""

from __future__ import annotations

from .config import WorkspaceConfig

__all__ = ["WorkspaceConfig", "docker", "firkin", "git", "local"]


def local(*, root: str | None = None, **kwargs: object) -> WorkspaceConfig:
    """Local workspace (`lv.workspace.local(root=".leaven/work")`)."""
    raise NotImplementedError("see leaven_python.md — Runtime / workspace")


def git(*, root: str | None = None, **kwargs: object) -> WorkspaceConfig:
    """Git workspace (`lv.workspace.git(...)`)."""
    raise NotImplementedError("see leaven_python.md — Runtime / workspace")


def docker(**kwargs: object) -> WorkspaceConfig:
    """Docker workspace (`lv.workspace.docker(...)`)."""
    raise NotImplementedError("see leaven_python.md — Runtime / workspace")


def firkin(**kwargs: object) -> WorkspaceConfig:
    """Firkin workspace (`lv.workspace.firkin(...)`)."""
    raise NotImplementedError("see leaven_python.md — Runtime / workspace")
