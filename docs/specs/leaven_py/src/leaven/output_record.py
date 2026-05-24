"""OutputRecord — typed output projection for assessments and stage results.

Carries visibility, data classes, and either inline content or a blob ref.
Constructors are explicit: `text(...)`, `blob(...)`, `structured(...)`.
"""

from __future__ import annotations

from typing import Any, Literal

from pydantic import BaseModel, ConfigDict, Field

from .data_class import OPTIMIZER_VISIBLE

Visibility = Literal["optimizer_visible", "evaluator_only", "trace_only", "private"]


class OutputRecord(BaseModel):
    """A visibility-labeled output projection. Build via classmethods."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: Literal["text", "blob", "structured"]
    visibility: Visibility = "optimizer_visible"
    data_classes: list[str] = Field(default_factory=lambda: [OPTIMIZER_VISIBLE])
    summary: str | None = None
    content: Any | None = None
    """Inline content for `text` and `structured`; `None` for `blob`."""
    blob_ref: str | None = None
    """Opaque blob reference for `blob` outputs; `None` for inline."""

    @classmethod
    def text(
        cls,
        *,
        summary: str,
        visibility: Visibility = "optimizer_visible",
        data_classes: list[str] | None = None,
    ) -> OutputRecord:
        """Inline text output. Summary IS the content."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    @classmethod
    def structured(
        cls,
        *,
        summary: str,
        content: dict[str, Any],
        visibility: Visibility = "optimizer_visible",
        data_classes: list[str] | None = None,
    ) -> OutputRecord:
        """Inline structured output (JSON-shaped). Summary is a human-readable label."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    @classmethod
    def blob(
        cls,
        *,
        summary: str,
        blob_ref: str,
        visibility: Visibility = "optimizer_visible",
        data_classes: list[str] | None = None,
    ) -> OutputRecord:
        """Blob-referenced output. The blob_ref is engine-minted."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["OutputRecord", "Visibility"]
