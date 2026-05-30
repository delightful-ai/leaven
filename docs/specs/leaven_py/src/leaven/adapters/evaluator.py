"""Evaluator support types — `lv.adapters.evaluator`.

Support types for `@lv.evaluator`. `EvaluationJob` / `AssessmentWrite` are
imported from `lv.wire`.

Governing spec: `docs/specs/leaven_python.md` — Advanced authoring.
"""

from __future__ import annotations

from collections.abc import Awaitable, Callable

from pydantic import BaseModel, ConfigDict

from ..trust import TrustProfile
from ..wire.evaluation_job import EvaluationJob
from .contexts import EvalContext

__all__ = ["Evaluator", "EvaluatorFn"]


type EvaluatorFn = Callable[[EvaluationJob, EvalContext], Awaitable[None]]
"""The advanced evaluator callable shape."""


class Evaluator(BaseModel):
    """A registered evaluator: id + trust + granularity + the bound function."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    id: str
    trust_profile: TrustProfile | str
    granularity: str
    fn: EvaluatorFn
