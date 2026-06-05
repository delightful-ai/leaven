import pytest

import leaven as lv
from leaven._receipts import CallReceipt, QueryReceipt
from leaven.assessment import AssessmentWrite
from leaven.evidence import EvidenceEnvelope


def _evidence() -> EvidenceEnvelope:
    return EvidenceEnvelope.public_only(
        payload={"feedback": "ok"},
        data_classes=[lv.data_class.OPTIMIZER_VISIBLE],
    )


def test_independent_case_assessment_sets_single_candidate_shape() -> None:
    read = QueryReceipt(receipt_id="qrec_case")
    effect = CallReceipt(receipt_id="lmrec_judge")

    assessment = AssessmentWrite.independent_case(
        candidate="cand_a",
        case="case_1",
        score=lv.Score(value=0.75, feedback="ok"),
        evidence=_evidence(),
        read_receipts=[read],
        effect_receipts=[effect],
        replayability="pure_read",
    )

    assert assessment.kind == "independent"
    assert assessment.candidate == "cand_a"
    assert assessment.candidates is None
    assert assessment.score is not None
    assert assessment.read_receipts == [read]
    assert assessment.effect_receipts == [effect]
    assert assessment.replayability == "pure_read"


def test_pairwise_requires_two_candidates_and_member_preference() -> None:
    assessment = AssessmentWrite.pairwise(
        candidates=["cand_a", "cand_b"],
        case="case_1",
        preference="cand_b",
        score=lv.Score(value=1.0, feedback="cand_b preferred"),
        evidence=_evidence(),
    )

    assert assessment.kind == "pairwise"
    assert assessment.candidates == ["cand_a", "cand_b"]
    assert assessment.preference == "cand_b"

    with pytest.raises(ValueError, match="exactly two"):
        AssessmentWrite.pairwise(
            candidates=["cand_a"],
            case="case_1",
            preference="cand_a",
            score=lv.Score(value=1.0),
            evidence=_evidence(),
        )

    with pytest.raises(ValueError, match="preference"):
        AssessmentWrite.pairwise(
            candidates=["cand_a", "cand_b"],
            case="case_1",
            preference="cand_c",
            score=lv.Score(value=1.0),
            evidence=_evidence(),
        )


def test_listwise_requires_ranking_to_cover_candidates() -> None:
    assessment = AssessmentWrite.listwise(
        candidates=["cand_a", "cand_b", "cand_c"],
        case="case_1",
        ranking=["cand_c", "cand_a", "cand_b"],
        score=lv.Score(value=0.8, feedback="cand_c first"),
        evidence=_evidence(),
    )

    assert assessment.kind == "listwise"
    assert assessment.ranking == ["cand_c", "cand_a", "cand_b"]

    with pytest.raises(ValueError, match="same candidates"):
        AssessmentWrite.listwise(
            candidates=["cand_a", "cand_b"],
            case="case_1",
            ranking=["cand_a"],
            score=lv.Score(value=0.8),
            evidence=_evidence(),
        )
