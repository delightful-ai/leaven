"""Tests for generated public-seam graph row records."""

import pytest
from msgspec import UNSET

from leaven._seam._wire import JsonRpcProtocolError, decode_response
from leaven._seam._wire.evidence import EvidenceEnvelope
from leaven._seam._wire.payloads import (
    AssessmentSummaryGraphRow,
    CandidateArtifactSummary,
    CandidateScoresSummary,
    CandidateSummaryGraphRow,
    EventSummaryGraphRow,
    ExtensionGraphRow,
    GraphExtensionSummaryPayload,
    PlanResultDocument,
    ProposalEffectSummary,
    ProposalSummaryGraphRow,
)
from leaven._seam._wire.refs import CandidateRefRecord, ExternalEventPayload


def test_plan_result_decodes_graph_rows_as_tagged_records() -> None:
    """Scenario: graph_set items are typed rows, not raw JSON objects."""

    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"pure_read",'
        b'"values":{"rows":{"kind":"graph_set","graph_revision":"rev_final",'
        b'"data_classes":["public"],"replayability":"pure_read",'
        b'"items":[{"kind":"event_summary","event_kind":"case.loaded","revision":"rev_final"}]}},'
        b'"receipts":[],"redactions":[],"charges":[],"errors":[]}}'
    )

    decoded = decode_response(body, PlanResultDocument)
    rows = decoded.values["rows"].items

    assert rows is not UNSET
    assert isinstance(rows[0], EventSummaryGraphRow)
    assert rows[0].event_kind == "case.loaded"


def test_plan_result_decodes_assessment_row_evidence_envelope() -> None:
    """Example: assessment graph rows carry typed evidence envelopes."""

    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"pure_read",'
        b'"values":{"rows":{"kind":"graph_set","graph_revision":"rev_final",'
        b'"data_classes":["public"],"replayability":"pure_read",'
        b'"items":[{"kind":"assessment_summary","assessment":"assess_1",'
        b'"score":{"value":0.75,"output":{"kind":"text","visibility":"public",'
        b'"data_classes":["public"],"summary":"score evidence"}},'
        b'"evidence":{"schema_version":"leaven.evidence_envelope.v1",'
        b'"target_derived":false,'
        b'"public":{"summary":"score evidence","data_classes":["public"]},'
        b'"redaction_policy":{"optimizer":"score_and_feedback",'
        b'"reflector":"score_only","operator":"full"},'
        b'"producer":{"stage_call_id":"sc_score"},'
        b'"source_receipts":{"read":[],"effect":["lmrec_1"]}}}]}},'
        b'"receipts":[],"redactions":[],"charges":[],"errors":[]}}'
    )

    decoded = decode_response(body, PlanResultDocument)
    rows = decoded.values["rows"].items

    assert rows is not UNSET
    assert isinstance(rows[0], AssessmentSummaryGraphRow)
    assert isinstance(rows[0].evidence, EvidenceEnvelope)
    assert rows[0].evidence.public.summary == "score evidence"


def test_plan_result_decodes_graph_row_json_fragments_with_owned_names() -> None:
    """Example: graph-row JSON fragments keep row-specific owners."""

    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"pure_read",'
        b'"values":{"rows":{"kind":"graph_set","graph_revision":"rev_final",'
        b'"data_classes":["public"],"replayability":"pure_read",'
        b'"items":[{"kind":"candidate_summary","candidate":"cand_alpha",'
        b'"artifact_identity":"artifact_sha256_alpha",'
        b'"scores":{"primary":0.9,"metrics":{"accuracy":0.9},'
        b'"cases":[{"case":"case_1","score":0.9}]},'
        b'"artifact":{"kind":"prompt","identity":"artifact_sha256_alpha",'
        b'"summary":"answer concisely","body":"answer concisely"}},'
        b'{"kind":"proposal_summary","proposal":"prop_alpha","batch":"pb_alpha",'
        b'"effect":{"kind":"change","target":"cand_alpha"}},'
        b'{"kind":"event_summary","event_kind":"case.loaded","revision":"rev_final",'
        b'"payload":{"kind":"external_event","ok":true}},'
        b'{"kind":"extension","namespace":"vendor.eval","op":"row",'
        b'"schema_fingerprint":"fp_schema_sha256_vendor_row",'
        b'"payload":{"kind":"summary","summary":"vendor score 7",'
        b'"data_classes":["public"],'
        b'"source_ref":{"kind":"candidate","id":"cand_alpha"}}}]}},'
        b'"receipts":[],"redactions":[],"charges":[],"errors":[]}}'
    )

    decoded = decode_response(body, PlanResultDocument)
    rows = decoded.values["rows"].items

    assert rows is not UNSET
    assert isinstance(rows[0], CandidateSummaryGraphRow)
    assert isinstance(rows[0].scores, CandidateScoresSummary)
    assert rows[0].scores.primary == 0.9
    assert rows[0].scores.metrics == {"accuracy": 0.9}
    assert rows[0].scores.cases is not UNSET
    assert rows[0].scores.cases[0].case == "case_1"
    assert rows[0].scores.cases[0].score == 0.9
    assert isinstance(rows[0].artifact, CandidateArtifactSummary)
    assert rows[0].artifact.kind == "prompt"
    assert rows[0].artifact.identity == "artifact_sha256_alpha"
    assert rows[0].artifact.summary == "answer concisely"
    assert rows[0].artifact.body == "answer concisely"
    assert isinstance(rows[1], ProposalSummaryGraphRow)
    assert isinstance(rows[1].effect, ProposalEffectSummary)
    assert rows[1].effect.kind == "change"
    assert rows[1].effect.target == "cand_alpha"
    assert isinstance(rows[2], EventSummaryGraphRow)
    assert isinstance(rows[2].payload, ExternalEventPayload)
    assert rows[2].payload.ok is True
    assert isinstance(rows[3], ExtensionGraphRow)
    assert isinstance(rows[3].payload, GraphExtensionSummaryPayload)
    assert rows[3].payload.summary == "vendor score 7"
    assert rows[3].payload.data_classes == ["public"]
    assert isinstance(rows[3].payload.source_ref, CandidateRefRecord)
    assert rows[3].payload.source_ref.id == "cand_alpha"


