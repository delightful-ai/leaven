"""Tests for generated top-level public-seam payload records."""

import msgspec
import pytest

from leaven._seam._wire import JsonRpcProtocolError, decode_response
from leaven._seam._wire.payloads import (
    PLAN_RESULT_SCHEMA_FINGERPRINT,
    PLAN_SCHEMA_FINGERPRINT,
    STAGE_RUN_SCHEMA_FINGERPRINT,
    PlanDocument,
    PlanResultDocument,
    StageRunRequest,
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
    assert bytes(decoded.return_) == b"[]"


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


def test_plan_result_decodes_as_method_specific_json_rpc_result() -> None:
    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{'
        b'"schema_version":"leaven.plan_result.v1","plan_id":"plan_1",'
        b'"capability_fingerprint":"fp_cap_sha256_test",'
        b'"policy_fingerprint":"fp_policy_sha256_test",'
        b'"base_revision":"run_base","final_revision":"run_final",'
        b'"replayability_summary":{"kind":"deterministic"},'
        b'"values":{},"receipts":[],"redactions":[],"charges":[],"errors":[]}}'
    )

    decoded = decode_response(body, PlanResultDocument)

    assert decoded.schema_version == "leaven.plan_result.v1"
    assert decoded.final_revision == "run_final"
    assert bytes(decoded.receipts) == b"[]"


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
        b'"stage":"runner","payload":{"role":"runner"}}'
    )

    decoded = msgspec.json.decode(body, type=StageRunRequest)

    assert decoded.stage == "runner"
    assert bytes(decoded.payload) == b'{"role":"runner"}'


def test_generated_schema_fingerprints_use_public_seam_prefix() -> None:
    assert PLAN_SCHEMA_FINGERPRINT.startswith("fp_schema_sha256_")
    assert PLAN_RESULT_SCHEMA_FINGERPRINT.startswith("fp_schema_sha256_")
    assert STAGE_RUN_SCHEMA_FINGERPRINT.startswith("fp_schema_sha256_")
