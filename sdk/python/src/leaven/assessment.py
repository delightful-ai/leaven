"""Assessment types — what evaluators submit, what results carry back.

`AssessmentWrite` is the wire-bound submission shape (multiple shape
classmethods for independent/pairwise/listwise). `Assessment` is the
result-side typed handle the user reads from `Optimized.test_assessments()`.
"""

from typing import Literal

from pydantic import BaseModel, ConfigDict, Field

from ._receipts import CallReceipt, QueryReceipt, WriteReceipt
from .case import Case
from .evidence import EvidenceEnvelope
from .output_record import OutputRecord
from .score import Score

Replayability = Literal[
    "pure_read",
    "fully_managed",
    "boundary_managed",
    "has_declared_external_effects",
    "has_untracked_external_effects",
]


class AssessmentWrite(BaseModel):
    """A typed assessment submission. Built via classmethods per granularity."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: Literal["independent", "pairwise", "listwise"]
    candidate: str | None = None
    """Single candidate id for independent shape."""
    candidates: list[str] | None = None
    """Multiple candidate ids for pairwise/listwise."""
    case: str
    score: Score | None = None
    """Single score for independent shape."""
    ranking: list[str] | None = None
    """Ranked candidate ids for listwise."""
    preference: str | None = None
    """Preferred candidate id for pairwise."""
    evidence: EvidenceEnvelope
    read_receipts: list[QueryReceipt] = Field(default_factory=list)
    effect_receipts: list[CallReceipt] = Field(default_factory=list)
    replayability: Replayability = "boundary_managed"

    @classmethod
    def independent_case(
        cls,
        *,
        candidate: str,
        case: str,
        score: Score,
        evidence: EvidenceEnvelope,
        read_receipts: list[QueryReceipt] | None = None,
        effect_receipts: list[CallReceipt] | None = None,
        replayability: Replayability = "boundary_managed",
    ) -> "AssessmentWrite":
        """Single-candidate-per-case assessment (the most common shape)."""
        return cls(
            kind="independent",
            candidate=candidate,
            case=case,
            score=score,
            evidence=evidence,
            read_receipts=list(read_receipts or []),
            effect_receipts=list(effect_receipts or []),
            replayability=replayability,
        )

    @classmethod
    def pairwise(
        cls,
        *,
        candidates: list[str],
        case: str,
        preference: str,
        score: Score,
        evidence: EvidenceEnvelope,
        read_receipts: list[QueryReceipt] | None = None,
        effect_receipts: list[CallReceipt] | None = None,
        replayability: Replayability = "boundary_managed",
    ) -> "AssessmentWrite":
        """Pairwise preference assessment (two candidates, one preferred)."""
        if len(candidates) != 2:
            raise ValueError("pairwise assessments require exactly two candidates")
        if preference not in candidates:
            raise ValueError("pairwise preference must be one of the candidates")
        return cls(
            kind="pairwise",
            candidates=list(candidates),
            case=case,
            preference=preference,
            score=score,
            evidence=evidence,
            read_receipts=list(read_receipts or []),
            effect_receipts=list(effect_receipts or []),
            replayability=replayability,
        )

    @classmethod
    def listwise(
        cls,
        *,
        candidates: list[str],
        case: str,
        ranking: list[str],
        score: Score,
        evidence: EvidenceEnvelope,
        read_receipts: list[QueryReceipt] | None = None,
        effect_receipts: list[CallReceipt] | None = None,
        replayability: Replayability = "boundary_managed",
    ) -> "AssessmentWrite":
        """Listwise ranking assessment over N candidates."""
        if len(candidates) < 2:
            raise ValueError("listwise assessments require at least two candidates")
        if set(ranking) != set(candidates) or len(ranking) != len(candidates):
            raise ValueError("listwise ranking must contain the same candidates exactly once")
        return cls(
            kind="listwise",
            candidates=list(candidates),
            case=case,
            ranking=list(ranking),
            score=score,
            evidence=evidence,
            read_receipts=list(read_receipts or []),
            effect_receipts=list(effect_receipts or []),
            replayability=replayability,
        )


class RewardAssessment(BaseModel):
    """One reward-vector dimension observed for a candidate/case assessment."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    id: str
    value: float
    weight: float
    feedback: str = ""
    output: OutputRecord | None = None


class Assessment(BaseModel):
    """Result-side view of one assessment (read via `Optimized.test_assessments()`)."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    case: Case
    candidate_id: str
    score: Score
    evidence: EvidenceEnvelope
    receipt: WriteReceipt
    read_receipts: list[QueryReceipt] = Field(default_factory=list)
    effect_receipts: list[CallReceipt] = Field(default_factory=list)
    replayability: Replayability
    rewards: list[RewardAssessment] = Field(default_factory=list)


__all__ = ["Assessment", "AssessmentWrite", "Replayability", "RewardAssessment"]
