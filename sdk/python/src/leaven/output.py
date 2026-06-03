"""Output contract helpers — `lv.output.files(...)`, `lv.output.json_schema(...)`, `lv.output.text(...)`.

These build the typed output contracts passed to `cx.sandbox.exec(output=...)`,
`cx.agent.run(output=...)`, etc. They constrain what the engine accepts back
and shape the typed result the user reads.
"""

from collections.abc import Sequence
from typing import Any, Literal

from pydantic import BaseModel, ConfigDict


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


class JsonSchemaOutput(OutputContract):
    """Enforce a JSON-schema-shaped return value."""

    kind: Literal["json_schema"] = "json_schema"
    schema_: dict[str, Any]
    """JSON Schema 2020-12 the response must validate against."""
    parse_to: Any | None = None
    """Optional pydantic model class to parse the response into."""


class TextOutput(OutputContract):
    """Plain text response with optional length cap."""

    kind: Literal["text"] = "text"
    max_chars: int | None = None


def files(paths: Sequence[str], *, max_bytes: int | None = None) -> FilesOutput:
    """Output contract: capture these workspace-relative paths."""
    return FilesOutput(paths=list(paths), max_bytes=max_bytes)


def json_schema(model_or_schema: Any) -> JsonSchemaOutput:
    """Output contract: response must match the given JSON Schema or pydantic model.

    Accepts a pydantic `BaseModel` subclass (extracts its JSON schema) or a
    raw JSON Schema dict. When given a model, the parsed result lifts back
    into an instance of that model.
    """
    raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


def text(*, max_chars: int | None = None) -> TextOutput:
    """Output contract: plain text response with optional length cap."""
    return TextOutput(max_chars=max_chars)


__all__ = [
    "FilesOutput",
    "JsonSchemaOutput",
    "OutputContract",
    "TextOutput",
    "files",
    "json_schema",
    "text",
]
