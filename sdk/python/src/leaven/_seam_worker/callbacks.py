"""Private callback receipt capture for Python command workers."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

EFFECT_CALLBACK_METHODS = frozenset({"leaven/lm.complete", "leaven/agent.run"})


@dataclass(frozen=True)
class CallbackReceipt:
    """One receipt reported by a nested callback result."""

    method: str
    receipt_id: str
    call_kind: str | None = None

    def to_json(self) -> dict[str, str]:
        value = {"method": self.method, "receipt": self.receipt_id}
        if self.call_kind is not None:
            value["call_kind"] = self.call_kind
        return value


class CallbackReceiptLog:
    """Append-only receipt log for one stage invocation."""

    def __init__(self) -> None:
        self._records: list[CallbackReceipt] = []

    def record_result(self, *, method: str, result: dict[str, Any]) -> None:
        """Capture callback receipts carried by one public-seam callback result."""
        self._records.extend(_receipts_from_result(method=method, result=result))

    def effect_receipts_json(self) -> list[dict[str, str]]:
        """Return effect-call receipts safe to attach to a private stage result."""
        return [
            record.to_json() for record in self._records if record.method in EFFECT_CALLBACK_METHODS
        ]


def _receipts_from_result(
    *,
    method: str,
    result: dict[str, Any],
) -> list[CallbackReceipt]:
    records = []
    for value in result.get("receipts", []):
        if not isinstance(value, dict):
            continue
        receipt = value.get("receipt")
        if not isinstance(receipt, str) or not receipt:
            continue
        call_kind = value.get("call_kind")
        records.append(
            CallbackReceipt(
                method=method,
                receipt_id=receipt,
                call_kind=call_kind if isinstance(call_kind, str) else None,
            )
        )
    return records


__all__ = ["CallbackReceiptLog"]
