"""Context — the product `cx` passed explicitly to every stage function.

`cx` is passed EXPLICITLY to every stage fn (NO ContextVar magic). Context
fields are UNIFORM across all context types (no field optional-across-boundary).

This module owns the structural product `Context` Protocol and the handle
protocols it exposes. The concrete typed `RunContext` / `StageContext` /
`EvalContext` live in `lv.adapters.contexts` and are the annotation surface for
advanced authoring.

`Context` is NOT a top-level product noun (cx is passed, not imported);
advanced annotation uses `lv.adapters.contexts`.

Governing spec: `docs/specs/leaven_python.md` — constraints on implementation
(no ContextVar; uniform context fields).
"""

from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, Protocol, runtime_checkable

from pydantic import BaseModel, ConfigDict

from ._handles import WorkspaceView

if TYPE_CHECKING:
    from .case import Case
    from .output import OutputContract

__all__ = [
    "AgentHandle",
    "AgentRunResult",
    "BatchContext",
    "CaseReader",
    "Context",
    "LmHandle",
    "SandboxHandle",
    "WorkspaceHandleProto",
]


class AgentRunResult[T](BaseModel):
    """Result of an engine-mediated `cx.agent.run(...)` call.

    Generic; an unparameterized `AgentRunResult` binds `T` to `Any` (pydantic
    default), so a bare annotation works, while `AgentRunResult[Verdict]` makes
    `verdict.parsed.score` type-check when `T` is given.
    """

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    parsed: T


@runtime_checkable
class LmHandle(Protocol):
    """Engine-mediated LM handle exposed on `cx.lm`."""

    async def complete(self, *args: object, **kwargs: object) -> object: ...
    async def complete_text(self, prompt: str, **kwargs: object) -> str: ...


@runtime_checkable
class AgentHandle(Protocol):
    """Engine-mediated agent handle exposed on `cx.agent`."""

    async def run(
        self,
        *,
        workspace: WorkspaceView | None = None,
        instructions: str,
        output: OutputContract | None = None,
        **kwargs: object,
    ) -> AgentRunResult: ...


@runtime_checkable
class SandboxHandle(Protocol):
    """Engine-mediated sandbox handle exposed on `cx.sandbox`."""

    async def exec(self, *, workspace: object, argv: Sequence[str], **kwargs: object) -> object: ...


@runtime_checkable
class WorkspaceHandleProto(Protocol):
    """Engine-mediated workspace handle exposed on `cx.workspace`."""

    async def materialize_candidate(self, candidate: object) -> WorkspaceView: ...


@runtime_checkable
class CaseReader(Protocol):
    """Engine-mediated case reader exposed on `cx.case`.

    `load(include=...)` re-reads the current case with an explicit projection
    set (e.g. include extra metadata or files); the engine enforces visibility,
    so a rollout still cannot pull a hidden target through it.
    """

    async def load(self, *, include: Sequence[str] | None = None) -> Case: ...


@runtime_checkable
class BatchContext(Protocol):
    """The `async with cx.batch() as b:` transaction handle.

    Its `.workspace` / `.sandbox` / `.agent` mirror the same handles, collapsing
    multiple ops into one transaction.
    """

    lm: LmHandle
    agent: AgentHandle
    sandbox: SandboxHandle
    workspace: WorkspaceHandleProto

    async def __aenter__(self) -> BatchContext: ...
    async def __aexit__(self, *exc: object) -> None: ...


@runtime_checkable
class Context(Protocol):
    """The product `cx` passed to every stage function.

    Uniform fields across all context types. No ContextVar; `cx` is an explicit
    parameter.

    Per-role capability differences (e.g. a reflector LM call may not egress
    `case.target`) are enforced at RUNTIME by the engine via capability tokens
    and data-class propagation (raising `CapabilityError` on violation), NOT
    modeled as a different Python type per role. There is ONE `Context`; the
    advanced `EvalContext` extends it for the batched-effect evaluator path.
    """

    lm: LmHandle
    agent: AgentHandle
    sandbox: SandboxHandle
    workspace: WorkspaceHandleProto
    case: CaseReader

    async def trace(self, *events: object) -> None: ...
    def batch(self) -> BatchContext: ...
