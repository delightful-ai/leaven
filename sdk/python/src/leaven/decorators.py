"""Stage decorators — `@lv.evaluator`, `@lv.reflector`, `@lv.proposer`, etc.

Each decorator wraps an async function into a stage handler that the engine
calls via the ACP wire. The decorator is sugar over `register_stage(...)`;
both forms produce the same `RegisteredStage` value.

A decorated function can run two ways:
- Composed into an `lv.optimize(...)` run (the engine reaches it in-process via
  the embedded ACP loop)
- Standalone via `if __name__ == "__main__": lv.serve_stage(my_stage)`
  (the engine spawns the script as a subprocess and reaches it over stdio)

The user code is identical in both cases; only the way the engine reaches the
stage differs. Scoring is authored with `@lv.reward` (a `Rubric`), not a stage
decorator.

Scaffold note: the decorators construct real `RegisteredStage` values so
user code composes cleanly. The engine binding lives in
`lv.optimize(...).run()` and `lv.serve_stage(...)` — both raise
NotImplementedError until the implementation lands.
"""

from __future__ import annotations

from collections.abc import Awaitable, Callable
from typing import Any, Literal, overload

from pydantic import BaseModel, ConfigDict

from .case import Case
from .contexts import (
    EvaluatorContext,
    JudgeContext,
    ProposeContext,
    ReflectContext,
    RolloutContext,
)
from .evaluation_job import EvaluationJob, Granularity
from .proposal import ProposalBatch
from .stage_payloads import JudgeRequest, ProposeRequest, ReflectionResult, ReflectRequest
from .trust import TrustProfile

StageRole = Literal["evaluator", "reflector", "proposer", "runner", "judge"]


class RegisteredStage[A, O](BaseModel):
    """A registered stage handle. Pass to `lv.optimize(...)` or `lv.serve_stage(...)`.

    `A` is the artifact type the stage operates on; `O` is the stage's
    return type. Both are erased at runtime; the typing is for IDE support.
    """

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    role: StageRole
    id: str
    trust_profile: TrustProfile = TrustProfile.MANAGED_SANDBOX
    granularity: Granularity | None = None
    func: Any
    """The wrapped async user function. Engine-internal."""


# Type aliases for stage function signatures.
EvaluatorFunc = Callable[[EvaluationJob, EvaluatorContext], Awaitable[Any]]
ReflectorFunc = Callable[[ReflectRequest, ReflectContext], Awaitable[ReflectionResult]]
ProposerFunc = Callable[[ProposeRequest, ProposeContext], Awaitable[ProposalBatch]]
RunnerFunc = Callable[[Any, Case, RolloutContext], Awaitable[Any]]
JudgeFunc = Callable[[JudgeRequest, JudgeContext], Awaitable[Any]]


def _resolve_trust(profile: TrustProfile | str) -> TrustProfile:
    return profile if isinstance(profile, TrustProfile) else TrustProfile(profile)


def _make_registered(
    role: StageRole,
    func: Callable[..., Awaitable[Any]],
    stage_id: str | None,
    trust_profile: TrustProfile | str,
    *,
    granularity: Granularity | None = None,
) -> RegisteredStage[Any, Any]:
    """Internal: build a RegisteredStage from a decorated function.

    Used by all stage decorators. The scaffold returns a real value so user
    code composes cleanly; engine wiring lives in `lv.optimize(...).run()`
    and `lv.serve_stage(...)`.
    """
    return RegisteredStage(
        role=role,
        id=stage_id
        or f"{getattr(func, '__module__', 'leaven')}.{getattr(func, '__name__', 'stage')}",
        trust_profile=_resolve_trust(trust_profile),
        granularity=granularity,
        func=func,
    )


@overload
def evaluator(func: EvaluatorFunc) -> RegisteredStage[Any, Any]: ...
@overload
def evaluator(
    *,
    id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
    granularity: Granularity = "per_case",
) -> Callable[[EvaluatorFunc], RegisteredStage[Any, Any]]: ...
def evaluator(
    func: EvaluatorFunc | None = None,
    *,
    id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
    granularity: Granularity = "per_case",
) -> Any:
    """Decorate an async function as an evaluator stage (advanced / seam).

    Evaluators drive a whole evaluation job with batched effects and custom
    assessments. Ordinary scoring is a `Rubric` (`@lv.reward`); reach for an
    evaluator only when you need batched effects across cases or hand-authored
    evidence.
    """

    def wrap(f: EvaluatorFunc) -> RegisteredStage[Any, Any]:
        return _make_registered("evaluator", f, id, trust_profile, granularity=granularity)

    return wrap(func) if func is not None else wrap


