"""AgentInstructions — typed instruction bundle for `cx.agent.run(instructions=...)`.

Carries per-run task text plus optional stable system context. Mutable behavior
belongs in the artifact/workspace, not in this instruction bundle.
"""

from pydantic import BaseModel, ConfigDict


class AgentInstructions(BaseModel):
    """Typed instructions for an agent session."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    task: str
    """The task description shown to the agent."""

    system: str | None = None
    """Stable system/developer context. Do not put mutable artifact behavior here."""

    rubric: str | None = None
    """Optional rubric block (rendered separately from task in the prompt)."""


class AgentRoles:
    """Stable prompt-context labels for common stage purposes.

    Use as `lv.AgentInstructions(task=..., system=lv.roles.JUDGE)` only when
    the role text is stable context. Mutable executor/proposer behavior belongs
    in the artifact being optimized.
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
