"""`lv.workspace.git(...)` — git-program workspace backend.

For artifacts that ARE git repositories (GitProgramArtifact). The backend
checks out a branch per candidate; proposals are commits/patches.
"""

from typing import Literal

from .config import WorkspaceConfig


class GitWorkspace(WorkspaceConfig):
    """Git-program workspace backend."""

    backend: Literal["git"] = "git"
    upstream: str
    """Upstream git URL or path."""
    branch_prefix: str = "leaven/"
    """Prefix for engine-managed candidate branches."""


def git(*, upstream: str, branch_prefix: str = "leaven/") -> GitWorkspace:
    """Git-program workspace backend config."""
    return GitWorkspace(upstream=upstream, branch_prefix=branch_prefix)


__all__ = ["GitWorkspace", "git"]
