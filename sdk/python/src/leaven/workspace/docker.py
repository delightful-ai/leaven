"""`lv.workspace.docker(...)` — Docker container workspace backend."""

from __future__ import annotations

from typing import Literal

from .config import WorkspaceConfig


class DockerWorkspace(WorkspaceConfig):
    """Docker container workspace; one container per materialization."""

    backend: Literal["docker"] = "docker"
    image: str
    network: str | None = None
    mounts: list[dict[str, str]] | None = None


def docker(
    *,
    image: str,
    network: str | None = None,
    mounts: list[dict[str, str]] | None = None,
) -> DockerWorkspace:
    """Docker container workspace backend config."""
    return DockerWorkspace(image=image, network=network, mounts=mounts)


__all__ = ["DockerWorkspace", "docker"]
