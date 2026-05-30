"""Wire records: `AssessmentWrite` / `Replayability`.

The typed assessment envelope an advanced `@lv.evaluator` submits via
`cx.submit(...)`. Governing spec: `docs/specs/leaven_python.md` — Advanced
authoring. Schema owned by `docs/specs/public-seam-v1/schemas/`.
"""

from __future__ import annotations

from enum import StrEnum

from pydantic import BaseModel, ConfigDict

__all__ = ["AssessmentWrite", "Replayability"]


class Replayability(StrEnum):
    """Per-assessment replayability honesty flag."""

    replayable = "replayable"
    non_replayable = "non_replayable"


class AssessmentWrite(BaseModel):
    """A per-case assessment write the engine applies via `RunContext`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    case_id: str
    value: float
    feedback: str = ""
    replayability: Replayability = Replayability.replayable

    @classmethod
    def independent_case(cls, *, case_id: str, value: float, feedback: str = "") -> AssessmentWrite:
        """Build a per-case assessment write (spec line 792)."""
        raise NotImplementedError(
            "see leaven_python.md — Advanced authoring / AssessmentWrite.independent_case"
        )
