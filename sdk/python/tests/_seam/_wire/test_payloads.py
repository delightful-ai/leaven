"""Tests for generated top-level public-seam payload records."""

import msgspec
import pytest
from msgspec import UNSET

from leaven._seam._wire import JsonRpcProtocolError, decode_response
from leaven._seam._wire.calls import LmCompleteCall, LmContentText, LmOutputFinalMessage
from leaven._seam._wire.expressions import (
    GraphSourceByCandidate,
    PlanExpressionGraphQuery,
    PlanExpressionLiteral,
    PreconditionCandidateExists,
    ValidationReceipt,
)
from leaven._seam._wire.payloads import (
    PLAN_RESULT_SCHEMA_FINGERPRINT,
    PLAN_SCHEMA_FINGERPRINT,
    CandidateRefRecord,
    CaseRefRecord,
    CommitPolicyNoGraphWrites,
    ConsistencyLatestAtStart,
    EvalModeExecute,
    ExternalInfoRefRecord,
    LeavenValue,
    OperationReceipt,
    PlanDocument,
    PlanErrorDetailsObject,
    PlanResultDocument,
    ReceiptRefRecord,
    TraceRefRecord,
)
from leaven._seam._wire.writes import EmitRunEventWrite


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
        b'"call":{"kind":"lm_complete","purpose":"python.test",'
        b'"model":"gpt-test","messages":[{"role":"user",'
        b'"content":[{"kind":"text","text":"say ok"}]}],'
        b'"output":{"kind":"final_message","max_bytes":64},'
        b'"input_classes":["public"]}},'
        b'{"kind":"write","name":"evt","idempotency_key":"idem_2",'
        b'"write":{"kind":"emit_run_event","event_kind":"stage_completed",'
        b'"payload_schema":"fp_schema_event","payload":{"ok":true},'
        b'"visibility":"optimizer_visible"}}'
        b'],"return":["evt"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)

    call = decoded.ops[1].call
    write = decoded.ops[2].write
    assert isinstance(call, LmCompleteCall)
    assert isinstance(write, EmitRunEventWrite)
    assert isinstance(decoded.ops[0].expr, PlanExpressionLiteral)
    assert decoded.ops[0].kind == "let"
    assert call.model == "gpt-test"
    content = call.messages[0].content[0]
    assert isinstance(content, LmContentText)
    assert content.text == "say ok"
    assert isinstance(call.output, LmOutputFinalMessage)
    assert write.event_kind == "stage_completed"


def test_plan_document_decodes_typed_plan_expression_source_refs() -> None:
    """Example: let expressions carry typed Plan expression and graph-source records."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"rows","expr":{"kind":"graph_query",'
        b'"source":{"kind":"by_candidate","candidate":{"kind":"candidate","id":"cand_1"}},'
        b'"projection":{"kind":"summary"}}}],'
        b'"return":["rows"],"commit":{"kind":"no_graph_writes"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    expr = decoded.ops[0].expr

    assert isinstance(expr, PlanExpressionGraphQuery)
    assert isinstance(expr.source, GraphSourceByCandidate)
    assert isinstance(expr.source.candidate, CandidateRefRecord)


def test_plan_document_rejects_unknown_expression_kind() -> None:
    """Regression: let expressions are no longer arbitrary JSON objects."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"let","name":"x","expr":{"kind":"private_expr","value":"ok"}}],'
        b'"return":["x"],"commit":{"kind":"no_graph_writes"}}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(body, type=PlanDocument)


