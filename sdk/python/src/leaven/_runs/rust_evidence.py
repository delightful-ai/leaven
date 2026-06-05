"""Typed projection from Rust-owned evidence exports into run inspection."""

from pydantic import BaseModel, ConfigDict, Field

from .._receipts import CallReceipt, WriteReceipt
from ..assessment import Assessment, RewardAssessment
from ..blob_ref import BlobRef
from ..case import Case
from ..evidence import EvidenceEnvelope
from ..json_value import JsonObject, JsonValue
from ..run_inspection import (
    AssessmentReadback,
    EvidenceSummary,
    RewardDimensionSummary,
    RustEvidenceReadback,
    RustRunReadback,
)
from ..score import Score


class RustScalarEvidence(BaseModel):
    """Serde shape of `leaven_evidence::ScalarEvidence`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    score: float


class RustOutputMetadata(BaseModel):
    """Serde shape of `leaven_evidence::OutputMetadata`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    visibility: str
    data_classes: list[str]


class RustInlineOutput(BaseModel):
    """Serde shape of the inline `OutputRecord` variant."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    text: str
    truncated: bool
    metadata: RustOutputMetadata


class RustBlobReference(BaseModel):
    """Serde shape of `leaven_kernel::BlobRef` inside output records."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    store: str
    key: str


class RustOutputBlobAudit(BaseModel):
    """Serde shape of public blob audit metadata."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    sha256: str
    bytes: int
    media_type: str | None = None
    uri: str | None = None


class RustBlobOutput(BaseModel):
    """Serde shape of the blob-backed `OutputRecord` variant."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    reference: RustBlobReference
    audit: RustOutputBlobAudit | None = None
    metadata: RustOutputMetadata


class RustOutputRecord(BaseModel):
    """Serde shape of `leaven_evidence::OutputRecord`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    inline: RustInlineOutput | None = Field(default=None, alias="Inline")
    blob_ref: RustBlobOutput | None = Field(default=None, alias="BlobRef")

    def report_text(self) -> str:
        """Return Rust's compact report text for the output record."""
        if self.inline is not None and self.blob_ref is None:
            return self.inline.text
        if self.blob_ref is not None and self.inline is None:
            return f"blob:{self.blob_ref.reference.store}:{self.blob_ref.reference.key}"
        raise ValueError("Rust OutputRecord must contain exactly one variant")

    def data_classes(self) -> list[str]:
        """Return the output data classes in Rust-provided order."""
        if self.inline is not None and self.blob_ref is None:
            return list(self.inline.metadata.data_classes)
        if self.blob_ref is not None and self.inline is None:
            return list(self.blob_ref.metadata.data_classes)
        raise ValueError("Rust OutputRecord must contain exactly one variant")


class RustCaseDataReadEvidence(BaseModel):
    """Serde shape of one audited case-data read."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    operation: str
    receipt: str
    case: int | str
    fields: list[str]
    data_classes: list[str]
    values: JsonObject = Field(default_factory=dict)

    def case_id(self) -> str:
        """Return the stable textual case id carried by Rust evidence."""
        if "case_id" not in self.values:
            raise TypeError("Rust case data read must carry case_id")
        value = self.values["case_id"]
        if not isinstance(value, str):
            raise TypeError("Rust case id read value must be a string")
        return value

    def target(self) -> JsonObject | None:
        """Return the target value read by Rust evidence, when present."""
        if "target" not in self.values:
            return None
        value = self.values["target"]
        if not isinstance(value, dict):
            raise TypeError("Rust target read value must be a JSON object")
        return value

    def rewards(self) -> list["RustRewardEvidence"]:
        """Return reward dimensions carried by Rust evidence."""
        if "rewards" not in self.values:
            return []
        value = self.values["rewards"]
        if not isinstance(value, list):
            raise TypeError("Rust rewards read value must be a JSON array")
        return [RustRewardEvidence.model_validate(item) for item in value]

    def effect_receipts(self) -> list[CallReceipt]:
        """Return effect receipts carried by Rust evidence."""
        if "effect_receipts" not in self.values:
            return []
        value = self.values["effect_receipts"]
        if not isinstance(value, list):
            raise TypeError("Rust effect receipts read value must be a JSON array")
        return [RustEffectReceiptEvidence.model_validate(item).to_public() for item in value]


class RustRewardEvidence(BaseModel):
    """Reward dimension carried through Rust case-data evidence."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    id: str
    value: float
    weight: float
    feedback: str


