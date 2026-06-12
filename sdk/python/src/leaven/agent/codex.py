"""`lv.agent.codex(...)` — Codex (app-server or CLI) agent runtime."""

from collections.abc import Sequence
from typing import Literal

from .config import AgentConfig


class CodexAgent(AgentConfig):
    """Codex app-server or CLI agent runtime config."""

    provider: Literal["codex"] = "codex"
    model: str
    transport: Literal["app_server", "cli"] = "app_server"
    """App-server (long-lived RPC) or CLI (per-session subprocess)."""
    approval_mode: Literal["bypass", "interactive"] = "bypass"
    bin_path_env: str | None = None
    """Env var holding the codex binary path. None = use PATH."""
    allowed_commands: list[str] | None = None
    codex_home: str | None = None
    """Override `CODEX_HOME` for the reflection codex. None = the driver prepares a
    run-scoped isolated home (copied `auth.json`, no personal `AGENTS.md`/config) so
    the operator's local codex doctrine never bleeds into the authored kit."""


def codex(
    *,
    model: str,
    transport: Literal["app_server", "cli"] = "app_server",
    approval_mode: Literal["bypass", "interactive"] = "bypass",
    bin_path_env: str | None = None,
    allowed_commands: Sequence[str] | None = None,
    codex_home: str | None = None,
    role: str | None = None,
    timeout_s: float | None = None,
) -> CodexAgent:
    """Codex agent runtime config builder."""
    return CodexAgent(
        model=model,
        transport=transport,
        approval_mode=approval_mode,
        bin_path_env=bin_path_env,
        allowed_commands=list(allowed_commands) if allowed_commands else None,
        codex_home=codex_home,
        role=role,
        timeout_s=timeout_s,
    )


__all__ = ["CodexAgent", "codex"]
