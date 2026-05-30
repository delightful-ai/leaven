"""Stages — the four-slot composition: `{rollout, score, reflect, propose}`.

`Stages` has EXACTLY four slots. There is no `evaluate`, `improve`, `judge`,
`layout`, or `sampler` slot. A `leaven-py` test locks `Stages.__init__` to
exactly these four params (keyword-only).

`rollout` + `score` are required; `reflect`/`propose` are optional (GEPA installs
Codex-backed defaults). `Stages.evaluator(...)` is the advanced alternate
constructor that replaces `rollout`+`score`.

This module also owns the stage-fn type aliases for annotation reuse.

Governing spec: `docs/specs/leaven_python.md` — Stages / Advanced authoring.
"""

from __future__ import annotations

from collections.abc import Awaitable, Callable, Sequence
from typing import TYPE_CHECKING, Any

from .case import Case
from .context import Context
from .reflect import Reflect
from .rollout import Rollout, RolloutResult
from .score import Scorer

if TYPE_CHECKING:
    from .adapters.evaluator import EvaluatorFn
    from .adapters.reflective import ReflectiveBatch
    from .artifacts.base import Artifact
    from .propose import Proposal, Propose

__all__ = ["EvaluatorFn", "ProposeFn", "ReflectFn", "RolloutFn", "Stages"]


type RolloutFn = Callable[[Any, Case, Context], Awaitable[Any | RolloutResult[Any]]]
"""A custom rollout function: `(artifact, case, cx) -> Out | RolloutResult`."""

type ReflectFn = Callable[[ReflectiveBatch, Context], Awaitable[str | None]]
"""A custom reflect function: `(batch, cx) -> str | None` (free-form diagnosis)."""

type ProposeFn = Callable[[Artifact, ReflectiveBatch, str | None, Context], Awaitable[Proposal]]
"""A custom propose function: `(parent, batch, reflection, cx) -> Proposal`."""

type EvaluatorFn = Callable[..., Awaitable[None]]
"""An advanced evaluator function: `(EvaluationJob, EvalContext) -> None`.

The precise signature is defined in `lv.adapters.evaluator`; this alias mirrors
it for `Stages.evaluator(...)` annotation reuse.
"""


class Stages:
    """The four-slot evolution composition.

    EXACTLY `{rollout, score, reflect, propose}`. Each slot takes a plain async
    function OR a declarative built-in; `score` takes one scorer or a list.
    """

    def __init__(
        self,
        *,
        rollout: RolloutFn | Rollout,
        score: Scorer | Sequence[Scorer],
        reflect: ReflectFn | Reflect | None = None,
        propose: ProposeFn | Propose | None = None,
    ) -> None:
        raise NotImplementedError("see leaven_python.md — Stages")

    @classmethod
    def evaluator(
        cls,
        evaluate: EvaluatorFn,
        *,
        reflect: ReflectFn | Reflect | None = None,
        propose: ProposeFn | Propose | None = None,
    ) -> Stages:
        """Advanced alternate constructor: `@lv.evaluator` replaces the
        `rollout`+`score` slots. Spec line 798."""
        raise NotImplementedError("see leaven_python.md — Stages.evaluator")
