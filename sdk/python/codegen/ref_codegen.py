"""Render generated reference records for public-seam payloads."""

REF_RECORDS = '''type DataClassSet = list[str]
type TraceVisibility = Literal["public", "optimizer_visible", "host_private", "external_private"]


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
'''

REF_EXPORTS = (
    "AssessmentRef AssessmentRefRecord CandidateRef CandidateRefRecord EvaluationAttemptRef "
    "EvaluationAttemptRefRecord EvaluationRequestRef EvaluationRequestRefRecord ExternalInfoRefRecord "
    "InfoRef ProposalBatchRef ProposalBatchRefRecord ProposalRef ProposalRefRecord ReceiptRef "
    "ReceiptRefRecord TraceRef TraceRefRecord TraceVisibility"
)
