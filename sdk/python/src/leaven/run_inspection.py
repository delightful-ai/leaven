"""Typed completed-run inspection projections for `lv.runs.inspect(...)`."""

import base64
import json
from collections.abc import Sequence
from typing import Literal

from pydantic import BaseModel, ConfigDict, Field

from .assessment import Assessment
from .blob_ref import BlobRef
from .json_value import JsonObject, JsonValue
from .result import Optimized
from .run_status import RunCostStatus, RunUsageStatus, UnsupportedRunFact

ReceiptKind = Literal["query", "call", "write"]


class BlobReadbackSummary(BaseModel):
    """One blob ref resolved through a Rust-owned run store export."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    store: str
    key: str
    schema_fingerprint: str = Field(alias="schema")
    format: str


class BlobByteReadbackRef(BaseModel):
    """Blob store/key resolved by Rust for byte readback."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    store: str
    key: str


class EvidenceReadbackRef(BaseModel):
    """Evidence store/key exposed by Rust checkpoint readback."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    store: str
    key: str


class CheckpointReadbackSummary(BaseModel):
    """Checkpoint-envelope facts read by Rust from the local run store."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    format_version: int
    graph_snapshot: BlobReadbackSummary
    artifact_refs: list[BlobByteReadbackRef] = Field(default_factory=list)
    artifact_ref_count: int
    evidence_refs: list[EvidenceReadbackRef] = Field(default_factory=list)
    evidence_ref_count: int
    stage_journal_refs: list[BlobByteReadbackRef] = Field(default_factory=list)
    stage_journal_ref_count: int
    workspace_journal_refs: list[BlobByteReadbackRef] = Field(default_factory=list)
    workspace_journal_ref_count: int
    has_optimizer_state: bool
    has_cache_index: bool


class GraphReadbackSummary(BaseModel):
    """Graph snapshot facts read by Rust from the checkpoint graph blob."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    blob: BlobReadbackSummary
    bytes: int
    run_id: str | None
    best_candidate_id: str | None = None
    candidates: list["CandidateReadback"] = Field(default_factory=list)
    assessments: list["AssessmentReadback"] = Field(default_factory=list)
    candidate_count: int
    proposal_batch_count: int
    proposal_count: int
    apply_attempt_count: int
    evaluation_request_count: int
    assessment_count: int
    event_count: int


class CandidateReadback(BaseModel):
    """Candidate facts read from a Rust-owned graph snapshot."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    id: str
    parent_id: str | None = None
    artifact: JsonValue


class AssessmentReadback(BaseModel):
    """Assessment facts read from a Rust-owned graph snapshot."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    id: str
    request_id: str
    evaluator: str
    target_kind: Literal["independent", "pairwise", "listwise"]
    candidate_ids: list[str]
    target: JsonValue
    evidence: EvidenceReadbackRef
    metadata: JsonValue
    created_at: str


class CostReadback(BaseModel):
    """Cost and usage axes read from Rust checkpoint state."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    metric_calls: int
    lm_calls: int
    prompt_tokens: int
    completion_tokens: int
    seconds: float

    @property
    def lm_tokens(self) -> int:
        """Total prompt plus completion tokens."""
        return self.prompt_tokens + self.completion_tokens


class RustRunReadback(BaseModel):
    """Rust-owned checkpoint and graph readback attached to run inspection."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    schema_version: Literal["leaven.run_inspection_export.v1"]
    run_id: str
    latest_checkpoint: str
    checkpoint: CheckpointReadbackSummary
    graph: GraphReadbackSummary
    cost: CostReadback


class RustBlobReadback(BaseModel):
    """Rust-owned bytes read from a local run blob store."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    schema_version: Literal["leaven.run_blob_export.v1"]
    blob: BlobByteReadbackRef
    bytes: int
    sha256: str
    content_base64: str

    def content_bytes(self) -> bytes:
        """Decode the blob contents exported by Rust."""
        return base64.b64decode(self.content_base64, validate=True)


class RustEvidenceReadback(BaseModel):
    """Rust-owned bytes read from a local run evidence store."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    schema_version: Literal["leaven.run_evidence_export.v1"]
    evidence: EvidenceReadbackRef
    bytes: int
    sha256: str
    content_base64: str

    def content_bytes(self) -> bytes:
        """Decode the evidence JSON contents exported by Rust."""
        return base64.b64decode(self.content_base64, validate=True)

    def content_json(self) -> JsonValue:
        """Decode and validate the Rust-exported evidence JSON payload."""
        return _json_value(json.loads(self.content_bytes()))


class ReceiptSummary(BaseModel):
    """One opaque receipt visible from a completed run."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: ReceiptKind
    receipt_id: str
    source: str
    """Stable source label such as `assessment:<case>` or `proposal_batch`."""
    blob_refs: list[BlobRef] = Field(default_factory=list)
    """Blob references associated with this receipt, such as agent transcripts."""
    proposal_ids: list[str] = Field(default_factory=list)
    """Proposal ids associated with a proposal-batch write receipt."""


