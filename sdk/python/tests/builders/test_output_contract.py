"""Tests for `leaven.builders._output_contract`."""

import leaven as lv
from leaven.builders._output_contract import schema_fingerprint


def test_schema_fingerprint_uses_public_seam_jcs_sha256_shape() -> None:
    """Example: Python computes the same public-seam inline-schema fingerprint shape."""

    schema = lv.output.json_schema(
        {
            "type": "object",
            "properties": {
                "answer": {
                    "type": "string",
                },
            },
            "required": ["answer"],
            "additionalProperties": False,
        }
    ).schema_

    assert schema_fingerprint(schema) == (
        "fp_schema_sha256_"
        "d7f69ea25824f613d0b60198abe050adc66a3bf45d9f2045d1997214a55498e5"
    )

    pretty_reordered = lv.output.json_schema(
        {
            "required": ["answer"],
            "properties": {"answer": {"type": "string"}},
            "additionalProperties": False,
            "type": "object",
        }
    )
    assert schema_fingerprint(pretty_reordered.schema_) == schema_fingerprint(schema)
