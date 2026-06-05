import pytest

import leaven as lv
from leaven._receipts import WriteReceipt
from leaven.assessment import Assessment, Replayability
from leaven.evidence import EvidenceEnvelope


def _result_with_assessments(*assessments: Assessment) -> lv.Optimized[lv.PromptArtifact]:
    best = lv.Candidate(
        id="cand_1",
        artifact=lv.PromptArtifact(template="Answer {question}"),
        summary_score=0.875,
    )
    return lv.Optimized(
        run_id="run_1",
        best=best,
        frontier=[best],
        summary=lv.RunSummary(
            run_id="run_1",
            started_at="2026-06-05T12:00:00Z",
            completed_at="2026-06-05T12:01:00Z",
            iterations=1,
            candidates_evaluated=1,
            total_cost_usd=0.0,
            total_calls=0,
            total_lm_tokens=0,
            replayability="fully_managed",
        ),
        assessment_rows=list(assessments),
    )


def _assessment(
    *,
    case_id: str,
    candidate_id: str = "cand_1",
    score: float = 0.75,
    replayability: Replayability = "fully_managed",
) -> Assessment:
    return Assessment(
        case=lv.Case(id=case_id, input={"question": "2+2"}, split="test"),
        candidate_id=candidate_id,
        score=lv.Score(value=score, feedback="ok"),
        evidence=EvidenceEnvelope.public_only(
            payload={"feedback": "ok"},
            data_classes=[lv.data_class.OPTIMIZER_VISIBLE],
        ),
        receipt=WriteReceipt(receipt_id=f"w_{case_id}_{candidate_id}"),
        replayability=replayability,
    )


def test_candidate_summary_includes_score_when_available() -> None:
    candidate = lv.Candidate(
        id="cand_1",
        artifact=lv.PromptArtifact(template="Answer {question}"),
        summary_score=0.875,
    )

    assert candidate.summary() == "cand_1: PromptArtifact score=0.875"


def test_candidate_summary_marks_unscored_candidates() -> None:
    candidate = lv.Candidate(id="cand_2", artifact=lv.SkillBank.empty())

    assert candidate.summary() == "cand_2: SkillBank unscored"


async def test_replay_returns_stored_result_for_fully_managed_assessment() -> None:
    result = _result_with_assessments(_assessment(case_id="case_1", score=0.875))

    replay = await result.replay("case_1")

    assert replay == lv.ReplayResult(
        case_id="case_1",
        candidate_id="cand_1",
        score=0.875,
        matches_original=True,
    )


async def test_replay_uses_candidate_id_when_case_has_multiple_assessments() -> None:
    result = _result_with_assessments(
        _assessment(case_id="case_1", candidate_id="cand_1", score=0.25),
        _assessment(case_id="case_1", candidate_id="cand_2", score=0.9),
    )

    replay = await result.replay("case_1", candidate_id="cand_2")

    assert replay == lv.ReplayResult(
        case_id="case_1",
        candidate_id="cand_2",
        score=0.9,
        matches_original=True,
    )


async def test_replay_refuses_boundary_managed_assessment() -> None:
    result = _result_with_assessments(
        _assessment(case_id="case_1", replayability="boundary_managed")
    )

    with pytest.raises(lv.ReplayUnavailableError, match="boundary_managed"):
        await result.replay("case_1")