class EvidenceSummary(BaseModel):
    """Optimizer-visible evidence projection for one assessment."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    case_id: str
    candidate_id: str
    data_classes: list[str] = Field(default_factory=list)
    payload: JsonObject = Field(default_factory=dict)
    target_derived: bool
    rewards: list["RewardDimensionSummary"] = Field(default_factory=list)


class RewardDimensionSummary(BaseModel):
    """One inspected reward-vector dimension for an assessment."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    id: str
    value: float
    weight: float
    feedback: str = ""


class RunInspection(BaseModel):
    """Flattened, read-only facts users need when auditing a completed run."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    run_id: str
    run_dir: str | None
    best_candidate_id: str
    best_lineage: list[str]
    receipts: list[ReceiptSummary] = Field(default_factory=list)
    evidence: list[EvidenceSummary] = Field(default_factory=list)
    total_cost_usd: float | None
    cost_status: RunCostStatus
    total_lm_tokens: int | None
    usage_status: RunUsageStatus
    rust_readback: RustRunReadback | None = None
    rust_graph_blob: RustBlobReadback | None = None
    rust_evidence: list[RustEvidenceReadback] = Field(default_factory=list)
    unsupported: tuple[UnsupportedRunFact, ...] = ()

    def receipt_ids(self, *, kind: ReceiptKind | None = None) -> list[str]:
        """Return receipt ids, optionally filtered by receipt kind."""
        return [
            receipt.receipt_id for receipt in self.receipts if kind is None or receipt.kind == kind
        ]


def inspect_optimized[A](
    result: Optimized[A],
    *,
    rust_readback: RustRunReadback | None = None,
    rust_graph_blob: RustBlobReadback | None = None,
    rust_evidence: Sequence[RustEvidenceReadback] = (),
) -> RunInspection:
    """Build a flattened inspection projection from an optimized result."""
    return RunInspection(
        run_id=result.run_id,
        run_dir=result.summary.run_dir,
        best_candidate_id=result.best.id,
        best_lineage=[candidate.id for candidate in result.lineage(result.best.id)],
        receipts=_receipts(result),
        evidence=[_evidence_summary(assessment) for assessment in result.assessment_rows],
        total_cost_usd=result.summary.total_cost_usd,
        cost_status=result.summary.cost_status,
        total_lm_tokens=result.summary.total_lm_tokens,
        usage_status=result.summary.usage_status,
        rust_readback=rust_readback,
        rust_graph_blob=rust_graph_blob,
        rust_evidence=list(rust_evidence),
        unsupported=result.summary.unsupported,
    )


def _json_value(value: object) -> JsonValue:
    if value is None or isinstance(value, str | int | float | bool):
        return value
    if isinstance(value, list):
        return [_json_value(item) for item in value]
    if isinstance(value, dict):
        output: JsonObject = {}
        for key, item in value.items():
            if not isinstance(key, str):
                raise TypeError("JSON object keys must be strings")
            output[key] = _json_value(item)
        return output
    raise TypeError(f"value is not JSON: {type(value).__name__}")


def _receipts[A](result: Optimized[A]) -> list[ReceiptSummary]:
    receipts: list[ReceiptSummary] = []
    for assessment in result.assessment_rows:
        source = f"assessment:{assessment.case.id}"
        receipts.append(
            ReceiptSummary(kind="write", receipt_id=assessment.receipt.receipt_id, source=source)
        )
        receipts.extend(
            ReceiptSummary(kind="query", receipt_id=receipt.receipt_id, source=source)
            for receipt in assessment.read_receipts
        )
        receipts.extend(
            ReceiptSummary(
                kind="call",
                receipt_id=receipt.receipt_id,
                source=source,
                blob_refs=receipt.blob_refs,
            )
            for receipt in assessment.effect_receipts
        )
    receipts.extend(
        ReceiptSummary(
            kind="call",
            receipt_id=receipt.receipt_id,
            source="proposer_stage",
            blob_refs=receipt.blob_refs,
        )
        for receipt in result.effect_receipts
    )
    receipts.extend(
        ReceiptSummary(
            kind="write",
            receipt_id=receipt.receipt_id,
            source="proposal_batch",
            proposal_ids=receipt.proposal_ids,
        )
        for receipt in result.proposal_receipts
    )
    return receipts


def _evidence_summary(assessment: Assessment) -> EvidenceSummary:
    public = assessment.evidence.public
    return EvidenceSummary(
        case_id=assessment.case.id,
        candidate_id=assessment.candidate_id,
        data_classes=list(public.data_classes) if public is not None else [],
        payload=dict(public.payload) if public is not None else {},
        target_derived=assessment.evidence.target_derived,
        rewards=[
            RewardDimensionSummary(
                id=reward.id,
                value=reward.value,
                weight=reward.weight,
                feedback=reward.feedback,
            )
            for reward in assessment.rewards
        ],
    )


__all__ = [
    "AssessmentReadback",
    "BlobByteReadbackRef",
    "BlobReadbackSummary",
    "CandidateReadback",
    "CheckpointReadbackSummary",
    "CostReadback",
    "EvidenceReadbackRef",
    "EvidenceSummary",
    "GraphReadbackSummary",
    "ReceiptKind",
    "ReceiptSummary",
    "RewardDimensionSummary",
    "RunInspection",
    "RustBlobReadback",
    "RustEvidenceReadback",
    "RustRunReadback",
    "inspect_optimized",
]