def test_plan_result_rejects_open_extension_graph_row_payload() -> None:
    """Regression: extension graph rows carry a closed typed payload union."""

    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"pure_read",'
        b'"values":{"rows":{"kind":"graph_set","graph_revision":"rev_final",'
        b'"data_classes":["public"],"replayability":"pure_read",'
        b'"items":[{"kind":"extension","namespace":"vendor.eval","op":"row",'
        b'"schema_fingerprint":"fp_schema_sha256_vendor_row",'
        b'"payload":{"vendor":{"score":7}}}]}},'
        b'"receipts":[],"redactions":[],"charges":[],"errors":[]}}'
    )

    with pytest.raises(JsonRpcProtocolError):
        decode_response(body, PlanResultDocument)


def test_plan_result_rejects_open_candidate_summary_fragments() -> None:
    """Regression: candidate scores/artifacts are closed summaries, not JSON leaves."""

    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"pure_read",'
        b'"values":{"rows":{"kind":"graph_set","graph_revision":"rev_final",'
        b'"data_classes":["public"],"replayability":"pure_read",'
        b'"items":[{"kind":"candidate_summary","candidate":"cand_alpha",'
        b'"scores":{"primary":0.9,"checks":["format"]}}]}},'
        b'"receipts":[],"redactions":[],"charges":[],"errors":[]}}'
    )

    with pytest.raises(JsonRpcProtocolError, match="unknown field `checks`"):
        decode_response(body, PlanResultDocument)


def test_plan_result_rejects_open_proposal_effect_summary_payload() -> None:
    """Regression: proposal graph-row effects are closed summaries, not JSON leaves."""

    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"pure_read",'
        b'"values":{"rows":{"kind":"graph_set","graph_revision":"rev_final",'
        b'"data_classes":["public"],"replayability":"pure_read",'
        b'"items":[{"kind":"proposal_summary","proposal":"prop_alpha",'
        b'"effect":{"kind":"change","target":"cand_alpha",'
        b'"prose":"proposal summaries are typed records"}}]}},'
        b'"receipts":[],"redactions":[],"charges":[],"errors":[]}}'
    )

    with pytest.raises(JsonRpcProtocolError, match="unknown field `prose`"):
        decode_response(body, PlanResultDocument)


def test_plan_result_rejects_malformed_assessment_row_evidence() -> None:
    """Regression: assessment graph row evidence is not arbitrary JSON."""

    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"pure_read",'
        b'"values":{"rows":{"kind":"graph_set","graph_revision":"rev_final",'
        b'"data_classes":["public"],"replayability":"pure_read",'
        b'"items":[{"kind":"assessment_summary","assessment":"assess_1",'
        b'"score":{"value":0.75,"output":{"kind":"text","visibility":"public",'
        b'"data_classes":["public"],"summary":"score evidence"}},'
        b'"evidence":{"schema_version":"leaven.evidence_envelope.v1",'
        b'"target_derived":false,'
        b'"redaction_policy":{"optimizer":"score_and_feedback",'
        b'"reflector":"score_only","operator":"full"},'
        b'"producer":{"stage_call_id":"sc_score"},'
        b'"source_receipts":{"read":[],"effect":["lmrec_1"]}}}]}},'
        b'"receipts":[],"redactions":[],"charges":[],"errors":[]}}'
    )

    with pytest.raises(JsonRpcProtocolError):
        decode_response(body, PlanResultDocument)


def test_plan_result_rejects_unknown_graph_row_kind() -> None:
    """Regression: graph_set rows cannot silently decode as arbitrary objects."""

    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"pure_read",'
        b'"values":{"rows":{"kind":"graph_set","graph_revision":"rev_final",'
        b'"data_classes":["public"],"replayability":"pure_read",'
        b'"items":[{"kind":"private_row","event_kind":"case.loaded","revision":"rev_final"}]}},'
        b'"receipts":[],"redactions":[],"charges":[],"errors":[]}}'
    )

    with pytest.raises(JsonRpcProtocolError):
        decode_response(body, PlanResultDocument)


__all__ = []
