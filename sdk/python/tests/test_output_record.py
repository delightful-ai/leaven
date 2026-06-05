from typing import cast

import pytest

import leaven as lv
from leaven.json_value import JsonObject, JsonValue
from leaven.output_record import OutputRecord


def test_text_output_record_uses_summary_as_inline_value() -> None:
    record = OutputRecord.text(summary="hello")

    assert record.kind == "text"
    assert record.summary == "hello"
    assert record.value == "hello"
    assert record.blob_ref is None
    assert record.data_classes == [lv.data_class.PUBLIC]


def test_structured_output_record_preserves_json_object() -> None:
    record = OutputRecord.structured(
        summary="judge output",
        value={"score": 1.0, "reason": "correct"},
        data_classes=[lv.data_class.OPTIMIZER_VISIBLE],
    )

    assert record.kind == "structured"
    assert record.value == {"score": 1.0, "reason": "correct"}
    assert record.data_classes == [lv.data_class.OPTIMIZER_VISIBLE]


def test_structured_output_record_rejects_non_json_object_value() -> None:
    with pytest.raises(TypeError, match="output record object must be a JSON object"):
        OutputRecord.structured(
            summary="bad",
            value=cast("JsonObject", ["not", "object"]),
        )


def test_json_value_output_record_rejects_non_json() -> None:
    record = OutputRecord.json_value(summary="numbers", value=[1, 2, {"ok": True}])
    assert record.kind == "json"
    assert record.value == [1, 2, {"ok": True}]

    with pytest.raises(TypeError, match="output record value contains non-JSON value"):
        OutputRecord.json_value(summary="bad", value=cast("JsonValue", object()))


def test_blob_output_record_carries_only_reference() -> None:
    record = OutputRecord.blob(summary="transcript", blob_ref="blob_123")

    assert record.kind == "blob_ref"
    assert record.value is None
    assert record.blob_ref == "blob_123"
