"""AgentInstructions — typed instruction bundle for `cx.agent.run(instructions=...)`.

Carries task text plus optional developer/system role content. The seam
validates structure; the agent runtime renders the prompt.
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict


class AgentInstructions(BaseModel):
    """Typed instructions for an agent session."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    task: str
    """The task description shown to the agent."""

    developer: str | None = None
    """Developer/system-role content. Optional; prepended to task."""

    rubric: str | None = None
    """Optional rubric block (rendered separately from task in the prompt)."""


class AgentRoles:
    """Convention strings for the `developer` field across common roles.

    Use as `lv.AgentInstructions(task=..., developer=lv.roles.EXECUTOR)`.
    """

    EXECUTOR = "executor"
    """Stage that executes the candidate against the case."""

    PROPOSER = "proposer"
    """Stage that proposes a change to the candidate."""

    BUILDER = "builder"
    """Stage that materializes a proposed change."""

    JUDGE = "judge"
    """Stage that judges or scores."""

    SKILL_PROPOSER = "skill_proposer"
    """Skill-bank-specific proposer convention."""


__all__ = ["AgentInstructions", "AgentRoles"]
