"""Stage decorators + the served-worker entry point.

`@lv.runner` `@lv.scorer` `@lv.reflector` `@lv.proposer` `@lv.evaluator` are
role-tagging sugar (optional in-process; load-bearing only for served ACP
workers). They do NOT register globally — each returns the function unchanged
(tagged) or, for `scorer`/`evaluator`, support a parameterized form.

`lv.serve(...)` is the out-of-process worker entry point: it registers ONLY
Python-authored stages; engine-mediated built-ins are not served.

Governing spec: `docs/specs/leaven_python.md` — The Python authoring surface /
How Python code reaches the engine.
"""

from __future__ import annotations

from collections.abc import Callable, Sequence
from typing import TYPE_CHECKING, overload

from .trust import TrustProfile

if TYPE_CHECKING:
    from .adapters.evaluator import EvaluatorFn
    from .score import Scorer
    from .stages import ProposeFn, ReflectFn, RolloutFn

__all__ = ["evaluator", "proposer", "reflector", "runner", "scorer", "serve"]


def runner[F: RolloutFn](fn: F) -> F:
    """Tag an async function as the rollout stage. Returns it unchanged.

    Optional sugar (a bare function in the slot works); load-bearing only when
    served out-of-process. The scaffold is a no-op pass-through.
    """
    return fn


@overload
def scorer[F: Scorer](fn: F) -> F: ...
@overload
def scorer[F: Scorer](*, name: str) -> Callable[[F], F]: ...
def scorer(fn=None, *, name=None):
    """Tag an async function as a scorer.

    Default name is the function's `__name__`. `@lv.scorer(name="...")` is a
    PURE OVERRIDE for when the report name should differ from the function name
    — do NOT pass `name=` redundantly when it equals `__name__`. The optimizer
    references the primary score by the scorer OBJECT (`gepa(score=correctness)`);
    the name-string is convenience only. The scaffold is a no-op pass-through.
    """
    if fn is None:
        return lambda f: f
    return fn


def reflector[F: ReflectFn](fn: F) -> F:
    """Tag an async function as the reflect stage. Returns it unchanged."""
    return fn


def proposer[F: ProposeFn](fn: F) -> F:
    """Tag an async function as the propose stage. Returns it unchanged."""
    return fn


def evaluator(
    *,
    id: str,
    trust_profile: TrustProfile | str = TrustProfile.managed_sandbox,
    granularity: str = "per_case",
) -> Callable[[EvaluatorFn], EvaluatorFn]:
    """The advanced evaluator decorator (returns the fn for `Stages.evaluator`).

    `granularity` is a free string at the product layer (wire owns the
    `Granularity` enum). Spec line 783.
    """
    raise NotImplementedError("see leaven_python.md — Advanced authoring")


def serve(
    *,
    rollout: RolloutFn | None = None,
    score: Scorer | Sequence[Scorer] | None = None,
    reflect: ReflectFn | None = None,
    propose: ProposeFn | None = None,
) -> None:
    """Out-of-process worker entry point (external-driver deployment mode only).

    Registers only Python-authored stages; engine-mediated built-ins are
    configured in the plan, not served here. Spec lines 963-980.
    """
    raise NotImplementedError("see leaven_python.md — lv.serve")
