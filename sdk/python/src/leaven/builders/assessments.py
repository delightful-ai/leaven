"""`cx.assessments.*` — submit assessment batches at the end of evaluation."""

import asyncio
from collections.abc import Sequence
from typing import Literal, Protocol

import msgspec
from msgspec import UNSET, UnsetType
from pydantic import BaseModel, ConfigDict

from .._errors import UnboundBuilderError
from .._receipts import WriteReceipt
from .._seam import AssessmentSubmitRequest
from .._seam._wire.evidence import (
    EvidenceEnvelope as WireEvidenceEnvelope,
)
from .._seam._wire.evidence import (
    EvidencePrivate as WireEvidencePrivate,
)
from .._seam._wire.evidence import (
    EvidenceProducer,
    EvidenceRedactionPolicy,
    EvidenceSourceReceipts,
)
from .._seam._wire.evidence import (
    EvidencePublic as WireEvidencePublic,
)
from .._seam._wire.refs import (
    AssessmentRankingValue,
    ReceiptRef,
    WireJsonExtensionPayload,
)
from .._seam._wire.results import AssessmentSubmitResult
from .._seam._wire.writes import SubmitAssessmentRecord, WriteOutputRecord, WriteScore
from ..assessment import AssessmentWrite, Replayability
from ..data_class import CANDIDATE_ARTIFACT, CANDIDATE_OUTPUT
from ..evidence import EvidenceEnvelope, EvidencePrivate, EvidencePublic
from ..json_value import JsonObject
from ..score import Score

PrivateEvidenceVisibility = Literal["evaluator_only", "operator_only", "scorer_private"]


