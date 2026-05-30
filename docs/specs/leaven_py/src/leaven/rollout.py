"""Rollout — `lv.Rollout`, `lv.RolloutResult`, `lv.RolloutStatus`.

A rollout is the interpretation of the current artifact on one case. It is
EITHER a function you write (custom logic; agentic work via `cx` primitives
inside) OR a declarative built-in (`Rollout.agent` / `.command` / `.manifest`)
for the engine-mediated no-Python-logic case.

`Rollout` is a NAMESPACE class (not a record): only the three classmethods are
public; they return opaque frozen spec objects the engine consumes.

Governing spec: `docs/specs/leaven_python.md` — Rollout / RolloutResult.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from enum import StrEnum
from typing import TYPE_CHECKING

from pydantic import BaseModel, ConfigDict

from ._handles import AgentSession, TrajectoryView, WorkspacePath, WorkspaceView

if TYPE_CHECKING:
    from .agent.config import AgentConfig
    from .layouts import Layout
    from .output import OutputContract

__all__ = ["Rollout", "RolloutError", "RolloutResult", "RolloutStatus"]


class RolloutStatus(StrEnum):
    """Terminal status of a rollout attempt."""

    ok = "ok"
    timeout = "timeout"
    crash = "crash"
    refused = "refused"
    budget_exceeded = "budget_exceeded"
    error = "error"


class RolloutError(BaseModel):
    """A structured failure record attached to a non-`ok` rollout.

    First-class because (per prime-rl) a failure mode is free feedback: a
    scorer/reflector reads `kind`/`message` and can turn it into actionable
    `Score.feedback` without re-parsing logs.
    """

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: str
    message: str = ""


class RolloutResult[Out](BaseModel):
    """A completed attempt — what a scorer/reflector reads.

    Generic; an unparameterized `lv.RolloutResult` binds `Out` to `Any`
    (pydantic default), so a bare annotation works, while `RolloutResult[Answer]`
    is opt-in precision when the output is structured.

    A bare-`Out` function return is wrapped by the engine into a
    `RolloutResult[Out]` with `sessions=()`, reduced trajectory, and
    `output_files=()`.

    `status` / `error` / `stop_condition` are first-class because (prime-rl
    insight) they are free feedback: the reason a rollout ended is high-signal
    evidence for scoring and reflection.
    """

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    output: Out
    workspace: WorkspaceView | None = None
    sessions: Sequence[AgentSession] = ()
    trajectory: TrajectoryView | None = None
    output_files: Sequence[WorkspacePath] = ()
    status: RolloutStatus = RolloutStatus.ok
    error: RolloutError | None = None
    stop_condition: str | None = None


class Rollout:
    """Namespace of declarative engine-mediated rollout built-ins.

    Not instantiable as a record; use the classmethods. `instructions=` is the
    STABLE invocation envelope — the mutable instructions live in the artifact.
    """

    @staticmethod
    def agent(
        agent: AgentConfig,
        *,
        layout: Layout | None = None,
        output: OutputContract | None = None,
        instructions: str | None = None,
    ) -> Rollout:
        """Engine runs the agent against the projected artifact (the artifact
        IS the behavior). Spec line 510."""
        raise NotImplementedError("see leaven_python.md — Rollout.agent")

    @staticmethod
    def command(
        argv: Sequence[str],
        *,
        layout: Layout | None = None,
        output: OutputContract | None = None,
        cwd: str | None = None,
        env: Mapping[str, str] | None = None,
    ) -> Rollout:
        """Engine runs a command against the projected artifact workspace.
        Spec line 513."""
        raise NotImplementedError("see leaven_python.md — Rollout.command")

    @staticmethod
    def manifest(
        path: str,
        *,
        layout: Layout | None = None,
        output: OutputContract | None = None,
    ) -> Rollout:
        """Engine reads the invocation from a file in the artifact, so the
        rollout shape itself is mutable artifact state. Spec line 517."""
        raise NotImplementedError("see leaven_python.md — Rollout.manifest")
