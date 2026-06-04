"""Tests for generated top-level public-seam payload records."""

import msgspec
import pytest
from msgspec import UNSET

from leaven._seam._wire import JsonRpcProtocolError, decode_response
from leaven._seam._wire.payloads import (
    PLAN_RESULT_SCHEMA_FINGERPRINT,
    PLAN_SCHEMA_FINGERPRINT,
    STAGE_RUN_SCHEMA_FINGERPRINT,
    CapabilityCall,
    CommitPolicyNoGraphWrites,
    ConsistencyLatestAtStart,
    EvalModeExecute,
    EventSummaryGraphRow,
    GraphWrite,
    LeavenValue,
    OperationReceipt,
    PlanDocument,
    PlanResultDocument,
    RunnerRequest,
    StageRunRequest,
    StageRunResult,
)


def test_plan_document_decodes_top_level_shape() -> None:
    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[],"return":[],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)

    assert decoded.schema_version == "leaven.plan.v1"
    assert decoded.plan_id == "plan_1"
    assert isinstance(decoded.consistency, ConsistencyLatestAtStart)
    assert isinstance(decoded.mode, EvalModeExecute)
    assert isinstance(decoded.commit, CommitPolicyNoGraphWrites)
    assert decoded.return_ == []


def test_plan_document_decodes_typed_operation_kinds() -> None:
    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":['
        b'{"kind":"let","name":"x","expr":{"kind":"literal","value":"ok"}},'
        b'{"kind":"call","name":"lm","idempotency_key":"idem_1",'
        b'"call":{"kind":"lm_complete","model":"gpt-test"}},'
        b'{"kind":"write","name":"evt","idempotency_key":"idem_2",'
        b'"write":{"kind":"emit_run_event","event":{"kind":"stage_completed"}}}'
        b'],"return":["evt"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)

    call = decoded.ops[1].call
    write = decoded.ops[2].write
    assert isinstance(call, CapabilityCall)
    assert isinstance(write, GraphWrite)
    assert decoded.ops[0].kind == "let"
    assert call.kind == "lm_complete"
    assert write.kind == "emit_run_event"


def test_plan_document_rejects_unknown_operation_payload_kind() -> None:
    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"call","name":"bad","idempotency_key":"idem_1",'
        b'"call":{"kind":"private_transport"}}],'
        b'"return":[],"commit":{"kind":"no_graph_writes"}}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(body, type=PlanDocument)


def test_plan_document_rejects_wrong_schema_version() -> None:
    body = (
        b'{"schema_version":"wrong","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[],"return":[],"commit":{"kind":"no_graph_writes"}}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(body, type=PlanDocument)


def test_plan_document_rejects_extra_top_level_fields() -> None:
    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[],"return":[],"commit":{"kind":"no_graph_writes"},"extra":true}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(body, type=PlanDocument)


def test_plan_document_rejects_unknown_nested_variant() -> None:
    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"whatever"},"mode":{"kind":"execute"},'
        b'"ops":[],"return":[],"commit":{"kind":"no_graph_writes"}}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(body, type=PlanDocument)


def test_plan_result_decodes_as_method_specific_json_rpc_result() -> None:
    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"fully_managed",'
        b'"values":{},"receipts":[],"redactions":[],"charges":[],"errors":[]}}'
    )

    decoded = decode_response(body, PlanResultDocument)

    assert decoded.schema_version == "leaven.plan_result.v1"
    assert decoded.final_revision == "rev_final"
    assert decoded.replayability_summary == "fully_managed"
    assert decoded.receipts == []


