"""Tests for generated public-seam method result records."""

import json

import msgspec
import pytest

from leaven._seam._wire.methods import LOCKED_METHODS
from leaven._seam._wire.payloads import CaseRefRecord, EventSummaryGraphRow
from leaven._seam._wire.results import (
    METHOD_RESULT_BINDINGS,
    AgentRunResult,
    AssessmentSubmitResult,
    CaseLoadResult,
    EvaluationRequestResult,
    EventEmitResult,
    GraphQueryResult,
    LmCompleteResult,
    ProposalApplyResult,
    ResultReceipt,
    SandboxExecResult,
    WorkspaceCaptureArtifactsResult,
    WorkspaceDigestResult,
    WorkspaceGitDiffResult,
    WorkspaceGitLogResult,
    WorkspaceGitStatusResult,
    WorkspaceListResult,
    WorkspaceMaterializeResult,
    WorkspaceReadFileResult,
    WorkspaceReleaseResult,
    WorkspaceSnapshotResult,
    WorkspaceStatResult,
    decode_agent_parsed,
    decode_lm_parsed,
)


class ParsedAnswer(msgspec.Struct, frozen=True):
    """Typed structured-output fixture."""

    answer: str
    scores: list[int]


class ParsedMutation(msgspec.Struct, frozen=True):
    """Typed agent structured-output fixture."""

    mutation: dict[str, str | bool]


def test_result_bindings_cover_every_locked_method() -> None:
    """Scenario: Rust-exported method result facts cover the locked method set."""

    assert [binding.method for binding in METHOD_RESULT_BINDINGS] == list(LOCKED_METHODS)


def test_result_bindings_name_primary_and_receipt_facts() -> None:
    """Example: generated bindings expose Rust-owned primary/receipt method facts."""

    bindings = {binding.method: binding for binding in METHOD_RESULT_BINDINGS}

    assert bindings["leaven/agent.run"].primary_kinds == ("agent_session",)
    assert bindings["leaven/agent.run"].receipt_kind == "call"
    assert bindings["leaven/agent.run"].call_kind == "agent_run"
    assert bindings["leaven/proposal.apply"].receipt_kind == "write"
    assert bindings["leaven/proposal.apply"].write_kind == "apply_proposal_batch"


def test_generated_lm_result_decodes_msgspec_payload() -> None:
    """Example: LM extension results decode into generated method-specific records."""

    payload = {
        "method": "leaven/lm.complete",
        "primary": {
            "kind": "lm_response",
            "message": {
                "role": "assistant",
                "content": [{"kind": "text", "text": "ok"}],
            },
            "receipt": "lmrec_1",
            "graph_revision": "rev_lm",
            "data_classes": ["public"],
            "replayability": "boundary_managed",
            "cost": {"usd_micro": 10, "input_tokens": 1, "output_tokens": 2},
        },
        "receipts": [
            {
                "kind": "call",
                "receipt": "lmrec_1",
                "status": "succeeded",
                "result_hash": "fp_result_lm",
                "call_kind": "lm_complete",
            }
        ],
        "redactions": [],
        "capability_fingerprint": "fp_cap",
        "policy_fingerprint": "fp_policy",
        "data_classes": ["public"],
    }

    decoded = msgspec.json.decode(json.dumps(payload).encode(), type=LmCompleteResult)

    assert decoded.primary.message.content[0].text == "ok"
    assert decoded.receipts == [
        ResultReceipt(
            kind="call",
            receipt="lmrec_1",
            status="succeeded",
            result_hash="fp_result_lm",
            call_kind="lm_complete",
        )
    ]


def test_generated_lm_result_decodes_schema_dependent_parsed_payload() -> None:
    """Regression: structured-output payloads require explicit typed decode."""

    payload = _extension_result(
        "leaven/lm.complete",
        {
            "kind": "lm_response",
            "message": {
                "role": "assistant",
                "content": [{"kind": "text", "text": '{"answer":"ok"}'}],
            },
            "receipt": "lmrec_1",
            "graph_revision": "rev_lm",
            "data_classes": ["public"],
            "replayability": "boundary_managed",
            "parsed": {"answer": "ok", "scores": [1, 2, 3]},
        },
    )

    decoded = msgspec.json.decode(json.dumps(payload).encode(), type=LmCompleteResult)

    assert isinstance(decoded.primary.parsed, msgspec.Raw)
    assert decode_lm_parsed(decoded, ParsedAnswer) == ParsedAnswer(
        answer="ok",
        scores=[1, 2, 3],
    )


