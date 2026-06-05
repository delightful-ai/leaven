"""Workspace backend configs — `lv.workspace.local()`, `lv.workspace.docker(...)`, etc."""

from .config import WorkspaceConfig
from .docker import DockerWorkspace, docker
from .git import GitWorkspace, git
from .local import LocalWorkspace, local

__all__ = [
    "DockerWorkspace",
    "GitWorkspace",
    "LocalWorkspace",
    "WorkspaceConfig",
    "docker",
    "git",
    "local",
]
