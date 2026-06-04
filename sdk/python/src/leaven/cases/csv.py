"""`lv.cases.from_csv(...)` — load cases from a CSV file."""

import csv
from pathlib import Path

from ..case import Case, CaseSet
from ..json_value import JsonObject


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
    if not input_columns:
        raise ValueError("input_columns must name at least one column")

    source = Path(path)
    cases: list[Case] = []
    with source.open(newline="", encoding="utf-8") as handle:
        reader = csv.DictReader(handle, delimiter=delimiter)
        for row in reader:
            cases.append(
                Case(
                    id=_required_cell(row, id_column),
                    input=_selected_cells(row, input_columns, "input_columns"),
                    target=_optional_selected_cells(row, target_columns, "target_columns"),
                    metadata=_optional_selected_cells(row, metadata_columns, "metadata_columns")
                    or {},
                )
            )
            if limit is not None and len(cases) >= limit:
                break
    return CaseSet(name=name or source.stem, cases=cases)


def _selected_cells(row: dict[str, str], columns: list[str], field: str) -> JsonObject:
    if not columns:
        raise ValueError(f"{field} must name at least one column")
    return {column: _required_cell(row, column) for column in columns}


def _optional_selected_cells(
    row: dict[str, str], columns: list[str] | None, field: str
) -> JsonObject | None:
    if columns is None:
        return None
    return _selected_cells(row, columns, field)


def _required_cell(row: dict[str, str], column: str) -> str:
    if column not in row:
        raise ValueError(f"CSV row is missing column {column!r}")
    return row[column]


__all__ = ["from_csv"]
