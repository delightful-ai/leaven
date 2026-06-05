"""Composable stage objects for the optimization surface.

`Rollout` is how the current artifact runs on a case (used by `Environment`).
`Reflect` and `Propose` are the optimizer's outer-loop stage overrides. Scoring
is authored with `@lv.reward` (a `Rubric`), not a stage object. Decorators
remain the authoring sugar for the function-backed forms (`Rollout.fn`,
`Reflect.fn`, `Propose.fn`).
"""

from collections.abc import Sequence
from typing import Literal

from pydantic import BaseModel, ConfigDict, Field

from .agent.config import AgentConfig
from .artifacts.prompt import PromptArtifact
from .decorators import RegisteredStage
from .layouts import WorkspaceLayout, case_workspace, edit_artifact
from .output import OutputContract
from .proposal import ProposalBatch
from .stage_payloads import ProposeRequest, ReflectionResult, ReflectRequest


class Rollout(BaseModel):
    """How to run the current artifact against a selected sample."""

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    kind: Literal["function", "command", "agent"]
    layout: WorkspaceLayout
    stage: RegisteredStage[PromptArtifact, str] | None = None
    argv: list[str] | None = None
    agent_config: AgentConfig | None = None
    instructions: str | None = None
    output: OutputContract | None = None

    @classmethod
    def fn(
        cls,
        stage: RegisteredStage[PromptArtifact, str],
        *,
        layout: WorkspaceLayout | None = None,
    ) -> "Rollout":
        """Use a registered `@lv.runner` stage as the rollout."""
        return cls(
            kind="function",
            stage=stage,
            layout=layout or case_workspace(),
        )

    @classmethod
    def command(
        cls,
        *,
        argv: Sequence[str],
        layout: WorkspaceLayout | None = None,
        output: OutputContract | None = None,
    ) -> "Rollout":
        """Run a command inside the stage workspace."""
        return cls(
            kind="command", argv=list(argv), layout=layout or case_workspace(), output=output
        )

    @classmethod
    def agent(
        cls,
        *,
        agent: AgentConfig | None = None,
        instructions: str | None = None,
        layout: WorkspaceLayout | None = None,
        output: OutputContract | None = None,
    ) -> "Rollout":
        """Run an agent inside the stage workspace.

        Codex-native default: with no `agent`, the runtime's configured agent
        runs; with no `instructions`, the engine derives them from the case
        input. The agent owns its own multi-turn loop.
        """
        return cls(
            kind="agent",
            agent_config=agent,
            instructions=instructions,
            layout=layout or case_workspace(),
            output=output,
        )


class Reflect(BaseModel):
    """Reflection stage configuration (optimizer outer-loop override)."""

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    kind: Literal["function", "default_gepa"]
    stage: RegisteredStage[ReflectRequest, ReflectionResult] | None = None

    @classmethod
    def fn(cls, stage: RegisteredStage[ReflectRequest, ReflectionResult]) -> "Reflect":
        """Use a registered `@lv.reflector` stage."""
        return cls(kind="function", stage=stage)

    @classmethod
    def default_gepa(cls) -> "Reflect":
        """Use the optimizer's default GEPA reflection stage."""
        return cls(kind="default_gepa")


class Propose(BaseModel):
    """Proposal stage configuration (optimizer outer-loop override)."""

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    kind: Literal["function", "agent_edit"]
    stage: RegisteredStage[ProposeRequest, ProposalBatch] | None = None
    agent_config: AgentConfig | None = None
    layout: WorkspaceLayout = Field(default_factory=edit_artifact)

    @classmethod
    def fn(cls, stage: RegisteredStage[ProposeRequest, ProposalBatch]) -> "Propose":
        """Use a registered `@lv.proposer` stage."""
        return cls(kind="function", stage=stage)

    @classmethod
    def agent_edit(
        cls,
        *,
        agent: AgentConfig,
        layout: WorkspaceLayout | None = None,
    ) -> "Propose":
        """Let an agent edit the artifact projection under the mutable root."""
        return cls(kind="agent_edit", agent_config=agent, layout=layout or edit_artifact())


__all__ = ["Propose", "Reflect", "Rollout"]
