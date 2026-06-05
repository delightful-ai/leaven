"""Output contract helpers — `lv.output.files(...)`, `lv.output.json_schema(...)`, `lv.output.text(...)`.

These build the typed output contracts passed to `cx.sandbox.exec(output=...)`,
`cx.agent.run(output=...)`, etc. They constrain what the engine accepts back
and shape the typed result the user reads.
"""

from collections.abc import Sequence
from typing import Literal, Never, overload

from pydantic import BaseModel, ConfigDict

from .json_value import JsonArray, JsonObject, JsonSchema, JsonValue


class OutputContract(BaseModel):
    """Base for output contracts. Don't construct directly; use the builders below."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: Literal["files", "json_schema", "text"]


class FilesOutput(OutputContract):
    """Capture named files from a sandbox/agent run."""

    kind: Literal["files"] = "files"
    paths: list[str]
    """Workspace-relative paths to capture after the call completes."""
    max_bytes: int | None = None
    """Cap on captured bytes per path."""


class JsonSchemaOutput[ParsedModelT: BaseModel](OutputContract):
    """Enforce a JSON-schema-shaped return value parsed as a pydantic model."""

    kind: Literal["json_schema"] = "json_schema"
    schema_: JsonSchema
    """JSON Schema 2020-12 the response must validate against."""
    parse_to: type[ParsedModelT]
    """Pydantic model class to parse the response into."""

    def parse_json(self, payload: bytes) -> ParsedModelT:
        """Parse provider output JSON into the declared pydantic model."""
        return self.parse_to.model_validate_json(payload)


class JsonSchemaValueOutput(OutputContract):
    """Enforce a JSON-schema-shaped return value parsed as public JSON."""

    kind: Literal["json_schema"] = "json_schema"
    schema_: JsonSchema
    """JSON Schema 2020-12 the response must validate against."""
    parse_to: None = None


class TextOutput(OutputContract):
    """Plain text response with optional length cap."""

    kind: Literal["text"] = "text"
    max_chars: int | None = None


def files(paths: Sequence[str], *, max_bytes: int | None = None) -> FilesOutput:
    """Output contract: capture these workspace-relative paths."""
    return FilesOutput(paths=list(paths), max_bytes=max_bytes)


@overload
def json_schema[ParsedModelT: BaseModel](
    model_or_schema: type[ParsedModelT],
) -> JsonSchemaOutput[ParsedModelT]: ...


@overload
def json_schema(model_or_schema: JsonObject) -> JsonSchemaValueOutput: ...


@overload
def json_schema(model_or_schema: object) -> Never: ...


def json_schema(
    model_or_schema: object,
) -> JsonSchemaOutput[BaseModel] | JsonSchemaValueOutput:
    """Output contract: response must match the given JSON Schema or pydantic model.

    Accepts a pydantic `BaseModel` subclass (extracts its JSON schema) or a
    raw JSON Schema dict. When given a model, the parsed result lifts back
    into an instance of that model.
    """
    if isinstance(model_or_schema, type) and issubclass(model_or_schema, BaseModel):
        return JsonSchemaOutput(
            schema_=_json_object(model_or_schema.model_json_schema()),
            parse_to=model_or_schema,
        )
    if isinstance(model_or_schema, dict):
        return JsonSchemaValueOutput(schema_=_json_object(model_or_schema))
    raise TypeError("expected a pydantic model class or JSON schema object")


def text(*, max_chars: int | None = None) -> TextOutput:
    """Output contract: plain text response with optional length cap."""
    return TextOutput(max_chars=max_chars)


def _json_object(value: object) -> JsonObject:
    if not isinstance(value, dict):
        raise TypeError("JSON value must be an object")
    output: JsonObject = {}
    for key, item in value.items():
        if not isinstance(key, str):
            raise TypeError("JSON object keys must be strings")
        output[key] = _json_value(item)
    return output


def _json_value(value: object) -> JsonValue:
    if value is None or isinstance(value, str | int | float | bool):
        return value
    if isinstance(value, list):
        return _json_array(value)
    if isinstance(value, dict):
        return _json_object(value)
    raise TypeError(f"value is not JSON: {type(value).__name__}")


def _json_array(value: Sequence[object]) -> JsonArray:
    return [_json_value(item) for item in value]


__all__ = [
    "FilesOutput",
    "JsonSchemaOutput",
    "JsonSchemaValueOutput",
    "OutputContract",
    "TextOutput",
    "files",
    "json_schema",
    "text",
]
