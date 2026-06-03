"""Private receipt projection for durable-seam optimize reports."""

from dataclasses import dataclass
from typing import Any

from .._receipts import CallReceipt, WriteReceipt
from ..blob_ref import BlobRef


@dataclass(frozen=True)
class EffectCostTotals:
    """Metered cost and LM-token totals from stage effect receipts."""

    cost_usd: float
    lm_tokens: int


def effect_receipts_from_stage_result(result: dict[str, Any]) -> list[CallReceipt]:
    """Extract opaque effect receipts from private worker stage metadata."""
    receipts = []
    for value in result.get("effect_receipts", []):
        if not isinstance(value, dict):
            continue
        receipt = value.get("receipt")
        if isinstance(receipt, str) and receipt:
            receipts.append(
                CallReceipt(
                    receipt_id=receipt,
                    blob_refs=_blob_refs_from_receipt(value),
                )
            )
    return receipts


def effect_cost_totals_from_stage_result(result: dict[str, Any]) -> EffectCostTotals:
    """Extract metered usage totals from private worker stage metadata."""
    usd_micro = 0
    lm_tokens = 0
    for value in result.get("effect_receipts", []):
        if not isinstance(value, dict):
            continue
        cost = value.get("cost")
        if not isinstance(cost, dict):
            continue
        usd_micro += _nonnegative_int(cost.get("usd_micro"))
        lm_tokens += _nonnegative_int(cost.get("input_tokens"))
        lm_tokens += _nonnegative_int(cost.get("output_tokens"))
    return EffectCostTotals(cost_usd=usd_micro / 1_000_000, lm_tokens=lm_tokens)


def proposal_receipts_from_stage_result(result: dict[str, Any]) -> list[WriteReceipt]:
    """Extract proposal write receipts from private worker stage metadata."""
    receipts = []
    for value in result.get("proposal_receipts", []):
        if not isinstance(value, dict):
            continue
        receipt = value.get("receipt")
        if not isinstance(receipt, str) or not receipt:
            continue
        proposal_ids = value.get("proposal_ids")
        receipts.append(
            WriteReceipt(
                receipt_id=receipt,
                proposal_ids=[
                    proposal_id
                    for proposal_id in proposal_ids
                    if isinstance(proposal_id, str)
                ]
                if isinstance(proposal_ids, list)
                else [],
            )
        )
    return receipts


def sum_effect_cost_totals(values: list[EffectCostTotals]) -> EffectCostTotals:
    """Sum per-stage cost totals."""
    return EffectCostTotals(
        cost_usd=sum(value.cost_usd for value in values),
        lm_tokens=sum(value.lm_tokens for value in values),
    )


def _nonnegative_int(value: object) -> int:
    if isinstance(value, int) and value >= 0:
        return value
    return 0


def _blob_refs_from_receipt(value: dict[str, Any]) -> list[BlobRef]:
    refs = []
    for ref in value.get("blob_refs", []):
        if not isinstance(ref, dict):
            continue
        blob_id = ref.get("blob_id") or ref.get("id")
        if not isinstance(blob_id, str):
            continue
        try:
            refs.append(
                BlobRef(
                    blob_id=blob_id,
                    sha256=ref.get("sha256") if isinstance(ref.get("sha256"), str) else None,
                    bytes=ref.get("bytes") if isinstance(ref.get("bytes"), int) else None,
                    data_classes=[
                        item for item in ref.get("data_classes", []) if isinstance(item, str)
                    ],
                )
            )
        except ValueError:
            continue
    return refs


__all__ = [
    "EffectCostTotals",
    "effect_cost_totals_from_stage_result",
    "effect_receipts_from_stage_result",
    "proposal_receipts_from_stage_result",
    "sum_effect_cost_totals",
]
