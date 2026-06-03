"""Private report records for durable-seam optimize mechanics."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from ..assessment import RewardAssessment
from ..run_status import UnsupportedRunFact
from ..score import Score
from .receipts import EffectCostTotals


@dataclass(frozen=True)
class SeamStageAssessment:
    """One runner-stage result observed through the durable seam."""

    case_id: str
    case_input: dict[str, Any]
    case_target: dict[str, Any] | None
    case_metadata: dict[str, Any]
    case_split: str | None
    output: Any
    score: Score
    rewards: list[RewardAssessment]
    receipt: str | None = None
    effect_receipts: list[str] = field(default_factory=list)
    effect_costs: EffectCostTotals = field(default_factory=lambda: EffectCostTotals(0.0, 0))


@dataclass(frozen=True)
class SeamOptimizeReport:
    """Current durable-seam optimize mechanics report."""

    seed_score: float
    best_score: float
    assessments: list[SeamStageAssessment]
    total_cost_usd: float = 0.0
    total_lm_tokens: int = 0
    proposal_receipts: list[str] = field(default_factory=list)
    unsupported: tuple[UnsupportedRunFact, ...] = ()


__all__ = ["SeamOptimizeReport", "SeamStageAssessment"]
