"""Tests for the flattened `RunInspection` projection."""

from leaven._runs import optimized_from_optimize_run
from leaven._seam import (
    ArtifactRecord,
    CandidateEntry,
    CostDocument,
    OptimizeRunResultDocument,
    RunReference,
)
from leaven._seam_optimize import OptimizeRunOutcome
from leaven.run_inspection import inspect_optimized


def _artifact(template: str) -> ArtifactRecord:
    return ArtifactRecord(
        artifact_type="prompt",
        artifact_schema="fp_schema_sha256_prompt",
        artifact={"template": template},
    )


def _optimized():  # noqa: ANN202 -- Optimized[PromptArtifact], local fixture
    seed = CandidateEntry(candidate="cand_seed", parent=None, score=0.0, artifact=_artifact("seed"))
    child = CandidateEntry(
        candidate="cand_child", parent="cand_seed", score=1.0, artifact=_artifact("child")
    )
    result = OptimizeRunResultDocument(
        schema_version="leaven.optimize_run.v1",
        message="optimize_run_result",
        best=child,
        frontier=[seed, child],
        iterations=1,
        metric_calls_used=4,
        cost=CostDocument(usd_micro=250_000, input_tokens=12, output_tokens=8, lm_calls=1),
        run=RunReference(run="run_inspect_test", revision="rev_inspect_test"),
    )
    outcome = OptimizeRunOutcome(
        result=result,
        runs_root="/tmp/runs",
        run_id="inspect_test",
        wire_run_id="run_inspect_test",
    )
    return optimized_from_optimize_run(outcome)


def test_inspect_optimized_flattens_best_lineage_and_cost() -> None:
    """Example: inspection flattens the best lineage and run-level cost facts."""
    optimized = _optimized()

    inspection = inspect_optimized(optimized)

    assert inspection.run_id == "run_inspect_test"
    assert inspection.run_dir == "/tmp/runs/run_inspect_test"
    assert inspection.best_candidate_id == "cand_child"
    assert inspection.best_lineage == ["cand_child", "cand_seed"]
    assert inspection.total_cost_usd == 0.25
    assert inspection.total_lm_tokens == 20
    # Per-case assessments are not yet readable for optimize.run, so the
    # inspection has no per-case receipts/evidence and names the gap.
    assert inspection.receipts == []
    assert inspection.evidence == []
    assert "run.inspection" in [fact.surface for fact in inspection.unsupported]