class AssessmentSubmission(BaseModel):
    """Receipt for a submit_assessments call. Per-assessment receipts are inside."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    receipt: WriteReceipt
    assessment_ids: list[str]
    submitted: int
    """Count of assessments admitted."""


class _SeamRequester(Protocol):
    """Small private protocol AssessmentsBuilder needs from the seam client."""

    def assessment_submit(self, request: AssessmentSubmitRequest) -> AssessmentSubmitResult: ...


class AssessmentsBuilder:
    """Assessment submission bound to a context."""

    def __init__(
        self,
        *,
        _client: "_SeamRequester | None" = None,
        _idempotency_prefix: str = "assessment-builder",
        _plan_id: str = "planpythonassessmentbuilder001",
        _stage_call_id: str = "python.assessments.submit",
    ) -> None:
        self._client = _client
        self._idempotency_prefix = _idempotency_prefix
        self._plan_id = _plan_id
        self._stage_call_id = _stage_call_id

    @classmethod
    def _for_seam(
        cls,
        client: "_SeamRequester",
        *,
        idempotency_prefix: str = "assessment-builder",
        plan_id: str = "planpythonassessmentbuilder001",
        stage_call_id: str = "python.assessments.submit",
    ) -> "AssessmentsBuilder":
        """Bind this builder to the private public-seam process client."""
        return cls(
            _client=client,
            _idempotency_prefix=idempotency_prefix,
            _plan_id=plan_id,
            _stage_call_id=stage_call_id,
        )

    async def submit(
        self,
        evaluation_request_id: str,
        assessments: Sequence[AssessmentWrite],
    ) -> AssessmentSubmission:
        """Submit a batch of assessments against an evaluation request.

        The batch is admitted atomically: either every assessment passes
        seam validation and admits, or the whole batch is rejected with
        per-assessment denial details.
        """
        if self._client is None:
            raise UnboundBuilderError(
                "AssessmentsBuilder.submit needs an engine-bound public-seam client; "
                "use the cx.assessments instance supplied to an evaluator stage"
            )
        request = AssessmentSubmitRequest(
            request_id=f"{self._idempotency_prefix}-submit",
            plan_id=self._plan_id,
            idempotency_key=f"{self._idempotency_prefix}-submit",
            evaluation_request_id=evaluation_request_id,
            assessments=[
                _assessment_to_wire(assessment, stage_call_id=self._stage_call_id)
                for assessment in assessments
            ],
        )
        result = await asyncio.to_thread(self._client.assessment_submit, request)
        return _assessment_submission_from_result(result)


def _assessment_submission_from_result(
    result: AssessmentSubmitResult,
) -> AssessmentSubmission:
    primary = result.primary
    assessment_ids = list(primary.assessment_ids)
    return AssessmentSubmission(
        receipt=WriteReceipt(receipt_id=primary.receipt),
        assessment_ids=assessment_ids,
        submitted=len(assessment_ids),
    )


def _assessment_to_wire(
    assessment: AssessmentWrite,
    *,
    stage_call_id: str,
) -> SubmitAssessmentRecord:
    score = _required_score(assessment)
    return SubmitAssessmentRecord(
        kind=assessment.kind,
        score=_score_to_wire(score, assessment.evidence),
        evidence=_evidence_to_wire(
            assessment.evidence,
            read_receipts=[receipt.receipt_id for receipt in assessment.read_receipts],
            effect_receipts=[receipt.receipt_id for receipt in assessment.effect_receipts],
            replayability=assessment.replayability,
            stage_call_id=stage_call_id,
        ),
        replayability=assessment.replayability,
        candidate=assessment.candidate if assessment.candidate is not None else UNSET,
        candidates=list(assessment.candidates) if assessment.candidates is not None else UNSET,
        preference=assessment.preference if assessment.preference is not None else UNSET,
        ranking=(
            _wire_assessment_ranking(assessment.ranking)
            if assessment.ranking is not None
            else UNSET
        ),
        read_receipts=(
            _receipt_refs([receipt.receipt_id for receipt in assessment.read_receipts])
            if assessment.read_receipts
            else UNSET
        ),
        effect_receipts=(
            _receipt_refs([receipt.receipt_id for receipt in assessment.effect_receipts])
            if assessment.effect_receipts
            else UNSET
        ),
    )


def _required_score(assessment: AssessmentWrite) -> Score:
    if assessment.score is None:
        raise ValueError(f"{assessment.kind} assessment submission requires score")
    return assessment.score


def _score_to_wire(score: Score, evidence: EvidenceEnvelope) -> WriteScore:
    public = _required_public_evidence(evidence)
    data_classes = list(public.data_classes)
    _require_assessed_output_class(data_classes)
    summary = _score_summary(score, public)
    return WriteScore(
        value=score.value,
        output=WriteOutputRecord(
            kind="text",
            visibility="public",
            data_classes=data_classes,
            summary=summary,
            value=summary,
        ),
    )


def _evidence_to_wire(
    evidence: EvidenceEnvelope,
    *,
    read_receipts: list[str],
    effect_receipts: list[str],
    replayability: Replayability,
    stage_call_id: str,
) -> WireEvidenceEnvelope:
    public = _required_public_evidence(evidence)
    private = (
        _private_evidence_to_wire(evidence.private)
        if evidence.private is not None
        else UNSET
    )
    return WireEvidenceEnvelope(
        schema_version="leaven.evidence_envelope.v1",
        target_derived=evidence.target_derived,
        public=_public_evidence_to_wire(public),
        private=private,
        redaction_policy=EvidenceRedactionPolicy(
            optimizer="score_and_feedback",
            reflector="score_only",
            operator="full",
        ),
        producer=EvidenceProducer(stage_call_id=stage_call_id),
        source_receipts=EvidenceSourceReceipts(
            read=_receipt_refs(read_receipts),
            effect=_receipt_refs(effect_receipts),
        ),
        data_classes=_evidence_data_classes(public, evidence.private),
        replayability=replayability,
    )


def _required_public_evidence(evidence: EvidenceEnvelope) -> EvidencePublic:
    if evidence.public is None:
        raise ValueError("assessment submission requires public evidence")
    return evidence.public


def _public_evidence_to_wire(public: EvidencePublic) -> WireEvidencePublic:
    payload = public.payload
    _reject_unknown_public_payload_keys(payload)
    return WireEvidencePublic(
        data_classes=list(public.data_classes),
        summary=_optional_string_payload(payload, "summary"),
        feedback=_optional_string_payload(payload, "feedback"),
        metrics=_optional_metrics_payload(payload),
    )


def _private_evidence_to_wire(private: EvidencePrivate) -> WireEvidencePrivate:
    return WireEvidencePrivate(
        visibility=_private_visibility(private.visibility),
        data_classes=list(private.data_classes),
        payload=msgspec.convert(private.payload, type=WireJsonExtensionPayload),
    )


def _score_summary(score: Score, public: EvidencePublic) -> str:
    if "summary" in public.payload:
        summary = public.payload["summary"]
        if not isinstance(summary, str):
            raise TypeError("assessment evidence `summary` must be a string")
        if summary:
            return summary
    if score.feedback:
        return score.feedback
    return f"score={score.value:.6g}"


def _optional_string_payload(
    payload: JsonObject,
    key: str,
) -> str | UnsetType:
    if key not in payload:
        return UNSET
    value = payload[key]
    if not isinstance(value, str):
        raise TypeError(f"assessment evidence `{key}` must be a string")
    return value


def _optional_metrics_payload(payload: JsonObject) -> dict[str, float] | UnsetType:
    if "metrics" not in payload:
        return UNSET
    value = payload["metrics"]
    if not isinstance(value, dict):
        raise TypeError("assessment evidence `metrics` must be an object")
    metrics: dict[str, float] = {}
    for key, metric in value.items():
        if not isinstance(key, str):
            raise TypeError("assessment evidence metric names must be strings")
        if not isinstance(metric, int | float) or isinstance(metric, bool):
            raise TypeError("assessment evidence metric values must be numbers")
        metrics[key] = float(metric)
    return metrics


def _reject_unknown_public_payload_keys(payload: JsonObject) -> None:
    allowed = {"summary", "feedback", "metrics"}
    unknown = [key for key in payload if key not in allowed]
    if unknown:
        raise ValueError(
            "assessment public evidence payload supports only summary, feedback, and metrics"
        )


def _evidence_data_classes(
    public: EvidencePublic,
    private: EvidencePrivate | None,
) -> list[str]:
    data_classes = list(public.data_classes)
    if private is not None:
        data_classes.extend(private.data_classes)
    return _dedupe(data_classes)


def _dedupe(values: Sequence[str]) -> list[str]:
    seen: set[str] = set()
    output: list[str] = []
    for value in values:
        if value not in seen:
            seen.add(value)
            output.append(value)
    return output


def _require_assessed_output_class(data_classes: Sequence[str]) -> None:
    if CANDIDATE_OUTPUT in data_classes or CANDIDATE_ARTIFACT in data_classes:
        return
    raise ValueError(
        "assessment score output must declare candidate.output or candidate.artifact"
    )


def _receipt_refs(values: Sequence[str]) -> list[ReceiptRef]:
    output: list[ReceiptRef] = []
    output.extend(values)
    return output


def _wire_assessment_ranking(values: Sequence[str]) -> AssessmentRankingValue:
    return list(values)


def _private_visibility(value: str) -> PrivateEvidenceVisibility:
    if value == "evaluator_only":
        return "evaluator_only"
    if value == "operator_only":
        return "operator_only"
    if value == "scorer_private":
        return "scorer_private"
    raise ValueError("assessment private evidence visibility is not locked V1")


__all__ = ["AssessmentSubmission", "AssessmentsBuilder"]
