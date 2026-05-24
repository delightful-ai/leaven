"""EvaluationJob — what `@lv.evaluator` stages receive.

Evaluators iterate over the candidate(s) and case(s) the job names, do the
evaluation work, and submit assessments via `cx.assessments.submit(...)`.
The job's granularity (`per_case` / `pairwise` / `listwise`) shapes which
iteration methods are available.
"""

from __future__ import annotations

from collections.abc import Iterable
from typing import Literal

from pydantic import BaseModel, ConfigDict

Granularity = Literal["per_case", "pairwise", "listwise"]


class EvaluationItem(BaseModel):
    """One unit of evaluation work — a (candidate, case) pair or grouping."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    candidate_id: str | None = None
    """Set for `per_case` jobs."""
    candidate_ids: list[str] | None = None
    """Set for `pairwise` / `listwise` jobs."""
    case_id: str


class EvaluationJob(BaseModel):
    """An evaluator's invocation. Carries everything needed to iterate the work."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    evaluation_request_id: str
    """Job id; pass back to `cx.assessments.submit(evaluation_request_id, ...)`."""

    granularity: Granularity
    evaluator_id: str
    """The evaluator id registered for this job."""

    base_revision: str | None = None
    deadline_seconds: float | None = None

    def independent_cases(self) -> Iterable[EvaluationItem]:
        """Yield items for a `per_case` job. Raises if granularity differs."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    def pairwise_cases(self) -> Iterable[EvaluationItem]:
        """Yield items for a `pairwise` job. Raises if granularity differs."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    def listwise_cases(self) -> Iterable[EvaluationItem]:
        """Yield items for a `listwise` job. Raises if granularity differs."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["EvaluationItem", "EvaluationJob", "Granularity"]
