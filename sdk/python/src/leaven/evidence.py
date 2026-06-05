"""EvidenceEnvelope — visibility-labeled evidence with public/private projection.

See `docs/specs/leaven_python.md` ("What is preserved" — evidence visibility).
The envelope splits what the optimizer sees from what stays private to the
evaluator, with explicit `target_derived` flagging so private state cannot
hide target material under non-target labels.
"""

from pydantic import BaseModel, ConfigDict, Field

from .data_class import CASE_TARGET
from .json_value import JsonObject


class EvidencePublicPayload(BaseModel):
    """Optimizer-visible assessment evidence payload."""

    model_config = ConfigDict(frozen=True, extra="forbid", strict=True)

    summary: str | None = None
    feedback: str | None = None
    metrics: dict[str, float] | None = None


class EvidencePublic(BaseModel):
    """The optimizer-visible projection of evidence."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    data_classes: list[str]
    """Must cover every data class in the public payload."""
    payload: EvidencePublicPayload = Field(default_factory=EvidencePublicPayload)
    """Closed optimizer-visible assessment payload."""


class EvidencePrivate(BaseModel):
    """The evaluator-private projection of evidence."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    visibility: str = "evaluator_only"
    data_classes: list[str]
    payload: JsonObject = Field(default_factory=dict)


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
        public: EvidencePublic,
        private: EvidencePrivate,
        target_derived: bool = False,
    ) -> "EvidenceEnvelope":
        """Build an envelope with both visibility projections.

        Target_derived must be true when private carries case.target classes.
        """
        if CASE_TARGET in private.data_classes and not target_derived:
            raise ValueError("private case.target evidence requires target_derived=True")
        return cls(public=public, private=private, target_derived=target_derived)

    @classmethod
    def public_only(
        cls,
        *,
        payload: EvidencePublicPayload,
        data_classes: list[str],
    ) -> "EvidenceEnvelope":
        """Public evidence with no private payload."""
        return cls(public=EvidencePublic(data_classes=list(data_classes), payload=payload))


__all__ = [
    "EvidenceEnvelope",
    "EvidencePrivate",
    "EvidencePublic",
    "EvidencePublicPayload",
]
