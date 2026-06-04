"""Case loaders — `lv.cases.from_jsonl(...)`, `lv.cases.from_csv(...)`, etc.

Generic loaders only. Paper-specific benchmark catalogs (OfficeQA, SealQA,
BrowseComp, ...) live in separate `leaven_benchmarks_*` packages that users
opt into. Per spec: no benchmark bundling in core.
"""

from .csv import from_csv
from .jsonl import from_iterable, from_jsonl
from .splits import splits

__all__ = ["from_csv", "from_iterable", "from_jsonl", "splits"]
