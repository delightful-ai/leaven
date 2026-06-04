import pytest

from leaven._runs.codec import RUN_RESULT_SCHEMA, decode_optimized, encode_optimized
from leaven.artifacts.prompt import PromptArtifact
from leaven.result import Candidate, Optimized, RunSummary


def test_encode_decode_optimized_reconstructs_prompt_artifacts() -> None:
    result = Optimized(
        run_id="run_codec",
        best=Candidate(
            id="cand_best",
            artifact=PromptArtifact(template="Hello {name}", candidate_id="cand_best"),
            summary_score=1.0,
        ),
        frontier=[
            Candidate(
                id="cand_best",
                artifact=PromptArtifact(template="Hello {name}", candidate_id="cand_best"),
                summary_score=1.0,
            )
        ],
        summary=RunSummary(
            run_id="run_codec",
            started_at="2026-06-04T00:00:00Z",
            completed_at="2026-06-04T00:00:01Z",
            iterations=1,
            candidates_evaluated=1,
            total_cost_usd=0.0,
            total_calls=0,
            total_lm_tokens=0,
            replayability="pure_read",
        ),
    )

    reopened = decode_optimized(encode_optimized(result))

    assert reopened.run_id == "run_codec"
    assert reopened.best.artifact == PromptArtifact(
        template="Hello {name}", candidate_id="cand_best"
    )
    assert reopened.frontier[0].artifact == reopened.best.artifact


def test_decode_optimized_rejects_unknown_schema() -> None:
    with pytest.raises(ValueError, match="unsupported run result schema"):
        decode_optimized(
            {
                "schema": "leaven.python.optimized.v0",
                "artifact_kind": "prompt",
                "optimized": {},
            }
        )


def test_decode_optimized_rejects_unknown_artifact_kind() -> None:
    with pytest.raises(ValueError, match="unsupported persisted artifact kind"):
        decode_optimized(
            {
                "schema": RUN_RESULT_SCHEMA,
                "artifact_kind": "directory",
                "optimized": {},
            }
        )
