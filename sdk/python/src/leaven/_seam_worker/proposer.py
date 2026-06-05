"""Run one registered Python proposer stage from a stage.run payload."""

from msgspec import UNSET, UnsetType

from .._receipts import CallReceipt, QueryReceipt
from .._seam._wire.payloads import (
    FailureMode,
    OutputRecord,
    StageRunRequest,
    StageRunResult,
    SurfaceSuggestion,
)
from .._seam._wire.payloads import (
    ProposeRequest as WireProposeRequest,
)
from .._seam._wire.payloads import (
    ReflectionResult as WireReflectionResult,
)
from .._seam._wire.refs import (
    AssessmentRefRecord,
    CandidateRef,
    CandidateRefRecord,
    CaseRefRecord,
    EvaluationAttemptRefRecord,
    EvaluationRequestRefRecord,
    ExternalInfoRefRecord,
    InfoRef,
    ProposalBatchRefRecord,
    ProposalRefRecord,
)
from ..decorators import RegisteredStage
from ..proposal import ProposalBatch
from ..stage_payloads import (
    ProposeRequest,
    ReflectionFailureMode,
    ReflectionResult,
    ReflectionSurfaceSuggestion,
    StageSourceRef,
)
from .context import JsonRpcCallbackClient, propose_context

_STRING_REF_PREFIX_KINDS = (
    ("evalattempt_", "evaluation_attempt"),
    ("evalreq_", "evaluation_request"),
    ("assess_", "assessment"),
    ("cand_", "candidate"),
    ("case_", "case"),
    ("prop_", "proposal"),
    ("pb_", "proposal_batch"),
)
type _RecordInfoRefType = (
    type[AssessmentRefRecord]
    | type[CandidateRefRecord]
    | type[CaseRefRecord]
    | type[EvaluationAttemptRefRecord]
    | type[EvaluationRequestRefRecord]
    | type[ProposalBatchRefRecord]
    | type[ProposalRefRecord]
)

_RECORD_REF_KINDS: tuple[tuple[_RecordInfoRefType, str], ...] = (
    (AssessmentRefRecord, "assessment"),
    (CandidateRefRecord, "candidate"),
    (CaseRefRecord, "case"),
    (EvaluationAttemptRefRecord, "evaluation_attempt"),
    (EvaluationRequestRefRecord, "evaluation_request"),
    (ProposalBatchRefRecord, "proposal_batch"),
    (ProposalRefRecord, "proposal"),
)


async def run_proposer_stage(
    stage: RegisteredStage[ProposeRequest, ProposalBatch],
    params: StageRunRequest,
    *,
    lm_model: str,
) -> StageRunResult:
    """Execute one proposer request and return a text stage_run_result summary."""
    payload = params.payload
    if not isinstance(payload, WireProposeRequest):
        raise TypeError(f"stage.run payload is not a proposer role: {payload!r}")
    if stage.role != "proposer":
        raise ValueError(f"configured stage must be a proposer; got {stage.role!r}")

    request = _propose_request_from_payload(payload)
    callback = JsonRpcCallbackClient(lm_model=lm_model)
    cx = propose_context(
        parent_candidate_id=request.parent_candidate_id,
        stage_call_id=payload.stage_call_id,
        capability_fingerprint=payload.capability_fingerprint,
        lm_model=lm_model,
        callback=callback,
    )
    batch = await stage.func(request, cx)
    if not isinstance(batch, ProposalBatch):
        raise TypeError(f"proposer stage must return ProposalBatch; got {type(batch).__name__}")
    submission = await cx.proposals.submit(batch)
    return StageRunResult(
        schema_version="leaven.stage_run.v1",
        message="stage_run_result",
        stage="proposer",
        stage_call_id=payload.stage_call_id,
        output=OutputRecord(
            kind="text",
            summary=f"submitted {len(submission.proposal_ids)} proposal(s)",
            value=submission.receipt.receipt_id,
            visibility="optimizer_visible",
            data_classes=["public"],
        ),
        effect_receipts=callback.effect_receipts(),
        proposal_receipts=callback.proposal_receipts(),
    )


