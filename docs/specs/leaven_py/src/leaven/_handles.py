"""Engine-owned read views referenced by `RolloutResult`.

These are the live, readable views a scorer/reflector reads off a completed
attempt: the workspace, agent sessions, the normalized trajectory, and output
file paths. `WorkspaceView` / `TrajectoryView` are `Protocol`s (engine-side
concrete); `WorkspacePath` / `AgentSession` are frozen models.

Exact trajectory normalization is downstream (spec line 1446); these are typed
placeholders. NONE of these are top-level product nouns — they are read off
`RolloutResult`, never imported by name from `lv`.

Governing spec: `docs/specs/leaven_python.md` — RolloutResult.
"""

from __future__ import annotations

from collections.abc import Sequence
from typing import Protocol, runtime_checkable

from pydantic import BaseModel, ConfigDict

__all__ = [
    "AgentSession",
    "Command",
    "Message",
    "ToolCall",
    "TrajectoryView",
    "WorkspacePath",
    "WorkspaceView",
]


class WorkspacePath(BaseModel):
    """A workspace-relative path produced by a rollout."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    path: str


class Command(BaseModel):
    """One normalized shell command in a trajectory."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    argv: Sequence[str] = ()
    exit_code: int | None = None


class ToolCall(BaseModel):
    """One normalized tool/function call in a trajectory."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    name: str
    ok: bool = True


class Message(BaseModel):
    """One normalized transcript message in a trajectory."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    role: str
    text: str = ""


@runtime_checkable
class WorkspaceView(Protocol):
    """Live, readable view of a rollout's workspace; cleanup deferred to scoring.

    Spec: `await run.workspace.read_text("output/run.log", missing_ok=True)`.
    """

    async def read_text(self, path: str, *, missing_ok: bool = False) -> str: ...
    async def read_bytes(self, path: str, *, missing_ok: bool = False) -> bytes: ...
    def exists(self, path: str) -> bool: ...


@runtime_checkable
class TrajectoryView(Protocol):
    """Normalized commands / tool calls / messages / files for a session.

    Accessors return typed records (`Command` / `ToolCall` / `Message`) a scorer
    can read structurally, not opaque `object`.
    """

    def commands(self) -> Sequence[Command]: ...
    def tool_calls(self) -> Sequence[ToolCall]: ...
    def messages(self) -> Sequence[Message]: ...
    def files(self) -> Sequence[WorkspacePath]: ...


class AgentSession(BaseModel):
    """An engine-mediated agent session captured during a rollout.

    Carries the fields a scorer needs: identity, which agent ran, terminal
    status, transcript/commands/output-files, and cost.
    """

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    id: str
    agent: str
    status: str = "ok"
    transcript: Sequence[Message] = ()
    commands: Sequence[Command] = ()
    output_files: Sequence[WorkspacePath] = ()
    cost_usd: float = 0.0
    trajectory: TrajectoryView | None = None
