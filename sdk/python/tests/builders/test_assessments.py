import json

import msgspec
import pytest

import leaven as lv
from leaven._receipts import CallReceipt, QueryReceipt
from leaven._seam import AssessmentSubmitRequest
from leaven._seam._wire.results import AssessmentBatchPrimary, AssessmentSubmitResult
from leaven.assessment import AssessmentWrite
from leaven.builders.assessments import AssessmentsBuilder
from leaven.evidence import EvidenceEnvelope
from leaven.json_value import JsonObject, JsonValue


@pytest.mark.asyncio
async def test_assessments_builder_submits_independent_assessment_through_seam() -> None:
    """Scenario: evaluator submits a typed assessment batch through the seam."""

    client = FakeAssessmentSeamClient()
    assessments = AssessmentsBuilder._for_seam(
        client,
        idempotency_prefix="assessment-builder-test",
        plan_id="planassessmentbuilder001",
        stage_call_id="sc_assessment_builder",
    )
    assessment = AssessmentWrite.independent_case(
        candidate="cand_seed",
        case="case_1",
        score=lv.Score(value=0.75, feedback="candidate answered correctly"),
        evidence=EvidenceEnvelope.public_only(
            payload={"feedback": "candidate answered correctly"},
            data_classes=[lv.data_class.CANDIDATE_OUTPUT, lv.data_class.OPTIMIZER_VISIBLE],
        ),
        read_receipts=[QueryReceipt(receipt_id="qrec_case")],
        effect_receipts=[CallReceipt(receipt_id="lmrec_score")],
        replayability="fully_managed",
    )

    submission = await assessments.submit("evalreq_1", [assessment])

    assert client.request_value.method == "leaven/assessment.submit"
    params = _params_object(client.request_value.to_params())
    assert params["plan_id"] == "planassessmentbuilder001"
    assert params["return"] == ["assessment_batch"]
    ops = _json_array(params["ops"])
    op = _json_object(ops[0])
    assert op["kind"] == "write"
    assert op["idempotency_key"] == "assessment-builder-test-submit"
    write = _json_object(op["write"])
    assert write["kind"] == "submit_assessments"
    assert write["evaluation_request_id"] == "evalreq_1"
    wire_assessments = _json_array(write["assessments"])
    wire_assessment = _json_object(wire_assessments[0])
    assert wire_assessment["kind"] == "independent"
    assert wire_assessment["candidate"] == "cand_seed"
    assert wire_assessment["read_receipts"] == ["qrec_case"]
    assert wire_assessment["effect_receipts"] == ["lmrec_score"]
    assert wire_assessment["replayability"] == "fully_managed"
    score = _json_object(wire_assessment["score"])
    assert score["value"] == 0.75
    output = _json_object(score["output"])
    assert output["kind"] == "text"
    assert output["value"] == "candidate answered correctly"
    assert output["data_classes"] == [
        lv.data_class.CANDIDATE_OUTPUT,
        lv.data_class.OPTIMIZER_VISIBLE,
    ]
    evidence = _json_object(wire_assessment["evidence"])
    assert evidence["schema_version"] == "leaven.evidence_envelope.v1"
    assert evidence["target_derived"] is False
    assert evidence["producer"] == {"stage_call_id": "sc_assessment_builder"}
    assert evidence["source_receipts"] == {
        "read": ["qrec_case"],
        "effect": ["lmrec_score"],
    }
    assert evidence["public"] == {
        "data_classes": [
            lv.data_class.CANDIDATE_OUTPUT,
            lv.data_class.OPTIMIZER_VISIBLE,
        ],
        "feedback": "candidate answered correctly",
    }
    assert submission.receipt.receipt_id == "wrec_assessment_submit"
    assert submission.assessment_ids == ["assess_1"]
    assert submission.submitted == 1


@pytest.mark.asyncio
async def test_assessments_builder_requires_candidate_output_data_class() -> None:
    """Boundary: the SDK must not invent assessed-output semantics."""

    client = FakeAssessmentSeamClient()
    assessments = AssessmentsBuilder._for_seam(client)
    assessment = AssessmentWrite.independent_case(
        candidate="cand_seed",
        case="case_1",
        score=lv.Score(value=0.75, feedback="ok"),
        evidence=EvidenceEnvelope.public_only(
            payload={"feedback": "ok"},
            data_classes=[lv.data_class.OPTIMIZER_VISIBLE],
        ),
    )

    with pytest.raises(ValueError, match=r"candidate\.output or candidate\.artifact"):
        await assessments.submit("evalreq_1", [assessment])


@pytest.mark.asyncio
async def test_assessments_builder_requires_bound_seam_client() -> None:
    """Regression: unbound public builders remain explicit about missing engine context."""

    assessment = AssessmentWrite.independent_case(
        candidate="cand_seed",
        case="case_1",
        score=lv.Score(value=0.75, feedback="ok"),
        evidence=EvidenceEnvelope.public_only(
            payload={"feedback": "ok"},
            data_classes=[lv.data_class.CANDIDATE_OUTPUT],
        ),
    )

    with pytest.raises(NotImplementedError, match="engine-bound public-seam client"):
        await AssessmentsBuilder().submit("evalreq_1", [assessment])


def _json_object(value: JsonValue) -> JsonObject:
    assert isinstance(value, dict)
    return value


def _json_array(value: JsonValue) -> list[JsonValue]:
    assert isinstance(value, list)
    return value


def _params_object(params: object) -> JsonObject:
    value = json.loads(msgspec.json.encode(params))
    assert isinstance(value, dict)
    return value


class FakeAssessmentSeamClient:
    def __init__(self) -> None:
        self.request_value = AssessmentSubmitRequest(
            request_id="unset",
            plan_id="unset",
            idempotency_key="unset",
            evaluation_request_id="evalreq_unset",
            assessments=[],
        )

    def assessment_submit(self, request: AssessmentSubmitRequest) -> AssessmentSubmitResult:
        self.request_value = request
        return AssessmentSubmitResult(
            method="leaven/assessment.submit",
            primary=AssessmentBatchPrimary(
                kind="assessment_batch_receipt",
                assessment_ids=["assess_1"],
                status="committed",
                graph_revision="rev_assessment_submit",
                data_classes=["public"],
                replayability="fully_managed",
                receipt="wrec_assessment_submit",
                evaluation_request_id="evalreq_1",
            ),
            receipts=[],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=["public"],
        )


__all__ = []
