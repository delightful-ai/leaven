"""Typed completed-run inspection projections for `lv.runs.inspect(...)`."""

from __future__ import annotations

from typing import Any, Literal

from pydantic import BaseModel, ConfigDict, Field

from .assessment import Assessment
from .result import Optimized
from .run_status import RunCostStatus, RunUsageStatus, UnsupportedRunFact

ReceiptKind = Literal["query", "call", "write"]


class ReceiptSummary(BaseModel):
    """One opaque receipt visible from a completed run."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: ReceiptKind
    receipt_id: str
    source: str
    """Stable source label such as `assessment:<case>` or `proposal_batch`."""


class EvidenceSummary(BaseModel):
    """Optimizer-visible evidence projection for one assessment."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    case_id: str
    candidate_id: str
    data_classes: list[str] = Field(default_factory=list)
    payload: dict[str, Any] = Field(default_factory=dict)
    target_derived: bool


class RunInspection(BaseModel):
    """Flattened, read-only facts users need when auditing a completed run."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    run_id: str
    run_dir: str | None
    best_candidate_id: str
    best_lineage: list[str]
    receipts: list[ReceiptSummary] = Field(default_factory=list)
    evidence: list[EvidenceSummary] = Field(default_factory=list)
    total_cost_usd: float | None
    cost_status: RunCostStatus
    total_lm_tokens: int | None
    usage_status: RunUsageStatus
    unsupported: tuple[UnsupportedRunFact, ...] = ()

    def receipt_ids(self, *, kind: ReceiptKind | None = None) -> list[str]:
        """Return receipt ids, optionally filtered by receipt kind."""
        return [
            receipt.receipt_id for receipt in self.receipts if kind is None or receipt.kind == kind
        ]


def inspect_optimized(result: Optimized[Any]) -> RunInspection:
    """Build a flattened inspection projection from an optimized result."""
    return RunInspection(
        run_id=result.run_id,
        run_dir=result.summary.run_dir,
        best_candidate_id=result.best.id,
        best_lineage=[candidate.id for candidate in result.lineage(result.best.id)],
        receipts=_receipts(result),
        evidence=[_evidence_summary(assessment) for assessment in result.assessment_rows],
        total_cost_usd=result.summary.total_cost_usd,
        cost_status=result.summary.cost_status,
        total_lm_tokens=result.summary.total_lm_tokens,
        usage_status=result.summary.usage_status,
        unsupported=result.summary.unsupported,
    )


def _receipts(result: Optimized[Any]) -> list[ReceiptSummary]:
    receipts: list[ReceiptSummary] = []
    for assessment in result.assessment_rows:
        source = f"assessment:{assessment.case.id}"
        receipts.append(
            ReceiptSummary(kind="write", receipt_id=assessment.receipt.receipt_id, source=source)
        )
        receipts.extend(
            ReceiptSummary(kind="query", receipt_id=receipt.receipt_id, source=source)
            for receipt in assessment.read_receipts
        )
        receipts.extend(
            ReceiptSummary(kind="call", receipt_id=receipt.receipt_id, source=source)
            for receipt in assessment.effect_receipts
        )
    receipts.extend(
        ReceiptSummary(
            kind="write",
            receipt_id=receipt.receipt_id,
            source="proposal_batch",
        )
        for receipt in result.proposal_receipts
    )
    return receipts


def _evidence_summary(assessment: Assessment) -> EvidenceSummary:
    public = assessment.evidence.public
    return EvidenceSummary(
        case_id=assessment.case.id,
        candidate_id=assessment.candidate_id,
        data_classes=list(public.data_classes) if public is not None else [],
        payload=dict(public.payload) if public is not None else {},
        target_derived=assessment.evidence.target_derived,
    )


__all__ = [
    "EvidenceSummary",
    "ReceiptKind",
    "ReceiptSummary",
    "RunInspection",
    "inspect_optimized",
]
