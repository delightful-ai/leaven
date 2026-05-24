"""Sandbox backend configs — `lv.sandbox.docker(...)`, `lv.sandbox.local()`."""

from __future__ import annotations

from .config import SandboxConfig
from .docker import DockerSandbox, docker
from .local import LocalSandbox, local

__all__ = ["DockerSandbox", "LocalSandbox", "SandboxConfig", "docker", "local"]
