"""`lv.sandbox.docker(...)` — Docker-backed sandbox for `cx.sandbox.exec`."""

from __future__ import annotations

from typing import Literal

from .config import SandboxConfig


class DockerSandbox(SandboxConfig):
    """Docker container sandbox."""

    backend: Literal["docker"] = "docker"
    image: str
    network: str | None = None
    """`None` for no network; explicit network name for controlled connectivity."""


def docker(*, image: str, network: str | None = None) -> DockerSandbox:
    """Docker container sandbox config."""
    return DockerSandbox(image=image, network=network)


__all__ = ["DockerSandbox", "docker"]
