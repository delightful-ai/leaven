"""`lv.agent.command(...)` — generic command-substrate agent runtime.

For agents that aren't packaged as a first-class provider but speak a
known protocol (stdio JSON-RPC, OpenAI Responses API, etc.).
"""

from __future__ import annotations

from collections.abc import Sequence
from typing import Literal

from .config import AgentConfig


class CommandAgent(AgentConfig):
    """Generic command-substrate agent."""

    provider: Literal["command"] = "command"
    argv: list[str]
    protocol: Literal["stdio_jsonrpc", "openai_responses"] = "stdio_jsonrpc"
    env: dict[str, str] | None = None


def command(
    *,
    argv: Sequence[str],
    protocol: Literal["stdio_jsonrpc", "openai_responses"] = "stdio_jsonrpc",
    env: dict[str, str] | None = None,
    role: str | None = None,
    timeout_s: float | None = None,
) -> CommandAgent:
    """Generic command-substrate agent runtime config builder."""
    return CommandAgent(
        argv=list(argv),
        protocol=protocol,
        env=env,
        role=role,
        timeout_s=timeout_s,
    )


__all__ = ["CommandAgent", "command"]
