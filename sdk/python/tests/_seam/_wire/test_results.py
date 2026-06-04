"""Tests for generated public-seam method result records."""

import json

import msgspec
import pytest

from leaven._seam._wire.methods import LOCKED_METHODS
from leaven._seam._wire.results import (
    METHOD_RESULT_BINDINGS,
    AgentRunResult,
    CaseLoadResult,
    LmCompleteResult,
    ResultReceipt,
)


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
    assert "leaven/human.review" not in bindings


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
