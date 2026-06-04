"""Tests for generated public-seam graph-write records."""

import msgspec
import pytest
from msgspec import UNSET

from leaven._seam._wire.expressions import ValueExprLiteral, ValueExprVar
from leaven._seam._wire.payloads import (
    PlanDocument,
)
from leaven._seam._wire.writes import (
    EmitRunEventWrite,
    ProposalEffectChange,
    ProposalEffectCreate,
    RequestEvaluationWrite,
    SubmitAssessmentsWrite,
    SubmitProposalBatchWrite,
)


def test_submit_proposal_batch_write_decodes_typed_effect_value_exprs() -> None:
    """Example: proposal effects and annotations keep their ValueExpr variants."""

    decoded = msgspec.json.decode(_proposal_plan(), type=PlanDocument)
    write = decoded.ops[0].write

    assert isinstance(write, SubmitProposalBatchWrite)
    create = write.proposals[0]
    change = write.proposals[1]
    assert isinstance(create.effect, ProposalEffectCreate)
    assert isinstance(create.effect.artifact, ValueExprLiteral)
    assert create.effect.artifact.value == "new prompt"
    assert isinstance(create.annotations, ValueExprVar)
    assert create.annotations.name == "annotation_value"
    assert isinstance(change.effect, ProposalEffectChange)
    assert isinstance(change.effect.change, ValueExprLiteral)
    assert change.effect.change.value == "patch text"


def test_submit_proposal_batch_rejects_unknown_effect_kind() -> None:
    """Boundary check: proposal effects are tagged records, not raw objects."""

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(
            _proposal_plan().replace(b'"kind":"create"', b'"kind":"mystery"', 1),
            type=PlanDocument,
        )


def test_assessment_evaluation_and_event_writes_decode_typed_records() -> None:
    """Example: retained non-proposal write methods expose typed owners."""

    decoded = msgspec.json.decode(_mixed_write_plan(), type=PlanDocument)
    assessment = decoded.ops[0].write
    evaluation = decoded.ops[1].write
    event = decoded.ops[2].write

    assert isinstance(assessment, SubmitAssessmentsWrite)
    assert assessment.assessments[0].kind == "independent"
    assert assessment.assessments[0].score.output.summary == "score evidence"
    assert assessment.assessments[0].cost_attribution is not UNSET
    assert assessment.assessments[0].cost_attribution.kind == "explicit"
    assert isinstance(evaluation, RequestEvaluationWrite)
    assert evaluation.request.shape == "independent"
    assert evaluation.request.candidates == ["cand_seed"]
    assert isinstance(event, EmitRunEventWrite)
    assert event.visibility == "optimizer_visible"


def test_assessment_write_rejects_unknown_assessment_kind() -> None:
    """Boundary check: assessment write records are not raw dictionaries."""

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(
            _mixed_write_plan().replace(b'"kind":"independent"', b'"kind":"mystery"', 1),
            type=PlanDocument,
        )


def test_event_write_rejects_unknown_visibility() -> None:
    """Boundary check: event visibility is a typed public-seam enum."""

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(
            _mixed_write_plan().replace(
                b'"visibility":"optimizer_visible"',
                b'"visibility":"secret"',
            ),
            type=PlanDocument,
        )


def _proposal_plan() -> bytes:
    return (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"write","name":"proposal_batch",'
        b'"idempotency_key":"idem_1",'
        b'"write":{"kind":"submit_proposal_batch","semantics":"sequence","proposals":['
        b'{"effect":{"kind":"create","artifact_type":"prompt",'
        b'"artifact_schema":"fp_schema_artifact",'
        b'"artifact":{"kind":"literal","value":"new prompt"}},'
        b'"causal":{},"informed_by":{"kind":"var","name":"seed"},'
        b'"annotations":{"kind":"var","name":"annotation_value"}},'
        b'{"effect":{"kind":"change","target":"cand_seed",'
        b'"surface_fingerprint":"fp_surface","change_schema":"fp_schema_change",'
        b'"change":{"kind":"literal","value":"patch text"}},'
        b'"causal":{},"informed_by":{"kind":"var","name":"seed"}}'
        b']} }],'
        b'"return":["proposal_batch"],"commit":{"kind":"graph_writes_atomic","on_stale":"reject"}}'
    )


def _mixed_write_plan() -> bytes:
    return (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":['
        b'{"kind":"write","name":"assess","idempotency_key":"idem_assess",'
        b'"write":{"kind":"submit_assessments","evaluation_request_id":"evalreq_1",'
        b'"assessments":[{"kind":"independent","candidate":"cand_seed",'
        b'"score":{"value":0.5,"output":{"kind":"text","visibility":"public",'
        b'"data_classes":["public"],"summary":"score evidence"}},'
        b'"evidence":{"schema_version":"leaven.evidence_envelope.v1"},'
        b'"replayability":"fully_managed",'
        b'"cost_attribution":{"kind":"explicit","cost":{"usd_micro":12}}}]}},'
        b'{"kind":"write","name":"eval","idempotency_key":"idem_eval",'
        b'"write":{"kind":"request_evaluation","request":{"shape":"independent",'
        b'"candidates":["cand_seed"],"set":{"kind":"named","name":"validation"},'
        b'"granularity":"per_case","purpose":"validation","evaluator":"judge"}}},'
        b'{"kind":"write","name":"event","idempotency_key":"idem_event",'
        b'"write":{"kind":"emit_run_event","event_kind":"optimizer.note",'
        b'"payload_schema":"fp_schema_event","payload":"noted",'
        b'"visibility":"optimizer_visible"}}],'
        b'"return":["assess","eval","event"],'
        b'"commit":{"kind":"graph_writes_atomic","on_stale":"reject"}}'
    )


__all__ = []