def test_plan_result_decodes_typed_values_and_receipts() -> None:
    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"fully_managed",'
        b'"values":{"evt":{"kind":"emit_run_event","event_id":"event_1",'
        b'"receipt":"wrec_1","graph_revision":"rev_final",'
        b'"data_classes":["public"],"replayability":"fully_managed"}},'
        b'"receipts":[{"kind":"write","receipt":"wrec_1","status":"succeeded",'
        b'"write_kind":"emit_run_event","request_hash":"fp_request_sha256_test",'
        b'"result_hash":"fp_result_sha256_test","base_revision":"rev_base",'
        b'"event_id":"event_1"}],'
        b'"redactions":[],"charges":[],"errors":[]}}'
    )

    decoded = decode_response(body, PlanResultDocument)

    value = decoded.values["evt"]
    receipt = decoded.receipts[0]
    assert isinstance(value, LeavenValue)
    assert isinstance(receipt, OperationReceipt)
    assert value.kind == "emit_run_event"
    assert value.event_id == "event_1"
    assert receipt.kind == "write"
    assert receipt.write_kind == "emit_run_event"


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


def test_plan_result_rejects_unknown_value_or_receipt_kind() -> None:
    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"fully_managed",'
        b'"values":{"x":{"kind":"private_value","graph_revision":"rev_final",'
        b'"data_classes":["public"],"replayability":"fully_managed"}},'
        b'"receipts":[{"kind":"private_receipt","receipt":"rec_1","status":"succeeded",'
        b'"result_hash":"fp_result_sha256_test"}],'
        b'"redactions":[],"charges":[],"errors":[]}}'
    )

    with pytest.raises(JsonRpcProtocolError):
        decode_response(body, PlanResultDocument)


def test_plan_result_failure_is_reported_as_protocol_error() -> None:
    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1"}}'
    )

    with pytest.raises(JsonRpcProtocolError):
        decode_response(body, PlanResultDocument)


def test_stage_run_request_decodes_top_level_dispatch_shape() -> None:
    body = (
        b'{"schema_version":"leaven.stage_run.v1","message":"stage_run_request",'
        b'"stage":"runner","payload":{'
        b'"schema_version":"leaven.stage_payloads.v1","role":"runner",'
        b'"run":"run_1","stage_call_id":"sc_1","candidate":"cand_1",'
        b'"case":"case_1","case_input":{"prompt":"hello"},"target_forbidden":true}}'
    )

    decoded = msgspec.json.decode(body, type=StageRunRequest)

    assert decoded.stage == "runner"
    assert isinstance(decoded.payload, RunnerRequest)
    assert decoded.payload.case_input == {"prompt": "hello"}


def test_stage_run_request_rejects_untyped_payload_role() -> None:
    body = (
        b'{"schema_version":"leaven.stage_run.v1","message":"stage_run_request",'
        b'"stage":"runner","payload":{"role":"callback","event":{}}}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(body, type=StageRunRequest)


def test_stage_run_result_decodes_output_and_receipts() -> None:
    body = (
        b'{"schema_version":"leaven.stage_run.v1","message":"stage_run_result",'
        b'"stage":"runner","stage_call_id":"sc_1",'
        b'"output":{"kind":"text","summary":"ok","visibility":"public","data_classes":["public"]},'
        b'"effect_receipts":[{"method":"leaven/lm.complete","receipt":"lmrec_1",'
        b'"call_kind":"lm_complete","cost":{"usd_micro":10}}],'
        b'"proposal_receipts":[{"method":"leaven/proposal.submit_batch","receipt":"wrec_1",'
        b'"write_kind":"submit_proposal_batch","proposal_ids":["prop_1"]}]}'
    )

    decoded = msgspec.json.decode(body, type=StageRunResult)

    assert decoded.effect_receipts is not UNSET
    assert decoded.proposal_receipts is not UNSET
    effect_cost = decoded.effect_receipts[0].cost
    assert effect_cost is not UNSET
    assert decoded.output.summary == "ok"
    assert effect_cost.usd_micro == 10
    assert decoded.proposal_receipts[0].proposal_ids == ["prop_1"]


def test_generated_schema_fingerprints_use_public_seam_prefix() -> None:
    assert PLAN_SCHEMA_FINGERPRINT.startswith("fp_schema_sha256_")
    assert PLAN_RESULT_SCHEMA_FINGERPRINT.startswith("fp_schema_sha256_")
    assert STAGE_RUN_SCHEMA_FINGERPRINT.startswith("fp_schema_sha256_")
