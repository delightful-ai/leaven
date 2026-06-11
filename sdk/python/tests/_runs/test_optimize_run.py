"""Tests for projecting a `leaven/optimize.run` result into `Optimized`."""

import pytest

import leaven as lv
from leaven._runs import optimized_from_optimize_run
from leaven._seam import (
    ArtifactRecord,
    CandidateEntry,
    CostDocument,
    OptimizeRunResultDocument,
    RunReference,
)
from leaven._seam_optimize import OptimizeRunOutcome


def _artifact(template: str) -> ArtifactRecord:
    return ArtifactRecord(
        artifact_type="prompt",
        artifact_schema="fp_schema_sha256_prompt",
        artifact={"template": template},
    )


def _outcome() -> OptimizeRunOutcome:
    seed = CandidateEntry(
        candidate="cand_seed", parent=None, score=0.0, artifact=_artifact("seed")
    )
    child = CandidateEntry(
        candidate="cand_child",
        parent="cand_seed",
        score=1.0,
        artifact=_artifact("child {question}"),
    )
    result = OptimizeRunResultDocument(
        schema_version="leaven.optimize_run.v1",
        message="optimize_run_result",
        best=child,
        frontier=[seed, child],
        iterations=1,
        metric_calls_used=4,
        cost=CostDocument(usd_micro=250_000, input_tokens=12, output_tokens=8, lm_calls=1),
        run=RunReference(run="run_proj_test", revision="rev_proj_test"),
        applied_proposals=["wrec_optimize_apply_0"],
    )
    return OptimizeRunOutcome(
        result=result,
        runs_root="/tmp/runs",
        run_id="proj_test",
        wire_run_id="run_proj_test",
    )


def test_projection_carries_best_frontier_lineage_and_cost() -> None:
    """Example: the result document projects into a typed Optimized handle."""
    optimized = optimized_from_optimize_run(_outcome())

    assert optimized.best.id == "cand_child"
    assert optimized.best.summary_score == 1.0
    assert isinstance(optimized.best.artifact, lv.PromptArtifact)
    assert optimized.best.artifact.template == "child {question}"
    seed = next(c for c in optimized.frontier if c.parent_id is None)
    assert seed.id == "cand_seed"
    assert seed.summary_score == 0.0
    assert optimized.best.parent_id == "cand_seed"
    assert optimized.summary.run_dir == "/tmp/runs/run_proj_test"
    assert optimized.summary.total_cost_usd == 0.25
    assert optimized.summary.total_lm_tokens == 20
    assert optimized.summary.iterations == 1
    assert optimized.summary.total_calls == 4


def test_assessments_raise_until_durable_readback_lands() -> None:
    """Law: per-case assessments are not fabricated; the accessor raises."""
    optimized = optimized_from_optimize_run(_outcome())
    assert "run.inspection" in [fact.surface for fact in optimized.summary.unsupported]
    with pytest.raises(lv.AssessmentsUnavailableError, match="not available"):
        list(optimized.assessments())
    with pytest.raises(lv.AssessmentsUnavailableError):
        optimized.assessment("case_x")
