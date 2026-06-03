"""Workspace backend configs — `lv.workspace.local()`, `lv.workspace.docker(...)`, etc."""

from .config import WorkspaceConfig
from .docker import DockerWorkspace, docker
from .firkin import FirkinWorkspace, firkin
from .git import GitWorkspace, git
from .local import LocalWorkspace, local

__all__ = [
    "DockerWorkspace",
    "FirkinWorkspace",
    "GitWorkspace",
    "LocalWorkspace",
    "WorkspaceConfig",
    "docker",
    "firkin",
    "git",
    "local",
]
