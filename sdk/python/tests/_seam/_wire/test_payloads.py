"""Tests for generated top-level public-seam payload records."""

import msgspec
import pytest
from msgspec import UNSET

from leaven._seam._wire import JsonRpcProtocolError, decode_response
from leaven._seam._wire.payloads import (
    PLAN_RESULT_SCHEMA_FINGERPRINT,
    PLAN_SCHEMA_FINGERPRINT,
    STAGE_RUN_SCHEMA_FINGERPRINT,
    CommitPolicyNoGraphWrites,
    ConsistencyLatestAtStart,
    EvalModeExecute,
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
