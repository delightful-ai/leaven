"""Private planning records for the optimize lowering path."""

from dataclasses import dataclass

from ..json_value import JsonObject


@dataclass(frozen=True)
class PlannedOptimizeCase:
    """One case selected for an optimize run, lowered into the wire manifest."""

    case_id: str
    input: JsonObject
    target: JsonObject | None
    metadata: JsonObject
    split: str | None


__all__ = ["PlannedOptimizeCase"]
