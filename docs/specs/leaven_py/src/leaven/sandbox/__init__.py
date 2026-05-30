"""`lv.sandbox.*` — sandbox backend builders.

Governing spec: `docs/specs/leaven_python.md` — Runtime / sandbox.
"""

from __future__ import annotations

from .config import SandboxConfig

__all__ = ["SandboxConfig", "docker", "local"]


def docker(*, image: str, **kwargs: object) -> SandboxConfig:
    """Docker sandbox (`lv.sandbox.docker(image="python:3.12")`)."""
    raise NotImplementedError("see leaven_python.md — Runtime / sandbox")


def local(**kwargs: object) -> SandboxConfig:
    """Local sandbox (`lv.sandbox.local()`)."""
    raise NotImplementedError("see leaven_python.md — Runtime / sandbox")
