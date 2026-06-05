"""Tests for `leaven._seam._wire.case_results`."""

import json

import msgspec

from leaven._seam._wire.case_results import CaseRecordPrimary
from leaven._seam._wire.payloads import CaseRefRecord


def test_case_record_primary_decodes_owned_case_read_values() -> None:
    """Regression: case read values are owned JSON fields, not object-only bags."""

    payload = {
        "kind": "case_record",
        "case": {"kind": "case", "run": "run_1", "id": "case_1"},
        "receipt": "qrec_case",
        "data_classes": ["case.input", "case.target", "case.metadata"],
        "replayability": "fully_managed",
        "input": ["question", {"text": "2+2?"}],
        "target": "4",
        "metadata": {"partition": ["validation", 1]},
    }

    decoded = msgspec.json.decode(json.dumps(payload).encode(), type=CaseRecordPrimary)

    assert isinstance(decoded.case, CaseRefRecord)
    assert decoded.input == ["question", {"text": "2+2?"}]
    assert decoded.target == "4"
    assert decoded.metadata == {"partition": ["validation", 1]}
