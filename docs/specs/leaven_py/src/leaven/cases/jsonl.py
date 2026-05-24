"""`lv.cases.from_jsonl(...)` — load cases from a JSONL file."""

from __future__ import annotations

from typing import Any

from ..case import CaseSet


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
    raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


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