def test_generated_agent_result_decodes_schema_dependent_parsed_payload() -> None:
    """Example: agent parsed payloads decode with the caller's schema type."""

    payload = _extension_result(
        "leaven/agent.run",
        {
            "kind": "agent_session",
            "status": "succeeded",
            "receipt": "agentrec_1",
            "graph_revision": "rev_agent",
            "commands": [],
            "data_classes": ["public"],
            "replayability": "boundary_managed",
            "parsed": {"mutation": {"skill": "alpha", "changed": True}},
        },
    )

    decoded = msgspec.json.decode(json.dumps(payload).encode(), type=AgentRunResult)

    assert isinstance(decoded.primary.parsed, msgspec.Raw)
    assert decode_agent_parsed(decoded, ParsedMutation) == ParsedMutation(
        mutation={"skill": "alpha", "changed": True}
    )


def test_generated_lm_parsed_helper_rejects_missing_payload() -> None:
    """Regression: callers cannot silently treat omitted parsed payload as empty."""

    payload = _extension_result(
        "leaven/lm.complete",
        {
            "kind": "lm_response",
            "message": {
                "role": "assistant",
                "content": [{"kind": "text", "text": "plain"}],
            },
            "receipt": "lmrec_1",
            "graph_revision": "rev_lm",
            "data_classes": ["public"],
            "replayability": "boundary_managed",
        },
    )
    decoded = msgspec.json.decode(json.dumps(payload).encode(), type=LmCompleteResult)

    with pytest.raises(ValueError, match="did not include parsed payload"):
        decode_lm_parsed(decoded, ParsedAnswer)


def test_generated_agent_result_rejects_wrong_primary_kind() -> None:
    """Regression: method-specific generated results reject mismatched primaries."""

    payload = {
        "method": "leaven/agent.run",
        "primary": {
            "kind": "lm_response",
            "message": {
                "role": "assistant",
                "content": [{"kind": "text", "text": "wrong"}],
            },
            "receipt": "lmrec_wrong",
        },
        "receipts": [],
        "redactions": [],
        "capability_fingerprint": "fp_cap",
        "policy_fingerprint": "fp_policy",
        "data_classes": ["public"],
    }

    with pytest.raises(msgspec.ValidationError, match="Invalid enum value"):
        msgspec.json.decode(json.dumps(payload).encode(), type=AgentRunResult)


def test_generated_case_result_accepts_locked_case_methods() -> None:
    """Example: the generated case result type covers every narrow case route."""

    base = {
        "primary": {
            "kind": "case_record",
            "case": "case_1",
            "receipt": "qrec_case",
            "data_classes": ["public"],
            "replayability": "fully_managed",
            "input": {"question": "2+2?"},
        },
        "receipts": [],
        "redactions": [],
        "capability_fingerprint": "fp_cap",
        "policy_fingerprint": "fp_policy",
        "data_classes": ["public"],
    }

    for method in (
        "leaven/case.load",
        "leaven/case.input",
        "leaven/case.target",
        "leaven/case.metadata",
    ):
        payload = {**base, "method": method}
        decoded = msgspec.json.decode(json.dumps(payload).encode(), type=CaseLoadResult)
        assert decoded.method == method


def test_generated_case_result_decodes_case_ref_object() -> None:
    """Example: case_record primary carries a schema-owned CaseRef."""

    payload = _extension_result(
        "leaven/case.load",
        {
            "kind": "case_record",
            "case": {"kind": "case", "run": "run_1", "id": "case_1"},
            "receipt": "qrec_case",
            "data_classes": ["public"],
            "replayability": "fully_managed",
            "input": {"question": "2+2?"},
        },
    )

    decoded = msgspec.json.decode(json.dumps(payload).encode(), type=CaseLoadResult)

    assert isinstance(decoded.primary.case, CaseRefRecord)
    assert decoded.primary.case.id == "case_1"


