"""Private report records for durable-seam optimize mechanics."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from ..run_status import UnsupportedRunFact


@dataclass(frozen=True)
class SeamStageAssessment:
    """One runner-stage result observed through the durable seam."""

    case_id: str
    output: Any
    score: float
    receipt: str | None = None


@dataclass(frozen=True)
class SeamOptimizeReport:
    """Current durable-seam optimize mechanics report."""

    seed_score: float
    best_score: float
    assessments: list[SeamStageAssessment]
    unsupported: tuple[UnsupportedRunFact, ...] = ()


__all__ = ["SeamOptimizeReport", "SeamStageAssessment"]
