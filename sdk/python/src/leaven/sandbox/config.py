"""Sandbox config base."""

from pydantic import BaseModel, ConfigDict


class SandboxConfig(BaseModel):
    """Common sandbox config."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    backend: str
    """Backend name (e.g. 'docker', 'local')."""


__all__ = ["SandboxConfig"]
