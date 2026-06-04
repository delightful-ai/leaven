"""EvidenceEnvelope — visibility-labeled evidence with public/private projection.

See `docs/specs/leaven_python.md` ("What is preserved" — evidence visibility).
The envelope splits what the optimizer sees from what stays private to the
evaluator, with explicit `target_derived` flagging so private state cannot
hide target material under non-target labels.
"""

from typing import Any

from pydantic import BaseModel, ConfigDict


class EvidencePublic(BaseModel):
    """The optimizer-visible projection of evidence."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    data_classes: list[str]
    """Must cover every data class in the public payload."""
    payload: dict[str, Any] = {}
    """Arbitrary JSON-shaped public state."""


class EvidencePrivate(BaseModel):
    """The evaluator-private projection of evidence."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    visibility: str = "evaluator_only"
    data_classes: list[str]
    payload: dict[str, Any] = {}


class EvidenceEnvelope(BaseModel):
    """Visibility-labeled evidence carrying source receipts. Build via classmethods."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    public: EvidencePublic | None = None
    private: EvidencePrivate | None = None
    target_derived: bool = False
    """When true, the envelope carries case.target data classes and must declare
    them. False evidence with target material is rejected by the seam."""

    @classmethod
    def public_private(
        cls,
        *,
        public: dict[str, Any],
        private: dict[str, Any],
        target_derived: bool = False,
    ) -> "EvidenceEnvelope":
        """Build an envelope with both visibility projections.

        Public dict must include a `data_classes` key listing all public classes.
        Private dict must include `data_classes` and optionally `visibility`.
        Target_derived must be true when private carries case.target classes.
        """
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    @classmethod
    def public_only(
        cls,
        *,
        payload: dict[str, Any],
        data_classes: list[str],
    ) -> "EvidenceEnvelope":
        """Public evidence with no private payload."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["EvidenceEnvelope", "EvidencePrivate", "EvidencePublic"]
