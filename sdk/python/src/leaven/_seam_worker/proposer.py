"""Run one registered Python proposer stage from a stage.run payload."""

from msgspec import UNSET, UnsetType

from .._receipts import CallReceipt, QueryReceipt
from .._seam._wire import JsonObject
from .._seam._wire.json_value import json_object
from .._seam._wire.payloads import (
    FailureMode,
    StageRunRequest,
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
from ..stage_payloads import ProposeRequest, ReflectionResult
from .context import JsonRpcCallbackClient, propose_context


async def run_proposer_stage(
    stage: RegisteredStage[object, object],
    params: StageRunRequest,
    *,
    lm_model: str,
) -> JsonObject:
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
        lm_model=lm_model,
        callback=callback,
    )
    batch = await stage.func(request, cx)
    if not isinstance(batch, ProposalBatch):
        raise TypeError(f"proposer stage must return ProposalBatch; got {type(batch).__name__}")
    submission = await cx.proposals.submit(batch)
    return json_object(
        {
            "schema_version": "leaven.stage_run.v1",
            "message": "stage_run_result",
            "stage": "proposer",
            "stage_call_id": payload.stage_call_id,
            "output": {
                "kind": "text",
                "summary": f"submitted {len(submission.proposal_ids)} proposal(s)",
                "value": submission.receipt.receipt_id,
                "visibility": "optimizer_visible",
                "data_classes": ["public"],
            },
            "effect_receipts": callback.effect_receipts_json(),
            "proposal_receipts": callback.proposal_receipts_json(),
        }
    )


def _propose_request_from_payload(payload: WireProposeRequest) -> ProposeRequest:
    reflection = payload.reflection_result
    return ProposeRequest(
        parent_candidate_id=_candidate_id(payload.parent),
        reflection=ReflectionResult(
            diagnosis=reflection.summary,
            diagnosis_source_refs=[],
            metadata={
                "failure_modes": _reflection_failure_modes(reflection),
                "surface_suggestions": _reflection_surface_suggestions(reflection),
            },
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


def _reflection_failure_modes(reflection: WireReflectionResult) -> list[JsonObject]:
    if reflection.failure_modes is UNSET:
        return []
    return [_failure_mode_json(mode) for mode in reflection.failure_modes]


def _reflection_surface_suggestions(reflection: WireReflectionResult) -> list[JsonObject]:
    if reflection.surface_suggestions is UNSET:
        return []
    return [_surface_suggestion_json(suggestion) for suggestion in reflection.surface_suggestions]


def _failure_mode_json(mode: FailureMode) -> JsonObject:
    output: JsonObject = {"label": mode.label, "description": mode.description}
    if mode.severity is not UNSET:
        output["severity"] = mode.severity
    if mode.source_refs is not UNSET:
        output["source_refs"] = [_ref_id(ref) for ref in mode.source_refs]
    return output


def _surface_suggestion_json(suggestion: SurfaceSuggestion) -> JsonObject:
    output: JsonObject = {
        "surface_fingerprint": suggestion.surface_fingerprint,
        "diagnosis": suggestion.diagnosis,
    }
    if suggestion.part_label is not UNSET:
        output["part_label"] = suggestion.part_label
    if suggestion.suggested_direction is not UNSET:
        output["suggested_direction"] = suggestion.suggested_direction
    if suggestion.constraints is not UNSET:
        output["constraints"] = list(suggestion.constraints)
    if suggestion.source_refs is not UNSET:
        output["source_refs"] = [_ref_id(ref) for ref in suggestion.source_refs]
    return output


def _ref_id(value: InfoRef) -> str:
    if isinstance(value, str):
        return value
    if isinstance(value, ExternalInfoRefRecord):
        return value.namespace
    if isinstance(
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
        return value.id
    raise TypeError(f"unsupported info ref: {value!r}")


__all__ = ["run_proposer_stage"]
