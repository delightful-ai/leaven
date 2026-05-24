"""`lv.cases.from_parquet(...)` — load cases from a Parquet file.

Backed by the Rust `leaven-eval-parquet` crate via the engine. Parquet
files are lowered into source-row manifests; this loader produces the
Python typed handles over that wire.
"""

from __future__ import annotations

from ..case import CaseSet


def from_parquet(
    path: str,
    *,
    id_column: str = "id",
    input_columns: list[str] | None = None,
    target_columns: list[str] | None = None,
    metadata_columns: list[str] | None = None,
    name: str | None = None,
    limit: int | None = None,
) -> CaseSet:
    """Load a CaseSet from a Parquet file.

    Columns map to case fields; multiple input/target/metadata columns nest
    into the corresponding dict (e.g. `input_columns=["question", "context"]`
    yields `case.input = {"question": ..., "context": ...}`).
    """
    raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["from_parquet"]
