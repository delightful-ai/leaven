from leaven._seam._wire.payloads import BlobRef, Cost
from leaven._seam._wire.results import (
    AgentCommandRecord,
    AgentRunResult,
    AgentSessionPrimary,
    LmCompleteResult,
    LmContentPart,
    LmMessageRecord,
    LmResponsePrimary,
    ProposalBatchPrimary,
    ProposalSubmitResult,
    ResultReceipt,
)
from leaven._seam_worker.callbacks import CallbackReceiptLog


def test_callback_receipts_extract_typed_lm_cost() -> None:
    log = CallbackReceiptLog()

    log.record_result(
        LmCompleteResult(
            method="leaven/lm.complete",
            primary=LmResponsePrimary(
                kind="lm_response",
                message=LmMessageRecord(
                    role="assistant",
                    content=[LmContentPart(kind="text", text="ok")],
                ),
                receipt="lmrec_main",
                graph_revision="rev_callback_lm",
                data_classes=["public"],
                replayability="boundary_managed",
                cost=Cost(input_tokens=7, output_tokens=3),
            ),
            receipts=[
                ResultReceipt(
                    kind="call",
                    receipt="lmrec_main",
                    status="succeeded",
                    result_hash="hash_lm_result",
                    call_kind="lm_complete",
                )
            ],
            redactions=[],
            capability_fingerprint="fp_cap",
            policy_fingerprint="fp_policy",
            data_classes=["public"],
        )
    )

    assert log.effect_receipts_json() == [
        {
            "method": "leaven/lm.complete",
            "receipt": "lmrec_main",
            "call_kind": "lm_complete",
            "cost": {"input_tokens": 7, "output_tokens": 3},
        }
    ]


def test_callback_receipts_extract_typed_agent_blob_refs() -> None:
    log = CallbackReceiptLog()

    log.record_result(
        AgentRunResult(
            method="leaven/agent.run",
            primary=AgentSessionPrimary(
                kind="agent_session",
                status="succeeded",
                receipt="agentrec_main",
                commands=[
                    AgentCommandRecord(
                        argv=["codex", "exec"],
                        status="succeeded",
                        receipt="agentcmdrec_main",
                    )
                ],
                graph_revision="rev_callback_agent",
                data_classes=["public", "transcript.raw"],
                replayability="boundary_managed",
                transcript_ref=BlobRef(
                    id="blob_transcript",
                    sha256="abc123",
                    bytes=42,
                    data_classes=["public"],
                ),
            ),
            receipts=[
                ResultReceipt(
                    kind="call",
                    receipt="agentrec_main",
                    status="succeeded",
                    result_hash="hash_agent_result",
                    call_kind="agent_run",
                )
            ],
            redactions=[],
            capability_fingerprint="fp_cap",
            policy_fingerprint="fp_policy",
            data_classes=["public"],
        )
    )

    assert log.effect_receipts_json() == [
        {
            "method": "leaven/agent.run",
            "receipt": "agentrec_main",
            "call_kind": "agent_run",
            "blob_refs": [
                {
                    "kind": "blob_ref",
                    "id": "blob_transcript",
                    "sha256": "abc123",
                    "bytes": 42,
                    "data_classes": ["public"],
                }
            ],
        }
    ]


def test_proposal_receipts_extract_typed_submitted_ids() -> None:
    log = CallbackReceiptLog()

    log.record_result(
        ProposalSubmitResult(
            method="leaven/proposal.submit_batch",
            primary=ProposalBatchPrimary(
                kind="proposal_batch_receipt",
                batch_id="batch_main",
                proposal_ids=["proposal_1", "proposal_2"],
                status="submitted",
                graph_revision="rev_callback_proposal",
                data_classes=["public"],
                replayability="fully_managed",
                receipt="proprec_main",
            ),
            receipts=[
                ResultReceipt(
                    kind="write",
                    receipt="proprec_main",
                    status="succeeded",
                    result_hash="hash_proposal_result",
                    write_kind="submit_proposal_batch",
                )
            ],
            redactions=[],
            capability_fingerprint="fp_cap",
            policy_fingerprint="fp_policy",
            data_classes=["public"],
        )
    )

    assert log.proposal_receipts_json() == [
        {
            "method": "leaven/proposal.submit_batch",
            "receipt": "proprec_main",
            "write_kind": "submit_proposal_batch",
            "proposal_ids": ["proposal_1", "proposal_2"],
        }
    ]
