"""Agent runtime config base."""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict


class AgentConfig(BaseModel):
    """Common agent runtime config. Provider-specific subclasses add fields."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    provider: str
    """Runtime name (e.g. 'codex', 'claude_code', 'command')."""

    role: str | None = None
    """Optional role binding (e.g. 'executor', 'proposer')."""

    timeout_s: float | None = None


__all__ = ["AgentConfig"]
