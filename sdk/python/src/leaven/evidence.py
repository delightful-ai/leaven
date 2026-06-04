"""EvidenceEnvelope — visibility-labeled evidence with public/private projection.

See `docs/specs/leaven_python.md` ("What is preserved" — evidence visibility).
The envelope splits what the optimizer sees from what stays private to the
evaluator, with explicit `target_derived` flagging so private state cannot
hide target material under non-target labels.
"""

from pydantic import BaseModel, ConfigDict, Field

from .data_class import CASE_TARGET
from .json_value import JsonObject, JsonValue


class EvidencePublic(BaseModel):
    """The optimizer-visible projection of evidence."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    data_classes: list[str]
    """Must cover every data class in the public payload."""
    payload: JsonObject = Field(default_factory=dict)
    """Arbitrary JSON-shaped public state."""


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
        public: JsonObject,
        private: JsonObject,
        target_derived: bool = False,
    ) -> "EvidenceEnvelope":
        """Build an envelope with both visibility projections.

        Public dict must include a `data_classes` key listing all public classes.
        Private dict must include `data_classes` and optionally `visibility`.
        Target_derived must be true when private carries case.target classes.
        """
        public_projection = _public_from_record(public)
        private_projection = _private_from_record(private)
        if CASE_TARGET in private_projection.data_classes and not target_derived:
            raise ValueError("private case.target evidence requires target_derived=True")
        return cls(
            public=public_projection,
            private=private_projection,
            target_derived=target_derived,
        )

    @classmethod
    def public_only(
        cls,
        *,
        payload: JsonObject,
        data_classes: list[str],
    ) -> "EvidenceEnvelope":
        """Public evidence with no private payload."""
        return cls(public=EvidencePublic(data_classes=list(data_classes), payload=dict(payload)))


def _public_from_record(record: JsonObject) -> EvidencePublic:
    data_classes = _required_data_classes(record)
    payload = _payload_without_reserved(record, reserved={"data_classes"})
    return EvidencePublic(data_classes=data_classes, payload=payload)


def _private_from_record(record: JsonObject) -> EvidencePrivate:
    data_classes = _required_data_classes(record)
    visibility = _optional_string(record, "visibility", default="evaluator_only")
    payload = _payload_without_reserved(record, reserved={"data_classes", "visibility"})
    return EvidencePrivate(visibility=visibility, data_classes=data_classes, payload=payload)


def _required_data_classes(record: JsonObject) -> list[str]:
    if "data_classes" not in record:
        raise KeyError("evidence record requires `data_classes`")
    value = record["data_classes"]
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise TypeError("evidence `data_classes` must be a list of strings")
    return list(value)


def _optional_string(record: JsonObject, key: str, *, default: str) -> str:
    if key not in record:
        return default
    value = record[key]
    if not isinstance(value, str):
        raise TypeError(f"evidence `{key}` must be a string")
    return value


def _payload_without_reserved(record: JsonObject, *, reserved: set[str]) -> JsonObject:
    payload: JsonObject = {}
    for key, value in record.items():
        if key not in reserved:
            payload[key] = _json_value(value)
    return payload


def _json_value(value: JsonValue) -> JsonValue:
    return value


__all__ = ["EvidenceEnvelope", "EvidencePrivate", "EvidencePublic"]
