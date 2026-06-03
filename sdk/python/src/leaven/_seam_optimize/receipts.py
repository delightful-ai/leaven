"""Private receipt projection for durable-seam optimize reports."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True)
class EffectCostTotals:
    """Metered cost and LM-token totals from stage effect receipts."""

    cost_usd: float
    lm_tokens: int


def effect_receipt_ids_from_stage_result(result: dict[str, Any]) -> list[str]:
    """Extract opaque effect receipt ids from private worker stage metadata."""
    ids = []
    for value in result.get("effect_receipts", []):
        if not isinstance(value, dict):
            continue
        receipt = value.get("receipt")
        if isinstance(receipt, str) and receipt:
            ids.append(receipt)
    return ids


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


__all__ = [
    "EffectCostTotals",
    "effect_cost_totals_from_stage_result",
    "effect_receipt_ids_from_stage_result",
    "sum_effect_cost_totals",
]
