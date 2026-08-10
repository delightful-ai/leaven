"""Build public run handles from Rust-owned checkpoint readback."""

from pathlib import Path

from pydantic import BaseModel, ConfigDict, Field, ValidationError

from .._receipts import WriteReceipt
from ..artifacts.prompt import PromptArtifact
from ..assessment import Assessment, Replayability
from ..json_value import JsonValue
from ..result import Candidate, Optimized, RunSummary
from ..run_inspection import RustRunReadback
from ..run_status import RunCostStatus, UnsupportedRunFact
from .rust_evidence import rust_assessment_rows
from .rust_export import load_rust_evidence_readback, load_rust_run_readback


class RustPromptArtifactReadback(BaseModel):
    """Prompt artifact payload read from a Rust-owned candidate row."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    template: str
    examples: list[str] = Field(default_factory=list)
    candidate_id: str | None = None

    def to_public(self) -> PromptArtifact:
        """Project the Rust readback payload into the public SDK artifact."""
        return PromptArtifact(
            template=self.template,
            examples=list(self.examples),
            candidate_id=self.candidate_id,
        )


def open_rust_optimized(path: str | Path) -> Optimized[PromptArtifact] | None:
    """Open a completed run from Rust-owned checkpoint state when present."""
    readback = load_rust_run_readback(path)
    if readback is None:
        return None
    evidence = [
        load_rust_evidence_readback(path, assessment.evidence)
        for assessment in readback.graph.assessments
    ]
    return optimized_from_rust_readback(
        readback,
        run_dir=str(_run_dir(path)),
        assessment_rows=rust_assessment_rows(readback, evidence),
    )


def optimized_from_rust_readback(
    readback: RustRunReadback,
    *,
    run_dir: str | None,
    assessment_rows: list[Assessment] | None = None,
) -> Optimized[PromptArtifact]:
    """Project Rust-owned graph readback into the public Optimized handle."""
    purpose_by_assessment = {
        assessment.id: assessment.purpose
        for assessment in readback.graph.assessments
        if assessment.purpose is not None
    }
    frontier = [
        Candidate[PromptArtifact](
            id=candidate.id,
            artifact=_artifact_from_readback(candidate.artifact),
            parent_id=candidate.parent_id,
            summary_score=_summary_score(
                candidate.id,
                assessment_rows,
                purpose_by_assessment=purpose_by_assessment,
            ),
        )
        for candidate in readback.graph.candidates
    ]
    if not frontier:
        raise ValueError("Rust run readback has no candidates")
    best_id = readback.graph.best_candidate_id
    if best_id is None:
        raise ValueError("Rust run readback has no completed-run best candidate")
    best = _candidate_by_id(frontier, best_id)
    unsupported = _unsupported(readback, assessment_rows)
    return Optimized[PromptArtifact](
        run_id=readback.run_id,
        best=best,
        frontier=frontier,
        summary=RunSummary(
            run_id=readback.run_id,
            started_at="",
            completed_at=None,
            iterations=readback.graph.event_count,
            candidates_evaluated=readback.graph.assessment_count,
            total_cost_usd=_total_cost_usd(readback),
            cost_status=_cost_status(readback),
            total_calls=readback.graph.event_count,
            total_lm_tokens=readback.cost.lm_tokens,
            usage_status="known",
            unsupported=unsupported,
            run_dir=run_dir,
            replayability=_replayability(),
        ),
        assessment_rows=list(assessment_rows or []),
        proposal_receipts=[
            WriteReceipt(
                receipt_id=batch.id,
                proposal_ids=list(batch.proposal_ids),
            )
            for batch in readback.graph.proposal_batches
        ],
    )


def _total_cost_usd(readback: RustRunReadback) -> float | None:
    if readback.cost.lm_calls == 0 and readback.cost.lm_tokens == 0:
        return 0.0
    return None


def _cost_status(readback: RustRunReadback) -> RunCostStatus:
    if _total_cost_usd(readback) == 0.0:
        return "known"
    return "unsupported_dependency"


def _candidate_by_id(
    candidates: list[Candidate[PromptArtifact]],
    candidate_id: str,
) -> Candidate[PromptArtifact]:
    for candidate in candidates:
        if candidate.id == candidate_id:
            return candidate
    raise KeyError(f"Rust run readback best candidate {candidate_id!r} is missing")


def _artifact_from_readback(value: JsonValue) -> PromptArtifact:
    try:
        return RustPromptArtifactReadback.model_validate(value).to_public()
    except ValidationError as error:
        raise TypeError("Rust run readback artifact is not a PromptArtifact payload") from error


_VALIDATION_PURPOSES = frozenset({"Validation", "Selection"})
_VALIDATION_SPLITS = frozenset({"validation", "val"})


def _summary_score(
    candidate_id: str,
    assessment_rows: list[Assessment] | None,
    *,
    purpose_by_assessment: dict[str, str] | None = None,
) -> float | None:
    """Aggregate held-out validation scores for one candidate.

    GEPA records train minibatch screening under Search/SeedBaseline purposes
    alongside Validation rows. Averaging every assessment would dilute the
    public validation-set ``summary_score`` on ``lv.runs.open``.
    """
    rows = [
        assessment
        for assessment in assessment_rows or []
        if assessment.candidate_id == candidate_id
    ]
    if not rows:
        return None
    purposes = purpose_by_assessment or {}
    validation_rows = [
        assessment
        for assessment in rows
        if _is_validation_assessment(assessment, purposes.get(assessment.receipt.receipt_id))
    ]
    if not validation_rows:
        return None
    return sum(assessment.score.value for assessment in validation_rows) / len(validation_rows)


def _is_validation_assessment(assessment: Assessment, purpose: str | None) -> bool:
    if purpose in _VALIDATION_PURPOSES:
        return True
    if purpose is not None:
        return False
    split = assessment.case.split
    return split is not None and split in _VALIDATION_SPLITS


def _replayability() -> Replayability:
    return "boundary_managed"


def _unsupported(
    readback: RustRunReadback,
    assessment_rows: list[Assessment] | None,
) -> tuple[UnsupportedRunFact, ...]:
    facts: list[UnsupportedRunFact] = []
    if _cost_status(readback) == "unsupported_dependency":
        facts.append(
            UnsupportedRunFact(
                surface="run.cost",
                dependency="Rust checkpoint inspection",
                reason="provider_cost_not_reported",
                detail="Rust run-open readback does not yet export provider dollar totals.",
            )
        )
    if assessment_rows is None and readback.graph.assessment_count > 0:
        facts.append(
            UnsupportedRunFact(
                surface="run.inspection",
                dependency="Rust checkpoint inspection",
                reason="assessment_evidence_not_loaded",
                detail="Rust run-open readback has assessment refs but no evidence rows.",
            )
        )
    return tuple(facts)


def _run_dir(path: str | Path) -> Path:
    candidate = Path(path)
    if candidate.is_file():
        return candidate.parent
    return candidate


__all__ = ["RustPromptArtifactReadback", "open_rust_optimized", "optimized_from_rust_readback"]
