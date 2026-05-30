"""`lv.cases.*` — generic dataset loaders -> `Sequence[Case]`.

Generic loaders only. `splits=` maps a label to a `slice` (spec line 459). NO
bundled benchmark catalogs — paper-specific catalogs live in separate
`leaven_benchmarks_*` packages.

Governing spec: `docs/specs/leaven_python.md` — Task and Case (loader sugar).
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence

from ..case import Case

__all__ = ["from_csv", "from_jsonl", "from_parquet"]


def from_jsonl(
    path: str, *, splits: Mapping[str, slice] | None = None, **kwargs: object
) -> Sequence[Case]:
    """Load cases from a JSONL file."""
    raise NotImplementedError("see leaven_python.md — cases loaders")


def from_parquet(
    path: str, *, splits: Mapping[str, slice] | None = None, **kwargs: object
) -> Sequence[Case]:
    """Load cases from a Parquet file."""
    raise NotImplementedError("see leaven_python.md — cases loaders")


def from_csv(
    path: str, *, splits: Mapping[str, slice] | None = None, **kwargs: object
) -> Sequence[Case]:
    """Load cases from a CSV file."""
    raise NotImplementedError("see leaven_python.md — cases loaders")
