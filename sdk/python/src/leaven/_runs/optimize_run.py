"""Project a `leaven/optimize.run` result into the public `Optimized` handle.

The host returns the optimized projection (best, frontier with parent lineage,
metric/iteration counts, cost totals, durable run reference) and writes a
durable run checkpoint under the client-configured runs root. This module
projects that result document into `Optimized[PromptArtifact]` and resolves the
durable run dir for `lv.runs.open(...)` readback.
"""

from pathlib import Path

from msgspec import UNSET

from .._seam import CandidateEntry, CostDocument, OptimizeRunResultDocument
from .._seam_optimize import OptimizeRunOutcome
from ..artifacts.prompt import PromptArtifact
from ..assessment import Replayability
from ..result import Candidate, Optimized, RunSummary
from ..run_status import RunCostStatus, RunUsageStatus, UnsupportedRunFact

# The optimize.run result document is authoritative for best/frontier/cost, but
# it does not carry per-case assessment rows, and the durable checkpoint readback
# does not yet project them in a compatible shape. The facade reports this so
# `result.assessments()` raises an actionable not-available-yet error instead of
# returning fabricated rows.
_ASSESSMENTS_UNAVAILABLE = UnsupportedRunFact(
    surface="run.inspection",
    dependency="leaven/optimize.run durable checkpoint",
    reason="assessment_evidence_not_loaded",
    detail=(
        "per-case assessments are not yet readable from the optimize.run durable "
        "checkpoint; best/frontier/cost are authoritative from the result document"
    ),
)


def optimized_from_optimize_run(outcome: OptimizeRunOutcome) -> Optimized[PromptArtifact]:
    """Project the host optimize.run result into the public Optimized handle."""
    result = outcome.result
    frontier = [_candidate(entry) for entry in result.frontier]
    if not frontier:
        raise ValueError("optimize.run result carried no frontier candidates")
    best = _candidate(result.best)
    run_dir = str(Path(outcome.runs_root) / outcome.wire_run_id)
    summary = _summary(result, run_dir=run_dir)
    return Optimized[PromptArtifact](
        run_id=result.run.run,
        best=best,
        frontier=frontier,
        summary=summary,
    )


def _candidate(entry: CandidateEntry) -> Candidate[PromptArtifact]:
    return Candidate[PromptArtifact](
        id=entry.candidate,
        artifact=_artifact(entry),
        parent_id=entry.parent,
        summary_score=entry.score,
    )


def _artifact(entry: CandidateEntry) -> PromptArtifact:
    payload = entry.artifact.artifact
    if "template" not in payload:
        raise TypeError(f"optimize.run candidate {entry.candidate!r} artifact has no template")
    template = payload["template"]
    if not isinstance(template, str):
        raise TypeError(
            f"optimize.run candidate {entry.candidate!r} artifact template is not a string"
        )
    return PromptArtifact(template=template, candidate_id=entry.candidate)


def _summary(result: OptimizeRunResultDocument, *, run_dir: str) -> RunSummary:
    cost = result.cost
    total_cost_usd, cost_status = _cost(cost)
    total_lm_tokens, usage_status = _usage(cost)
    return RunSummary(
        run_id=result.run.run,
        started_at="",
        completed_at=None,
        iterations=result.iterations,
        candidates_evaluated=len(result.frontier),
        total_cost_usd=total_cost_usd,
        cost_status=cost_status,
        total_calls=result.metric_calls_used,
        total_lm_tokens=total_lm_tokens,
        usage_status=usage_status,
        unsupported=(_ASSESSMENTS_UNAVAILABLE,),
        run_dir=run_dir,
        replayability=_replayability(),
    )


def _cost(cost: CostDocument) -> tuple[float | None, RunCostStatus]:
    usd_micro = cost.usd_micro
    if usd_micro is UNSET:
        return 0.0, "known"
    return usd_micro / 1_000_000, "known"


def _usage(cost: CostDocument) -> tuple[int | None, RunUsageStatus]:
    tokens = 0
    if cost.input_tokens is not UNSET:
        tokens += cost.input_tokens
    if cost.output_tokens is not UNSET:
        tokens += cost.output_tokens
    return tokens, "known"


def _replayability() -> Replayability:
    return "boundary_managed"


__all__ = ["optimized_from_optimize_run"]
