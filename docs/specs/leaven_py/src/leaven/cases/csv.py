"""`lv.cases.from_csv(...)` — load cases from a CSV file."""

from __future__ import annotations

from ..case import CaseSet


def from_csv(
    path: str,
    *,
    id_column: str = "id",
    input_columns: list[str] | None = None,
    target_columns: list[str] | None = None,
    metadata_columns: list[str] | None = None,
    name: str | None = None,
    limit: int | None = None,
    delimiter: str = ",",
) -> CaseSet:
    """Load a CaseSet from a CSV file. Same column conventions as `from_parquet`."""
    raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["from_csv"]
