import pytest
from pydantic import BaseModel

import leaven as lv


class StructuredAnswer(BaseModel):
    answer: str
    confidence: float


def test_json_schema_output_from_pydantic_model() -> None:
    output = lv.output.json_schema(StructuredAnswer)

    assert output.kind == "json_schema"
    assert output.parse_to is StructuredAnswer
    assert output.schema_["type"] == "object"
    properties = output.schema_["properties"]
    assert isinstance(properties, dict)
    answer = properties["answer"]
    assert isinstance(answer, dict)
    assert answer["type"] == "string"


def test_json_schema_output_from_raw_schema() -> None:
    schema = {"type": "object", "properties": {"answer": {"type": "string"}}}

    output = lv.output.json_schema(schema)

    assert output.parse_to is None
    assert output.schema_ == schema


def test_json_schema_output_rejects_non_json_schema_values() -> None:
    with pytest.raises(TypeError, match="JSON object keys must be strings"):
        lv.output.json_schema({"properties": {1: {"type": "string"}}})

    with pytest.raises(TypeError, match="expected a pydantic model class or JSON schema object"):
        lv.output.json_schema("not a schema")
