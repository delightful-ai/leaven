"""Render generated reference records for public-seam payloads."""

REF_RECORDS = """from typing import Literal

from msgspec import UNSET, Struct, UnsetType, field

type WireJsonScalar = str | int | float | bool | None
type WireJsonLeafArray = list[WireJsonScalar]
type WireJsonLeafObject = dict[str, WireJsonScalar | WireJsonLeafArray]
type WireJsonField = WireJsonScalar | WireJsonLeafArray | WireJsonLeafObject
type WireJsonValue = WireJsonScalar | list[WireJsonValue] | dict[str, WireJsonValue]
type WireJsonLiteralDepth0 = WireJsonScalar
type WireJsonLiteralDepth1 = (
    WireJsonScalar | list[WireJsonLiteralDepth0] | dict[str, WireJsonLiteralDepth0]
)
type WireJsonLiteralDepth2 = (
    WireJsonScalar | list[WireJsonLiteralDepth1] | dict[str, WireJsonLiteralDepth1]
)
type WireJsonLiteralDepth3 = (
    WireJsonScalar | list[WireJsonLiteralDepth2] | dict[str, WireJsonLiteralDepth2]
)
type WireJsonLiteralDepth4 = (
    WireJsonScalar | list[WireJsonLiteralDepth3] | dict[str, WireJsonLiteralDepth3]
)
type WireJsonLiteralDepth5 = (
    WireJsonScalar | list[WireJsonLiteralDepth4] | dict[str, WireJsonLiteralDepth4]
)
type WireJsonLiteralDepth6 = (
    WireJsonScalar | list[WireJsonLiteralDepth5] | dict[str, WireJsonLiteralDepth5]
)
type WireJsonLiteralDepth7 = (
    WireJsonScalar | list[WireJsonLiteralDepth6] | dict[str, WireJsonLiteralDepth6]
)
type WireJsonLiteralDepth8 = (
    WireJsonScalar | list[WireJsonLiteralDepth7] | dict[str, WireJsonLiteralDepth7]
)
type WireJsonExtensionPayload = WireJsonLiteralDepth8
type WireJsonGraphEventFilter = dict[str, WireJsonLiteralDepth7]
type WireJsonLiteralValue = WireJsonLiteralDepth8
type WireJsonOutputValue = WireJsonLiteralDepth8
type WireJsonAssessmentPreference = WireJsonLiteralDepth8
type WireJsonAssessmentRanking = WireJsonLiteralDepth8
type WireJsonAssessmentTarget = WireJsonLiteralDepth8
type WireJsonArtifactSelector = WireJsonLiteralDepth8
type WireJsonCaseReadInput = WireJsonLiteralDepth8
type WireJsonCaseReadMetadata = WireJsonLiteralDepth8
type WireJsonCaseReadTarget = WireJsonLiteralDepth8
type WireJsonCaseInput = dict[str, WireJsonLiteralDepth7]
type WireJsonCostScope = WireJsonLiteralDepth8
type WireJsonObject = dict[str, WireJsonField]
type DataClassSet = list[str]
type TraceVisibility = Literal["public", "optimizer_visible", "host_private", "external_private"]
type WireJsonSchemaTypeName = Literal[
    "array",
    "boolean",
    "integer",
    "null",
    "number",
    "object",
    "string",
]


class WireJsonSchema(Struct, frozen=True, forbid_unknown_fields=True, omit_defaults=True):
    schema_uri: str | UnsetType = field(default=UNSET, name="$schema")
    id_: str | UnsetType = field(default=UNSET, name="$id")
    ref_: str | UnsetType = field(default=UNSET, name="$ref")
    defs: dict[str, "WireJsonSchema"] | UnsetType = field(default=UNSET, name="$defs")
    comment: str | UnsetType = field(default=UNSET, name="$comment")
    title: str | UnsetType = UNSET
    description: str | UnsetType = UNSET
    type_: WireJsonSchemaTypeName | list[WireJsonSchemaTypeName] | UnsetType = field(default=UNSET, name="type")
    enum: list[WireJsonField] | UnsetType = UNSET
    const: WireJsonField | UnsetType = UNSET
    properties: dict[str, "WireJsonSchema"] | UnsetType = UNSET
    required: list[str] | UnsetType = UNSET
    additional_properties: "bool | WireJsonSchema | UnsetType" = field(default=UNSET, name="additionalProperties")
    items: "WireJsonSchema | list[WireJsonSchema] | UnsetType" = UNSET
    prefix_items: list["WireJsonSchema"] | UnsetType = field(default=UNSET, name="prefixItems")
    one_of: list["WireJsonSchema"] | UnsetType = field(default=UNSET, name="oneOf")
    any_of: list["WireJsonSchema"] | UnsetType = field(default=UNSET, name="anyOf")
    all_of: list["WireJsonSchema"] | UnsetType = field(default=UNSET, name="allOf")
    not_: "WireJsonSchema | UnsetType" = field(default=UNSET, name="not")
    format: str | UnsetType = UNSET
    pattern: str | UnsetType = UNSET
    min_length: int | UnsetType = field(default=UNSET, name="minLength")
    max_length: int | UnsetType = field(default=UNSET, name="maxLength")
    minimum: int | float | UnsetType = UNSET
    maximum: int | float | UnsetType = UNSET
    exclusive_minimum: int | float | UnsetType = field(default=UNSET, name="exclusiveMinimum")
    exclusive_maximum: int | float | UnsetType = field(default=UNSET, name="exclusiveMaximum")
    multiple_of: int | float | UnsetType = field(default=UNSET, name="multipleOf")
    min_items: int | UnsetType = field(default=UNSET, name="minItems")
    max_items: int | UnsetType = field(default=UNSET, name="maxItems")
    unique_items: bool | UnsetType = field(default=UNSET, name="uniqueItems")
    min_properties: int | UnsetType = field(default=UNSET, name="minProperties")
    max_properties: int | UnsetType = field(default=UNSET, name="maxProperties")


type WireJsonSchemaObject = WireJsonSchema


class ExternalEventPayload(
    Struct, frozen=True, forbid_unknown_fields=True, omit_defaults=True, tag="external_event", tag_field="kind"
):
    ok: bool
    stage_call_id: str | UnsetType = UNSET


class EventEmittedSummaryPayload(
    Struct, frozen=True, forbid_unknown_fields=True, tag="event_emitted", tag_field="kind"
):
    event_id: str
    event_kind: str
    payload_schema: str
    value: ExternalEventPayload
    visibility: TraceVisibility


class ProposalBatchSubmittedEventPayload(
    Struct, frozen=True, forbid_unknown_fields=True, tag="proposal_batch_submitted", tag_field="kind"
):
    proposal_batch: "ProposalBatchRef"
    proposal_ids: list["ProposalRef"]


class ProposalBatchAppliedEventPayload(
    Struct, frozen=True, forbid_unknown_fields=True, tag="proposal_batch_applied", tag_field="kind"
):
    proposal_batch: "ProposalBatchRef"
    created_candidates: list["CandidateRef"]


class AssessmentsSubmittedEventPayload(
    Struct, frozen=True, forbid_unknown_fields=True, tag="assessments_submitted", tag_field="kind"
):
    evaluation_request_id: "EvaluationRequestRef"
    assessment_ids: list["AssessmentRef"]


class EvaluationRequestedEventPayload(
    Struct, frozen=True, forbid_unknown_fields=True, tag="evaluation_requested", tag_field="kind"
):
    name: str
    evaluation_request_id: "EvaluationRequestRef"
    evaluator_id: str


class RunContextSummaryEventPayload(
    Struct, frozen=True, forbid_unknown_fields=True, tag="run_context_summary", tag_field="kind"
):
    source: Literal["leaven-seam-service-run-context"]
    candidate_count: int
    proposal_batch: "ProposalBatchRef"
    created_candidates: list["CandidateRef"]
    event_count: int
    emitted_events: list[EventEmittedSummaryPayload]
    assessment_ids: list["AssessmentRef"]
    applied: bool
    evaluation_request_id: "EvaluationRequestRef | None | UnsetType" = UNSET


type EventSummaryPayload = (
    ExternalEventPayload
    | EventEmittedSummaryPayload
    | ProposalBatchSubmittedEventPayload
    | ProposalBatchAppliedEventPayload
    | AssessmentsSubmittedEventPayload
    | EvaluationRequestedEventPayload
    | RunContextSummaryEventPayload
)


class BlobRef(Struct, frozen=True, forbid_unknown_fields=True, tag="blob_ref", tag_field="kind"):
    id: str
    sha256: str
    bytes: int
    data_classes: DataClassSet
    media_type: str | UnsetType = UNSET
    uri: str | UnsetType = UNSET


class CandidateRefRecord(Struct, frozen=True, forbid_unknown_fields=True, tag="candidate", tag_field="kind"):
    id: str
    run: str | UnsetType = UNSET


class ProposalRefRecord(Struct, frozen=True, forbid_unknown_fields=True, tag="proposal", tag_field="kind"):
    id: str
    run: str | UnsetType = UNSET


class ProposalBatchRefRecord(Struct, frozen=True, forbid_unknown_fields=True, tag="proposal_batch", tag_field="kind"):
    id: str
    run: str | UnsetType = UNSET


class AssessmentRefRecord(Struct, frozen=True, forbid_unknown_fields=True, tag="assessment", tag_field="kind"):
    id: str
    run: str | UnsetType = UNSET

class EvaluationRequestRefRecord(Struct, frozen=True, forbid_unknown_fields=True, tag="evaluation_request", tag_field="kind"):
    id: str
    run: str | UnsetType = UNSET

class EvaluationAttemptRefRecord(Struct, frozen=True, forbid_unknown_fields=True, tag="evaluation_attempt", tag_field="kind"):
    id: str
    run: str | UnsetType = UNSET

class ExternalInfoRefRecord(Struct, frozen=True, forbid_unknown_fields=True, tag="external", tag_field="kind"):
    namespace: str
    id: str
    schema_fingerprint: str | UnsetType = UNSET

class ReceiptRefRecord(Struct, frozen=True, forbid_unknown_fields=True, tag="receipt", tag_field="kind"):
    id: str
    fingerprint: str | UnsetType = UNSET

type ReceiptRef = str | ReceiptRefRecord


class CaseRefRecord(Struct, frozen=True, forbid_unknown_fields=True, tag="case", tag_field="kind"):
    id: str
    run: str | UnsetType = UNSET


class WorkspaceRefRecord(
    Struct, frozen=True, forbid_unknown_fields=True, tag="workspace", tag_field="kind"
):
    id: str
    run: str | UnsetType = UNSET
    snapshot_fingerprint: str | UnsetType = UNSET


class TraceRefRecord(Struct, frozen=True, forbid_unknown_fields=True, omit_defaults=True):
    kind: str
    id: str
    visibility: TraceVisibility
    data_classes: DataClassSet | UnsetType = UNSET
    receipt: ReceiptRef | UnsetType = UNSET


type CandidateRef = str | CandidateRefRecord
type ProposalRef = str | ProposalRefRecord
type ProposalBatchRef = str | ProposalBatchRefRecord
type AssessmentRef = str | AssessmentRefRecord
type CaseRef = str | CaseRefRecord
type WorkspaceRef = str | WorkspaceRefRecord
type EvaluationRequestRef = str | EvaluationRequestRefRecord
type EvaluationAttemptRef = str | EvaluationAttemptRefRecord
type InfoRef = (
    str
    | CandidateRefRecord
    | ProposalRefRecord
    | ProposalBatchRefRecord
    | AssessmentRefRecord
    | EvaluationRequestRefRecord
    | EvaluationAttemptRefRecord
    | ExternalInfoRefRecord
)
type MetadataBag = WireJsonObject
type TraceRef = TraceRefRecord
"""

