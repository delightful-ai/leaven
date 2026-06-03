"""Case loaders — `lv.cases.from_jsonl(...)`, `lv.cases.from_parquet(...)`, etc.

Generic loaders only. Paper-specific benchmark catalogs (OfficeQA, SealQA,
BrowseComp, ...) live in separate `leaven_benchmarks_*` packages that users
opt into. Per spec: no benchmark bundling in core.
"""

from __future__ import annotations

from .csv import from_csv
from .jsonl import from_jsonl
from .parquet import from_parquet
from .splits import splits

__all__ = ["from_csv", "from_jsonl", "from_parquet", "splits"]
