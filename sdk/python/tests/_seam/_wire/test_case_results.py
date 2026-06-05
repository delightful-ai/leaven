"""Tests for `leaven._seam._wire.case_results`."""

import json

import msgspec

from leaven._seam._wire.case_results import CaseRecordPrimary
from leaven._seam._wire.payloads import CaseRefRecord
from leaven._seam._wire.refs import CaseReadInputValue, CaseReadMetadataValue, CaseReadTargetValue


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


def test_case_read_value_owners_decode_json_values() -> None:
    """Regression: case read values use branded owners, not generic wire aliases."""

    input_value = msgspec.json.decode(
        b'{"question":"2+2?","choices":["3","4"]}',
        type=CaseReadInputValue,
    )
    target_value = msgspec.json.decode(b'"4"', type=CaseReadTargetValue)
    metadata_value = msgspec.json.decode(
        b'{"partition":["validation",1]}',
        type=CaseReadMetadataValue,
    )

    assert input_value == {"question": "2+2?", "choices": ["3", "4"]}
    assert target_value == "4"
    assert metadata_value == {"partition": ["validation", 1]}
