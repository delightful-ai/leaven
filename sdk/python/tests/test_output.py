from typing import cast

import pytest
from pydantic import BaseModel

import leaven as lv
from leaven.json_value import JsonObject


class StructuredAnswer(BaseModel):
    answer: str
    confidence: float


def test_json_schema_output_from_pydantic_model() -> None:
    contract = lv.output.json_schema(StructuredAnswer)

    assert contract.kind == "json_schema"
    assert contract.parse_to is StructuredAnswer
    assert contract.schema_["type"] == "object"
    properties = contract.schema_["properties"]
    assert isinstance(properties, dict)
    answer = properties["answer"]
    assert isinstance(answer, dict)
    assert answer["type"] == "string"


def test_json_schema_output_from_raw_schema() -> None:
    schema: JsonObject = {"type": "object", "properties": {"answer": {"type": "string"}}}

    contract = lv.output.json_schema(schema)

    assert isinstance(contract, lv.output.JsonSchemaValueOutput)
    assert contract.parse_to is None
    assert contract.schema_ == schema


def test_json_schema_output_rejects_non_json_schema_values() -> None:
    bad_key_schema = cast("JsonObject", {"properties": {1: {"type": "string"}}})
    with pytest.raises(TypeError, match="JSON object keys must be strings"):
        lv.output.json_schema(bad_key_schema)

    not_a_schema = cast("JsonObject", "not a schema")
    with pytest.raises(TypeError, match="expected a pydantic model class or JSON schema object"):
        lv.output.json_schema(not_a_schema)
