"""Tests for generated public-seam stage payload records."""

import msgspec
import pytest
from msgspec import UNSET

from leaven._seam._wire.payloads import (
    STAGE_RUN_SCHEMA_FINGERPRINT,
    CandidateRefRecord,
    CaseRefRecord,
    ProposeRequest,
    RunnerRequest,
    StageRunRequest,
    StageRunResult,
)


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


def test_stage_run_request_decodes_typed_runner_refs() -> None:
    body = (
        b'{"schema_version":"leaven.stage_run.v1","message":"stage_run_request",'
        b'"stage":"runner","payload":{'
        b'"schema_version":"leaven.stage_payloads.v1","role":"runner",'
        b'"run":"run_1","stage_call_id":"sc_1",'
        b'"candidate":{"kind":"candidate","run":"run_1","id":"cand_1"},'
        b'"case":{"kind":"case","run":"run_1","id":"case_1"},'
        b'"case_input":{"prompt":"hello"},"target_forbidden":true}}'
    )

    decoded = msgspec.json.decode(body, type=StageRunRequest)

    assert isinstance(decoded.payload, RunnerRequest)
    assert isinstance(decoded.payload.candidate, CandidateRefRecord)
    assert isinstance(decoded.payload.case, CaseRefRecord)


def test_stage_run_request_decodes_typed_proposer_parent_ref() -> None:
    body = (
        b'{"schema_version":"leaven.stage_run.v1","message":"stage_run_request",'
        b'"stage":"proposer","payload":{'
        b'"schema_version":"leaven.stage_payloads.v1","role":"proposer",'
        b'"run":"run_1","stage_call_id":"sc_1","base_revision":"rev_base",'
        b'"parent":{"kind":"candidate","run":"run_1","id":"cand_parent"},'
        b'"reflection_result":{"schema_version":"leaven.stage_payloads.v1",'
        b'"role":"reflection_result","summary":"try concise answer",'
        b'"source_refs":["cand_parent"],"read_receipts":["qrec_1"],'
        b'"data_classes":["optimizer.visible"]},'
        b'"allowed_effects":["change"],"capability_fingerprint":"fp_cap_sha256_test"}}'
    )

    decoded = msgspec.json.decode(body, type=StageRunRequest)

    assert isinstance(decoded.payload, ProposeRequest)
    assert isinstance(decoded.payload.parent, CandidateRefRecord)


def test_stage_run_request_rejects_arbitrary_runner_ref_objects() -> None:
    body = (
        b'{"schema_version":"leaven.stage_run.v1","message":"stage_run_request",'
        b'"stage":"runner","payload":{'
        b'"schema_version":"leaven.stage_payloads.v1","role":"runner",'
        b'"run":"run_1","stage_call_id":"sc_1",'
        b'"candidate":{"kind":"workspace","id":"ws_1"},'
        b'"case":{"kind":"case","id":"case_1"},'
        b'"case_input":{"prompt":"hello"},"target_forbidden":true}}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(body, type=StageRunRequest)


def test_stage_run_request_rejects_non_object_case_input() -> None:
    body = (
        b'{"schema_version":"leaven.stage_run.v1","message":"stage_run_request",'
        b'"stage":"runner","payload":{'
        b'"schema_version":"leaven.stage_payloads.v1","role":"runner",'
        b'"run":"run_1","stage_call_id":"sc_1",'
        b'"candidate":"cand_1","case":"case_1",'
        b'"case_input":["prompt","hello"],"target_forbidden":true}}'
    )

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(body, type=StageRunRequest)


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


def test_generated_stage_schema_fingerprint_uses_public_seam_prefix() -> None:
    assert STAGE_RUN_SCHEMA_FINGERPRINT.startswith("fp_schema_sha256_")
