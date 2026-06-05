"""Typed projection from Rust-owned evidence exports into run inspection."""

from pydantic import BaseModel, ConfigDict, Field

from ..run_inspection import (
    AssessmentReadback,
    EvidenceSummary,
    RewardDimensionSummary,
    RustEvidenceReadback,
    RustRunReadback,
)


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

    def case_id(self) -> str:
        """Return the stable textual case id carried by Rust evidence."""
        return str(self.case)


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

    def data_classes(self) -> list[str]:
        """Return stable public data classes represented by this evidence."""
        classes = {"public"}
        classes.update(self.output.data_classes())
        for read in self.case_data_reads:
            classes.update(read.data_classes)
        return sorted(classes)


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
        payload={"output": evidence.output.report_text()},
        target_derived="case.target" in evidence.data_classes(),
        rewards=[
            RewardDimensionSummary(
                id="score",
                value=evidence.score.score,
                weight=1.0,
                feedback=evidence.feedback,
            )
        ],
    )


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
    "RustInlineOutput",
    "RustOutputBlobAudit",
    "RustOutputMetadata",
    "RustOutputRecord",
    "RustScalarEvidence",
    "rust_evidence_summaries",
]
