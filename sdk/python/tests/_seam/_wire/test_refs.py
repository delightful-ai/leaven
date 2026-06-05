"""Tests for `leaven._seam._wire.refs`."""

import msgspec
import pytest

from leaven._seam._wire.refs import BlobRef, CaseInputPayload, MetadataBag, WireJsonCaseReadTarget


def test_blob_ref_decodes_schema_owned_reference() -> None:
    """Example: generated reference records decode with msgspec validation."""

    decoded = msgspec.json.decode(
        b'{"kind":"blob_ref","id":"blob_1","sha256":"abc","bytes":3,"data_classes":["public"]}',
        type=BlobRef,
    )

    assert decoded.id == "blob_1"
    assert decoded.bytes == 3
    assert decoded.data_classes == ["public"]


def test_case_read_target_alias_accepts_json_scalar() -> None:
    """Regression: case target owner follows the schema's JSON-value slot."""

    decoded = msgspec.json.decode(b'"answer"', type=WireJsonCaseReadTarget)

    assert decoded == "answer"


def test_case_input_payload_is_a_branded_stage_input_owner() -> None:
    """Regression: runner case input is exposed through a branded owner."""

    decoded = msgspec.json.decode(
        b'{"question":"2+2?","context":{"difficulty":"easy"},"choices":["3","4"]}',
        type=CaseInputPayload,
    )

    assert decoded == {
        "question": "2+2?",
        "context": {"difficulty": "easy"},
        "choices": ["3", "4"],
    }


def test_metadata_bag_is_a_branded_recursive_json_owner() -> None:
    """Regression: metadata is not the shallow `WireJsonObject` carrier."""

    decoded = msgspec.json.decode(
        b'{"source":"client-test","labels":["train",{"fold":1}],"nested":{"ok":true}}',
        type=MetadataBag,
    )

    assert decoded == {
        "source": "client-test",
        "labels": ["train", {"fold": 1}],
        "nested": {"ok": True},
    }


def test_metadata_bag_rejects_non_json_values() -> None:
    """Boundary check: metadata uses the bounded recursive JSON owner."""

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(
            b'{"too_deep":[[[[[[[[[0]]]]]]]]]}',
            type=MetadataBag,
        )