def _propose_request_from_payload(payload: WireProposeRequest) -> ProposeRequest:
    reflection = payload.reflection_result
    return ProposeRequest(
        parent_candidate_id=_candidate_id(payload.parent),
        reflection=ReflectionResult(
            diagnosis=reflection.summary,
            diagnosis_source_refs=[_stage_source_ref(ref) for ref in reflection.source_refs],
            failure_modes=_reflection_failure_modes(reflection),
            surface_suggestions=_reflection_surface_suggestions(reflection),
            negative_constraints=_optional_strings(reflection.negative_constraints),
            positive_constraints=_optional_strings(reflection.positive_constraints),
            confidence=None if reflection.confidence is UNSET else reflection.confidence,
        ),
        reflection_receipt=CallReceipt(receipt_id=_reflection_receipt(reflection)),
        allowed_change_schemas=_optional_strings(payload.allowed_change_schemas),
        allowed_surfaces=_optional_surface(payload.surface_fingerprint),
        read_receipts=[
            QueryReceipt(receipt_id=receipt)
            for receipt in reflection.read_receipts
            if isinstance(receipt, str) and receipt.startswith("qrec_")
        ],
    )


def _candidate_id(value: CandidateRef) -> str:
    if isinstance(value, str):
        return value
    if isinstance(value, CandidateRefRecord):
        return value.id
    raise TypeError(f"unsupported candidate ref: {value!r}")


def _reflection_receipt(reflection: WireReflectionResult) -> str:
    for receipt in reflection.read_receipts:
        if isinstance(receipt, str) and receipt.startswith("stagerec_"):
            return receipt
    return "stagerec_reflection"


def _optional_strings(value: list[str] | UnsetType) -> list[str]:
    if value is UNSET:
        return []
    return list(value)


def _optional_surface(value: str | UnsetType) -> list[str]:
    if value is UNSET:
        return []
    return [value]


def _reflection_failure_modes(reflection: WireReflectionResult) -> list[ReflectionFailureMode]:
    if reflection.failure_modes is UNSET:
        return []
    return [_failure_mode(mode) for mode in reflection.failure_modes]


def _reflection_surface_suggestions(reflection: WireReflectionResult) -> list[ReflectionSurfaceSuggestion]:
    if reflection.surface_suggestions is UNSET:
        return []
    return [_surface_suggestion(suggestion) for suggestion in reflection.surface_suggestions]


def _failure_mode(mode: FailureMode) -> ReflectionFailureMode:
    return ReflectionFailureMode(
        label=mode.label,
        description=mode.description,
        severity=None if mode.severity is UNSET else mode.severity,
        source_refs=[] if mode.source_refs is UNSET else [_stage_source_ref(ref) for ref in mode.source_refs],
    )


def _surface_suggestion(suggestion: SurfaceSuggestion) -> ReflectionSurfaceSuggestion:
    return ReflectionSurfaceSuggestion(
        surface_fingerprint=suggestion.surface_fingerprint,
        diagnosis=suggestion.diagnosis,
        part_label=None if suggestion.part_label is UNSET else suggestion.part_label,
        suggested_direction=None if suggestion.suggested_direction is UNSET else suggestion.suggested_direction,
        constraints=[] if suggestion.constraints is UNSET else list(suggestion.constraints),
        source_refs=[] if suggestion.source_refs is UNSET else [_stage_source_ref(ref) for ref in suggestion.source_refs],
    )


def _stage_source_ref(value: InfoRef) -> StageSourceRef:
    kind: str
    ref_id: str
    if isinstance(value, str):
        kind = _string_ref_kind(value)
        ref_id = value
    elif isinstance(value, ExternalInfoRefRecord):
        kind = "external"
        ref_id = value.id
    elif isinstance(
        value,
        (
            AssessmentRefRecord,
            CandidateRefRecord,
            CaseRefRecord,
            EvaluationAttemptRefRecord,
            EvaluationRequestRefRecord,
            ProposalBatchRefRecord,
            ProposalRefRecord,
        ),
    ):
        kind = _record_ref_kind(value)
        ref_id = value.id
    else:
        raise TypeError(f"unsupported info ref: {value!r}")
    return StageSourceRef(kind=kind, id=ref_id)


def _string_ref_kind(value: str) -> str:
    for prefix, kind in _STRING_REF_PREFIX_KINDS:
        if value.startswith(prefix):
            return kind
    return "info"


def _record_ref_kind(
    value: AssessmentRefRecord
    | CandidateRefRecord
    | CaseRefRecord
    | EvaluationAttemptRefRecord
    | EvaluationRequestRefRecord
    | ProposalBatchRefRecord
    | ProposalRefRecord,
) -> str:
    for record_type, kind in _RECORD_REF_KINDS:
        if isinstance(value, record_type):
            return kind
    raise TypeError(f"unsupported typed info ref: {value!r}")


__all__ = ["run_proposer_stage"]
