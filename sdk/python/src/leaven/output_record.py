"""OutputRecord — typed output projection for assessments and stage results.

Carries visibility, data classes, and either inline value or a blob ref.
Constructors are explicit: `text(...)`, `json_value(...)`, `blob(...)`,
`structured(...)`.
"""

from typing import Any, Literal

from pydantic import BaseModel, ConfigDict, Field

from .data_class import PUBLIC

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
    value: Any | None = None
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
    ) -> OutputRecord:
        """Inline text output. Summary IS the content."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    @classmethod
    def structured(
        cls,
        *,
        summary: str,
        value: dict[str, Any],
        visibility: Visibility = "public",
        data_classes: list[str] | None = None,
    ) -> OutputRecord:
        """Inline structured output (JSON-shaped). Summary is a human-readable label."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    @classmethod
    def json_value(
        cls,
        *,
        summary: str,
        value: Any,
        visibility: Visibility = "public",
        data_classes: list[str] | None = None,
    ) -> OutputRecord:
        """Inline JSON-shaped output."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    @classmethod
    def blob(
        cls,
        *,
        summary: str,
        blob_ref: str,
        visibility: Visibility = "public",
        data_classes: list[str] | None = None,
    ) -> OutputRecord:
        """Blob-referenced output. The blob_ref is engine-minted."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["OutputKind", "OutputRecord", "Visibility"]