REF_EXPORTS = (
    "AssessmentRef AssessmentRefRecord AssessmentsSubmittedEventPayload BlobRef CandidateRef CandidateRefRecord CaseRef CaseRefRecord "
    "EvaluationAttemptRef EvaluationAttemptRefRecord EvaluationRequestRef EvaluationRequestRefRecord EvaluationRequestedEventPayload "
    "EventEmittedSummaryPayload EventSummaryPayload ExternalEventPayload ExternalInfoRefRecord "
    "InfoRef ProposalBatchAppliedEventPayload ProposalBatchRef ProposalBatchRefRecord ProposalBatchSubmittedEventPayload "
    "ProposalRef ProposalRefRecord ReceiptRef RunContextSummaryEventPayload "
    "ReceiptRefRecord TraceRef TraceRefRecord TraceVisibility WorkspaceRef WorkspaceRefRecord"
)


def render_refs() -> str:
    """Render generated reference records as their own private wire module."""
    return f'''"""Generated public-seam reference records.

Generated by `codegen/generate_seam_wire.py`; do not edit by hand.
"""

{REF_RECORDS}

__all__ = (  # noqa: PLE0605, SIM905
    "{REF_EXPORTS} "
    "DataClassSet MetadataBag WireJsonArtifactSelector WireJsonAssessmentPreference WireJsonAssessmentRanking WireJsonAssessmentTarget WireJsonCaseInput WireJsonCaseReadInput WireJsonCaseReadMetadata WireJsonCaseReadTarget WireJsonCostScope WireJsonExtensionPayload WireJsonField WireJsonGraphEventFilter WireJsonLeafArray WireJsonLeafObject WireJsonLiteralDepth0 WireJsonLiteralDepth1 WireJsonLiteralDepth2 WireJsonLiteralDepth3 WireJsonLiteralDepth4 WireJsonLiteralDepth5 WireJsonLiteralDepth6 WireJsonLiteralDepth7 WireJsonLiteralDepth8 WireJsonLiteralValue WireJsonObject WireJsonOutputValue WireJsonScalar WireJsonSchema WireJsonSchemaObject WireJsonSchemaTypeName WireJsonValue"
).split()
'''
