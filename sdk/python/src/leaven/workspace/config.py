"""Workspace backend config base."""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict


class WorkspaceConfig(BaseModel):
    """Common workspace config. Backend-specific subclasses add fields."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    backend: str
    """Backend name (e.g. 'local', 'docker', 'git', 'firkin')."""


__all__ = ["WorkspaceConfig"]
