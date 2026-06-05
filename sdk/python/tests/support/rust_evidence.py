"""Shared Rust evidence export fixtures."""

import json


def rust_case_assessment_evidence_bytes() -> bytes:
    """Return a Rust case-assessment evidence export with effect blob refs."""
    return json.dumps(
        {
            "score": {"score": 0.75},
            "output": {
                "Inline": {
                    "text": "42",
                    "truncated": False,
                    "metadata": {
                        "visibility": "public",
                        "data_classes": ["candidate.output", "public"],
                    },
                }
            },
            "feedback": "exact match",
            "trace": [],
            "case_data_reads": [
                {
                    "operation": "case_query.load",
                    "receipt": "qrec_case_1",
                    "case": 1,
                    "fields": ["input", "target"],
                    "data_classes": ["case.input", "case.target"],
                    "values": {
                        "case_id": "1",
                        "target": {"answer": "42"},
                        "effect_receipts": [
                            {
                                "receipt_id": "lmrec_completion",
                                "blob_refs": [
                                    {
                                        "blob_id": "blob_transcript",
                                        "sha256": "abc",
                                        "bytes": 128,
                                        "data_classes": ["transcript.raw"],
                                    }
                                ],
                            }
                        ],
                    },
                }
            ],
        }
    ).encode()


__all__ = ["rust_case_assessment_evidence_bytes"]
