"""`lv.agent.claude_code(...)` — Claude Code agent runtime."""

from __future__ import annotations

from typing import Literal

from .config import AgentConfig


class ClaudeCodeAgent(AgentConfig):
    """Claude Code agent runtime config."""

    provider: Literal["claude_code"] = "claude_code"
    model: str
    bin_path_env: str | None = None


def claude_code(
    *,
    model: str,
    bin_path_env: str | None = None,
    role: str | None = None,
    timeout_s: float | None = None,
) -> ClaudeCodeAgent:
    """Claude Code agent runtime config builder."""
    return ClaudeCodeAgent(
        model=model,
        bin_path_env=bin_path_env,
        role=role,
        timeout_s=timeout_s,
    )


__all__ = ["ClaudeCodeAgent", "claude_code"]
