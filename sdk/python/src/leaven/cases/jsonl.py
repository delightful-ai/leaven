"""`lv.cases.from_jsonl(...)` — load cases from a JSONL file."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from ..case import Case, CaseSet


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
    rows: list[dict[str, Any]] = []
    with source.open(encoding="utf-8") as handle:
        for raw in handle:
            line = raw.strip()
            if not line:
                continue
            rows.append(json.loads(line))
            if limit is not None and len(rows) >= limit:
                break
    cases = [_row_to_case(row, id_field, input_field, target_field, metadata_field) for row in rows]
    return CaseSet(name=name or source.stem, cases=cases)


def _row_to_case(
    row: dict[str, Any],
    id_field: str,
    input_field: str,
    target_field: str,
    metadata_field: str | None,
) -> Case:
    """Project one JSONL row into a `Case`, reading the configured field names."""
    metadata = row.get(metadata_field, {}) if metadata_field is not None else {}
    return Case(
        id=str(row[id_field]),
        input=dict(row[input_field]),
        target=row.get(target_field),
        metadata=dict(metadata) if metadata else {},
    )


def from_iterable(
    items: list[dict[str, Any]],
    *,
    id_field: str = "id",
    input_field: str = "input",
    target_field: str = "target",
    metadata_field: str | None = "metadata",
    name: str = "inline",
) -> CaseSet:
    """Build a CaseSet from an in-memory list of dicts. For tests and tiny demos."""
    raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["from_iterable", "from_jsonl"]
