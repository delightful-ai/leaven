"""Wire records: `EvaluationJob` / `EvaluationItem` / `Granularity` / `Purpose`.

Governing spec: `docs/specs/leaven_python.md` — Advanced authoring
(`@lv.evaluator`). Schema owned by `docs/specs/public-seam-v1/schemas/`.
"""

from __future__ import annotations

from collections.abc import Sequence
from enum import StrEnum

from pydantic import BaseModel, ConfigDict

__all__ = ["EvaluationItem", "EvaluationJob", "Granularity", "Purpose"]


class Granularity(StrEnum):
    """Assessment granularity for an evaluation job."""

    per_case = "per_case"
    aggregate = "aggregate"


class Purpose(StrEnum):
    """Why an evaluation job runs (stage purpose)."""

    rollout = "rollout"
    score = "score"
    reflect = "reflect"
    propose = "propose"
    judge = "judge"


class EvaluationItem(BaseModel):
    """One case+candidate pair within an evaluation job."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    case_id: str
    candidate: object


class EvaluationJob(BaseModel):
    """A batch of evaluation items handed to an advanced `@lv.evaluator`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    id: str
    items: Sequence[EvaluationItem]
    granularity: Granularity
    purpose: Purpose