class RustEffectReceiptEvidence(BaseModel):
    """Effect receipt metadata carried through Rust case-data evidence."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    receipt_id: str
    blob_refs: list[BlobRef] = Field(default_factory=list)

    def to_public(self) -> CallReceipt:
        """Return the public SDK receipt handle."""
        return CallReceipt(receipt_id=self.receipt_id, blob_refs=list(self.blob_refs))


class RustCandidateAssessmentOutput(BaseModel):
    """Serde shape of a pairwise/listwise candidate output row."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    candidate: str
    output: RustOutputRecord


class RustCaseAssessmentEvidence(BaseModel):
    """Serde shape of `leaven_evidence::CaseAssessmentEvidence`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    score: RustScalarEvidence
    output: RustOutputRecord
    feedback: str
    candidate_outputs: list[RustCandidateAssessmentOutput] = Field(default_factory=list)
    trace: list[str] = Field(default_factory=list)
    case_data_reads: list[RustCaseDataReadEvidence] = Field(default_factory=list)

    @classmethod
    def from_readback(cls, readback: RustEvidenceReadback) -> "RustCaseAssessmentEvidence":
        """Decode one Rust evidence byte export as case assessment evidence."""
        return cls.model_validate(readback.content_json())

    def case_id(self) -> str | None:
        """Return the case id when the evidence audited a case-data read."""
        if not self.case_data_reads:
            return None
        return self.case_data_reads[0].case_id()

    def target(self) -> JsonObject | None:
        """Return the target object when Rust evidence exported it."""
        for read in self.case_data_reads:
            target = read.target()
            if target is not None:
                return target
        return None

    def data_classes(self) -> list[str]:
        """Return stable public data classes represented by this evidence."""
        classes = {"public"}
        classes.update(self.output.data_classes())
        for read in self.case_data_reads:
            classes.update(read.data_classes)
        return sorted(classes)

    def rewards(self) -> list[RustRewardEvidence]:
        """Return reward dimensions carried by the first audited case read."""
        for read in self.case_data_reads:
            rewards = read.rewards()
            if rewards:
                return rewards
        return [
            RustRewardEvidence(
                id="score",
                value=self.score.score,
                weight=1.0,
                feedback=self.feedback,
            )
        ]

    def effect_receipts(self) -> list[CallReceipt]:
        """Return effect receipt ids carried by audited case reads."""
        receipts: list[CallReceipt] = []
        for read in self.case_data_reads:
            receipts.extend(read.effect_receipts())
        return receipts


def rust_evidence_summaries(
    readback: RustRunReadback,
    evidence_readbacks: list[RustEvidenceReadback],
) -> list[EvidenceSummary]:
    """Project Rust-owned case assessment evidence into public inspection rows."""
    by_ref = {
        (evidence.evidence.store, evidence.evidence.key): evidence
        for evidence in evidence_readbacks
    }
    summaries: list[EvidenceSummary] = []
    for assessment in readback.graph.assessments:
        evidence = by_ref[(assessment.evidence.store, assessment.evidence.key)]
        summary = _summary_from_rust_assessment(assessment, evidence)
        summaries.append(summary)
    return summaries


def rust_assessment_rows(
    readback: RustRunReadback,
    evidence_readbacks: list[RustEvidenceReadback],
) -> list[Assessment]:
    """Project Rust-owned case assessment evidence into public Assessment rows."""
    by_ref = {
        (evidence.evidence.store, evidence.evidence.key): evidence
        for evidence in evidence_readbacks
    }
    rows: list[Assessment] = []
    for assessment in readback.graph.assessments:
        evidence = by_ref[(assessment.evidence.store, assessment.evidence.key)]
        rows.append(_assessment_from_rust(assessment, evidence))
    return rows


def _summary_from_rust_assessment(
    assessment: AssessmentReadback,
    evidence_readback: RustEvidenceReadback,
) -> EvidenceSummary:
    evidence = RustCaseAssessmentEvidence.from_readback(evidence_readback)
    case_id = evidence.case_id()
    if case_id is None:
        raise ValueError("Rust case assessment evidence did not audit a case id")
    return EvidenceSummary(
        case_id=case_id,
        candidate_id=_primary_candidate_id(assessment),
        data_classes=evidence.data_classes(),
        payload=_public_payload(evidence),
        target_derived="case.target" in evidence.data_classes(),
        rewards=[
            RewardDimensionSummary(
                id=reward.id,
                value=reward.value,
                weight=reward.weight,
                feedback=reward.feedback,
            )
            for reward in evidence.rewards()
        ],
    )


def _assessment_from_rust(
    assessment: AssessmentReadback,
    evidence_readback: RustEvidenceReadback,
) -> Assessment:
    evidence = RustCaseAssessmentEvidence.from_readback(evidence_readback)
    case_id = evidence.case_id()
    if case_id is None:
        raise ValueError("Rust case assessment evidence did not audit a case id")
    public = _public_payload(evidence)
    return Assessment(
        case=Case(
            id=case_id,
            input={},
            target=evidence.target(),
            metadata=_metadata_object(assessment.metadata),
            split=_split(assessment.metadata),
        ),
        candidate_id=_primary_candidate_id(assessment),
        score=Score(value=evidence.score.score, feedback=evidence.feedback),
        evidence=EvidenceEnvelope.public_only(
            payload=public,
            data_classes=evidence.data_classes(),
        ),
        receipt=WriteReceipt(receipt_id=assessment.id),
        effect_receipts=evidence.effect_receipts(),
        replayability="boundary_managed",
        rewards=[
            RewardAssessment(
                id=reward.id,
                value=reward.value,
                weight=reward.weight,
                feedback=reward.feedback,
            )
            for reward in evidence.rewards()
        ],
    )


def _public_payload(evidence: RustCaseAssessmentEvidence) -> JsonObject:
    return {"output": evidence.output.report_text(), "reward_count": len(evidence.rewards())}


def _metadata_object(value: JsonValue) -> JsonObject:
    if value is None:
        return {}
    if isinstance(value, dict):
        return value
    raise TypeError("Rust assessment metadata must be a JSON object or null")


def _split(value: JsonValue) -> str | None:
    if not isinstance(value, dict):
        return None
    if "split" not in value:
        return None
    split = value["split"]
    if not isinstance(split, str):
        if not isinstance(split, dict):
            raise TypeError("Rust assessment metadata split must be a string metadata value")
        if split["kind"] != "string":
            raise TypeError("Rust assessment metadata split must be string metadata")
        split_value = split["value"]
        if not isinstance(split_value, str):
            raise TypeError("Rust assessment metadata split value must be a string")
        return split_value
    return split


def _primary_candidate_id(assessment: AssessmentReadback) -> str:
    if len(assessment.candidate_ids) != 1:
        raise ValueError("Rust inspection summary requires one candidate id")
    return assessment.candidate_ids[0]


__all__ = [
    "RustBlobOutput",
    "RustBlobReference",
    "RustCandidateAssessmentOutput",
    "RustCaseAssessmentEvidence",
    "RustCaseDataReadEvidence",
    "RustEffectReceiptEvidence",
    "RustInlineOutput",
    "RustOutputBlobAudit",
    "RustOutputMetadata",
    "RustOutputRecord",
    "RustRewardEvidence",
    "RustScalarEvidence",
    "rust_assessment_rows",
    "rust_evidence_summaries",
]
