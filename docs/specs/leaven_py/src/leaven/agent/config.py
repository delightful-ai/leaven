"""Agent config — internal frozen dataclass.

Governing spec: `docs/specs/leaven_python.md` — Codex as the default agent.
"""

from __future__ import annotations

from dataclasses import dataclass

__all__ = ["AgentConfig"]


@dataclass(frozen=True, slots=True)
class AgentConfig:
    """Agent config produced by `lv.agent.*` builders."""

    kind: str
    model: str | None = None
