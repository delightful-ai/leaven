"""Run one registered Python proposer stage from a stage.run payload."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any, cast

from .._receipts import CallReceipt, QueryReceipt
from ..decorators import RegisteredStage
from ..proposal import ProposalBatch
from ..stage_payloads import ProposeRequest, ReflectionResult
from .context import propose_context


async def run_proposer_stage(
    stage: RegisteredStage[Any, Any],
    params: Mapping[str, Any],
) -> dict[str, Any]:
    """Execute one proposer request and return a text stage_run_result summary."""
    payload = params["payload"]
    if payload.get("role") != "proposer":
        raise ValueError(f"stage.run payload is not a proposer role: {payload!r}")
    if stage.role != "proposer":
        raise ValueError(f"configured stage must be a proposer; got {stage.role!r}")

    request = _propose_request_from_payload(payload)
    cx = propose_context(
        parent_candidate_id=request.parent_candidate_id,
        stage_call_id=payload["stage_call_id"],
    )
    batch = await stage.func(request, cx)
    if not isinstance(batch, ProposalBatch):
        raise TypeError(f"proposer stage must return ProposalBatch; got {type(batch).__name__}")
    submission = await cx.proposals.submit(batch)
    return {
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "proposer",
        "stage_call_id": payload["stage_call_id"],
        "output": {
            "kind": "text",
            "summary": f"submitted {len(submission.proposal_ids)} proposal(s)",
            "value": submission.receipt.receipt_id,
            "visibility": "optimizer_visible",
            "data_classes": ["public"],
        },
    }


def _propose_request_from_payload(payload: Mapping[str, Any]) -> ProposeRequest:
    reflection = payload["reflection_result"]
    return ProposeRequest(
        parent_candidate_id=_candidate_id(payload["parent"]),
        reflection=ReflectionResult(
            diagnosis=str(reflection.get("summary", "")),
            diagnosis_source_refs=[],
            metadata={
                "failure_modes": list(reflection.get("failure_modes", [])),
                "surface_suggestions": list(reflection.get("surface_suggestions", [])),
            },
        ),
        reflection_receipt=CallReceipt(receipt_id=_reflection_receipt(reflection)),
        allowed_change_schemas=list(payload.get("allowed_change_schemas", [])),
        allowed_surfaces=[payload["surface_fingerprint"]]
        if "surface_fingerprint" in payload
        else [],
        read_receipts=[
            QueryReceipt(receipt_id=receipt)
            for receipt in reflection.get("read_receipts", [])
            if isinstance(receipt, str) and receipt.startswith("qrec_")
        ],
    )


def _candidate_id(value: object) -> str:
    if isinstance(value, str):
        return value
    if isinstance(value, Mapping):
        candidate = cast(Mapping[str, object], value)
        candidate_id = candidate.get("id")
        if isinstance(candidate_id, str):
            return candidate_id
    raise ValueError(f"unsupported candidate ref: {value!r}")


def _reflection_receipt(reflection: Mapping[str, Any]) -> str:
    for receipt in reflection.get("read_receipts", []):
        if isinstance(receipt, str) and receipt.startswith("stagerec_"):
            return receipt
    return "stagerec_reflection"


__all__ = ["run_proposer_stage"]
