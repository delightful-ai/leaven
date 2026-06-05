"""Tests for generated public-seam graph row records."""

import pytest
from msgspec import UNSET

from leaven._seam._wire import JsonRpcProtocolError, decode_response
from leaven._seam._wire.evidence import EvidenceEnvelope
from leaven._seam._wire.payloads import (
    AssessmentSummaryGraphRow,
    EventSummaryGraphRow,
    PlanResultDocument,
)


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
