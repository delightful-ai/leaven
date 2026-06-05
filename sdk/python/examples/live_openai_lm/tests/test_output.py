import json

from leaven.assessment import Assessment, RewardAssessment
from leaven.case import Case
from leaven.evidence import EvidenceEnvelope, EvidencePublic
from leaven.score import Score

from live_openai_lm.config import EXPECTED_TEXT
from live_openai_lm.output import LiveLmOutput, LiveLmUsage, live_lm_output_from_assessment


def test_live_lm_output_from_assessment_reads_public_runner_output() -> None:
    output = {
        "text": EXPECTED_TEXT,
        "receipt": "lmrec_completion",
        "usage": {"total_tokens": 35},
        "cost_usd": None,
        "model": "gpt-4.1-mini",
    }
    assessment = Assessment.model_validate(
        {
            "case": Case(id="case_live_openai_lm_001", input={}),
            "candidate_id": "cand_seed",
            "score": Score(value=1.0),
            "evidence": EvidenceEnvelope(
                public=EvidencePublic(
                    data_classes=["public"],
                    payload={"output": json.dumps(output, sort_keys=True), "reward_count": 1},
                )
            ),
            "receipt": {"receipt_id": "assessmentrec_case_live_openai_lm_001_1"},
            "effect_receipts": [{"receipt_id": "lmrec_completion"}],
            "replayability": "fully_managed",
            "rewards": [
                RewardAssessment(
                    id="live_openai_lm.scenario.exact",
                    value=1.0,
                    weight=1.0,
                )
            ],
        }
    )

    assert live_lm_output_from_assessment(assessment) == LiveLmOutput(
        text=EXPECTED_TEXT,
        receipt="lmrec_completion",
        usage=LiveLmUsage(total_tokens=35),
        cost_usd=None,
        model="gpt-4.1-mini",
    )
