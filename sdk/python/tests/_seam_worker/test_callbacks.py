from leaven._seam_worker.callbacks import CallbackReceiptLog


def test_callback_receipts_extract_typed_lm_cost_and_blob_refs() -> None:
    log = CallbackReceiptLog()

    log.record_result(
        method="leaven/lm.complete",
        result={
            "primary": {
                "receipt": "lmrec_main",
                "cost": {"input_tokens": 7, "output_tokens": 3},
                "transcript_ref": {
                    "id": "blob_transcript",
                    "sha256": "abc123",
                    "bytes": 42,
                    "data_classes": ["public", 5],
                },
            },
            "receipts": [{"receipt": "lmrec_main", "call_kind": "lm.complete"}],
        },
    )

    assert log.effect_receipts_json() == [
        {
            "method": "leaven/lm.complete",
            "receipt": "lmrec_main",
            "call_kind": "lm.complete",
            "cost": {"input_tokens": 7, "output_tokens": 3},
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


def test_proposal_receipts_extract_submitted_ids() -> None:
    log = CallbackReceiptLog()

    log.record_result(
        method="leaven/proposal.submit_batch",
        result={
            "primary": {
                "receipt": "proprec_main",
                "proposal_ids": ["proposal_1", 17, "proposal_2"],
            },
            "receipts": [{"receipt": "proprec_main", "write_kind": "proposal.batch"}],
        },
    )

    assert log.proposal_receipts_json() == [
        {
            "method": "leaven/proposal.submit_batch",
            "receipt": "proprec_main",
            "write_kind": "proposal.batch",
            "proposal_ids": ["proposal_1", "proposal_2"],
        }
    ]
