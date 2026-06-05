"""EvaluationJob — what `@lv.evaluator` stages receive.

Evaluators iterate over the candidate(s) and case(s) the job names, do the
evaluation work, and submit assessments via `cx.assessments.submit(...)`.
The job's granularity (`per_case` / `pairwise` / `listwise`) shapes which
iteration methods are available.
"""

from collections.abc import Iterable
from typing import Literal

from pydantic import BaseModel, ConfigDict, Field

EvaluationKind = Literal["independent", "pairwise", "listwise"]
Granularity = Literal["aggregate", "per_case"]
Purpose = Literal["train", "validation", "test", "diagnostic", "custom"]


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

    kind: EvaluationKind
    """Assessment shape requested by the engine."""

    granularity: Granularity
    """Whether the engine requested aggregate or per-case assessments."""

    purpose: Purpose = "validation"
    """Why this evaluation is being run."""

    evaluator_id: str
    """The evaluator id registered for this job."""

    base_revision: str | None = None
    deadline_seconds: float | None = None
    items: list[EvaluationItem] = Field(default_factory=list)
    """Evaluation work items assigned to this evaluator invocation."""

    def independent_cases(self) -> Iterable[EvaluationItem]:
        """Yield items for an independent per-candidate job."""
        self._require_kind("independent")
        for item in self.items:
            _require_candidate_id(item)
            yield item

    def pairwise_cases(self) -> Iterable[EvaluationItem]:
        """Yield items for a `pairwise` job. Raises if granularity differs."""
        self._require_kind("pairwise")
        for item in self.items:
            _require_candidate_ids(item, expected_count=2)
            yield item

    def listwise_cases(self) -> Iterable[EvaluationItem]:
        """Yield items for a `listwise` job. Raises if granularity differs."""
        self._require_kind("listwise")
        for item in self.items:
            _require_candidate_ids(item, expected_count=None)
            yield item

    def _require_kind(self, expected: EvaluationKind) -> None:
        if self.kind != expected:
            raise TypeError(f"{expected} iterator cannot read {self.kind} evaluation jobs")


def _require_candidate_id(item: EvaluationItem) -> None:
    if item.candidate_id is None or item.candidate_ids is not None:
        raise ValueError("independent evaluation items require candidate_id only")


def _require_candidate_ids(item: EvaluationItem, *, expected_count: int | None) -> None:
    if item.candidate_ids is None or item.candidate_id is not None:
        raise ValueError("grouped evaluation items require candidate_ids only")
    if expected_count is not None and len(item.candidate_ids) != expected_count:
        raise ValueError(f"grouped evaluation items require {expected_count} candidate_ids")
    if expected_count is None and len(item.candidate_ids) < 2:
        raise ValueError("listwise evaluation items require at least two candidate_ids")


__all__ = ["EvaluationItem", "EvaluationJob", "EvaluationKind", "Granularity", "Purpose"]
