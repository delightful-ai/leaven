"""Stage decorators — `@lv.evaluator`, `@lv.reflector`, `@lv.proposer`, etc.

Each decorator wraps an async function into a stage handler that the engine
calls via the Leaven public seam. The decorator is sugar over `register_stage(...)`;
both forms produce the same `RegisteredStage` value.

A decorated function can run two ways:
- Composed into an `lv.optimize(...)` run (the engine reaches it through
  `leaven/stage.run`)
- Out-of-process worker mode after the standalone loop exists.

The user code is identical in both cases; only the way the engine reaches the
stage differs. Scoring is authored with `@lv.reward` (a `Rubric`), not a stage
decorator.

Decorators construct real `RegisteredStage` values so user code composes
cleanly. `lv.optimize(...).run()` can dispatch runner stages through the
durable seam; standalone worker service mode is not exported until it uses the
current worker runtime.
"""

from collections.abc import Awaitable, Callable
from typing import Literal, overload

from pydantic import BaseModel, ConfigDict

from .case import InputCaseView
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
    """A registered stage handle. Pass to `lv.optimize(...)` composition APIs.

    `A` is the artifact type the stage operates on; `O` is the stage's
    return type. Both are erased at runtime; the typing is for IDE support.
    """

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    role: StageRole
    id: str
    trust_profile: TrustProfile = TrustProfile.MANAGED_SANDBOX
    granularity: Granularity | None = None
    func: Callable[..., Awaitable[O]]
    """The wrapped async user function. Engine-internal."""


# Type aliases for stage function signatures.
type EvaluatorFunc[O] = Callable[[EvaluationJob, EvaluatorContext], Awaitable[O]]
type ReflectorFunc = Callable[[ReflectRequest, ReflectContext], Awaitable[ReflectionResult]]
type ProposerFunc = Callable[[ProposeRequest, ProposeContext], Awaitable[ProposalBatch]]
type RunnerFunc[A, O] = Callable[[A, InputCaseView, RolloutContext], Awaitable[O]]
type JudgeFunc[O] = Callable[[JudgeRequest, JudgeContext], Awaitable[O]]


def _resolve_trust(profile: TrustProfile | str) -> TrustProfile:
    return profile if isinstance(profile, TrustProfile) else TrustProfile(profile)


