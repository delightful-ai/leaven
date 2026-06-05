"""OutputRecord — typed output projection for assessments and stage results.

Carries visibility, data classes, and either inline value or a blob ref.
Constructors are explicit: `text(...)`, `json_value(...)`, `blob(...)`,
`structured(...)`.
"""

from collections.abc import Sequence
from typing import Literal

from pydantic import BaseModel, ConfigDict, Field

from .data_class import PUBLIC
from .json_value import JsonObject, JsonValue

Visibility = Literal[
    "public",
    "optimizer_visible",
    "reflector_visible",
    "evaluator_only",
    "operator_only",
    "private",
    "redacted",
]
OutputKind = Literal["text", "json", "blob_ref", "structured", "agent_session", "workspace_diff"]


class OutputRecord(BaseModel):
    """A visibility-labeled output projection. Build via classmethods."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: OutputKind
    visibility: Visibility = "public"
    data_classes: list[str] = Field(default_factory=lambda: [PUBLIC])
    summary: str | None = None
    value: JsonValue | None = None
    """Inline value for text/json/structured outputs; None for blob refs."""
    blob_ref: str | None = None
    """Opaque blob reference for blob-backed outputs; None for inline."""

    @classmethod
    def text(
        cls,
        *,
        summary: str,
        visibility: Visibility = "public",
        data_classes: list[str] | None = None,
    ) -> "OutputRecord":
        """Inline text output. Summary IS the content."""
        return cls(
            kind="text",
            visibility=visibility,
            data_classes=_data_classes(data_classes),
            summary=summary,
            value=summary,
        )

    @classmethod
    def structured(
        cls,
        *,
        summary: str,
        value: JsonObject,
        visibility: Visibility = "public",
        data_classes: list[str] | None = None,
    ) -> "OutputRecord":
        """Inline structured output (JSON-shaped). Summary is a human-readable label."""
        return cls(
            kind="structured",
            visibility=visibility,
            data_classes=_data_classes(data_classes),
            summary=summary,
            value=_json_object(value),
        )

    @classmethod
    def json_value(
        cls,
        *,
        summary: str,
        value: object,
        visibility: Visibility = "public",
        data_classes: list[str] | None = None,
    ) -> "OutputRecord":
        """Inline JSON-shaped output."""
        return cls(
            kind="json",
            visibility=visibility,
            data_classes=_data_classes(data_classes),
            summary=summary,
            value=_json_value(value),
        )

    @classmethod
    def blob(
        cls,
        *,
        summary: str,
        blob_ref: str,
        visibility: Visibility = "public",
        data_classes: list[str] | None = None,
    ) -> "OutputRecord":
        """Blob-referenced output. The blob_ref is engine-minted."""
        return cls(
            kind="blob_ref",
            visibility=visibility,
            data_classes=_data_classes(data_classes),
            summary=summary,
            blob_ref=blob_ref,
        )


def _data_classes(data_classes: list[str] | None) -> list[str]:
    return [PUBLIC] if data_classes is None else list(data_classes)


def _json_object(raw_json: object) -> JsonObject:
    if not isinstance(raw_json, dict):
        raise TypeError("JSON value must be an object")
    output: JsonObject = {}
    for key, item in raw_json.items():
        if not isinstance(key, str):
            raise TypeError("JSON object keys must be strings")
        output[key] = _json_value(item)
    return output


def _json_value(raw_json: object) -> JsonValue:
    if raw_json is None or isinstance(raw_json, str | int | float | bool):
        return raw_json
    if isinstance(raw_json, dict):
        return _json_object(raw_json)
    if isinstance(raw_json, Sequence) and not isinstance(raw_json, str | bytes | bytearray):
        return [_json_value(item) for item in raw_json]
    raise TypeError(f"value is not JSON: {type(raw_json).__name__}")


__all__ = ["OutputKind", "OutputRecord", "Visibility"]
