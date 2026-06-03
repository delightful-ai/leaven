"""`lv.workspace.local(...)` — local filesystem workspace backend."""

from typing import Literal

from .config import WorkspaceConfig


class LocalWorkspace(WorkspaceConfig):
    """Local-filesystem workspace; one directory per materialization."""

    backend: Literal["local"] = "local"
    root: str = ".agents"
    """Root directory under which workspaces materialize."""


def local(*, root: str = ".agents") -> LocalWorkspace:
    """Local-filesystem workspace backend config.

    The root path is created if it doesn't exist; materialized workspaces
    are subdirectories. Use for single-machine paper repros.
    """
    return LocalWorkspace(root=root)


__all__ = ["LocalWorkspace", "local"]
