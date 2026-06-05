"""Tests for `leaven._seam._wire.refs`."""

import msgspec

from leaven._seam._wire.refs import BlobRef, WireJsonCaseReadTarget


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
