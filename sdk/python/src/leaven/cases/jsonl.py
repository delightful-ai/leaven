"""`lv.cases.from_jsonl(...)` — load cases from a JSONL file."""

import json
from pathlib import Path

from ..case import Case, CaseSet
from ..json_value import JsonObject, JsonValue


def from_jsonl(
    path: str,
    *,
    id_field: str = "id",
    input_field: str = "input",
    target_field: str = "target",
    metadata_field: str | None = "metadata",
    name: str | None = None,
    limit: int | None = None,
) -> CaseSet:
    """Load a CaseSet from a JSONL file.

    Each line must be a JSON object with at minimum the `id_field` and
    `input_field` populated. `target_field` and `metadata_field` are
    optional. The case set's `name` defaults to the file's stem.
    """
    source = Path(path)
    rows: list[JsonObject] = []
    with source.open(encoding="utf-8") as handle:
        for raw in handle:
            line = raw.strip()
            if not line:
                continue
            rows.append(_json_object(json.loads(line), "JSONL row"))
            if limit is not None and len(rows) >= limit:
                break
    cases = [_row_to_case(row, id_field, input_field, target_field, metadata_field) for row in rows]
    return CaseSet(name=name or source.stem, cases=cases)


def _row_to_case(
    row: JsonObject,
    id_field: str,
    input_field: str,
    target_field: str,
    metadata_field: str | None,
) -> Case:
    """Project one JSONL row into a `Case`, reading the configured field names."""
    metadata = _optional_json_object(row.get(metadata_field)) if metadata_field is not None else {}
    return Case(
        id=str(row[id_field]),
        input=_json_object(row[input_field], input_field),
        target=_optional_json_object(row.get(target_field)),
        metadata=metadata or {},
    )


def from_iterable(
    items: list[JsonObject],
    *,
    id_field: str = "id",
    input_field: str = "input",
    target_field: str = "target",
    metadata_field: str | None = "metadata",
    name: str = "inline",
) -> CaseSet:
    """Build a CaseSet from an in-memory list of dicts. For tests and tiny demos."""
    cases = [_row_to_case(row, id_field, input_field, target_field, metadata_field) for row in items]
    return CaseSet(name=name, cases=cases)


def _optional_json_object(value: JsonValue | None) -> JsonObject | None:
    if value is None:
        return None
    return _json_object(value, "optional JSON object field")


def _json_object(value: JsonValue, field: str) -> JsonObject:
    if isinstance(value, dict):
        return value
    raise ValueError(f"{field} must be a JSON object")


__all__ = ["from_iterable", "from_jsonl"]
