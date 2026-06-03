"""Private receipt projection for durable-seam optimize reports."""

from __future__ import annotations

from typing import Any


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


__all__ = ["effect_receipt_ids_from_stage_result"]
