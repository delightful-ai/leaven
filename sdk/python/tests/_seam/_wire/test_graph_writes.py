"""Tests for generated public-seam graph-write records."""

import msgspec
import pytest
from msgspec import UNSET, Struct

from leaven._seam._wire.evidence import decode_evidence_private_payload
from leaven._seam._wire.expressions import (
    EvaluationSetCases,
    EvaluationSetNamed,
    EvaluationSetSample,
    EvaluationSetUnion,
    ValueExprLiteral,
    ValueExprVar,
)
from leaven._seam._wire.payloads import (
    PlanDocument,
)
from leaven._seam._wire.refs import BlobRef, ExternalEventPayload, TraceRefRecord
from leaven._seam._wire.writes import (
    EmitRunEventWrite,
    ProposalCausalInputs,
    ProposalEffectChange,
    ProposalEffectCreate,
    RequestEvaluationWrite,
    SubmitAssessmentsWrite,
    SubmitProposalBatchWrite,
)


class PrivateEvidencePayload(Struct, frozen=True, forbid_unknown_fields=True):
    rationale: str
    confidence: float


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
    assert isinstance(create.causal, ProposalCausalInputs)
    assert create.causal.inputs == []
    assert isinstance(create.annotations, ValueExprVar)
    assert create.annotations.name == "annotation_value"
    assert isinstance(change.effect, ProposalEffectChange)
    assert isinstance(change.effect.change, ValueExprLiteral)
    assert change.effect.change.value == "patch text"
    assert change.causal.inputs == ["cand_seed"]


def test_submit_proposal_batch_rejects_unknown_effect_kind() -> None:
    """Boundary check: proposal effects are tagged records, not raw objects."""

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(
            _proposal_plan().replace(b'"kind":"create"', b'"kind":"mystery"', 1),
            type=PlanDocument,
        )