def test_generated_case_result_rejects_receipt_object_as_case_ref() -> None:
    """Regression: case_record.case is a CaseRef, not a ReceiptRef."""

    payload = _extension_result(
        "leaven/case.load",
        {
            "kind": "case_record",
            "case": {"kind": "receipt", "id": "qrec_case"},
            "receipt": "qrec_case",
            "data_classes": ["public"],
            "replayability": "fully_managed",
        },
    )

    with pytest.raises(msgspec.ValidationError, match="Invalid value 'receipt'"):
        msgspec.json.decode(json.dumps(payload).encode(), type=CaseLoadResult)


def test_generated_result_records_decode_remaining_locked_method_families() -> None:
    """Scenario: every retained non-case extension family has a typed result target."""

    cases = [
        ("leaven/graph.query", GraphQueryResult, _graph_set_primary()),
        ("leaven/workspace.materialize", WorkspaceMaterializeResult, _workspace_handle_primary()),
        ("leaven/workspace.snapshot", WorkspaceSnapshotResult, _workspace_snapshot_primary()),
        ("leaven/workspace.list", WorkspaceListResult, _workspace_listing_primary()),
        ("leaven/workspace.read_file", WorkspaceReadFileResult, _workspace_file_primary()),
        ("leaven/workspace.stat", WorkspaceStatResult, _workspace_listing_primary()),
        ("leaven/workspace.digest", WorkspaceDigestResult, _workspace_snapshot_primary()),
        ("leaven/workspace.git_log", WorkspaceGitLogResult, _workspace_diff_primary()),
        ("leaven/workspace.git_diff", WorkspaceGitDiffResult, _workspace_diff_primary()),
        ("leaven/workspace.git_status", WorkspaceGitStatusResult, _workspace_diff_primary()),
        ("leaven/workspace.capture_artifacts", WorkspaceCaptureArtifactsResult, _workspace_listing_primary()),
        ("leaven/workspace.release", WorkspaceReleaseResult, _released_workspace_handle_primary()),
        ("leaven/sandbox.exec", SandboxExecResult, _sandbox_exec_primary()),
        ("leaven/proposal.apply", ProposalApplyResult, _apply_receipt_primary()),
        ("leaven/assessment.submit", AssessmentSubmitResult, _assessment_batch_primary()),
        ("leaven/evaluation.request", EvaluationRequestResult, _evaluation_request_primary()),
        ("leaven/event.emit", EventEmitResult, _emit_run_event_primary()),
    ]

    for method, result_type, primary in cases:
        decoded = msgspec.json.decode(
            json.dumps(_extension_result(method, primary)).encode(),
            type=result_type,
        )
        assert decoded.method == method
        assert decoded.primary.kind == primary["kind"]


def test_generated_graph_query_result_decodes_typed_rows() -> None:
    """Example: graph.query result rows keep their tagged shape in Python."""

    decoded = msgspec.json.decode(
        json.dumps(_extension_result("leaven/graph.query", _graph_set_primary())).encode(),
        type=GraphQueryResult,
    )

    assert isinstance(decoded.primary.items[0], EventSummaryGraphRow)
    assert decoded.primary.items[0].event_kind == "case.loaded"


def test_generated_workspace_result_rejects_wrong_primary_kind() -> None:
    """Regression: workspace method result classes bind the primary kind too."""

    payload = _extension_result("leaven/workspace.read_file", _workspace_listing_primary())

    with pytest.raises(msgspec.ValidationError, match="Invalid enum value"):
        msgspec.json.decode(json.dumps(payload).encode(), type=WorkspaceReadFileResult)


def _extension_result(method: str, primary: dict[str, object]) -> dict[str, object]:
    return {
        "method": method,
        "primary": primary,
        "receipts": [
            {
                "kind": _receipt_kind(method),
                "receipt": _primary_receipt(primary),
                "status": "succeeded",
                "result_hash": "fp_result",
            }
        ],
        "redactions": [],
        "capability_fingerprint": "fp_cap",
        "policy_fingerprint": "fp_policy",
        "data_classes": ["public"],
    }