@overload
def reflector(func: ReflectorFunc) -> RegisteredStage[Any, ReflectionResult]: ...
@overload
def reflector(
    *,
    stage_id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
) -> Callable[[ReflectorFunc], RegisteredStage[Any, ReflectionResult]]: ...
def reflector(
    func: ReflectorFunc | None = None,
    *,
    stage_id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
) -> Any:
    """Decorate an async function as a reflector stage.

    Reflectors produce typed `ReflectionResult`s. They are forbidden from
    carrying `case.target` data classes — the seam enforces target egress.
    """

    def wrap(f: ReflectorFunc) -> RegisteredStage[Any, ReflectionResult]:
        return _make_registered("reflector", f, stage_id, trust_profile)

    return wrap(func) if func is not None else wrap


@overload
def proposer(func: ProposerFunc) -> RegisteredStage[Any, ProposalBatch]: ...
@overload
def proposer(
    *,
    stage_id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
    repair_attempts: int = 0,
) -> Callable[[ProposerFunc], RegisteredStage[Any, ProposalBatch]]: ...
def proposer(
    func: ProposerFunc | None = None,
    *,
    stage_id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
    repair_attempts: int = 0,
) -> Any:
    """Decorate an async function as a proposer stage.

    Proposers consume a `ReflectionResult` and emit a `ProposalBatch`.
    `repair_attempts` configures parse-retry budget on malformed output.
    """

    def wrap(f: ProposerFunc) -> RegisteredStage[Any, ProposalBatch]:
        return _make_registered("proposer", f, stage_id, trust_profile)

    return wrap(func) if func is not None else wrap


@overload
def runner(func: RunnerFunc) -> RegisteredStage[Any, Any]: ...
@overload
def runner(
    *,
    id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
) -> Callable[[RunnerFunc], RegisteredStage[Any, Any]]: ...
def runner(
    func: RunnerFunc | None = None,
    *,
    id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
) -> Any:
    """Decorate an async function as a runner stage (a function rollout).

    Runners execute one candidate against one case and return the output the
    rubric will score. Wrap with `Rollout.fn(run)` to use as an environment
    rollout.
    """

    def wrap(f: RunnerFunc) -> RegisteredStage[Any, Any]:
        return _make_registered("runner", f, id, trust_profile)

    return wrap(func) if func is not None else wrap


@overload
def judge(func: JudgeFunc) -> RegisteredStage[Any, Any]: ...
@overload
def judge(
    *,
    stage_id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
) -> Callable[[JudgeFunc], RegisteredStage[Any, Any]]: ...
def judge(
    func: JudgeFunc | None = None,
    *,
    stage_id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
) -> Any:
    """Decorate an async function as a judge stage (pairwise or listwise)."""

    def wrap(f: JudgeFunc) -> RegisteredStage[Any, Any]:
        return _make_registered("judge", f, stage_id, trust_profile)

    return wrap(func) if func is not None else wrap


def register_stage(
    role: StageRole,
    func: Callable[..., Awaitable[Any]],
    *,
    id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
    **role_kwargs: Any,
) -> RegisteredStage[Any, Any]:
    """Function form of the stage decorators; equivalent to `@lv.<role>(...)`.

    Useful for dynamic registration. The decorator forms are sugar over this.
    """
    return _make_registered(role, func, id, trust_profile)


def serve_stage(*stages: RegisteredStage[Any, Any]) -> None:
    """Run one or more stages as a standalone ACP worker process.

    Usage:
        if __name__ == "__main__":
            lv.serve_stage(my_stage)

    Reads `LEAVEN_CAPABILITY_TOKEN`, `LEAVEN_ENDPOINT`, and
    `LEAVEN_CAPABILITY_FINGERPRINT` from env per the locked ACP profile,
    spawns the ACP server loop, and dispatches stage calls until the
    session terminates.
    """
    raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = [
    "EvaluatorFunc",
    "JudgeFunc",
    "ProposerFunc",
    "ReflectorFunc",
    "RegisteredStage",
    "RunnerFunc",
    "StageRole",
    "evaluator",
    "judge",
    "proposer",
    "reflector",
    "register_stage",
    "runner",
    "serve_stage",
]
