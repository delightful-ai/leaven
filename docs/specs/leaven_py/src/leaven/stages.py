"""Composable stage objects for `lv.evolve(...)`.

Decorators remain useful for authoring Python stage functions, but the high-level
surface composes explicit, swappable stage objects. This mirrors FlashEvolve's
stage composition while lowering into Leaven's evaluator/proposer/assessment
machinery.
"""

from __future__ import annotations

from collections.abc import Sequence
from typing import Any, Literal

from pydantic import BaseModel, ConfigDict, Field

from .agent.config import AgentConfig
from .decorators import RegisteredStage
from .layouts import WorkspaceLayout, case_workspace, edit_artifact
from .output import OutputContract


class Rollout(BaseModel):
    """How to run the current artifact against a selected sample."""

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    kind: Literal["function", "command", "agent"]
    layout: WorkspaceLayout
    stage: RegisteredStage[Any, Any] | None = None
    argv: list[str] | None = None
    agent_config: AgentConfig | None = None
    instructions: str | None = None
    output: OutputContract | None = None

    @classmethod
    def fn(
        cls,
        stage: RegisteredStage[Any, Any],
        *,
        layout: WorkspaceLayout | None = None,
    ) -> Rollout:
        """Use a registered `@lv.runner` stage as the rollout."""
        return cls(kind="function", stage=stage, layout=layout or case_workspace())

    @classmethod
    def command(
        cls,
        *,
        argv: Sequence[str],
        layout: WorkspaceLayout | None = None,
        output: OutputContract | None = None,
    ) -> Rollout:
        """Run a command inside the stage workspace."""
        return cls(kind="command", argv=list(argv), layout=layout or case_workspace(), output=output)

    @classmethod
    def agent(
        cls,
        *,
        agent: AgentConfig,
        instructions: str,
        layout: WorkspaceLayout | None = None,
        output: OutputContract | None = None,
    ) -> Rollout:
        """Run an agent inside the stage workspace."""
        return cls(
            kind="agent",
            agent_config=agent,
            instructions=instructions,
            layout=layout or case_workspace(),
            output=output,
        )


class Reflect(BaseModel):
    """Reflection stage configuration."""

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    kind: Literal["function", "default_gepa"]
    stage: RegisteredStage[Any, Any] | None = None

    @classmethod
    def fn(cls, stage: RegisteredStage[Any, Any]) -> Reflect:
        """Use a registered `@lv.reflector` stage."""
        return cls(kind="function", stage=stage)

    @classmethod
    def default_gepa(cls) -> Reflect:
        """Use the optimizer's default GEPA reflection stage."""
        return cls(kind="default_gepa")


class ScoreStage(BaseModel):
    """How to score one rollout result for one case."""

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    kind: Literal["function"]
    stage: RegisteredStage[Any, Any]

    @classmethod
    def fn(cls, stage: RegisteredStage[Any, Any]) -> ScoreStage:
        """Use a registered `@lv.scorer` stage."""
        return cls(kind="function", stage=stage)


class Propose(BaseModel):
    """Proposal stage configuration."""

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    kind: Literal["function", "agent_edit"]
    stage: RegisteredStage[Any, Any] | None = None
    agent_config: AgentConfig | None = None
    layout: WorkspaceLayout = Field(default_factory=edit_artifact)

    @classmethod
    def fn(cls, stage: RegisteredStage[Any, Any]) -> Propose:
        """Use a registered `@lv.proposer` stage."""
        return cls(kind="function", stage=stage)

    @classmethod
    def agent_edit(
        cls,
        *,
        agent: AgentConfig,
        layout: WorkspaceLayout | None = None,
    ) -> Propose:
        """Let an agent edit the artifact projection under the mutable root."""
        return cls(kind="agent_edit", agent_config=agent, layout=layout or edit_artifact())


class Evaluate(BaseModel):
    """Evaluation stage configuration."""

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    kind: Literal["function", "pipeline"]
    stage: RegisteredStage[Any, Any] | None = None
    rollout_stage: Rollout | None = None
    score_stage: ScoreStage | None = None
    split: str = "val"

    @classmethod
    def fn(cls, stage: RegisteredStage[Any, Any]) -> Evaluate:
        """Use a registered `@lv.evaluator` stage."""
        return cls(kind="function", stage=stage)

    @classmethod
    def pipeline(
        cls,
        *,
        rollout: Rollout,
        score: ScoreStage,
        split: str = "val",
    ) -> Evaluate:
        """Evaluate candidates by running a rollout and then a score stage."""
        return cls(kind="pipeline", rollout_stage=rollout, score_stage=score, split=split)


class Stages(BaseModel):
    """The swappable stage composition for an evolution run."""

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    rollout: Rollout
    score: ScoreStage
    reflect: Reflect | None = None
    propose: Propose | None = None
    evaluate: Evaluate


__all__ = ["Evaluate", "Propose", "Reflect", "Rollout", "ScoreStage", "Stages"]
