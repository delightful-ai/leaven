"""`cx.agent.*` — agentic run inside a workspace, with typed output contracts."""

from __future__ import annotations

from collections.abc import Sequence
from typing import Any

from pydantic import BaseModel, ConfigDict

from .._handles import WorkspaceHandle
from .._receipts import CallReceipt
from ..agent_instructions import AgentInstructions
from ..output import OutputContract


class AgentSession(BaseModel):
    """Result of `cx.agent.run(...)`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    transcript_ref: str
    """Blob ref to the full session transcript."""

    parsed: Any | None = None
    """Parsed structured output when `output=lv.output.json_schema(...)`."""

    final_message: str | None = None
    """Final assistant message text (when output is plain text)."""

    files: dict[str, bytes] | None = None
    """Captured files when `output=lv.output.files(...)`."""

    commands: list[dict[str, Any]]
    """Recorded commands the agent ran in the workspace."""

    cost_usd: float | None = None
    receipt: CallReceipt


class AgentBuilder:
    """Agent runs bound to a context. Requires a materialized workspace."""

    async def run(
        self,
        *,
        workspace: WorkspaceHandle,
        instructions: AgentInstructions,
        runtime: str | None = None,
        output: OutputContract | None = None,
        timeout_s: float | None = None,
        allowed_commands: Sequence[str] | None = None,
        input_classes: Sequence[str] | None = None,
        forbidden_input_classes: Sequence[str] | None = None,
    ) -> AgentSession:
        """Run an agent session against the workspace.

        `runtime` selects a configured agent (default if only one is configured;
        explicit when multiple are wired). `output` constrains the structured
        return; without it the session text is the result.
        """
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["AgentBuilder", "AgentSession"]