def _make_registered[A, O](
    role: StageRole,
    func: Callable[..., Awaitable[O]],
    stage_id: str | None,
    trust_profile: TrustProfile | str,
    *,
    granularity: Granularity | None = None,
) -> RegisteredStage[A, O]:
    """Internal: build a RegisteredStage from a decorated function.

    Used by all stage decorators. The scaffold returns a real value so user
    code composes cleanly; runner-stage wiring lives in `lv.optimize(...).run()`.
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
def evaluator[O](func: EvaluatorFunc[O]) -> RegisteredStage[object, O]: ...
@overload
def evaluator[O](
    *,
    id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
    granularity: Granularity = "per_case",
) -> Callable[[EvaluatorFunc[O]], RegisteredStage[object, O]]: ...
def evaluator[O](
    func: EvaluatorFunc[O] | None = None,
    *,
    id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
    granularity: Granularity = "per_case",
) -> RegisteredStage[object, O] | Callable[[EvaluatorFunc[O]], RegisteredStage[object, O]]:
    """Decorate an async function as an evaluator stage (advanced / seam).

    Evaluators drive a whole evaluation job with batched effects and custom
    assessments. Ordinary scoring is a `Rubric` (`@lv.reward`); reach for an
    evaluator only when you need batched effects across cases or hand-authored
    evidence.
    """

    def wrap(f: EvaluatorFunc[O]) -> RegisteredStage[object, O]:
        return _make_registered("evaluator", f, id, trust_profile, granularity=granularity)

    return wrap(func) if func is not None else wrap


@overload
def reflector(func: ReflectorFunc) -> RegisteredStage[object, ReflectionResult]: ...
@overload
def reflector(
    *,
    stage_id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
) -> Callable[[ReflectorFunc], RegisteredStage[object, ReflectionResult]]: ...
def reflector(
    func: ReflectorFunc | None = None,
    *,
    stage_id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
) -> (
    RegisteredStage[object, ReflectionResult]
    | Callable[[ReflectorFunc], RegisteredStage[object, ReflectionResult]]
):
    """Decorate an async function as a reflector stage.

    Reflectors produce typed `ReflectionResult`s. They are forbidden from
    carrying `case.target` data classes — the seam enforces target egress.
    """

    def wrap(f: ReflectorFunc) -> RegisteredStage[object, ReflectionResult]:
        return _make_registered("reflector", f, stage_id, trust_profile)

    return wrap(func) if func is not None else wrap


@overload
def proposer(func: ProposerFunc) -> RegisteredStage[object, ProposalBatch]: ...
@overload
def proposer(
    *,
    stage_id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
    repair_attempts: int = 0,
) -> Callable[[ProposerFunc], RegisteredStage[object, ProposalBatch]]: ...
def proposer(
    func: ProposerFunc | None = None,
    *,
    stage_id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
    repair_attempts: int = 0,
) -> (
    RegisteredStage[object, ProposalBatch]
    | Callable[[ProposerFunc], RegisteredStage[object, ProposalBatch]]
):
    """Decorate an async function as a proposer stage.

    Proposers consume a `ReflectionResult` and emit a `ProposalBatch`.
    `repair_attempts` configures parse-retry budget on malformed output.
    """
    _ = repair_attempts

    def wrap(f: ProposerFunc) -> RegisteredStage[object, ProposalBatch]:
        return _make_registered("proposer", f, stage_id, trust_profile)

    return wrap(func) if func is not None else wrap


@overload
def runner[A, O](func: RunnerFunc[A, O]) -> RegisteredStage[A, O]: ...
@overload
def runner[A, O](
    *,
    id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
) -> Callable[[RunnerFunc[A, O]], RegisteredStage[A, O]]: ...
def runner[A, O](
    func: RunnerFunc[A, O] | None = None,
    *,
    id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
) -> RegisteredStage[A, O] | Callable[[RunnerFunc[A, O]], RegisteredStage[A, O]]:
    """Decorate an async function as a runner stage (a function rollout).

    Runners execute one candidate against one case and return the output the
    rubric will score. Wrap with `Rollout.fn(run)` to use as an environment
    rollout.
    """

    def wrap(f: RunnerFunc[A, O]) -> RegisteredStage[A, O]:
        return _make_registered("runner", f, id, trust_profile)

    return wrap(func) if func is not None else wrap


@overload
def judge[O](func: JudgeFunc[O]) -> RegisteredStage[object, O]: ...
@overload
def judge[O](
    *,
    stage_id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
) -> Callable[[JudgeFunc[O]], RegisteredStage[object, O]]: ...
def judge[O](
    func: JudgeFunc[O] | None = None,
    *,
    stage_id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
) -> RegisteredStage[object, O] | Callable[[JudgeFunc[O]], RegisteredStage[object, O]]:
    """Decorate an async function as a judge stage (pairwise or listwise)."""

    def wrap(f: JudgeFunc[O]) -> RegisteredStage[object, O]:
        return _make_registered("judge", f, stage_id, trust_profile)

    return wrap(func) if func is not None else wrap


def register_stage[O](
    role: StageRole,
    func: Callable[..., Awaitable[O]],
    *,
    id: str | None = None,
    trust_profile: TrustProfile | str = TrustProfile.MANAGED_SANDBOX,
    **role_kwargs: object,
) -> RegisteredStage[object, O]:
    """Function form of the stage decorators; equivalent to `@lv.<role>(...)`.

    Useful for dynamic registration. The decorator forms are sugar over this.
    """
    _ = role_kwargs
    return _make_registered(role, func, id, trust_profile)


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
]