def test_plan_document_decodes_typed_write_preconditions() -> None:
    """Example: write preconditions carry tagged records, not arbitrary objects."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"write","name":"evt","idempotency_key":"idem_1",'
        b'"write":{"kind":"emit_run_event","event_kind":"stage_completed",'
        b'"payload_schema":"fp_schema_event","payload":{"ok":true},'
        b'"visibility":"optimizer_visible"},'
        b'"preconditions":[{"kind":"candidate_exists",'
        b'"candidate":{"kind":"candidate","id":"cand_1"}}]}],'
        b'"return":["evt"],"commit":{"kind":"graph_writes_atomic","on_stale":"reject"}}'
    )

    decoded = msgspec.json.decode(body, type=PlanDocument)
    preconditions = decoded.ops[0].preconditions

    assert preconditions is not UNSET
    assert isinstance(preconditions[0], PreconditionCandidateExists)
    assert isinstance(preconditions[0].candidate, CandidateRefRecord)


def test_plan_document_rejects_unknown_precondition_kind() -> None:
    """Regression: write preconditions are no longer arbitrary JSON objects."""

    body = (
        b'{"schema_version":"leaven.plan.v1","plan_id":"plan_1",'
        b'"consistency":{"kind":"latest_at_start"},"mode":{"kind":"execute"},'
        b'"ops":[{"kind":"write","name":"evt","idempotency_key":"idem_1",'
        b'"write":{"kind":"emit_run_event","event_kind":"stage_completed",'
        b'"payload_schema":"fp_schema_event","payload":{"ok":true},'
        b'"visibility":"optimizer_visible"},'
        b'"preconditions":[{"kind":"private_condition","candidate":"cand_1"}]}],'
        b'"return":["evt"],"commit":{"kind":"graph_writes_atomic","on_stale":"reject"}}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(body, type=PlanDocument)


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


def test_plan_result_decodes_closed_plan_error_details() -> None:
    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"fully_managed","values":{},'
        b'"receipts":[],"redactions":[],"charges":[],'
        b'"errors":[{"code":"rate_limited","message":"wait",'
        b'"receipt":{"kind":"receipt","id":"err_1"},"retryable":true,'
        b'"details":{"summary":"slow down","reason":"provider_limit",'
        b'"retry_after_ms":250}}]}}'
    )

    decoded = decode_response(body, PlanResultDocument)
    details = decoded.errors[0].details

    assert isinstance(details, PlanErrorDetailsObject)
    assert details.summary == "slow down"
    assert details.reason == "provider_limit"
    assert details.retry_after_ms == 250


def test_plan_result_accepts_plan_error_detail_summary_string() -> None:
    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"fully_managed","values":{},'
        b'"receipts":[],"redactions":[],"charges":[],'
        b'"errors":[{"code":"provider_error","message":"provider failed",'
        b'"receipt":{"kind":"receipt","id":"err_1"},'
        b'"details":"provider returned unavailable"}]}}'
    )

    decoded = decode_response(body, PlanResultDocument)

    assert decoded.errors[0].details == "provider returned unavailable"


def test_plan_result_rejects_unknown_plan_error_code() -> None:
    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"fully_managed","values":{},'
        b'"receipts":[],"redactions":[],"charges":[],'
        b'"errors":[{"code":"made_up","message":"bad",'
        b'"receipt":{"kind":"receipt","id":"err_1"}}]}}'
    )

    with pytest.raises(JsonRpcProtocolError):
        decode_response(body, PlanResultDocument)


def test_plan_result_rejects_plan_error_detail_unknown_or_empty_object() -> None:
    unknown = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"fully_managed","values":{},'
        b'"receipts":[],"redactions":[],"charges":[],'
        b'"errors":[{"code":"provider_error","message":"bad",'
        b'"receipt":{"kind":"receipt","id":"err_1"},'
        b'"details":{"extra":"no"}}]}}'
    )
    empty = unknown.replace(b'{"extra":"no"}', b"{}")

    with pytest.raises(JsonRpcProtocolError):
        decode_response(unknown, PlanResultDocument)
    with pytest.raises(JsonRpcProtocolError):
        decode_response(empty, PlanResultDocument)


def test_plan_result_case_record_uses_case_ref_not_receipt_ref() -> None:
    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"fully_managed",'
        b'"values":{"case":{"kind":"case_record",'
        b'"case":{"kind":"case","run":"run_1","id":"case_1"},'
        b'"receipt":"qrec_case","graph_revision":"rev_final",'
        b'"data_classes":["public"],"replayability":"fully_managed"}},'
        b'"receipts":[],"redactions":[],"charges":[],"errors":[]}}'
    )

    decoded = decode_response(body, PlanResultDocument)
    case_value = decoded.values["case"]

    assert isinstance(case_value.case, CaseRefRecord)
    assert case_value.case.id == "case_1"


def test_plan_result_rejects_receipt_object_as_case_record_ref() -> None:
    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"fully_managed",'
        b'"values":{"case":{"kind":"case_record",'
        b'"case":{"kind":"receipt","id":"qrec_case"},'
        b'"receipt":"qrec_case","graph_revision":"rev_final",'
        b'"data_classes":["public"],"replayability":"fully_managed"}},'
        b'"receipts":[],"redactions":[],"charges":[],"errors":[]}}'
    )

    with pytest.raises(JsonRpcProtocolError):
        decode_response(body, PlanResultDocument)


def test_plan_result_decodes_typed_info_receipt_and_trace_refs() -> None:
    """Scenario: ref leaves keep their schema-owned object identities."""

    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"fully_managed",'
        b'"values":{"evt":{"kind":"emit_run_event","event_id":"event_1",'
        b'"receipt":{"kind":"receipt","id":"wrec_value","fingerprint":"fp_receipt_sha256_value"},'
        b'"graph_revision":"rev_final","data_classes":["public"],'
        b'"replayability":"fully_managed",'
        b'"trace_refs":[{"kind":"agent.trace","id":"trace_1","visibility":"public",'
        b'"receipt":{"kind":"receipt","id":"wrec_trace"}}]}},'
        b'"receipts":[{"kind":"write","receipt":{"kind":"receipt","id":"wrec_1"},'
        b'"status":"succeeded","write_kind":"emit_run_event",'
        b'"source_refs":[{"kind":"external","namespace":"bench","id":"row_1"},'
        b'"cand_string_ref"],'
        b'"trace_refs":[{"kind":"agent.trace","id":"trace_2","visibility":"public"}],'
        b'"result_hash":"fp_result_sha256_test","base_revision":"rev_base",'
        b'"event_id":"event_1"}],'
        b'"redactions":[],"charges":[],"errors":[]}}'
    )

    decoded = decode_response(body, PlanResultDocument)
    value = decoded.values["evt"]
    receipt = decoded.receipts[0]

    assert isinstance(value.receipt, ReceiptRefRecord)
    assert isinstance(receipt.receipt, ReceiptRefRecord)
    assert value.trace_refs is not UNSET
    assert isinstance(value.trace_refs[0], TraceRefRecord)
    assert isinstance(value.trace_refs[0].receipt, ReceiptRefRecord)
    assert receipt.source_refs is not UNSET
    assert isinstance(receipt.source_refs[0], ExternalInfoRefRecord)
    assert receipt.source_refs[1] == "cand_string_ref"
    assert receipt.trace_refs is not UNSET
    assert isinstance(receipt.trace_refs[0], TraceRefRecord)


def test_plan_result_decodes_typed_receipt_preconditions_and_validations() -> None:
    """Example: result receipts keep typed precondition and validation records."""

    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"fully_managed","values":{},'
        b'"receipts":[{"kind":"write","receipt":"wrec_1","status":"succeeded",'
        b'"write_kind":"emit_run_event","request_hash":"fp_request_sha256_test",'
        b'"result_hash":"fp_result_sha256_test","base_revision":"rev_base",'
        b'"event_id":"event_1",'
        b'"preconditions":[{"kind":"candidate_exists",'
        b'"candidate":{"kind":"candidate","id":"cand_1"}}],'
        b'"validation_receipts":[{"receipt":{"kind":"receipt","id":"vrec_1"},'
        b'"status":"passed","schema_fingerprint":"fp_schema_sha256_test"}]}],'
        b'"redactions":[],"charges":[],"errors":[]}}'
    )

    decoded = decode_response(body, PlanResultDocument)
    receipt = decoded.receipts[0]

    assert receipt.preconditions is not UNSET
    assert isinstance(receipt.preconditions[0], PreconditionCandidateExists)
    assert receipt.validation_receipts is not UNSET
    assert isinstance(receipt.validation_receipts[0], ValidationReceipt)
    assert isinstance(receipt.validation_receipts[0].receipt, ReceiptRefRecord)


def test_plan_result_rejects_unknown_receipt_precondition_kind() -> None:
    """Regression: result receipt preconditions are no longer arbitrary objects."""

    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"fully_managed","values":{},'
        b'"receipts":[{"kind":"write","receipt":"wrec_1","status":"succeeded",'
        b'"result_hash":"fp_result_sha256_test",'
        b'"preconditions":[{"kind":"private_condition","candidate":"cand_1"}]}],'
        b'"redactions":[],"charges":[],"errors":[]}}'
    )

    with pytest.raises(JsonRpcProtocolError):
        decode_response(body, PlanResultDocument)


def test_plan_result_rejects_unknown_validation_receipt_field() -> None:
    """Regression: validation receipts have their own closed schema."""

    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"fully_managed","values":{},'
        b'"receipts":[{"kind":"write","receipt":"wrec_1","status":"succeeded",'
        b'"result_hash":"fp_result_sha256_test",'
        b'"validation_receipts":[{"receipt":"vrec_1","status":"passed",'
        b'"unchecked":true}]}],'
        b'"redactions":[],"charges":[],"errors":[]}}'
    )

    with pytest.raises(JsonRpcProtocolError):
        decode_response(body, PlanResultDocument)


def test_plan_result_rejects_arbitrary_info_ref_object() -> None:
    """Regression: object-form InfoRef is a tagged record, not arbitrary JSON."""

    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"fully_managed",'
        b'"values":{},'
        b'"receipts":[{"kind":"write","receipt":"wrec_1","status":"succeeded",'
        b'"write_kind":"emit_run_event","source_refs":[{"kind":"private_row","id":"x"}],'
        b'"result_hash":"fp_result_sha256_test","base_revision":"rev_base",'
        b'"event_id":"event_1"}],'
        b'"redactions":[],"charges":[],"errors":[]}}'
    )

    with pytest.raises(JsonRpcProtocolError):
        decode_response(body, PlanResultDocument)


def test_plan_result_rejects_arbitrary_trace_ref_object() -> None:
    """Regression: trace refs must carry required trace visibility."""

    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"rev_base","final_revision":"rev_final",'
        b'"replayability_summary":"fully_managed",'
        b'"values":{"evt":{"kind":"emit_run_event","event_id":"event_1",'
        b'"receipt":"wrec_value","graph_revision":"rev_final",'
        b'"data_classes":["public"],"replayability":"fully_managed",'
        b'"trace_refs":[{"kind":"agent.trace","id":"trace_1"}]}},'
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


def test_generated_schema_fingerprints_use_public_seam_prefix() -> None:
    assert PLAN_SCHEMA_FINGERPRINT.startswith("fp_schema_sha256_")
    assert PLAN_RESULT_SCHEMA_FINGERPRINT.startswith("fp_schema_sha256_")
