"""Private report records for durable-seam optimize mechanics."""

from dataclasses import dataclass, field

from .._receipts import CallReceipt, WriteReceipt
from ..assessment import RewardAssessment
from ..json_value import JsonObject, JsonValue
from ..run_status import UnsupportedRunFact
from ..score import Score
from .receipts import EffectBlobContent, EffectCostTotals


@dataclass(frozen=True)
class PlannedOptimizeCase:
    """One case selected for the current optimize mechanics run."""

    case_id: str
    input: JsonObject
    target: JsonObject | None
    metadata: JsonObject
    split: str | None


@dataclass(frozen=True)
class SeamStageAssessment:
    """One runner-stage result observed through the durable seam."""

    case_id: str
    case_input: JsonObject
    case_target: JsonObject | None
    case_metadata: JsonObject
    case_split: str | None
    output: JsonValue | None
    score: Score
    rewards: list[RewardAssessment]
    receipt: str | None = None
    effect_receipts: list[CallReceipt] = field(default_factory=list)
    effect_blob_contents: list[EffectBlobContent] = field(default_factory=list)
    effect_costs: EffectCostTotals = field(default_factory=lambda: EffectCostTotals(0.0, 0))


@dataclass(frozen=True)
class SeamOptimizeReport:
    """Current durable-seam optimize mechanics report."""

    seed_score: float
    best_score: float
    assessments: list[SeamStageAssessment]
    total_cost_usd: float = 0.0
    total_lm_tokens: int = 0
    proposal_receipts: list[WriteReceipt] = field(default_factory=list)
    effect_receipts: list[CallReceipt] = field(default_factory=list)
    unsupported: tuple[UnsupportedRunFact, ...] = ()


@dataclass(frozen=True)
class ProposerStageReport:
    """Effect and write receipts returned by the configured proposer stage."""

    proposal_receipts: list[WriteReceipt] = field(default_factory=list)
    effect_receipts: list[CallReceipt] = field(default_factory=list)


__all__ = [
    "PlannedOptimizeCase",
    "ProposerStageReport",
    "SeamOptimizeReport",
    "SeamStageAssessment",
]
