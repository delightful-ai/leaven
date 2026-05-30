"""`lv.agent.*` — agent builders.

`codex` is the ONLY first-class agent in V1. `command` / `config` are generic
escape hatches. `claude_code` / `opencode` are RESERVED scaffold names that
raise `NotImplementedError` until real.

Governing spec: `docs/specs/leaven_python.md` — Codex as the default agent.
"""

from __future__ import annotations

from collections.abc import Sequence

from .config import AgentConfig

__all__ = ["AgentConfig", "claude_code", "codex", "command", "config", "opencode"]


def codex(*, model: str | None = None, **kwargs: object) -> AgentConfig:
    """The first-class Codex agent (`lv.agent.codex()`)."""
    raise NotImplementedError("see leaven_python.md — Codex as the default agent")


def command(argv: Sequence[str], **kwargs: object) -> AgentConfig:
    """Generic escape hatch: run an arbitrary CLI agent (`lv.agent.command([...])`)."""
    raise NotImplementedError("see leaven_python.md — Codex as the default agent")


def config(**kwargs: object) -> AgentConfig:
    """Generic escape hatch: custom provider config (`lv.agent.config(...)`)."""
    raise NotImplementedError("see leaven_python.md — Codex as the default agent")


def claude_code(*args: object, **kwargs: object) -> AgentConfig:
    """Reserved scaffold name; not blessed in V1."""
    raise NotImplementedError("reserved scaffold name; see leaven_python.md agents section")


def opencode(*args: object, **kwargs: object) -> AgentConfig:
    """Reserved scaffold name; not blessed in V1."""
    raise NotImplementedError("reserved scaffold name; see leaven_python.md agents section")