def test_submit_proposal_batch_rejects_open_causal_payload() -> None:
    """Boundary check: proposal causal data is a closed typed record."""

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(
            _proposal_plan().replace(
                b'"causal":{"inputs":["cand_seed"]}',
                b'"causal":{"field":"prose"}',
                1,
            ),
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
    output = assessment.assessments[0].score.output
    assert output.summary == "score evidence"
    assert output.value == {"candidate": "cand_seed", "output": ["answer", {"unit": "text"}]}
    assert assessment.assessments[0].target == {
        "case": "case_1",
        "answer": ["42", {"unit": "text"}],
    }
    assert assessment.assessments[0].cost_attribution is not UNSET
    assert assessment.assessments[0].cost_attribution.kind == "explicit"
    assert isinstance(evaluation, RequestEvaluationWrite)
    assert evaluation.request.shape == "independent"
    assert evaluation.request.candidates == ["cand_seed"]
    assert isinstance(evaluation.request.set, EvaluationSetNamed)
    assert evaluation.request.set.name == "validation"
    assert isinstance(event, EmitRunEventWrite)
    assert event.visibility == "optimizer_visible"
    assert isinstance(event.payload, ExternalEventPayload)
    assert event.payload.ok is True


def test_assessment_output_decodes_typed_blob_and_trace_refs() -> None:
    """Example: graph-write output refs use generated ref owners."""

    decoded = msgspec.json.decode(
        _mixed_write_plan().replace(
            b'"output":{"kind":"text","visibility":"public",'
            b'"data_classes":["candidate.output"],"summary":"score evidence",'
            b'"value":{"candidate":"cand_seed","output":["answer",{"unit":"text"}]}}',
            b'"output":{"kind":"blob_ref","visibility":"public",'
            b'"data_classes":["public"],"summary":"score evidence",'
            b'"blob_ref":{"kind":"blob_ref","id":"blob_1","sha256":"abc",'
            b'"bytes":3,"data_classes":["public"]},'
            b'"trace_refs":[{"kind":"agent.trace","id":"trace_1",'
            b'"visibility":"public"}]}',
        ),
        type=PlanDocument,
    )
    assessment = decoded.ops[0].write

    assert isinstance(assessment, SubmitAssessmentsWrite)
    output = assessment.assessments[0].score.output
    assert isinstance(output.blob_ref, BlobRef)
    assert output.blob_ref.id == "blob_1"
    assert output.trace_refs is not UNSET
    assert isinstance(output.trace_refs[0], TraceRefRecord)


def test_assessment_preference_and_ranking_decode_owned_json_values() -> None:
    """Example: pairwise/listwise judgment leaves keep their assessment owner."""

    decoded = msgspec.json.decode(
        b'{"kind":"submit_assessments","evaluation_request_id":"evalreq_1",'
        b'"assessments":['
        b'{"kind":"pairwise","candidates":["cand_a","cand_b"],'
        b'"target":{"case":"case_1"},'
        b'"score":{"value":0.5,"output":{"kind":"structured",'
        b'"visibility":"public","data_classes":["candidate.output"],'
        b'"summary":"pairwise compared candidate outputs",'
        b'"value":[{"candidate":"cand_a","output":"answer a"},'
        b'{"candidate":"cand_b","output":"answer b"}]}},'
        b'"preference":{"winner":"cand_a","margin":[0.2,{"unit":"score"}]},'
        b'"evidence":' + _evidence_envelope() + b',"replayability":"pure_read"},'
        b'{"kind":"listwise","candidates":["cand_a","cand_b","cand_c"],'
        b'"target":{"case":"case_1"},'
        b'"score":{"value":0.75,"output":{"kind":"structured",'
        b'"visibility":"public","data_classes":["candidate.output"],'
        b'"summary":"listwise ranked candidate outputs",'
        b'"value":[{"candidate":"cand_a","output":"answer a"},'
        b'{"candidate":"cand_b","output":"answer b"},'
        b'{"candidate":"cand_c","output":"answer c"}]}},'
        b'"ranking":["cand_a",{"candidate":"cand_b","rank":2},"cand_c"],'
        b'"evidence":' + _evidence_envelope() + b',"replayability":"pure_read"}]}',
        type=SubmitAssessmentsWrite,
    )

    assert decoded.assessments[0].preference == {
        "winner": "cand_a",
        "margin": [0.2, {"unit": "score"}],
    }
    assert decoded.assessments[1].ranking == [
        "cand_a",
        {"candidate": "cand_b", "rank": 2},
        "cand_c",
    ]


def test_assessment_output_rejects_malformed_blob_ref() -> None:
    """Boundary check: output blob refs do not pass as arbitrary objects."""

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(
            _mixed_write_plan().replace(
                b'"output":{"kind":"text","visibility":"public",'
                b'"data_classes":["candidate.output"],"summary":"score evidence",'
                b'"value":{"candidate":"cand_seed","output":["answer",{"unit":"text"}]}}',
                b'"output":{"kind":"blob_ref","visibility":"public",'
                b'"data_classes":["public"],"summary":"score evidence",'
                b'"blob_ref":{"id":"blob_1"}}',
            ),
            type=PlanDocument,
        )


def test_assessment_evidence_decodes_typed_envelope() -> None:
    """Example: assessment evidence preserves generated envelope structure."""

    decoded = msgspec.json.decode(
        _mixed_write_plan().replace(
            _evidence_envelope(),
            _private_payload_evidence_envelope(),
        ),
        type=PlanDocument,
    )
    assessment = decoded.ops[0].write

    assert isinstance(assessment, SubmitAssessmentsWrite)
    evidence = assessment.assessments[0].evidence
    assert evidence.producer.stage_call_id == "sc_score"
    assert evidence.public.summary == "score evidence"
    assert evidence.private is not UNSET
    assert evidence.private.visibility == "evaluator_only"
    assert isinstance(evidence.private.payload_ref, BlobRef)
    assert evidence.private.payload == {"rationale": "grader trace", "confidence": 0.75}
    payload = decode_evidence_private_payload(evidence.private, PrivateEvidencePayload)
    assert payload.rationale == "grader trace"
    assert payload.confidence == 0.75
    assert evidence.source_receipts.effect == ["lmrec_1"]


def test_assessment_evidence_rejects_missing_public_projection() -> None:
    """Boundary check: evidence envelopes do not pass as arbitrary objects."""

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(
            _mixed_write_plan().replace(
                _evidence_envelope(),
                b'{"schema_version":"leaven.evidence_envelope.v1",'
                b'"target_derived":false,'
                b'"redaction_policy":{"optimizer":"score_and_feedback",'
                b'"reflector":"score_only","operator":"full"},'
                b'"producer":{"stage_call_id":"sc_score"},'
                b'"source_receipts":{"read":[],"effect":["lmrec_1"]}}',
            ),
            type=PlanDocument,
        )


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


def test_evaluation_request_decodes_typed_cases_set() -> None:
    """Example: evaluation request set variants preserve their identity."""

    decoded = msgspec.json.decode(
        _mixed_write_plan().replace(
            b'"set":{"kind":"named","name":"validation"}',
            b'"set":{"kind":"cases","cases":["case_1"],"requires_partition_resolution":true}',
        ),
        type=PlanDocument,
    )
    evaluation = decoded.ops[1].write

    assert isinstance(evaluation, RequestEvaluationWrite)
    assert isinstance(evaluation.request.set, EvaluationSetCases)
    assert evaluation.request.set.cases == ["case_1"]
    assert evaluation.request.set.requires_partition_resolution is True


def test_evaluation_request_decodes_recursive_set_exprs() -> None:
    """Example: composite evaluation sets recursively preserve set identity."""

    decoded = msgspec.json.decode(
        _mixed_write_plan().replace(
            b'"set":{"kind":"named","name":"validation"}',
            b'"set":{"kind":"union","sets":['
            b'{"kind":"named","name":"validation"},'
            b'{"kind":"sample","base":{"kind":"cases","cases":["case_1"],'
            b'"requires_partition_resolution":true},"n":1,"seed":7}]}',
        ),
        type=PlanDocument,
    )
    evaluation = decoded.ops[1].write

    assert isinstance(evaluation, RequestEvaluationWrite)
    assert isinstance(evaluation.request.set, EvaluationSetUnion)
    assert isinstance(evaluation.request.set.sets[0], EvaluationSetNamed)
    assert isinstance(evaluation.request.set.sets[1], EvaluationSetSample)
    assert isinstance(evaluation.request.set.sets[1].base, EvaluationSetCases)


def test_evaluation_request_rejects_malformed_recursive_set_expr() -> None:
    """Boundary check: composite evaluation sets do not accept raw child objects."""

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(
            _mixed_write_plan().replace(
                b'"set":{"kind":"named","name":"validation"}',
                b'"set":{"kind":"union","sets":[{"name":"validation"}]}',
            ),
            type=PlanDocument,
        )


def test_evaluation_request_rejects_unknown_set_kind() -> None:
    """Boundary check: evaluation set expressions are tagged records."""

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(
            _mixed_write_plan().replace(
                b'"set":{"kind":"named","name":"validation"}',
                b'"set":{"kind":"mystery","name":"validation"}',
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
        b'"causal":{"inputs":[]},"informed_by":{"kind":"var","name":"seed"},'
        b'"annotations":{"kind":"var","name":"annotation_value"}},'
        b'{"effect":{"kind":"change","target":"cand_seed",'
        b'"surface_fingerprint":"fp_surface","change_schema":"fp_schema_change",'
        b'"change":{"kind":"literal","value":"patch text"}},'
        b'"causal":{"inputs":["cand_seed"]},"informed_by":{"kind":"var","name":"seed"}}'
        b"]} }],"
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
        b'"target":{"case":"case_1","answer":["42",{"unit":"text"}]},'
        b'"score":{"value":0.5,"output":{"kind":"text","visibility":"public",'
        b'"data_classes":["candidate.output"],"summary":"score evidence",'
        b'"value":{"candidate":"cand_seed","output":["answer",{"unit":"text"}]}}},'
        b'"evidence":' + _evidence_envelope() + b","
        b'"replayability":"fully_managed",'
        b'"cost_attribution":{"kind":"explicit","cost":{"usd_micro":12}}}]}},'
        b'{"kind":"write","name":"eval","idempotency_key":"idem_eval",'
        b'"write":{"kind":"request_evaluation","request":{"shape":"independent",'
        b'"candidates":["cand_seed"],"set":{"kind":"named","name":"validation"},'
        b'"granularity":"per_case","purpose":"validation","evaluator":"judge"}}},'
        b'{"kind":"write","name":"event","idempotency_key":"idem_event",'
        b'"write":{"kind":"emit_run_event","event_kind":"optimizer.note",'
        b'"payload_schema":"fp_schema_event","payload":{"kind":"external_event","ok":true},'
        b'"visibility":"optimizer_visible"}}],'
        b'"return":["assess","eval","event"],'
        b'"commit":{"kind":"graph_writes_atomic","on_stale":"reject"}}'
    )


def _evidence_envelope() -> bytes:
    return (
        b'{"schema_version":"leaven.evidence_envelope.v1",'
        b'"target_derived":false,'
        b'"public":{"summary":"score evidence","data_classes":["public"]},'
        b'"redaction_policy":{"optimizer":"score_and_feedback",'
        b'"reflector":"score_only","operator":"full"},'
        b'"producer":{"stage_call_id":"sc_score"},'
        b'"source_receipts":{"read":[],"effect":["lmrec_1"]}}'
    )


def _private_payload_evidence_envelope() -> bytes:
    return (
        b'{"schema_version":"leaven.evidence_envelope.v1",'
        b'"target_derived":false,'
        b'"public":{"summary":"score evidence","data_classes":["public"]},'
        b'"private":{"visibility":"evaluator_only","data_classes":["private"],'
        b'"payload":{"rationale":"grader trace","confidence":0.75},'
        b'"payload_schema_fingerprint":"fp_schema_sha256_private_evidence",'
        b'"payload_ref":{"kind":"blob_ref","id":"blob_private","sha256":"def",'
        b'"bytes":5,"data_classes":["private"]}},'
        b'"redaction_policy":{"optimizer":"score_and_feedback",'
        b'"reflector":"score_only","operator":"full"},'
        b'"producer":{"stage_call_id":"sc_score"},'
        b'"source_receipts":{"read":[],"effect":["lmrec_1"]}}'
    )


__all__ = []
