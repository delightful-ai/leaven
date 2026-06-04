"""Run one registered Python proposer stage from a stage.run payload."""

from .._receipts import CallReceipt, QueryReceipt
from .._seam._wire import JsonObject, JsonValue
from .._seam._wire.json_value import json_object
from ..decorators import RegisteredStage
from ..proposal import ProposalBatch
from ..stage_payloads import ProposeRequest, ReflectionResult
from .context import JsonRpcCallbackClient, propose_context


async def run_proposer_stage(
    stage: RegisteredStage[object, object],
    params: JsonObject,
    *,
    lm_model: str,
) -> JsonObject:
    """Execute one proposer request and return a text stage_run_result summary."""
    payload = json_object(params["payload"])
    if payload.get("role") != "proposer":
        raise ValueError(f"stage.run payload is not a proposer role: {payload!r}")
    if stage.role != "proposer":
        raise ValueError(f"configured stage must be a proposer; got {stage.role!r}")

    request = _propose_request_from_payload(payload)
    callback = JsonRpcCallbackClient(lm_model=lm_model)
    cx = propose_context(
        parent_candidate_id=request.parent_candidate_id,
        stage_call_id=_string(payload["stage_call_id"], "stage_call_id"),
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
            "stage_call_id": _string(payload["stage_call_id"], "stage_call_id"),
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


def _propose_request_from_payload(payload: JsonObject) -> ProposeRequest:
    reflection = json_object(payload["reflection_result"])
    return ProposeRequest(
        parent_candidate_id=_candidate_id(payload["parent"]),
        reflection=ReflectionResult(
            diagnosis=str(reflection.get("summary", "")),
            diagnosis_source_refs=[],
            metadata={
                "failure_modes": _json_array(reflection.get("failure_modes")),
                "surface_suggestions": _json_array(reflection.get("surface_suggestions")),
            },
        ),
        reflection_receipt=CallReceipt(receipt_id=_reflection_receipt(reflection)),
        allowed_change_schemas=_string_list(payload.get("allowed_change_schemas")),
        allowed_surfaces=[_string(payload["surface_fingerprint"], "surface_fingerprint")]
        if "surface_fingerprint" in payload
        else [],
        read_receipts=[
            QueryReceipt(receipt_id=receipt)
            for receipt in _string_list(reflection.get("read_receipts"))
            if isinstance(receipt, str) and receipt.startswith("qrec_")
        ],
    )


def _candidate_id(value: object) -> str:
    if isinstance(value, str):
        return value
    if isinstance(value, dict):
        candidate = json_object(value)
        candidate_id = candidate.get("id")
        if isinstance(candidate_id, str):
            return candidate_id
    raise ValueError(f"unsupported candidate ref: {value!r}")


def _reflection_receipt(reflection: JsonObject) -> str:
    for receipt in _string_list(reflection.get("read_receipts")):
        if isinstance(receipt, str) and receipt.startswith("stagerec_"):
            return receipt
    return "stagerec_reflection"


def _string(value: JsonValue, field: str) -> str:
    if isinstance(value, str):
        return value
    raise ValueError(f"stage.run proposer payload field {field} must be a string")


def _string_list(value: JsonValue | None) -> list[str]:
    if value is None:
        return []
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, str)]


def _json_array(value: JsonValue | None) -> list[JsonValue]:
    if value is None:
        return []
    if not isinstance(value, list):
        return []
    return value


__all__ = ["run_proposer_stage"]