def _primary_receipt(primary: dict[str, object]) -> object:
    if "receipt" not in primary:
        return "qrec_result"
    return primary["receipt"]


def _receipt_kind(method: str) -> str:
    if method in {
        "leaven/workspace.materialize",
        "leaven/workspace.release",
        "leaven/sandbox.exec",
    }:
        return "call"
    if method.startswith(("leaven/proposal.", "leaven/assessment.", "leaven/evaluation.", "leaven/event.")):
        return "write"
    return "query"


def _graph_set_primary() -> dict[str, object]:
    return {
        "kind": "graph_set",
        "items": [{"kind": "event_summary", "event_kind": "case.loaded", "revision": "rev"}],
        "graph_revision": "rev",
        "data_classes": ["public"],
        "replayability": "pure_read",
        "receipt": "qrec_graph",
    }


def _workspace_handle_primary() -> dict[str, object]:
    return {
        "kind": "workspace_handle",
        "workspace": "ws",
        "lifetime": "stage_call",
        "released": False,
        "graph_revision": "rev",
        "data_classes": ["workspace.file"],
        "replayability": "fully_managed",
        "receipt": "wrec_workspace",
    }


def _released_workspace_handle_primary() -> dict[str, object]:
    primary = _workspace_handle_primary()
    primary["released"] = True
    primary["receipt"] = "wrec_release"
    return primary


def _workspace_snapshot_primary() -> dict[str, object]:
    return {
        "kind": "workspace_snapshot",
        "workspace": "ws",
        "digest": "sha256:workspace",
        "graph_revision": "rev",
        "data_classes": ["workspace.file"],
        "replayability": "pure_read",
    }


def _workspace_listing_primary() -> dict[str, object]:
    return {
        "kind": "workspace_listing",
        "entries": [{"path": "src/lib.rs", "kind": "file", "data_classes": ["workspace.file"]}],
        "graph_revision": "rev",
        "data_classes": ["workspace.file"],
        "replayability": "pure_read",
    }


def _workspace_file_primary() -> dict[str, object]:
    return {
        "kind": "workspace_file",
        "path": "src/lib.rs",
        "content": "pub fn demo() {}",
        "graph_revision": "rev",
        "data_classes": ["workspace.file"],
        "replayability": "pure_read",
        "receipt": "qrec_workspace_file",
    }


def _workspace_diff_primary() -> dict[str, object]:
    return {
        "kind": "workspace_diff",
        "text": " M src/lib.rs",
        "graph_revision": "rev",
        "data_classes": ["workspace.file"],
        "replayability": "pure_read",
    }


def _sandbox_exec_primary() -> dict[str, object]:
    blob = {
        "kind": "blob_ref",
        "id": "blob",
        "sha256": "a" * 64,
        "bytes": 12,
        "data_classes": ["public"],
    }
    return {
        "kind": "sandbox_exec",
        "status": "completed",
        "exit_code": 0,
        "cost": {"usd_micro": 10, "sandbox_calls": 1},
        "stdout_ref": blob,
        "stderr_ref": blob,
        "graph_revision": "rev",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "execrec",
    }


def _apply_receipt_primary() -> dict[str, object]:
    return {
        "kind": "apply_receipt",
        "created_candidates": ["cand_created"],
        "status": "committed",
        "graph_revision": "rev",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "wrec_apply",
    }


def _assessment_batch_primary() -> dict[str, object]:
    return {
        "kind": "assessment_batch_receipt",
        "assessment_ids": ["assess_1"],
        "evaluation_request_id": "evalreq_1",
        "per_assessment": [{"assessment": "assess_1", "replayability": "fully_managed"}],
        "status": "committed",
        "graph_revision": "rev",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "wrec_assessment",
    }


def _evaluation_request_primary() -> dict[str, object]:
    return {
        "kind": "evaluation_request_receipt",
        "evaluation_request_id": "evalreq_1",
        "status": "recorded",
        "graph_revision": "rev",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "wrec_evaluation",
    }


def _emit_run_event_primary() -> dict[str, object]:
    return {
        "kind": "emit_run_event",
        "event_id": "event_1",
        "receipt": "wrec_event",
        "data_classes": ["public"],
        "replayability": "fully_managed",
    }
